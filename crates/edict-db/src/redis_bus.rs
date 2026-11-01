//! Redis-Streams und Pub/Sub als Ereignisbus.
//!
//! Der Datenstrom trägt die dauerhafte Arbeitsteilung, der Pub/Sub-Kanal die
//! sofortige Benachrichtigung der Oberfläche. Beides nutzt dieselbe
//! Ereignisform, damit ein Empfänger im Stream und ein Empfänger im Kanal
//! dasselbe sehen.
//!
//! Das Einfügen in den Strom und die Veröffentlichung im Kanal passieren in
//! einem Lua-Skript. Zwischen beiden kann kein weiterer Leser einsehen.

use std::collections::HashMap;

use redis::aio::{MultiplexedConnection, PubSub};
use serde_json::{Map, Value};

use edict_core::error::{AppError, AppResult};
use edict_core::model::{Event, PUBSUB_PREFIX, STREAM_PREFIX};

/// Maximale Länge eines Ereignisstroms.
pub const MAX_STREAM_LEN: usize = 10_000;

/// Lua-Skript für das gleichzeitige Einfügen und Veröffentlichen.
const PUBLISH_SCRIPT: &str = r#"
local entry = redis.call('XADD', KEYS[1], 'MAXLEN', ARGV[1], '*',
    'event_id', ARGV[2],
    'trace_id', ARGV[3],
    'timestamp', ARGV[4],
    'topic', ARGV[5],
    'event_type', ARGV[6],
    'producer', ARGV[7],
    'payload', ARGV[8],
    'meta', ARGV[9])
redis.call('PUBLISH', KEYS[2], ARGV[10])
return entry
"#;

/// Eine Nachricht aus einem Ereignisstrom.
#[derive(Debug, Clone)]
pub struct StreamMessage {
    /// Kennung des Eintrags im Strom, etwa `1720000000000-0`.
    pub id: String,
    /// Inhalt des Eintrags.
    pub event: Event,
}

/// Verbindung zum Ereignisbus.
#[derive(Clone)]
pub struct RedisBus {
    client: redis::Client,
    max_len: usize,
}

impl RedisBus {
    /// Öffnet einen Bus für die angegebene Redis-URL.
    pub fn new(redis_url: &str) -> AppResult<Self> {
        let client = redis::Client::open(redis_url).map_err(|error| {
            AppError::Redis(format!("Redis-Verbindung ungültig: {error}"))
        })?;
        Ok(Self {
            client,
            max_len: MAX_STREAM_LEN,
        })
    }

    /// Baut eine Verbindung zum Redis-Server auf.
    pub async fn connection(&self) -> AppResult<MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| AppError::Redis(format!("Redis nicht erreichbar: {error}")))
    }

    /// Name des Stroms zu einem Thema.
    pub fn stream_key(topic: &str) -> String {
        format!("{STREAM_PREFIX}{topic}")
    }

    /// Name des Pub/Sub-Kanals zu einem Thema.
    pub fn pubsub_key(topic: &str) -> String {
        format!("{PUBSUB_PREFIX}{topic}")
    }

    /// Veröffentlicht ein Ereignis im Strom und im Kanal.
    ///
    /// Die Nutzlast und die Metadaten werden als JSON-Zeichenkette abgelegt.
    /// Das entspricht dem Vertrag des bisherigen Python-Busses.
    pub async fn publish_event(&self, event: &Event) -> AppResult<String> {
        let payload_text = serde_json::to_string(&event.payload)?;
        let meta_text = serde_json::to_string(&event.meta)?;

        let mut notification = Map::new();
        notification.insert("event_id".into(), Value::String(event.event_id.clone()));
        notification.insert("trace_id".into(), Value::String(event.trace_id.clone()));
        notification.insert("timestamp".into(), Value::String(event.timestamp.clone()));
        notification.insert("topic".into(), Value::String(event.topic.clone()));
        notification.insert(
            "event_type".into(),
            Value::String(event.event_type.clone()),
        );
        notification.insert("producer".into(), Value::String(event.producer.clone()));
        notification.insert("payload".into(), Value::String(payload_text.clone()));
        notification.insert("meta".into(), Value::String(meta_text.clone()));
        let notification_text = serde_json::to_string(&Value::Object(notification))?;

        let mut connection = self.connection().await?;
        let script = redis::Script::new(PUBLISH_SCRIPT);
        script
            .key(Self::stream_key(&event.topic))
            .key(Self::pubsub_key(&event.topic))
            .arg(self.max_len)
            .arg(&event.event_id)
            .arg(&event.trace_id)
            .arg(&event.timestamp)
            .arg(&event.topic)
            .arg(&event.event_type)
            .arg(&event.producer)
            .arg(&payload_text)
            .arg(&meta_text)
            .arg(&notification_text)
            .invoke_async::<String>(&mut connection)
            .await
            .map_err(|error| AppError::Redis(format!("Ereignis nicht veröffentlicht: {error}")))
    }

    /// Legt eine Konsumentengruppe an, falls sie noch fehlt.
    pub async fn ensure_group(&self, topic: &str, group: &str) -> AppResult<()> {
        let mut connection = self.connection().await?;
        let result: redis::RedisResult<String> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(Self::stream_key(topic))
            .arg(group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut connection)
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) if error.to_string().contains("BUSYGROUP") => Ok(()),
            Err(error) => Err(AppError::Redis(format!(
                "Konsumentengruppe {group} konnte nicht angelegt werden: {error}"
            ))),
        }
    }

    /// Liest neue Nachrichten für eine Konsumentengruppe.
    pub async fn read_group(
        &self,
        topic: &str,
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: u64,
    ) -> AppResult<Vec<StreamMessage>> {
        let mut connection = self.connection().await?;
        let raw: redis::Value = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(count)
            .arg("BLOCK")
            .arg(block_ms)
            .arg("STREAMS")
            .arg(Self::stream_key(topic))
            .arg(">")
            .query_async(&mut connection)
            .await
            .map_err(|error| AppError::Redis(format!("Strom nicht lesbar: {error}")))?;

        Ok(parse_read_reply(&raw))
    }

    /// Bestätigt eine Nachricht.
    pub async fn ack(&self, topic: &str, group: &str, entry_id: &str) -> AppResult<()> {
        let mut connection = self.connection().await?;
        let _: i64 = redis::cmd("XACK")
            .arg(Self::stream_key(topic))
            .arg(group)
            .arg(entry_id)
            .query_async(&mut connection)
            .await
            .map_err(|error| AppError::Redis(format!("Bestätigung fehlgeschlagen: {error}")))?;
        Ok(())
    }

    /// Übernimmt Nachrichten, deren Consumer abgestürzt ist.
    pub async fn claim_stale(
        &self,
        topic: &str,
        group: &str,
        consumer: &str,
        min_idle_ms: u64,
        count: usize,
    ) -> AppResult<Vec<StreamMessage>> {
        let mut connection = self.connection().await?;
        let raw: redis::Value = redis::cmd("XAUTOCLAIM")
            .arg(Self::stream_key(topic))
            .arg(group)
            .arg(consumer)
            .arg(min_idle_ms)
            .arg("0-0")
            .arg("COUNT")
            .arg(count)
            .query_async(&mut connection)
            .await
            .map_err(|error| {
                AppError::Redis(format!("Überfällige Nachrichten nicht übernommen: {error}"))
            })?;

        Ok(parse_claim_reply(&raw))
    }

    /// Öffnet eine Pub/Sub-Verbindung für ein Muster.
    pub async fn subscribe_pattern(&self, pattern: &str) -> AppResult<PubSub> {
        let mut pubsub = self
            .client
            .get_async_pubsub()
            .await
            .map_err(|error| AppError::Redis(format!("Pub/Sub nicht erreichbar: {error}")))?;
        pubsub
            .psubscribe(pattern)
            .await
            .map_err(|error| AppError::Redis(format!("Muster {pattern} nicht abonnierbar: {error}")))?;
        Ok(pubsub)
    }

    /// Prüft, ob der Bus erreichbar ist.
    pub async fn is_healthy(&self) -> bool {
        match self.connection().await {
            Ok(mut connection) => redis::cmd("PING")
                .query_async::<String>(&mut connection)
                .await
                .is_ok(),
            Err(_) => false,
        }
    }
}

