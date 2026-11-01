//! Prüfung der Webhook-Signatur.
//!
//! Der Host gitclaw erwartet eine Signatur nach dem Muster `sha256=<hex>`.
//! Die Prüfung rechnet HMAC-SHA-256 über den Rohkörper nach. Das
//! Vergleichsverfahren prüft alle Bytes, damit die Prüfzeit nicht von der
//! Stelle der ersten Abweichung abhängt.

use sha2::{Digest, Sha256};

/// Länge eines SHA-256-Werts in Bytes.
const HASH_LEN: usize = 32;

/// Blockgröße von HMAC nach RFC 2104.
const BLOCK_LEN: usize = 64;

/// Prüfer für eine Webhook-Signatur.
///
/// Die Prüfung sitzt hinter einem Trait, damit Tests und der Betrieb
/// unterschiedliche Prüfungen einsetzen können.
pub trait SignaturPruefer: Send + Sync {
    /// Meldet, ob die Signatur zum Körper gehört.
    fn pruefe(&self, koerper: &[u8], signatur: &str) -> bool;

    /// Meldet, ob der Prüfer überhaupt eingerichtet ist.
    fn bereit(&self) -> bool {
        true
    }
}

/// Prüfer für HMAC-SHA-256.
pub struct HmacSha256 {
    schluessel: Vec<u8>,
}

impl HmacSha256 {
    /// Legt einen Prüfer mit dem angegebenen Schlüssel an.
    pub fn new(schluessel: impl Into<Vec<u8>>) -> Self {
        Self {
            schluessel: schluessel.into(),
        }
    }

    /// Berechnet die Signatur über den Körper.
    pub fn signiere(&self, koerper: &[u8]) -> String {
        let mac = hmac_sha256(&self.schluessel, koerper);
        format!("sha256={}", hex(&mac))
    }
}

impl SignaturPruefer for HmacSha256 {
    fn pruefe(&self, koerper: &[u8], signatur: &str) -> bool {
        let erwartet = match signatur.strip_prefix("sha256=") {
            Some(teil) => teil,
            None => return false,
        };
        let erwartete_bytes = match dehex(erwartet) {
            Some(bytes) if bytes.len() == HASH_LEN => bytes,
            _ => return false,
        };
        let berechnet = hmac_sha256(&self.schluessel, koerper);
        konstante_gleichheit(&berechnet, &erwartete_bytes)
    }

    fn bereit(&self) -> bool {
        !self.schluessel.is_empty()
    }
}

/// Prüfer, der jede Signatur ablehnt.
///
/// Der Prüfer kommt zum Einsatz, wenn kein Schlüssel hinterlegt ist. So
/// nimmt das Plugin keine Webhooks ohne Prüfung an.
pub struct KeinSchluessel;

impl SignaturPruefer for KeinSchluessel {
    fn pruefe(&self, _koerper: &[u8], _signatur: &str) -> bool {
        false
    }

    fn bereit(&self) -> bool {
        false
    }
}

/// Berechnet HMAC-SHA-256 über die Daten.
///
/// Die Implementierung folgt RFC 2104. Sie nutzt nur `sha2`, damit keine
/// weitere Abhängigkeit nötig ist.
pub fn hmac_sha256(schluessel: &[u8], daten: &[u8]) -> [u8; HASH_LEN] {
    let mut block = [0u8; BLOCK_LEN];
    if schluessel.len() > BLOCK_LEN {
        let digest = Sha256::digest(schluessel);
        block[..HASH_LEN].copy_from_slice(&digest);
    } else {
        block[..schluessel.len()].copy_from_slice(schluessel);
    }

    let mut inner_pad = [0x36u8; BLOCK_LEN];
    let mut outer_pad = [0x5cu8; BLOCK_LEN];
    for index in 0..BLOCK_LEN {
        inner_pad[index] ^= block[index];
        outer_pad[index] ^= block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(daten);
    let innere = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(innere);
    let ergebnis = outer.finalize();

    let mut ausgabe = [0u8; HASH_LEN];
    ausgabe.copy_from_slice(&ergebnis);
    ausgabe
}

/// Vergleicht zwei Bytefolgen ohne frühzeitigen Abbruch.
pub fn konstante_gleichheit(links: &[u8], rechts: &[u8]) -> bool {
    if links.len() != rechts.len() {
        return false;
    }
    let mut unterschied = 0u8;
    for index in 0..links.len() {
        unterschied |= links[index] ^ rechts[index];
    }
    unterschied == 0
}

/// Wandelt Bytes in Kleinbuchstaben-Hexadezimal.
pub fn hex(bytes: &[u8]) -> String {
    let mut ausgabe = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        ausgabe.push(nibble(byte >> 4));
        ausgabe.push(nibble(byte & 0x0f));
    }
    ausgabe
}

/// Wandelt eine Hexadezimalzahl in ein Byte.
fn nibble(wert: u8) -> char {
    match wert {
        0..=9 => (b'0' + wert) as char,
        10..=15 => (b'a' + wert - 10) as char,
        _ => '0',
    }
}

/// Liest Hexadezimalzeichen als Bytes.
pub fn dehex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut ausgabe = Vec::with_capacity(bytes.len() / 2);
    for paar in bytes.chunks(2) {
        let hoch = hexwert(paar[0])?;
        let niedrig = hexwert(paar[1])?;
        ausgabe.push((hoch << 4) | niedrig);
    }
    Some(ausgabe)
}

/// Liest ein Hexadezimalzeichen.
fn hexwert(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_entspricht_der_norm() {
        // Prüfvektor aus RFC 4231, Fall 1.
        let schluessel = [0x0bu8; 20];
        let daten = b"Hi There";
        let erwartet = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        let mac = hmac_sha256(&schluessel, daten);
        assert_eq!(hex(&mac), erwartet);
    }

    #[test]
    fn hmac_arbeitet_mit_langen_schluesseln() {
        // Prüfvektor aus RFC 4231, Fall 6.
        let schluessel = [0xaau8; 131];
        let daten = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let erwartet = "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54";
        let mac = hmac_sha256(&schluessel, daten);
        assert_eq!(hex(&mac), erwartet);
    }

    #[test]
    fn gueltige_signatur_wird_angenommen() {
        let pruefer = HmacSha256::new(b"geheim");
        let koerper = b"{\"event\":\"push\"}";
        let signatur = pruefer.signiere(koerper);
        assert!(pruefer.pruefe(koerper, &signatur));
    }

    #[test]
    fn veraenderter_koerper_wird_abgewiesen() {
        let pruefer = HmacSha256::new(b"geheim");
        let signatur = pruefer.signiere(b"{\"event\":\"push\"}");
        assert!(!pruefer.pruefe(b"{\"event\":\"issue\"}", &signatur));
    }

    #[test]
    fn falsches_prefix_wird_abgewiesen() {
        let pruefer = HmacSha256::new(b"geheim");
        let signatur = pruefer.signiere(b"{}");
        let ohne = signatur.trim_start_matches("sha256=");
        assert!(!pruefer.pruefe(b"{}", ohne));
    }

    #[test]
    fn hex_und_dehex_sind_gegenseitig() {
        let bytes = vec![0x00, 0x0f, 0xa5, 0xff];
        assert_eq!(hex(&bytes), "000fa5ff");
        assert_eq!(dehex("000fa5ff"), Some(bytes));
    }

    #[test]
    fn ohne_schluessel_wird_alles_abgewiesen() {
        let pruefer = KeinSchluessel;
        assert!(!pruefer.bereit());
        assert!(!pruefer.pruefe(b"{}", "sha256=00"));
    }
}
