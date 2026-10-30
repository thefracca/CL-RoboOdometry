// FTC Path HUD - punto di ingresso.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod field;
mod segment; // <-- nuovo modulo
mod ui;

use app::HudApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([800.0, 560.0])
            .with_icon(assets::load_window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "FTC Path HUD",
        options,
        Box::new(|_cc| Ok(Box::new(HudApp::default()))),
    )
}
