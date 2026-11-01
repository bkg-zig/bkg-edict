//! Formatierung der Ausgaben der Kommandozeile.

use std::io::IsTerminal;

use anyhow::Result;
use serde::Serialize;

/// Zurücksetzen der Terminalfarben.
pub const RESET: &str = "\u{1b}[0m";
/// Fette Schrift.
pub const BOLD: &str = "\u{1b}[1m";
/// Grüne Schrift.
pub const GREEN: &str = "\u{1b}[32m";
/// Gelbe Schrift.
pub const YELLOW: &str = "\u{1b}[33m";
/// Rote Schrift.
pub const RED: &str = "\u{1b}[31m";
/// Cyanfarbene Schrift.
pub const CYAN: &str = "\u{1b}[36m";

/// Zelle einer Tabelle.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Sichtbarer Text der Zelle.
    pub text: String,
    /// Optionaler Farbcode der Zelle.
    pub style: Option<&'static str>,
}

impl Cell {
    /// Zelle ohne Farbe.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
        }
    }

    /// Zelle mit Farbe.
    pub fn styled(text: impl Into<String>, style: &'static str) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
        }
    }
}

/// Prüft, ob die Standardausgabe ein Terminal ist.
pub fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

/// Entscheidet, ob JSON ausgegeben wird.
///
/// Ohne Terminal wird immer JSON ausgegeben, damit Skripte eine
/// maschinenlesbare Antwort erhalten.
pub fn wants_json(flag: bool) -> bool {
    flag || !stdout_is_terminal()
}

/// Gibt einen Wert als JSON aus.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}

/// Baut eine Tabelle mit gleich breiten Spalten.
pub fn render_table(headers: &[Cell], rows: &[Vec<Cell>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|cell| width(&cell.text)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(width(&cell.text));
            }
        }
    }

    let mut out = String::new();
    let header_cells: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(index, cell)| render_cell(cell, widths[index]))
        .collect();
    out.push_str(header_cells.join("  ").trim_end());
    out.push('\n');

    let separator: Vec<String> = widths.iter().map(|size| "-".repeat(*size)).collect();
    out.push_str(separator.join("  ").trim_end());
    out.push('\n');

    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                if index < widths.len() {
                    render_cell(cell, widths[index])
                } else {
                    cell.text.clone()
                }
            })
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out
}

/// Sichtbare Breite eines Textes.
fn width(text: &str) -> usize {
    text.chars().count()
}

/// Schreibt eine Zelle mit Farbe und fester Breite.
fn render_cell(cell: &Cell, size: usize) -> String {
    let padding = size.saturating_sub(width(&cell.text));
    let body = match cell.style {
        Some(style) => format!("{style}{}{RESET}", cell.text),
        None => cell.text.clone(),
    };
    format!("{body}{}", " ".repeat(padding))
}

/// Kürzt einen Text auf die angegebene Länge.
pub fn shorten(text: &str, max_chars: usize) -> String {
    edict_core::labels::truncate(text, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spalten_werden_ausgeglichen() {
        let headers = vec![Cell::plain("ID"), Cell::plain("Titel")];
        let rows = vec![vec![Cell::plain("JJC-1"), Cell::plain("Kurzer Text")]];
        let table = render_table(&headers, &rows);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[0], "ID     Titel");
        assert_eq!(lines[1], "-----  -----------");
        assert_eq!(lines[2], "JJC-1  Kurzer Text");
    }

    #[test]
    fn leere_tabelle_zeigt_nur_den_kopf() {
        let headers = vec![Cell::plain("ID")];
        let table = render_table(&headers, &[]);
        assert_eq!(table.lines().count(), 2);
    }

    #[test]
    fn farbe_veraendert_die_sichtbare_breite_nicht() {
        let cell = Cell::styled("Done", GREEN);
        let rendered = render_cell(&cell, 6);
        assert!(rendered.ends_with("Done\u{1b}[0m  "));
        assert_eq!(width("Done"), 4);
    }

    #[test]
    fn texte_werden_gekuerzt() {
        assert_eq!(shorten("abcdef", 4), "abc…");
        assert_eq!(shorten("ab", 4), "ab");
    }
}
