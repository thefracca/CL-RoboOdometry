use eframe::egui::{self, Button, Color32, RichText, Vec2};
use crate::app::{EditMode, HudApp};
use crate::segment::{Mechanism, RobotAction, Segment};
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
            mode_btn(ui, app, EditMode::Select,      "✋ Select");
            mode_btn(ui, app, EditMode::AddLine,     "📏 Line");
            ui.end_row();
            mode_btn(ui, app, EditMode::AddCurve,   "〜 Curva");
            mode_btn(ui, app, EditMode::AddBarrier, "🚧 Barriera");
            ui.end_row();
            mode_btn(ui, app, EditMode::AddFocalLine,"📐 Focale");
            mode_btn(ui, app, EditMode::AddSearch,   "🔍 Ricerca");
            ui.end_row();
            mode_btn(ui, app, EditMode::AddHighlight,"🖍 Evidenzia");
            ui.end_row();
        });
        ui.label(RichText::new(match app.mode {
            EditMode::Select      => "Trascina punti/robot\nShift+drag robot = ruota",
            EditMode::AddLine     => "Click = nodo linea",
            EditMode::AddCurve    => "Click = nodo curva\nDoppio click = +ctrl pt",
            EditMode::AddBarrier  => "Click = vertice\nDoppio click = chiudi\nTasto dx = annulla",
            EditMode::AddHighlight=> "Trascina sul percorso per\nevidenziare un tratto\n(tipo scelto sotto)",
            EditMode::AddSearch   => "Click = punto di arrivo\nArea: dall'ultimo punto a qui",
            EditMode::AddFocalLine=> "Click 1 = punto A\nClick 2 = punto B → crea linea",
        }).small().weak());

        // Lista linee focali
        if !app.focal_lines.is_empty() {
            ui.separator();
            ui.label(RichText::new("Linee focali").small().strong());
            let mut remove_fl: Option<usize> = None;
            for (i, fl) in app.focal_lines.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut fl.visible, "");
                    ui.label(RichText::new(format!("F{i}")).small());
                    ui.add(egui::DragValue::new(&mut fl.snap_radius_cm)
                        .speed(1.0).range(2.0..=50.0).suffix("cm"));
                    if ui.small_button("✖").clicked() { remove_fl = Some(i); }
                });
            }
            if let Some(i) = remove_fl { app.focal_lines.remove(i); }
        }

        // ── Evidenziatore intake / flywheel ──────────────────
        ui.separator();
        ui.label(RichText::new("Evidenziatore").small().strong());
        ui.horizontal(|ui| {
            for (k, label, col) in [
                (Mechanism::Intake,   "INTAKE",   Color32::from_rgb(0, 200, 100)),
                (Mechanism::Flywheel, "FLYWHEEL", Color32::from_rgb(255, 160, 0)),
            ] {
                let sel = app.highlight_kind == k;
                let btn = Button::new(RichText::new(label).color(Color32::WHITE).small())
                    .fill(if sel { col } else { Color32::from_gray(60) });
                if ui.add_sized(Vec2::new(80.0, 26.0), btn).clicked() {
                    app.highlight_kind = k;
                    app.mode = EditMode::AddHighlight;
                }
            }
        });
        if app.highlights.is_empty() {
            ui.label(RichText::new("(nessun tratto evidenziato)").small().weak());
        }
        let mut remove_hl: Option<usize> = None;
        for (i, h) in app.highlights.iter().enumerate() {
            ui.horizontal(|ui| {
                let (name, col) = match h.kind {
                    Mechanism::Intake   => ("INTAKE",   Color32::from_rgb(0, 200, 100)),
                    Mechanism::Flywheel => ("FLYWHEEL", Color32::from_rgb(255, 160, 0)),
                };
                ui.label(RichText::new(format!("{name} {} → {}", fmt_pos(h.from), fmt_pos(h.to)))
                    .small().color(col));
                if ui.small_button("✖").clicked() { remove_hl = Some(i); }
            });
        }
        if let Some(i) = remove_hl { app.highlights.remove(i); }

        ui.separator();

        // ── Azioni ──────────────────────────────────────────
        ui.label("Azioni");
        egui::Grid::new("action_grid").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            if ui.add_sized(BTN, Button::new("🗑 Percorso")).clicked() {
                app.clear_path();
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
        indicator(ui, "🔍 SEARCH", app.sim.state == SimState::Searching, Color32::from_rgb(170, 90, 255));
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
                    app.sim.start(&app.segments);
                    app.robot_pos     = app.start;
                    app.robot_heading = 90.0;
                }
                if app.sim.state == SimState::Finished {
                    ui.label(RichText::new("✅ Percorso completato").color(Color32::GREEN).small());
                }
            }
            SimState::Running | SimState::Rotating { .. } | SimState::Waiting { .. } | SimState::ShootStop { .. } | SimState::ShootMove | SimState::Searching => {
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
                    SimState::Rotating { current, to, .. } =>
                        ui.label(RichText::new(format!("↺ ROT {:.0}° → {:.0}°", current, to))
                            .color(Color32::from_rgb(180, 180, 255)).size(14.0)),
                    SimState::Waiting { remaining } =>
                        ui.label(RichText::new(format!("⏸ WAIT {:.1}s", remaining))
                            .color(Color32::from_rgb(150, 150, 255)).size(14.0)),
                    SimState::ShootStop { remaining } =>
                        ui.label(RichText::new(format!("🎯 SHOOT+STOP {:.1}s", remaining))
                            .color(Color32::YELLOW).size(14.0)),
                    SimState::ShootMove =>
                        ui.label(RichText::new("🎯 SHOOT+MOVE")
                            .color(Color32::from_rgb(255,180,0)).size(14.0)),
                    SimState::Searching => {
                        let (el, lim) = match app.segments.get(app.sim.seg_index) {
                            Some(Segment::Search { search, .. }) => (app.sim.seg_time, search.time_limit),
                            _ => (0.0, 0.0),
                        };
                        if el >= lim {
                            ui.label(RichText::new("🔍 LIMITE → DIRETTO")
                                .color(Color32::from_rgb(255, 160, 0)).size(14.0))
                        } else {
                            ui.label(RichText::new(format!("🔍 CERCO PALLINE {:.1}/{:.1}s", el, lim))
                                .color(Color32::from_rgb(200, 140, 255)).size(14.0))
                        }
                    }
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
                Segment::Search { to, .. } => format!("#{i} 🔍 Search → ({:.0},{:.0})", to[0], to[1]),
            };
            let active = app.sim.is_running() && app.sim.seg_index == i;
            egui::CollapsingHeader::new(
                RichText::new(&label)
                    .color(if active { Color32::YELLOW } else { Color32::WHITE })
                    .small()
            )
            .id_source(format!("seg_{i}"))
            .show(ui, |ui| {
                // Ricerca palline: pannello dedicato, niente controllo velocità completo
                if let Segment::Search { search, opts, .. } = &mut app.segments[i] {
                    search_ui(ui, search, opts);
                    if ui.small_button("✖ elimina segmento").clicked() {
                        to_remove = Some(i);
                    }
                    return;
                }

                let is_line  = matches!(app.segments[i], Segment::Line { .. });
                let opts = app.segments[i].opts_mut();

                // Velocità
                ui.label(RichText::new("Velocità").small().strong());
                ui.add(egui::Slider::new(&mut opts.speed.v_start, 0.0..=300.0)
                    .text("v_start cm/s").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.v_max, 1.0..=300.0)
                    .text("v_max cm/s").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.speed.accel, 1.0..=500.0)
                    .text("accel cm/s²").step_by(1.0));
                if is_line {
                    ui.add(egui::Slider::new(&mut opts.speed.decel, 1.0..=500.0)
                        .text("decel cm/s²").step_by(1.0));
                    ui.add(egui::Slider::new(&mut opts.speed.v_min, 1.0..=150.0)
                        .text("v_min frenata cm/s").step_by(1.0));
                    ui.add(egui::Slider::new(&mut opts.speed.brake_point, 0.1..=0.99)
                        .text("frena a (% lungh.)").step_by(0.01));
                    ui.label(RichText::new("(trascina il punto ▼ sul campo)").small().weak());
                } else {
                    ui.label(RichText::new("Curva: nessun punto di frenata,\nsi ferma sul posto a fine segmento").small().weak());
                }

                ui.separator();

                // Heading robot (mecanum — indipendente dal movimento)
                ui.label(RichText::new("Heading robot (mecanum)").small().strong());
                ui.label(RichText::new("Il robot guarda in questa direzione\nmentre si muove lungo il segmento").small().weak());
                ui.add(egui::Slider::new(&mut opts.robot_heading_start, 0.0..=359.0)
                    .text("inizio °").step_by(1.0));
                ui.add(egui::Slider::new(&mut opts.robot_heading_end, 0.0..=359.0)
                    .text("fine °").step_by(1.0));
                ui.label(RichText::new(format!(
                    "Rotazione segmento: {:.0}°",
                    {
                        let mut d = (opts.robot_heading_end - opts.robot_heading_start).rem_euclid(360.0);
                        if d > 180.0 { d -= 360.0; }
                        d
                    }
                )).small().weak());
                ui.add(egui::Slider::new(&mut opts.rotation_speed, 10.0..=720.0)
                    .text("vel. rot. °/s").step_by(5.0));
                ui.label(RichText::new("(velocità rotazione all'inizio di questo segmento)").small().weak());

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


                if ui.small_button("✖ elimina segmento").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove { app.remove_segment(i); }

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

        // ── Tempo stimato ─────────────────────────────────────
        if !app.segments.is_empty() {
            let estimated = crate::simulation::estimate_total_time(app.start, &app.segments);
            let color = if estimated > 30.0 { Color32::RED }
                        else if estimated > 25.0 { Color32::YELLOW }
                        else { Color32::GREEN };
            ui.label(RichText::new(format!("⏱ Tempo stimato: {:.1}s / 30s", estimated))
                .color(color).small());
        }

        if !app.segments.is_empty() {
            ui.separator();
            speed_graph(ui, app);
        }
        ui.separator();

        // ── Progetto ──────────────────────────────────────────
        ui.label("Progetto");
        egui::Grid::new("proj_grid").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            if ui.add_sized(BTN, Button::new(
                RichText::new("💾 Salva").color(Color32::BLACK)
            ).fill(Color32::from_rgb(80, 180, 80))).clicked() {
                let p = std::path::Path::new("progetto.hudproject");
                if let Err(e) = crate::project::save(app, p) {
                    eprintln!("Salvataggio fallito: {e}");
                }
            }
            if ui.add_sized(BTN, Button::new(
                RichText::new("📂 Carica").color(Color32::BLACK)
            ).fill(Color32::from_rgb(80, 130, 200))).clicked() {
                let p = std::path::Path::new("progetto.hudproject");
                if let Err(e) = crate::project::load(app, p) {
                    eprintln!("Caricamento fallito: {e}");
                }
            }
            ui.end_row();
        });
        ui.label(RichText::new("→ progetto.hudproject nella cartella exe").small().weak());

        ui.separator();

        // ── Export JSON ───────────────────────────────────────
        ui.label("Export");
        if ui.add_sized(
            Vec2::new(246.0, 36.0),
            Button::new(RichText::new("📤 Esporta JSON").color(Color32::BLACK))
                .fill(Color32::from_rgb(200, 200, 50))
        ).clicked() {
            let json = crate::export::to_json(app);
            let path = std::path::Path::new("path.json");
            if let Err(e) = std::fs::write(path, &json) {
                eprintln!("Export fallito: {e}");
            }
            ui.output_mut(|o| o.copied_text = json);
        }
        ui.label(RichText::new("→ path.json + copia negli appunti").small().weak());
        ui.label(RichText::new("Snap: Ctrl=griglia  Shift=orto(barriere)").small().weak());

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

