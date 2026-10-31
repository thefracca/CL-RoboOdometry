use eframe::egui;
use crate::app::{EditMode, HudApp};
use crate::segment::{HeadingMode, Segment};

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::Panel::left("side").resizable(true).show(ui, |ui| {
        ui.heading("FTC Path HUD");
        ui.separator();

        ui.label("Modalità:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.mode, EditMode::Select,     "✋ Seleziona");
            ui.selectable_value(&mut app.mode, EditMode::AddLine,    "📏 Line");
            ui.selectable_value(&mut app.mode, EditMode::AddCurve,   "〜 Curva");
            ui.selectable_value(&mut app.mode, EditMode::AddBarrier, "🚧 Barriera");
        });
        ui.label(match app.mode {
            EditMode::Select     => "Trascina punti/robot\nShift+drag robot = ruota",
            EditMode::AddLine    => "Click = aggiunge nodo linea",
            EditMode::AddCurve   => "Click = aggiunge nodo curva",
            EditMode::AddBarrier => "Click = vertice\nDoppio click = chiude poligono\nTasto destro = annulla ultimo vertice",
        });

        ui.separator();

        if ui.button("🗑 Cancella percorso").clicked() {
            app.segments.clear();
        }
        if ui.button("🗑 Cancella barriere").clicked() {
            app.barriers.clear();
        }
        ui.checkbox(&mut app.hide_grid, "Nascondi griglia");

        ui.separator();

        ui.heading("Robot");
        ui.label(format!("Pos: x={:.1}  y={:.1} cm", app.robot_pos[0], app.robot_pos[1]));
        ui.label(format!("Heading: {:.1}°", app.robot_heading));
        ui.add(
            egui::Slider::new(&mut app.robot_heading, 0.0..=359.9)
                .text("°")
                .step_by(1.0),
        );

        ui.separator();

        ui.heading("Percorso");
        ui.label(format!("Start: x={:.1}  y={:.1} cm", app.start[0], app.start[1]));
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

        ui.separator();

        ui.heading("Barriere");
        if app.barriers.is_empty() {
            ui.label("(nessuna)");
        }
        let mut barrier_remove: Option<usize> = None;
        for (i, b) in app.barriers.iter().enumerate() {
            ui.group(|ui| {
                let status = if b.closed { "chiusa" } else { "aperta" };
                ui.label(format!("#{i} — {} vertici ({})", b.vertices.len(), status));
                if ui.small_button("✖ elimina").clicked() {
                    barrier_remove = Some(i);
                }
            });
        }
        if let Some(i) = barrier_remove {
            app.barriers.remove(i);
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
