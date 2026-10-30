// Pannello laterale: modalità di editing e lista segmenti.

use eframe::egui;
use crate::app::{EditMode, HudApp};
use crate::segment::{HeadingMode, Segment};

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::Panel::left("side").resizable(true).show(ui, |ui| {
        ui.heading("FTC Path HUD");
        ui.separator();

        // --- Modalità editing ---
        ui.label("Modalità:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.mode, EditMode::Select, "✋ Seleziona");
            ui.selectable_value(&mut app.mode, EditMode::AddLine, "📏 Line");
            ui.selectable_value(&mut app.mode, EditMode::AddCurve, "〜 Curva");
        });
        ui.label(match app.mode {
            EditMode::Select => "Trascina: sposta punti/robot\nShift+trascina robot: ruota",
            EditMode::AddLine => "Click: aggiunge nodo linea",
            EditMode::AddCurve => "Click: aggiunge nodo curva\n(poi sposta il control point)",
        });

        ui.separator();

        // --- Controlli generali ---
        if ui.button("🗑 Cancella percorso").clicked() {
            app.segments.clear();
        }
        ui.checkbox(&mut app.hide_grid, "Nascondi griglia");

        ui.separator();

        // --- Robot ---
        ui.heading("Robot");
        ui.label(format!(
            "Pos: x={:.1}  y={:.1} cm",
            app.robot_pos[0], app.robot_pos[1]
        ));
        ui.label(format!("Heading: {:.1}°", app.robot_heading));
        ui.add(
            egui::Slider::new(&mut app.robot_heading, 0.0..=359.9)
                .text("°")
                .step_by(1.0),
        );

        ui.separator();

        // --- Lista segmenti ---
        ui.heading("Percorso");
        ui.label(format!(
            "Start: x={:.1}  y={:.1} cm",
            app.start[0], app.start[1]
        ));
        if app.segments.is_empty() {
            ui.label("(nessun segmento)");
        }
        let mut to_remove: Option<usize> = None;
        for (i, seg) in app.segments.iter_mut().enumerate() {
            ui.group(|ui| {
                match seg {
                    Segment::Line { to, heading } => {
                        ui.label(format!("#{i} Line → ({:.1}, {:.1})", to[0], to[1]));
                        heading_selector(ui, heading);
                    }
                    Segment::Curve { controls, to, heading } => {
                        ui.label(format!(
                            "#{i} Curve → ({:.1}, {:.1})  [{} ctrl]",
                            to[0], to[1], controls.len()
                        ));
                        heading_selector(ui, heading);
                    }
                }
                if ui.small_button("✖ elimina").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove {
            app.segments.remove(i);
        }
    });
}

fn heading_selector(ui: &mut egui::Ui, heading: &mut HeadingMode) {
    let is_tangent = *heading == HeadingMode::Tangent;
    let mut tangent = is_tangent;
    ui.horizontal(|ui| {
        ui.checkbox(&mut tangent, "Tangent");
        if tangent != is_tangent {
            *heading = if tangent {
                HeadingMode::Tangent
            } else {
                HeadingMode::Fixed(0.0)
            };
        }
        if let HeadingMode::Fixed(angle) = heading {
            ui.add(egui::DragValue::new(angle).speed(1.0).suffix("°"));
        }
    });
}