fn indicator(ui: &mut egui::Ui, label: &str, active: bool, color: Color32) {
    let fill = if active { color } else { Color32::from_gray(40) };
    let text = RichText::new(label).color(Color32::WHITE).small();
    ui.add_sized(Vec2::new(80.0, 24.0), Button::new(text).fill(fill).sense(egui::Sense::hover()));
}

/// Pannello di un segmento "Ricerca palline".
fn search_ui(
    ui: &mut egui::Ui,
    search: &mut crate::segment::SearchParams,
    opts: &mut crate::segment::SegmentOptions,
) {
    ui.label(RichText::new("🔍 Ricerca palline (camera)").small().strong());
    ui.label(RichText::new("Linea retta a velocità costante,\nsi ferma sul posto all'arrivo").small().weak());
    ui.add(egui::Slider::new(&mut search.v, 1.0..=300.0).text("v cm/s").step_by(1.0));
    ui.add(egui::Slider::new(&mut search.width, 10.0..=200.0).text("larghezza area cm").step_by(1.0));
    ui.add(egui::Slider::new(&mut search.time_limit, 0.5..=15.0).text("tempo limite s").step_by(0.1));
    ui.label(RichText::new("(scaduto il tempo va diretto al punto finale)").small().weak());

    ui.separator();
    ui.checkbox(&mut search.shoot_after, "Spara fermo a fine ricerca");
    if search.shoot_after {
        ui.add(egui::Slider::new(&mut search.shoot_secs, 0.1..=10.0).text("s").step_by(0.1));
    }

    ui.separator();
    ui.label(RichText::new("Heading robot (mecanum)").small().strong());
    ui.add(egui::Slider::new(&mut opts.robot_heading_start, 0.0..=359.0).text("inizio °").step_by(1.0));
    ui.add(egui::Slider::new(&mut opts.robot_heading_end, 0.0..=359.0).text("fine °").step_by(1.0));
    ui.add(egui::Slider::new(&mut opts.rotation_speed, 10.0..=720.0).text("vel. rot. °/s").step_by(5.0));

    ui.separator();
}

