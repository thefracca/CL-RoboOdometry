use eframe::egui::{self, Button, Color32, RichText, Vec2};
use crate::app::{EditMode, HudApp};
use crate::segment::{HeadingMode, RobotAction, Segment};
use crate::simulation::SimState as SS;
use crate::simulation::{SimState, MATCH_DURATION};

const BTN: Vec2 = Vec2::new(120.0, 36.0);

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(6.0);
        ui.heading("FTC Path HUD");
        ui.separator();

        // ── Modalità ────────────────────────────────────────
        ui.label("Modalità");
        egui::Grid::new("mode_grid").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            mode_btn(ui, app, EditMode::Select,     "✋ Select");
            mode_btn(ui, app, EditMode::AddLine,    "📏 Line");
            ui.end_row();
            mode_btn(ui, app, EditMode::AddCurve,  "〜 Curva");
            mode_btn(ui, app, EditMode::AddBarrier,"🚧 Barriera");
            ui.end_row();
        });
        ui.label(RichText::new(match app.mode {
            EditMode::Select     => "Trascina punti/robot\nShift+drag robot = ruota",
            EditMode::AddLine    => "Click = nodo linea",
            EditMode::AddCurve   => "Click = nodo curva\nDoppio click = +ctrl pt",
            EditMode::AddBarrier => "Click = vertice\nDoppio click = chiudi\nTasto dx = annulla",
        }).small().weak());

        ui.separator();

        // ── Azioni ──────────────────────────────────────────
        ui.label("Azioni");
        egui::Grid::new("action_grid").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            if ui.add_sized(BTN, Button::new("🗑 Percorso")).clicked() {
                app.segments.clear();
            }
            if ui.add_sized(BTN, Button::new("🗑 Barriere")).clicked() {
                app.barriers.clear();
            }
            ui.end_row();
            let gl = if app.hide_grid { "🔲 Griglia ON" } else { "🔳 Griglia OFF" };
            if ui.add_sized(BTN, Button::new(gl)).clicked() {
                app.hide_grid = !app.hide_grid;
            }
            if ui.add_sized(BTN, Button::new("📍 Robot → Start")).clicked() {
                app.robot_pos     = app.start;
                app.robot_heading = 90.0;
            }
            ui.end_row();
        });

        ui.separator();

        // ── Simulazione ──────────────────────────────────────
        ui.label("Simulazione");

        // Indicatori intake / flywheel
        ui.horizontal(|ui| {
            indicator(ui, "INTAKE",   app.intake_active,   Color32::from_rgb(0, 200, 100));
            indicator(ui, "FLYWHEEL", app.flywheel_active, Color32::from_rgb(255, 160, 0));
        });
        ui.add_space(4.0);

        let remaining = (MATCH_DURATION - app.sim.elapsed).max(0.0);
        let timer_color = if remaining < 5.0 { Color32::RED }
                          else if remaining < 10.0 { Color32::YELLOW }
                          else { Color32::GREEN };

        match &app.sim.state.clone() {
            SimState::Idle | SimState::Finished => {
                let label = if app.sim.state == SimState::Finished { "▶ Riavvia" } else { "▶ Avvia" };
                let enabled = !app.segments.is_empty();
                let btn = Button::new(RichText::new(label).color(Color32::BLACK))
                    .fill(if enabled { Color32::from_rgb(80, 200, 80) } else { Color32::from_gray(80) });
                if ui.add_sized(BTN, btn).clicked() && enabled {
                    app.sim.start();
                    app.robot_pos     = app.start;
                    app.robot_heading = 90.0;
                }
                if app.sim.state == SimState::Finished {
                    ui.label(RichText::new("✅ Percorso completato").color(Color32::GREEN).small());
                }
            }
            SimState::Running | SimState::Waiting { .. } | SimState::ShootStop { .. } | SimState::ShootMove => {
                if ui.add_sized(BTN, Button::new(
                    RichText::new("⏹ Stop").color(Color32::WHITE)
                ).fill(Color32::from_rgb(180, 50, 50))).clicked() {
                    app.sim.stop();
                    app.intake_active   = false;
                    app.flywheel_active = false;
                }
                ui.add_space(4.0);
                ui.label(RichText::new(format!("⏱ {:.1}s", remaining))
                    .color(timer_color).size(22.0));
                match &app.sim.state {
                    SimState::Waiting { remaining } =>
                        ui.label(RichText::new(format!("⏸ WAIT {:.1}s", remaining))
                            .color(Color32::from_rgb(150, 150, 255)).size(14.0)),
                    SimState::ShootStop { remaining } =>
                        ui.label(RichText::new(format!("🎯 SHOOT+STOP {:.1}s", remaining))
                            .color(Color32::YELLOW).size(14.0)),
                    SimState::ShootMove =>
                        ui.label(RichText::new("🎯 SHOOT+MOVE")
                            .color(Color32::from_rgb(255,180,0)).size(14.0)),
                    _ => ui.label(""),
                };
            }
        }

        ui.separator();

        // ── Robot ─────────────────────────────────────────────
        ui.label("Robot");
        ui.label(RichText::new(format!(
            "x={:.1}  y={:.1}\nHeading {:.1}°",
            app.robot_pos[0], app.robot_pos[1], app.robot_heading
        )).small());
        ui.add(egui::Slider::new(&mut app.robot_heading, 0.0..=359.9).text("°").step_by(1.0));

        ui.separator();

        // ── Percorso ──────────────────────────────────────────
        ui.label("Percorso");
        ui.label(RichText::new(format!("Start  ({:.0}, {:.0})", app.start[0], app.start[1])).small());

        if app.segments.is_empty() {
            ui.label(RichText::new("(nessun segmento)").small().weak());
        }

        let mut to_remove: Option<usize> = None;
        let n = app.segments.len();
        for i in 0..n {
            let label = match &app.segments[i] {
                Segment::Line  { to, .. } => format!("#{i} Line  → ({:.0},{:.0})", to[0], to[1]),
                Segment::Curve { to, .. } => format!("#{i} Curve → ({:.0},{:.0})", to[0], to[1]),
            };
            let active = app.sim.is_running() && app.sim.seg_index == i;
            egui::CollapsingHeader::new(
                RichText::new(&label)
                    .color(if active { Color32::YELLOW } else { Color32::WHITE })
                    .small()
            )
            .id_source(format!("seg_{i}"))
            .show(ui, |ui| {
                let opts = app.segments[i].opts_mut();

                // Velocità
                ui.label(RichText::new("Velocità").small().strong());
                ui.add(egui::Slider::new(&mut opts.speed.v_start, 0.0..=300.0)
                    .text("v_start cm/s").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.v_max, 1.0..=300.0)
                    .text("v_max cm/s").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.accel, 1.0..=500.0)
                    .text("accel cm/s²").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.decel, 1.0..=500.0)
                    .text("decel cm/s²").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.brake_point, 0.1..=0.99)
                    .text("frena a t=").step_by(0.01));
                ui.label(RichText::new("(trascina il punto ▼ sul campo)").small().weak());

                ui.separator();

                // Heading percorso (tangent/fixed)
                ui.label(RichText::new("Heading percorso").small().strong());
                heading_selector(ui, &mut opts.heading);

                ui.separator();

                // Heading robot (mecanum — indipendente dal movimento)
                ui.label(RichText::new("Heading robot (mecanum)").small().strong());
                ui.label(RichText::new("Il robot guarda in questa direzione\nmentre si muove lungo il segmento").small().weak());
                ui.add(egui::Slider::new(&mut opts.robot_heading_start, 0.0..=359.0)
                    .text("inizio °").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.robot_heading_end, 0.0..=359.0)
                    .text("fine °").step_by(1.0));
                ui.label(RichText::new(format!(
                    "Rotazione: {:.0}°",
                    {
                        let mut d = (opts.robot_heading_end - opts.robot_heading_start).rem_euclid(360.0);
                        if d > 180.0 { d -= 360.0; }
                        d
                    }
                )).small().weak());

                ui.separator();

                // Azione
                ui.label(RichText::new("Azione").small().strong());
                let is_wait      = matches!(opts.action, RobotAction::Wait(_));
                let is_stop      = matches!(opts.action, RobotAction::ShootStop(_));
                let is_move_shoot= matches!(opts.action, RobotAction::ShootMove(_));
                ui.horizontal(|ui| {
                    if ui.selectable_label(
                        matches!(opts.action, RobotAction::Move), "Move"
                    ).clicked() { opts.action = RobotAction::Move; }
                    if ui.selectable_label(is_wait, "Wait").clicked() {
                        opts.action = RobotAction::Wait(1.0);
                    }
                    if ui.selectable_label(is_stop, "Shoot+Stop").clicked() {
                        opts.action = RobotAction::ShootStop(1.0);
                    }
                    if ui.selectable_label(is_move_shoot, "Shoot+Move").clicked() {
                        opts.action = RobotAction::ShootMove(1.0);
                    }
                });
                if let RobotAction::Wait(secs)
                     | RobotAction::ShootStop(secs)
                     | RobotAction::ShootMove(secs) = &mut opts.action {
                    ui.add(egui::Slider::new(secs, 0.1..=10.0).text("s").step_by(0.1));
                }

                ui.separator();

                // Meccanismi
                ui.label(RichText::new("Meccanismi").small().strong());
                ui.horizontal(|ui| {
                    toggle_btn(ui, &mut opts.intake,   "INTAKE",   Color32::from_rgb(0, 200, 100));
                    toggle_btn(ui, &mut opts.flywheel, "FLYWHEEL", Color32::from_rgb(255, 160, 0));
                });

                if ui.small_button("✖ elimina segmento").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove { app.segments.remove(i); }

        ui.separator();

        // ── Barriere ──────────────────────────────────────────
        ui.label("Barriere");
        if app.barriers.is_empty() {
            ui.label(RichText::new("(nessuna)").small().weak());
        }
        let mut barrier_remove: Option<usize> = None;
        for (i, b) in app.barriers.iter().enumerate() {
            ui.group(|ui| {
                let st = if b.closed { "✓" } else { "…" };
                ui.label(RichText::new(format!("{st} B{i} — {} v", b.vertices.len())).small());
                if ui.small_button("✖").clicked() { barrier_remove = Some(i); }
            });
        }
        if let Some(i) = barrier_remove { app.barriers.remove(i); }

        ui.separator();

        // ── Export ────────────────────────────────────────────
        ui.label("Export");
        if ui.add_sized(
            Vec2::new(246.0, 36.0),
            Button::new(
                RichText::new("💾 Esporta JSON").color(Color32::BLACK)
            ).fill(Color32::from_rgb(200, 200, 50))
        ).clicked() {
            let json = crate::export::to_json(app);
            // Salva nella directory corrente come "path.json"
            let path = std::path::Path::new("path.json");
            match std::fs::write(path, &json) {
                Ok(_) => { /* ok */ }
                Err(e) => { eprintln!("Errore export: {e}"); }
            }
            // Copia anche negli appunti se possibile (non blocca se fallisce)
            ui.output_mut(|o| o.copied_text = json);
        }
        ui.label(RichText::new("→ salva path.json + copia negli appunti").small().weak());

        ui.add_space(8.0);
    });
}