/// Liest die Antwort von `XREADGROUP`.
fn parse_read_reply(raw: &redis::Value) -> Vec<StreamMessage> {
    let mut messages = Vec::new();
    let redis::Value::Array(streams) = raw else {
        return messages;
    };
    for stream in streams {
        let redis::Value::Array(parts) = stream else {
            continue;
        };
        let Some(entries) = parts.get(1) else { continue };
        let redis::Value::Array(entries) = entries else {
            continue;
        };
        for entry in entries {
            if let Some(message) = parse_entry(entry) {
                messages.push(message);
            }
        }
    }
    messages
}

/// Liest die Antwort von `XAUTOCLAIM`.
fn parse_claim_reply(raw: &redis::Value) -> Vec<StreamMessage> {
    let redis::Value::Array(parts) = raw else {
        return Vec::new();
    };
    let Some(entries) = parts.get(1) else {
        return Vec::new();
    };
    let redis::Value::Array(entries) = entries else {
        return Vec::new();
    };
    entries.iter().filter_map(parse_entry).collect()
}

/// Wandelt einen Strom-Eintrag in eine Nachricht.
fn parse_entry(entry: &redis::Value) -> Option<StreamMessage> {
    let redis::Value::Array(parts) = entry else {
        return None;
    };
    let id = value_to_string(parts.first()?);
    let fields = value_to_map(parts.get(1)?);
    let event = Event {
        event_id: fields.get("event_id").cloned().unwrap_or_default(),
        trace_id: fields.get("trace_id").cloned().unwrap_or_default(),
        timestamp: fields.get("timestamp").cloned().unwrap_or_default(),
        topic: fields.get("topic").cloned().unwrap_or_default(),
        event_type: fields.get("event_type").cloned().unwrap_or_default(),
        producer: fields.get("producer").cloned().unwrap_or_default(),
        payload: parse_json_field(fields.get("payload")),
        meta: parse_json_field(fields.get("meta")),
    };
    Some(StreamMessage { id, event })
}

/// Liest eine flache Liste aus Schlüssel-Wert-Paaren.
fn value_to_map(value: &redis::Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    match value {
        redis::Value::Array(items) => {
            let mut iterator = items.iter();
            while let (Some(key), Some(item)) = (iterator.next(), iterator.next()) {
                out.insert(value_to_string(key), value_to_string(item));
            }
        }
        redis::Value::Map(entries) => {
            for (key, item) in entries {
                out.insert(value_to_string(key), value_to_string(item));
            }
        }
        _ => {}
    }
    out
}

/// Liest einen skalaren Wert als Zeichenkette.
fn value_to_string(value: &redis::Value) -> String {
    match value {
        redis::Value::Nil => String::new(),
        redis::Value::Int(number) => number.to_string(),
        redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        redis::Value::SimpleString(text) => text.clone(),
        redis::Value::Double(number) => number.to_string(),
        redis::Value::BigNumber(number) => number.clone(),
        _ => String::new(),
    }
}

/// Liest ein JSON-Feld und liefert eine leere Map bei Fehlern.
fn parse_json_field(raw: Option<&String>) -> Map<String, Value> {
    let Some(raw) = raw else {
        return Map::new();
    };
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}
