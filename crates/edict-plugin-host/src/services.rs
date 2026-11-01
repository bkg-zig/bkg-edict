//! Ports zwischen Host und der Umgebung des Hosts.
//!
//! Der Host kennt keine Datenbank und keinen Ereignisbus. Er spricht mit
//! Ersetzungen dieser Traits. Der Server setzt sie mit Datenbank und Redis
//! um, die Tests mit einfachen Mitschreibern.

use std::pin::Pin;

use async_trait::async_trait;
use edict_core::{AppError, AppResult, Event};
use futures_util::Stream;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_stream::wrappers::ReceiverStream;

/// Fähigkeiten, die ein Plugin beim Host aufrufen darf.
///
/// Ein Plugin ruft den Host über die Methode `invoke` auf. Der Host leitet
/// den Aufruf an die Umgebung weiter, etwa an `task.lookup`.
#[async_trait]
pub trait HostServices: Send + Sync {
    /// Führt eine benannte Fähigkeit des Hosts aus.
    async fn invoke(&self, capability: &str, params: &Value) -> AppResult<Value>;
}

/// Ablage für Domainereignisse, die ein Plugin meldet.
///
/// Der Host vergibt Kennung und Zeitstempel und legt das Ereignis über
/// diesen Port ab. Die Datenbank schreibt dabei Ereignis und Outbox in
/// derselben Transaktion.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Legt ein Ereignis ab.
    async fn append_event(&self, event: &Event) -> AppResult<()>;
}

/// Quelle der Ereignisse, die der Host an Plugins verteilt.
///
/// Der Host ruft die Quelle je Plugin einmal auf und liest den Kanal in
/// einer eigenen Task. Der Kanal ist unbeschränkt, die begrenzte Warteschlange
/// des Hosts begrenzt erst die Zustellung an das Plugin.
#[async_trait]
pub trait EventSource: Send + Sync {
    /// Öffnet den Ereigniskanal für ein Plugin.
    async fn subscribe(&self, plugin: &str) -> AppResult<Pin<Box<dyn Stream<Item = Event> + Send>>>;
}

/// Umgebung ohne Fähigkeiten und ohne Ereignisse.
///
/// Der Host braucht diese Traits immer. Für Tests und für einen Betrieb ohne
//! Ereignisbus genügt diese leere Umsetzung.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopServices;

#[async_trait]
impl HostServices for NoopServices {
    async fn invoke(&self, capability: &str, _params: &Value) -> AppResult<Value> {
        Err(AppError::Validation(format!(
            "Fähigkeit {capability} ist in dieser Umgebung nicht vorhanden"
        )))
    }
}

/// Ereignisablage, die alles verwirft.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEventSink;

#[async_trait]
impl EventSink for NoopEventSink {
    async fn append_event(&self, _event: &Event) -> AppResult<()> {
        Ok(())
    }
}

/// Ereignisquelle ohne Ereignisse.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEventSource;

#[async_trait]
impl EventSource for NoopEventSource {
    async fn subscribe(&self, _plugin: &str) -> AppResult<Pin<Box<dyn Stream<Item = Event> + Send>>> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(Box::pin(ReceiverStream::new(receiver)))
    }
}

/// Ereignisquelle aus einem Sendepuffer.
///
/// Der Puffer wird von allen Plugins geteilt. Jeder abonnierte Kanal sieht
/// ab seinem Anmeldezeitpunkt alle Ereignisse.
#[derive(Debug, Clone)]
pub struct ChannelEventSource {
    sender: broadcast::Sender<Event>,
}

impl ChannelEventSource {
    /// Legt eine Quelle mit der angegebenen Puffergröße an.
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    /// Veröffentlicht ein Ereignis an alle Abonnenten.
    pub fn publish(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

#[async_trait]
impl EventSource for ChannelEventSource {
    async fn subscribe(&self, _plugin: &str) -> AppResult<Pin<Box<dyn Stream<Item = Event> + Send>>> {
        Ok(Box::pin(ReceiverStream::new(self.sender.subscribe())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    struct Mitschreiber {
        ereignisse: Mutex<Vec<Event>>,
    }

    #[async_trait]
    impl EventSink for Mitschreiber {
        async fn append_event(&self, event: &Event) -> AppResult<()> {
            self.ereignisse.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    #[test]
    fn ereignisquelle_verteilt_ereignisse_an_abonnenten() {
        let quelle = ChannelEventSource::new(8);
        let sender = quelle.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut kanal = quelle.subscribe("gitclaw").await.unwrap();
            sender.publish(Event {
                event_id: "1".to_string(),
                trace_id: "t".to_string(),
                timestamp: "jetzt".to_string(),
                topic: "task.created".to_string(),
                event_type: "created".to_string(),
                producer: "test".to_string(),
                payload: serde_json::Map::new(),
                meta: serde_json::Map::new(),
            });
            let ereignis = kanal.next().await.unwrap();
            assert_eq!(ereignis.topic, "task.created");
        });
    }

    #[tokio::test]
    async fn mitschreiber_merkt_sich_ereignisse() {
        let mitschreiber = Arc::new(Mitschreiber {
            ereignisse: Mutex::new(Vec::new()),
        });
        let sink = mitschreiber.clone();
        sink.append_event(&Event {
            event_id: "1".to_string(),
            trace_id: "t".to_string(),
            timestamp: "jetzt".to_string(),
            topic: "task.created".to_string(),
            event_type: "created".to_string(),
            producer: "test".to_string(),
            payload: serde_json::Map::new(),
            meta: serde_json::Map::new(),
        })
        .await
        .unwrap();
        assert_eq!(mitschreiber.ereignisse.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn leere_umgebung_kennt_keine_faehigkeiten() {
        let fehler = NoopServices.invoke("task.lookup", &json!({})).await;
        assert!(fehler.is_err());
    }
}
