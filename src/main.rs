#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod barrier;
mod export;
mod field;
mod project;
mod segment;
mod simulation;
mod ui;

use app::HudApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(assets::load_window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "FTC Path HUD",
        options,
        Box::new(|_cc| Ok(Box::new(HudApp::default()))),
    )
}
