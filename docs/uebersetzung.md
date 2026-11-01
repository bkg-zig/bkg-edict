# Übersetzung

Die Datenbank hält die kanonischen Werte des Altbestands. Die Anzeige löst
sie über die Tabellen in `crates/edict-core/src/labels.rs` ins Deutsche auf.
So bleibt der Datenbestand mit dem Altbestand vergleichbar, während die
Oberfläche und die Werkzeuge deutsch sprechen.

Unbekannte Werte bleiben unverändert. Die Übersetzung erfindet keine
Bezeichnung für einen Wert, der nicht in der Tabelle steht.

## Zustände

| Kanonischer Wert | Anzeige |
| --- | --- |
| `Taizi` | Taizi |
| `Zhongshu` | Zhongshu |
| `Menxia` | Menxia |
| `Assigned` | Zugewiesen |
| `Next` | Bereit |
| `Doing` | In Arbeit |
| `Review` | Prüfung |
| `Done` | Abgeschlossen |
| `Blocked` | Blockiert |
| `Cancelled` | Abgebrochen |
| `Pending` | Offen |

Die Werte `Taizi`, `Zhongshu` und `Menxia` bleiben in der Anzeige stehen.
Sie bezeichnen die drei Kanzleien, für die es keine deutsche Entsprechung
gibt.

## Organisationen

| Kanonischer Wert | Anzeige |
| --- | --- |
| `中书省` | Zhongshu |
| `门下省` | Menxia |
| `尚书省` | Shangshu |
| `六部` | Sechs Ministerien |
| `礼部` | Rites-Ministerium |
| `兵部` | Kriegsministerium |
| `户部` | Finanzministerium |
| `刑部` | Justizministerium |
| `工部` | Bauministerium |
| `吏部` | Personalministerium |
| `钦天监` | Astronomisches Amt |
| `皇上` | Kaiser |
| `太子` | Taizi |

## Amtsbezeichnungen

| Kanonischer Wert | Anzeige |
| --- | --- |
| `中书令` | Leiter der Zhongshu-Kanzlei |
| `侍中` | Palastprüfer |
| `尚书令` | Leiter der Shangshu-Kanzlei |
| `吏部尚书` | Personalminister |
| `户部尚书` | Finanzminister |
| `礼部尚书` | Rites-Minister |
| `兵部尚书` | Kriegsminister |
| `刑部尚书` | Justizminister |
| `工部尚书` | Bauminister |

## Weitere Werte

| Kanonischer Wert | Anzeige |
| --- | --- |
| `无` | Keine |
| `执行中` | In Ausführung |

Die Prioritäten lösen sich über eigene Regeln auf. `low` und `niedrig`
werden zu `Niedrig`, `high` und `hoch` zu `Hoch`, `urgent` und `dringend`
zu `Dringend`, `中` zu `Mittel`. Jeder andere Wert gilt als `Normal`.

## Spalten des Boards

| Schlüssel | Anzeige | Aktion |
| --- | --- | --- |
| `Inbox` | Eingang | Anweisen |
| `Taizi` | Taizi | Sortieren |
| `Zhongshu` | Zhongshu | Entwurf |
| `Menxia` | Menxia | Prüfen |
| `Assigned` | Shangshu | Zuweisen |
| `Doing` | Sechs Ministerien | Ausführen |
| `Review` | Shangshu | Sammeln |
| `Done` | Bericht | Abschließen |

## Demodaten

Die Datei `data/tasks_source.json` enthält vier Aufgaben mit ins Deutsche
übersetzten Texten. Die Legacy-Kennungen `JJC-YYYYMMDD-NNN` bleiben
unverändert, damit ein Abgleich mit dem Altbestand möglich ist. Die
Zustandswerte bleiben in der Schreibweise, die die Datenbank erwartet.

Die Übersetzung betrifft den Titel, den aktuellen Stand, das
Abnahmekriterium, den Amtsinhaber, die Organisation und jede Bemerkung im
Zustandsprotokoll. Die Zeitstempel, die Kennungen und der Dateipfad im
Feld `output` bleiben unverändert.

## Dokumentation und Oberfläche

- Die Dateien unter `docs/` sind auf Deutsch geschrieben.
- Die Fehlermeldungen des Kerns, des Servers und der CLI sind auf Deutsch.
- Die Kommentare im Quellcode sind auf Deutsch, damit der Bestand ohne
  Übersetzungsschritt lesbar bleibt.
- Die Debatten unter `docs/debate/` halten die Entscheidungen und ihre
  Begründung in deutscher Sprache fest.

Der Quellcode des Vorgängers unter `edict-2.0` bleibt unverändert. Er dient
als Referenz und wird nicht mehr produktiv ausgeführt.
