//! Startpunkt der Weboberflaeche von Edict 2.0.

mod api;
mod app;
mod components;
mod models;
mod styles;
mod views;

fn main() {
    // Ohne diesen Hook bleiben Rust-Panics im Browser stumm. Der Hook schreibt
    // die Panic-Meldung in die Konsole, damit Fehler dort lesbar bleiben.
    console_error_panic_hook::set_once();
    dioxus::launch(app::App);
}
