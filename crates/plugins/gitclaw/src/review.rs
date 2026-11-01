//! Auswertung der Webhook-Ereignisse von gitclaw.
//!
//! Das Plugin versteht vier Arten von Ereignissen:
//!
//! - `push` mit den geänderten Dateien aus den Commits,
//! - `pull_request` mit den Dateien des Vergleichs,
//! - `issue` mit einem neuen Ticket,
//! - `comment` mit einem Kommentar zu einem Ticket.
//!
//! Jede Art liefert die betroffenen Dateien. Ohne Dateien gibt es nichts zu
//! prüfen, und das Plugin meldet das an den Host zurück.

use serde_json::{Map, Value};

/// Art eines eingehenden Webhooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookArt {
    /// Änderung in einem Zweig.
    Push,
    /// Änderung über einen Merge-Auftrag.
    PullRequest,
    /// Neues Ticket.
    Issue,
    /// Kommentar zu einem Ticket.
    Comment,
    /// Unbekannte Art.
    Unbekannt,
}

impl WebhookArt {
    /// Liest die Art aus der Kopfzeile des Webhooks.
    pub fn aus_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "push" => WebhookArt::Push,
            "pull_request" | "pullrequest" => WebhookArt::PullRequest,
            "issue" => WebhookArt::Issue,
            "comment" | "issue_comment" => WebhookArt::Comment,
            _ => WebhookArt::Unbekannt,
        }
    }

    /// Name der Art, wie er im Ereignis steht.
    pub fn name(self) -> &'static str {
        match self {
            WebhookArt::Push => "push",
            WebhookArt::PullRequest => "pull_request",
            WebhookArt::Issue => "issue",
            WebhookArt::Comment => "comment",
            WebhookArt::Unbekannt => "unknown",
        }
    }

    /// Meldet, ob die Art vom Plugin verarbeitet wird.
    pub fn bekannt(self) -> bool {
        self != WebhookArt::Unbekannt
    }
}

/// Ausgewertetes Webhook-Ereignis.
#[derive(Debug, Clone)]
pub struct WebhookEreignis {
    /// Art des Ereignisses.
    pub art: WebhookArt,
    /// Name des Lagers, soweit vorhanden.
    pub lager: Option<String>,
    /// Betroffene Dateien.
    pub dateien: Vec<String>,
    /// Kurze Zusammenfassung des Ereignisses.
    pub zusammenfassung: String,
}

