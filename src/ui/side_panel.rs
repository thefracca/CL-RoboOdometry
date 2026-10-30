// Pannello laterale: comandi e lista dei punti.

use eframe::egui;

use crate::app::HudApp;

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::Panel::left("side").show(ui, |ui| {
        ui.heading("Punti");
        if ui.button("Cancella tutto").clicked() {
            app.points.clear();
            app.selected = None;
        }
        ui.label("Click: aggiungi punto\nTrascina: sposta\nTasto destro: elimina");
        ui.separator();
        for (i, p) in app.points.iter().enumerate() {
            ui.label(format!("{}: x={:.1}  y={:.1} cm", i, p[0], p[1]));
        }
    });
}