/// Posizione lungo il percorso → "#segmento·percentuale"
fn fmt_pos(p: f32) -> String {
    format!("#{}·{:.0}%", p.floor().max(0.0) as usize, (p - p.floor()) * 100.0)
}

/// Piccolo grafico velocità/tempo dell'intero percorso (stima), con cursore durante la simulazione.
fn speed_graph(ui: &mut egui::Ui, app: &HudApp) {
    let trace = crate::simulation::speed_trace(app.start, &app.segments);
    let total = trace.last().map(|p| p[0]).unwrap_or(0.0);
    let t_max = total.max(MATCH_DURATION);
    let v_top = trace.iter().map(|p| p[1]).fold(10.0_f32, f32::max) * 1.1;

    ui.label(RichText::new("Velocità nel tempo").small().strong());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 90.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_gray(25));

    let to_p = |t: f32, v: f32| egui::Pos2::new(
        rect.left() + t / t_max * rect.width(),
        rect.bottom() - v / v_top * rect.height(),
    );

    // Limite dei 30 s
    let x30 = to_p(MATCH_DURATION, 0.0).x;
    painter.line_segment(
        [egui::Pos2::new(x30, rect.top()), egui::Pos2::new(x30, rect.bottom())],
        egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 80, 80, 120)),
    );

    let pts: Vec<egui::Pos2> = trace.iter().map(|p| to_p(p[0], p[1])).collect();
    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, Color32::from_rgb(100, 180, 255))));

    // Cursore del tempo di simulazione
    if app.sim.is_running() || app.sim.state == SimState::Finished {
        let x = to_p(app.sim.elapsed.min(t_max), 0.0).x;
        painter.line_segment(
            [egui::Pos2::new(x, rect.top()), egui::Pos2::new(x, rect.bottom())],
            egui::Stroke::new(1.5, Color32::YELLOW),
        );
    }

    painter.text(
        rect.left_top() + Vec2::new(3.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0} cm/s", v_top / 1.1),
        egui::FontId::proportional(9.0),
        Color32::GRAY,
    );
    painter.text(
        rect.right_bottom() + Vec2::new(-3.0, -2.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0} s", t_max),
        egui::FontId::proportional(9.0),
        Color32::GRAY,
    );
}