/// Wertet einen Webhook-Körper aus.
pub fn werte_aus(art: WebhookArt, koerper: &[u8]) -> Option<WebhookEreignis> {
    let wert: Value = serde_json::from_slice(koerper).ok()?;
    let objekt = wert.as_object()?;
    let lager = objekt
        .get("repository")
        .and_then(Value::as_object)
        .and_then(|repo| repo.get("full_name"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let dateien = betroffene_dateien(art, objekt);
    let zusammenfassung = zusammenfassung(art, objekt, &dateien);
    Some(WebhookEreignis {
        art,
        lager,
        dateien,
        zusammenfassung,
    })
}

/// Ermittelt die betroffenen Dateien eines Ereignisses.
pub fn betroffene_dateien(art: WebhookArt, objekt: &Map<String, Value>) -> Vec<String> {
    let mut dateien: Vec<String> = Vec::new();
    match art {
        WebhookArt::Push => {
            if let Some(commits) = objekt.get("commits").and_then(Value::as_array) {
                for commit in commits {
                    sammle_dateien(commit.get("added"), &mut dateien);
                    sammle_dateien(commit.get("modified"), &mut dateien);
                }
            }
            sammle_dateien(objekt.get("files"), &mut dateien);
        }
        WebhookArt::PullRequest => {
            sammle_dateien(objekt.get("files"), &mut dateien);
            if let Some(auftrag) = objekt.get("pull_request").and_then(Value::as_object) {
                sammle_dateien(auftrag.get("files"), &mut dateien);
            }
        }
        WebhookArt::Issue | WebhookArt::Comment | WebhookArt::Unbekannt => {}
    }
    dateien.sort();
    dateien.dedup();
    dateien
}

/// Nimmt Dateinamen aus einem Feld auf.
///
/// Das Feld kann eine Liste von Namen oder eine Liste von Objekten mit dem
/// Feld `filename` sein.
fn sammle_dateien(feld: Option<&Value>, ziel: &mut Vec<String>) {
    let Some(liste) = feld.and_then(Value::as_array) else {
        return;
    };
    for eintrag in liste {
        match eintrag {
            Value::String(name) => ziel.push(name.clone()),
            Value::Object(objekt) => {
                if let Some(name) = objekt
                    .get("filename")
                    .or_else(|| objekt.get("path"))
                    .and_then(Value::as_str)
                {
                    ziel.push(name.to_string());
                }
            }
            _ => {}
        }
    }
}

/// Baut eine kurze Zusammenfassung des Ereignisses.
fn zusammenfassung(art: WebhookArt, objekt: &Map<String, Value>, dateien: &[String]) -> String {
    match art {
        WebhookArt::Push => {
            let anzahl = objekt
                .get("commits")
                .and_then(Value::as_array)
                .map(|commits| commits.len())
                .unwrap_or_default();
            format!("{anzahl} Commits, {} Dateien betroffen", dateien.len())
        }
        WebhookArt::PullRequest => {
            let titel = objekt
                .get("pull_request")
                .and_then(Value::as_object)
                .and_then(|auftrag| auftrag.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("ohne Titel");
            format!("Merge-Auftrag {titel}, {} Dateien betroffen", dateien.len())
        }
        WebhookArt::Issue => {
            let titel = objekt
                .get("issue")
                .and_then(Value::as_object)
                .and_then(|ticket| ticket.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("ohne Titel");
            format!("Ticket {titel}")
        }
        WebhookArt::Comment => {
            let text = objekt
                .get("comment")
                .and_then(Value::as_object)
                .and_then(|kommentar| kommentar.get("body"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let kurz: String = text.chars().take(120).collect();
            format!("Kommentar {kurz}")
        }
        WebhookArt::Unbekannt => "Unbekanntes Ereignis".to_string(),
    }
}

/// Baut die Nutzlast für den Prüfauftrag an den Host.
pub fn auftrag_nutzlast(ereignis: &WebhookEreignis) -> Value {
    serde_json::json!({
        "art": ereignis.art.name(),
        "lager": ereignis.lager,
        "dateien": ereignis.dateien,
        "zusammenfassung": ereignis.zusammenfassung,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn push_liefert_die_geaenderten_dateien() {
        let koerper = json!({
            "repository": {"full_name": "beispiel/projekt"},
            "commits": [
                {"added": ["neu.rs"], "modified": ["alt.rs"]},
                {"modified": ["alt.rs", "weitere.rs"]}
            ]
        });
        let ereignis = werte_aus(WebhookArt::Push, koerper.to_string().as_bytes()).unwrap();
        assert_eq!(ereignis.dateien, vec!["alt.rs", "neu.rs", "weitere.rs"]);
        assert_eq!(ereignis.lager.as_deref(), Some("beispiel/projekt"));
    }

    #[test]
    fn pull_request_liefert_die_dateien_des_vergleichs() {
        let koerper = json!({
            "pull_request": {"title": "Änderung"},
            "files": [{"filename": "src/lib.rs"}]
        });
        let ereignis = werte_aus(WebhookArt::PullRequest, koerper.to_string().as_bytes()).unwrap();
        assert_eq!(ereignis.dateien, vec!["src/lib.rs"]);
        assert!(ereignis.zusammenfassung.contains("Änderung"));
    }

    #[test]
    fn issue_hat_keine_dateien() {
        let koerper = json!({"issue": {"title": "Fehler"}});
        let ereignis = werte_aus(WebhookArt::Issue, koerper.to_string().as_bytes()).unwrap();
        assert!(ereignis.dateien.is_empty());
    }

    #[test]
    fn kaputtes_json_liefert_kein_ereignis() {
        assert!(werte_aus(WebhookArt::Push, b"kein json").is_none());
    }

    #[test]
    fn unbekannte_art_wird_benannt() {
        let art = WebhookArt::aus_name("release");
        assert_eq!(art, WebhookArt::Unbekannt);
        assert!(!art.bekannt());
    }
}
