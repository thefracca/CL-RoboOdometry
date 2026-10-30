// FTC Path HUD - punto di ingresso.
// Qui si dichiarano solo i moduli e si avvia la finestra.

mod app;   // src/app.rs        -> stato dell'app
mod field; // src/field.rs      -> costanti del campo e conversione coordinate
mod ui;    // src/ui/mod.rs     -> pannelli dell'interfaccia

use app::HudApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FTC Path HUD",
        options,
        Box::new(|_cc| Ok(Box::new(HudApp::default()))),
    )
}