// ── Widget helpers ────────────────────────────────────────────

fn mode_btn(ui: &mut egui::Ui, app: &mut HudApp, mode: EditMode, label: &str) {
    let active = app.mode == mode;
    let fill = if active { Color32::from_rgb(0, 100, 200) }
               else { ui.visuals().widgets.inactive.bg_fill };
    let text = if active { RichText::new(label).color(Color32::WHITE) }
               else { RichText::new(label) };
    if ui.add_sized(BTN, Button::new(text).fill(fill)).clicked() {
        app.mode = mode;
    }
}

fn toggle_btn(ui: &mut egui::Ui, val: &mut bool, label: &str, on_color: Color32) {
    let fill = if *val { on_color } else { Color32::from_gray(60) };
    let text = RichText::new(label).color(Color32::WHITE).small();
    if ui.add_sized(Vec2::new(80.0, 28.0), Button::new(text).fill(fill)).clicked() {
        *val = !*val;
    }
}

fn indicator(ui: &mut egui::Ui, label: &str, active: bool, color: Color32) {
    let fill = if active { color } else { Color32::from_gray(40) };
    let text = RichText::new(label).color(Color32::WHITE).small();
    ui.add_sized(Vec2::new(80.0, 24.0), Button::new(text).fill(fill).sense(egui::Sense::hover()));
}

fn heading_selector(ui: &mut egui::Ui, heading: &mut HeadingMode) {
    let is_tangent = *heading == HeadingMode::Tangent;
    let mut tangent = is_tangent;
    ui.horizontal(|ui| {
        ui.checkbox(&mut tangent, "Tangent");
        if tangent != is_tangent {
            *heading = if tangent { HeadingMode::Tangent } else { HeadingMode::Fixed(0.0) };
        }
        if let HeadingMode::Fixed(a) = heading {
            ui.add(egui::DragValue::new(a).speed(1.0).suffix("°"));
        }
    });
}
