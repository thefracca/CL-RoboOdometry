use eframe::egui::{
    self, pos2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense,
    Stroke, Vec2,
};
use std::f32::consts::PI;

use crate::app::{Dragging, EditMode, HudApp};
use crate::barrier::Barrier;
use crate::field::{FieldView, FIELD_CM, TILE_CM};
use crate::segment::Segment;

const WAYPOINT_RADIUS: f32 = 12.0;
/// Margine di esclusione intorno alle barriere: metà robot + 1 cm di errore
const EXCLUSION_CM: f32 = ROBOT_SIZE_CM / 2.0 + 1.0;
const CTRL_RADIUS: f32 = 10.0;
const VERTEX_RADIUS: f32 = 10.0;
const HIT_EXTRA: f32 = 8.0;
const ROBOT_SIZE_CM: f32 = 45.0;

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    // ── Shortcut da tastiera ──────────────────────────────────
    ui.ctx().input(|i| {
        // Modalità
        if i.key_pressed(egui::Key::Num1) { app.mode = EditMode::Select; }
        if i.key_pressed(egui::Key::Num2) { app.mode = EditMode::AddLine; }
        if i.key_pressed(egui::Key::Num3) { app.mode = EditMode::AddCurve; }
        if i.key_pressed(egui::Key::Num4) { app.mode = EditMode::AddBarrier; }
        if i.key_pressed(egui::Key::Num5) { app.mode = EditMode::AddFocalLine; }
        if i.key_pressed(egui::Key::Num6) { app.mode = EditMode::AddSearch; }
        // Griglia
        if i.key_pressed(egui::Key::G) { app.hide_grid = !app.hide_grid; }
        // Robot → Start
        if i.key_pressed(egui::Key::Home) {
            app.robot_pos     = app.start;
            app.robot_heading = 90.0;
        }
        // Simulazione play/stop
        if i.key_pressed(egui::Key::Space) {
            if app.sim.is_running() {
                app.sim.stop();
                app.intake_active   = false;
                app.flywheel_active = false;
            } else if !app.segments.is_empty() {
                app.sim.start(&app.segments);
                app.robot_pos     = app.start;
                app.robot_heading = 90.0;
            }
        }
        // Cancella percorso
        if i.key_pressed(egui::Key::Delete) && i.modifiers.shift {
            app.segments.clear();
        }
        // Salva / Carica
        if i.key_pressed(egui::Key::S) && i.modifiers.ctrl {
            let _ = crate::project::save(app, std::path::Path::new("progetto.hudproject"));
        }
        if i.key_pressed(egui::Key::O) && i.modifiers.ctrl {
            let _ = crate::project::load(app, std::path::Path::new("progetto.hudproject"));
        }
        // Export JSON
        if i.key_pressed(egui::Key::E) && i.modifiers.ctrl {
            let json = crate::export::to_json(app);
            let _ = std::fs::write("path.json", &json);
        }
        // Undo ultimo segmento
        if i.key_pressed(egui::Key::Z) && i.modifiers.ctrl {
            app.segments.pop();
        }
        // Undo ultimo vertice barriera
        if i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && i.modifiers.shift {
            if let Some(b) = app.barriers.last_mut() {
                if !b.closed { b.vertices.pop(); }
            }
        }
    });

    let shift = ui.ctx().input(|i| i.modifiers.shift);

    {
        let avail = ui.available_size();
        let side = avail.x.min(avail.y);
        let (response, painter) =
            ui.allocate_painter(Vec2::splat(side), Sense::click_and_drag());
        let field = FieldView { rect: response.rect };

        draw_field(&painter, &field, app);
        handle_input(&response, &field, app, ui.ctx());
        draw_path(&painter, &field, app);
        draw_brake_points(&painter, &field, app);
        draw_barriers(&painter, &field, app);
        draw_barrier_exclusion_zones(&painter, &field, app);
        draw_preview(&painter, &field, app, response.hover_pos(), shift);
        draw_robot(&painter, &field, app);
        draw_mouse_coords(&painter, &response, &field);
    }
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("1", "Select"),
    ("2", "Line"),
    ("3", "Curva"),
    ("4", "Barriera"),
    ("5", "Focale"),
    ("6", "Ricerca palline"),
    ("G", "Griglia + snap ON/OFF"),
    ("Space", "Play / Stop sim"),
    ("Home", "Robot → Start"),
    ("Ctrl+Z", "Undo ultimo segmento"),
    ("Ctrl+Shift+Z", "Undo vertice barriera"),
    ("Ctrl+S", "Salva progetto"),
    ("Ctrl+O", "Carica progetto"),
    ("Ctrl+E", "Export JSON"),
    ("Shift+Del", "Cancella percorso"),
    ("Shift + click", "Snap ortho (H/V)"),
    ("Shift + drag", "Ruota robot"),
    ("Tasto dx", "Elimina punto"),
    ("Dbl click", "+ctrl point curva"),
];

/// Pannello shortcut (colonna destra).
pub fn show_shortcuts(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.heading("⌨ Shortcut");
    ui.separator();
    egui::Grid::new("shortcut_grid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        for (key, desc) in SHORTCUTS {
            ui.label(egui::RichText::new(*key).monospace().strong());
            ui.label(egui::RichText::new(*desc).small());
            ui.end_row();
        }
    });
}

// ════════════════════════════════════════════════════════════
// Input
// ════════════════════════════════════════════════════════════

fn handle_input(response: &Response, field: &FieldView, app: &mut HudApp, ctx: &egui::Context) {
    let shift = ctx.input(|i| i.modifiers.shift);
    let ctrl  = ctx.input(|i| i.modifiers.ctrl);

    // ── Drag start ──────────────────────────────────────────
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);

            if is_near_robot(pos, field, app) {
                app.dragging = if shift {
                    Dragging::RotateRobot { start_x: pos.x, start_heading: app.robot_heading }
                } else {
                    Dragging::Robot
                };
            } else if let Some((si, ci)) = control_point_at(pos, field, app) {
                app.dragging = Dragging::ControlPoint(si, ci);
            } else if let Some(ni) = waypoint_at(pos, field, app) {
                app.dragging = Dragging::Node(ni);
            } else if let Some(si) = brake_point_at(pos, field, app) {
                app.dragging = Dragging::BrakePoint(si);
            } else if let Some((bi, vi)) = barrier_vertex_at(pos, field, app) {
                app.dragging = Dragging::BarrierVertex(bi, vi);
            } else if let Some(bi) = barrier_body_at(pos, field, app) {
                app.dragging = Dragging::Barrier(bi);
            } else {
                app.dragging = Dragging::Nothing;
            }
        }
    }

    // ── Durante drag ────────────────────────────────────────
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = apply_snap(fp, None, !app.hide_grid, shift, &app.focal_lines);
            let delta_screen = response.drag_delta();
            let scale = field.rect.width() / FIELD_CM;
            let delta_cm = [delta_screen.x / scale, -delta_screen.y / scale];

            match app.dragging.clone() {
                Dragging::Robot => { app.robot_pos = clamped; }
                Dragging::RotateRobot { start_x, start_heading } => {
                    let delta = pos.x - start_x;
                    app.robot_heading = (start_heading - delta).rem_euclid(360.0);
                }
                Dragging::Node(0) => { app.start = clamped; }
                Dragging::Node(i) => {
                    if let Some(seg) = app.segments.get_mut(i - 1) {
                        seg.set_end(clamped);
                    }
                }
                Dragging::ControlPoint(si, ci) => {
                    if let Some(Segment::Curve { controls, .. }) = app.segments.get_mut(si) {
                        if let Some(cp) = controls.get_mut(ci) {
                            *cp = clamped;
                        }
                    }
                }
                Dragging::BrakePoint(si) => {
                    // Calcola il t più vicino al punto del mouse sul segmento
                    if let Some(seg) = app.segments.get(si) {
                        let from = if si == 0 { app.start } else { app.segments[si-1].end() };
                        let pts = seg.sample(from, 120);
                        let t = closest_t_on_path(&pts, field, pos);
                        if let Some(opts) = app.segments.get_mut(si).map(|s| s.opts_mut()) {
                            opts.speed.brake_point = t.clamp(0.1, 0.99);
                        }
                    }
                }
                Dragging::Barrier(bi) => {
                    if let Some(b) = app.barriers.get_mut(bi) {
                        b.translate(delta_cm[0], delta_cm[1]);
                    }
                }
                Dragging::BarrierVertex(bi, vi) => {
                    if let Some(b) = app.barriers.get_mut(bi) {
                        if let Some(v) = b.vertices.get_mut(vi) {
                            *v = clamped;
                        }
                    }
                }
                Dragging::Nothing => {}
            }
        }
    }

    if response.drag_stopped() {
        // Se il punto finale è bloccato, rimanda l'elemento alla posizione più vicina valida.
        // Per semplicità: se è bloccato lo lasciamo dove era prima (il clamp nel drag
        // lo ha già tenuto nel campo; qui ricontrolliamo le barriere).
        match &app.dragging {
            Dragging::Node(0) => {
                if point_blocked_by_barrier(app.start, app) {
                    // cerca la posizione valida più vicina lungo la normale
                    // Per ora: snap al bordo della exclusion zone (push_out)
                    // Importiamo direttamente la logica da barrier
                    use crate::barrier::push_out_of_barriers;
                    app.start = push_out_of_barriers(app.start, &app.barriers, EXCLUSION_CM);
                }
            }
            Dragging::Node(i) => {
                let i = *i;
                if i > 0 && i <= app.segments.len() {
                    let end = app.segments[i-1].end();
                    if point_blocked_by_barrier(end, app) {
                        use crate::barrier::push_out_of_barriers;
                        let fixed = push_out_of_barriers(end, &app.barriers, EXCLUSION_CM);
                        app.segments[i-1].set_end(fixed);
                    }
                }
            }
            _ => {}
        }
        app.dragging = Dragging::Nothing;
    }

    // ── Click sinistro ──────────────────────────────────────
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            // Riferimento per snap ortho: ultimo vertice della barriera aperta
            let ortho_ref = match app.mode {
                EditMode::AddBarrier => app.barriers.last().and_then(|b| {
                    if b.closed { None } else { b.vertices.last().copied() }
                }),
                EditMode::AddLine | EditMode::AddCurve | EditMode::AddSearch => Some(last_point(app)),
                EditMode::AddFocalLine => app.focal_line_first,
                _ => None,
            };
            let clamped = apply_snap(fp, ortho_ref, !app.hide_grid, shift, &app.focal_lines);

            let on_existing = is_near_robot(pos, field, app)
                || waypoint_at(pos, field, app).is_some()
                || control_point_at(pos, field, app).is_some()
                || barrier_vertex_at(pos, field, app).is_some();

            if !on_existing {
                let blocked = point_blocked_by_barrier(clamped, app);
                match app.mode {
                    EditMode::AddLine => {
                        if !blocked {
                            let mut opts = crate::segment::SegmentOptions::default();
                            // Continua dall'heading finale del segmento precedente
                            if let Some(prev_seg) = app.segments.last() {
                                opts.robot_heading_start = prev_seg.opts().robot_heading_end;
                                opts.robot_heading_end   = prev_seg.opts().robot_heading_end;
                            }
                            app.segments.push(Segment::Line { to: clamped, opts });
                        }
                    }
                    EditMode::AddCurve => {
                        if !blocked {
                            let prev = last_point(app);
                            let ctrl = [
                                (prev[0] + clamped[0]) / 2.0,
                                (prev[1] + clamped[1]) / 2.0 + 30.0,
                            ];
                            let mut opts = crate::segment::SegmentOptions::default();
                            if let Some(prev_seg) = app.segments.last() {
                                opts.robot_heading_start = prev_seg.opts().robot_heading_end;
                                opts.robot_heading_end   = prev_seg.opts().robot_heading_end;
                            }
                            app.segments.push(Segment::Curve {
                                controls: vec![ctrl],
                                to: clamped,
                                opts,
                            });
                        }
                    }
                    EditMode::AddSearch => {
                        if !blocked {
                            let mut opts = crate::segment::SegmentOptions::default();
                            if let Some(prev_seg) = app.segments.last() {
                                opts.robot_heading_start = prev_seg.opts().robot_heading_end;
                                opts.robot_heading_end   = prev_seg.opts().robot_heading_end;
                            }
                            app.segments.push(Segment::Search {
                                to: clamped,
                                search: crate::segment::SearchParams::default(),
                                opts,
                            });
                        }
                    }
                    EditMode::AddBarrier => {
                        // Per le barriere il check non si applica (stai costruendo la barriera stessa)
                        if let Some(b) = app.barriers.last_mut() {
                            if !b.closed {
                                b.vertices.push(clamped);
                                return;
                            }
                        }
                        let mut b = Barrier::new();
                        b.vertices.push(clamped);
                        app.barriers.push(b);
                    }
                    EditMode::AddFocalLine => {
                        if let Some(first) = app.focal_line_first {
                            // Secondo click: crea la linea
                            if first != clamped {
                                app.focal_lines.push(crate::app::FocalLine::new(first, clamped));
                            }
                            app.focal_line_first = None;
                            app.mode = EditMode::Select;
                        } else {
                            // Primo click: salva punto A
                            app.focal_line_first = Some(clamped);
                        }
                    }
                    EditMode::Select => {}
                }
            }
        }
    }

    // ── Doppio click ─────────────────────────────────────────
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = apply_snap(fp, None, !app.hide_grid, shift, &app.focal_lines);

            match app.mode {
                EditMode::AddBarrier => {
                    if let Some(b) = app.barriers.last_mut() {
                        if !b.closed && b.vertices.len() >= 3 {
                            b.closed = true;
                        }
                    }
                    push_out_all_waypoints(app);
                }
                EditMode::AddFocalLine => {} // doppio click ignorato
                // In Select o AddCurve: aggiunge control point alla curva più vicina
                EditMode::Select | EditMode::AddCurve => {
                    let mut best_si: Option<usize> = None;
                    let mut best_dist = f32::INFINITY;
                    let mut prev = app.start;
                    for (si, seg) in app.segments.iter().enumerate() {
                        if matches!(seg, Segment::Curve { .. }) {
                            // Distanza dal click al percorso campionato
                            let pts = seg.sample(prev, 40);
                            let d = pts.iter()
                                .map(|p| field.to_screen(*p).distance(pos))
                                .fold(f32::INFINITY, f32::min);
                            if d < best_dist {
                                best_dist = d;
                                best_si = Some(si);
                            }
                        }
                        prev = seg.end();
                    }
                    // Solo se abbastanza vicino (30px) e non bloccato
                    if let Some(si) = best_si {
                        if best_dist < 30.0 && !point_blocked_by_barrier(clamped, app) {
                            if let Segment::Curve { controls, .. } = &mut app.segments[si] {
                                controls.push(clamped);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ── Tasto destro ────────────────────────────────────────
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            match app.mode {
                EditMode::AddBarrier => {
                    if let Some(b) = app.barriers.last_mut() {
                        if !b.closed {
                            b.vertices.pop();
                            if b.vertices.is_empty() {
                                app.barriers.pop();
                            }
                        }
                    }
                }
                EditMode::AddFocalLine => {
                    // Annulla il primo punto se era stato piazzato
                    app.focal_line_first = None;
                }
                _ => {
                    if let Some(ni) = waypoint_at(pos, field, app) {
                        remove_node(app, ni);
                    } else if let Some((si, ci)) = control_point_at(pos, field, app) {
                        if let Some(Segment::Curve { controls, .. }) = app.segments.get_mut(si) {
                            if controls.len() > 1 {
                                controls.remove(ci);
                            }
                        }
                    } else if let Some((bi, vi)) = barrier_vertex_at(pos, field, app) {
                        let b = &mut app.barriers[bi];
                        b.vertices.remove(vi);
                        // Se era chiusa la riapre, se rimane vuota la elimina
                        if b.vertices.is_empty() {
                            app.barriers.remove(bi);
                        } else {
                            b.closed = false; // torna aperta così puoi continuare
                        }
                    }
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════
// Disegno
// ════════════════════════════════════════════════════════════

/// Disegna la zona di esclusione (offset della barriera verso l'esterno di EXCLUSION_CM)
/// come poligono tratteggiato giallo. Ogni lato viene spostato lungo la sua normale esterna.
fn draw_barrier_exclusion_zones(painter: &Painter, field: &FieldView, app: &HudApp) {
    let scale = field.rect.width() / FIELD_CM;
    let exclusion_px = EXCLUSION_CM * scale;
    let dash_len = 6.0_f32;
    let gap_len  = 4.0_f32;
    let color = Color32::from_rgba_unmultiplied(255, 230, 0, 180);
    let stroke = Stroke::new(1.5, color);

    for b in &app.barriers {
        if !b.closed || b.vertices.len() < 3 { continue; }

        // Calcola il poligono espanso in coordinate schermo.
        // Per ogni vertice calcoliamo la bisettrice delle due normali ai lati adiacenti.
        let n = b.vertices.len();
        let screen: Vec<Pos2> = b.vertices.iter().map(|v| field.to_screen(*v)).collect();

        // Normale esterna di ogni lato (verso l'esterno del poligono).
        // Assumiamo che i vertici siano in ordine; calcoliamo il segno dell'area
        // per determinare il verso corretto.
        let signed_area: f32 = (0..n).map(|i| {
            let j = (i + 1) % n;
            screen[i].x * screen[j].y - screen[j].x * screen[i].y
        }).sum();
        // Se signed_area > 0 i vertici sono CCW in coords schermo (y verso il basso = CW in campo)
        let flip = if signed_area > 0.0 { 1.0_f32 } else { -1.0_f32 };

        let edge_normals: Vec<Vec2> = (0..n).map(|i| {
            let j = (i + 1) % n;
            let d = screen[j] - screen[i];
            let len = d.length().max(0.001);
            // Normale perpendicolare, verso l'esterno
            Vec2::new(d.y / len, -d.x / len) * flip
        }).collect();

        // Vertici espansi: bisettrice tra normale del lato precedente e quello corrente
        let expanded: Vec<Pos2> = (0..n).map(|i| {
            let prev_n = edge_normals[(i + n - 1) % n];
            let curr_n = edge_normals[i];
            let bisector = (prev_n + curr_n).normalized();
            // Correggi la lunghezza della bisettrice (miter)
            let cos_half = bisector.dot(curr_n).max(0.1);
            let miter_len = exclusion_px / cos_half;
            screen[i] + bisector * miter_len
        }).collect();

        // Disegna ogni lato del poligono espanso come linea tratteggiata
        for i in 0..n {
            let j = (i + 1) % n;
            draw_dashed_line(painter, expanded[i], expanded[j], dash_len, gap_len, stroke);
        }
    }
}

/// Disegna una linea tratteggiata tra due punti schermo.
fn draw_dashed_line(painter: &Painter, a: Pos2, b: Pos2, dash: f32, gap: f32, stroke: Stroke) {
    let d = b - a;
    let total = d.length();
    if total < 0.001 { return; }
    let dir = d / total;
    let period = dash + gap;
    let mut t = 0.0_f32;
    while t < total {
        let t_end = (t + dash).min(total);
        painter.line_segment([a + dir * t, a + dir * t_end], stroke);
        t += period;
    }
}

fn draw_field(painter: &Painter, field: &FieldView, app: &HudApp) {
    painter.rect_filled(field.rect, 0.0, Color32::from_gray(30));

    if let Some(tex) = &app.field_texture {
        painter.image(
            tex.id(),
            field.rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Linee gialle di esclusione dai bordi del campo — sempre visibili
    {
        let m = EXCLUSION_CM;
        let dash_stroke = Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 230, 0, 180));
        let corners = [
            ([m, m],              [FIELD_CM - m, m]),
            ([FIELD_CM - m, m],   [FIELD_CM - m, FIELD_CM - m]),
            ([FIELD_CM - m, FIELD_CM - m], [m, FIELD_CM - m]),
            ([m, FIELD_CM - m],   [m, m]),
        ];
        for (a, b) in corners {
            draw_dashed_line(
                painter,
                field.to_screen(a),
                field.to_screen(b),
                6.0, 4.0, dash_stroke,
            );
        }
    }

    if app.hide_grid { return; }

    // Griglia principale (tile interi) — più visibile
    let grid_main = Stroke::new(1.0, Color32::from_white_alpha(70));
    // Griglia fine (quarti di tile) — più sottile
    let grid_fine = Stroke::new(0.5, Color32::from_white_alpha(25));

    // Griglia parte e finisce alle linee gialle (EXCLUSION_CM)
    let m = EXCLUSION_CM;
    let grid_start = m;
    let grid_end   = FIELD_CM - m;
    let steps = ((grid_end - grid_start) / GRID_SNAP_STEP).round() as usize;
    for i in 0..=steps {
        let v = grid_start + i as f32 * GRID_SNAP_STEP;
        let is_main = ((v - grid_start) / TILE_CM).fract() < 0.02
                   || ((v - grid_start) / TILE_CM).fract() > 0.98;
        let stroke = if is_main { grid_main } else { grid_fine };
        painter.line_segment(
            [field.to_screen([v, grid_start]), field.to_screen([v, grid_end])],
            stroke,
        );
        painter.line_segment(
            [field.to_screen([grid_start, v]), field.to_screen([grid_end, v])],
            stroke,
        );
    }
}

fn draw_barriers(painter: &Painter, field: &FieldView, app: &HudApp) {
    for (bi, b) in app.barriers.iter().enumerate() {
        if b.vertices.is_empty() { continue; }

        let screen_verts: Vec<Pos2> = b.vertices.iter()
            .map(|v| field.to_screen(*v))
            .collect();

        // Riempimento
        if b.closed && screen_verts.len() >= 3 {
            painter.add(egui::Shape::convex_polygon(
                screen_verts.clone(),
                Color32::from_rgba_unmultiplied(200, 40, 40, 60),
                Stroke::new(2.0, Color32::from_rgb(220, 60, 60)),
            ));
        } else {
            // Poligono aperto: solo linee tra i vertici
            for w in screen_verts.windows(2) {
                painter.line_segment(
                    [w[0], w[1]],
                    Stroke::new(2.0, Color32::from_rgb(220, 60, 60)),
                );
            }
            // Linea tratteggiata dal primo all'ultimo per mostrare dove chiuderà
            if screen_verts.len() >= 2 {
                painter.line_segment(
                    [*screen_verts.last().unwrap(), screen_verts[0]],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 60, 60, 100)),
                );
            }
        }

        // Vertici
        for (vi, sp) in screen_verts.iter().enumerate() {
            let color = if vi == 0 {
                Color32::from_rgb(255, 100, 100)
            } else {
                Color32::from_rgb(200, 60, 60)
            };
            painter.circle_filled(*sp, VERTEX_RADIUS - 3.0, color);
            painter.circle_stroke(*sp, VERTEX_RADIUS - 3.0, Stroke::new(1.5, Color32::WHITE));
        }

        // Label
        let c = b.centroid();
        let sc = field.to_screen(c);
        painter.text(
            sc,
            Align2::CENTER_CENTER,
            format!("B{}", bi),
            FontId::proportional(12.0),
            Color32::from_rgb(255, 150, 150),
        );
    }
}

fn draw_path(painter: &Painter, field: &FieldView, app: &HudApp) {
    let mut prev = app.start;

    draw_waypoint(painter, field, app.start, 0);

    for (i, seg) in app.segments.iter().enumerate() {
        let pts = seg.sample(prev, 40);
        let is_search = matches!(seg, Segment::Search { .. });

        // Area di ricerca: rettangolo con partenza/arrivo ai punti medi dei lati corti
        if let Segment::Search { search, .. } = seg {
            if let Some(c) = search_rect(prev, seg.end(), search.width) {
                let sc: Vec<Pos2> = c.iter().map(|p| field.to_screen(*p)).collect();
                painter.add(egui::Shape::convex_polygon(
                    sc,
                    Color32::from_rgba_unmultiplied(170, 90, 255, 40),
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(190, 120, 255, 200)),
                ));
                let mid = field.to_screen([
                    (prev[0] + seg.end()[0]) / 2.0,
                    (prev[1] + seg.end()[1]) / 2.0,
                ]);
                painter.text(
                    mid + Vec2::new(0.0, -10.0),
                    Align2::CENTER_CENTER,
                    format!("🔍 {:.0}s", search.time_limit),
                    FontId::proportional(12.0),
                    Color32::from_rgb(220, 170, 255),
                );
            }
        }

        let path_color = if is_search { Color32::from_rgb(190, 110, 255) }
                         else { Color32::from_rgb(100, 180, 255) };
        for w in pts.windows(2) {
            painter.line_segment(
                [field.to_screen(w[0]), field.to_screen(w[1])],
                Stroke::new(2.5, path_color),
            );
        }

        if let Segment::Curve { controls, .. } = seg {
            for cp in controls.iter() {
                let sp = field.to_screen(*cp);
                let from_s = field.to_screen(prev);
                let to_s = field.to_screen(seg.end());
                painter.line_segment(
                    [from_s, sp],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 0, 100)),
                );
                painter.line_segment(
                    [to_s, sp],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 0, 100)),
                );
                painter.circle_stroke(sp, CTRL_RADIUS - 2.0, Stroke::new(1.5, Color32::GOLD));
                painter.circle_filled(sp, CTRL_RADIUS - 3.5, Color32::from_rgb(80, 60, 0));
            }
        }

        draw_waypoint(painter, field, seg.end(), i + 1);
        prev = seg.end();
    }
}

fn draw_waypoint(painter: &Painter, field: &FieldView, p: [f32; 2], index: usize) {
    let pos = field.to_screen(p);
    let color = if index == 0 { Color32::GREEN } else { Color32::from_rgb(255, 140, 0) };
    painter.circle_filled(pos, WAYPOINT_RADIUS - 4.0, color);
    painter.circle_stroke(pos, WAYPOINT_RADIUS - 4.0, Stroke::new(1.5, Color32::WHITE));
    painter.text(
        pos + Vec2::new(12.0, -12.0),
        Align2::LEFT_BOTTOM,
        index.to_string(),
        FontId::proportional(13.0),
        Color32::WHITE,
    );
}

fn draw_robot(painter: &Painter, field: &FieldView, app: &HudApp) {
    let center = field.to_screen(app.robot_pos);
    let scale = field.rect.width() / FIELD_CM;
    let half = (ROBOT_SIZE_CM / 2.0) * scale;
    let angle_rad = -(app.robot_heading - 90.0) * PI / 180.0;

    let rotate = |lx: f32, ly: f32| -> Pos2 {
        let cos = angle_rad.cos();
        let sin = angle_rad.sin();
        Pos2::new(
            center.x + lx * cos - ly * sin,
            center.y + lx * sin + ly * cos,
        )
    };

    let corners: Vec<Pos2> = [[-half,-half],[half,-half],[half,half],[-half,half]]
        .iter()
        .map(|c| rotate(c[0], c[1]))
        .collect();

    // Durante la ricerca palline il robot cambia colore (viola)
    let searching = app.sim.state == crate::simulation::SimState::Searching;
    let (body_fill, body_stroke) = if searching {
        (Color32::from_rgba_unmultiplied(180, 80, 255, 150), Color32::from_rgb(215, 140, 255))
    } else {
        (Color32::from_rgba_unmultiplied(0, 220, 220, 120), Color32::from_rgb(0, 255, 255))
    };
    painter.add(egui::Shape::convex_polygon(
        corners,
        body_fill,
        Stroke::new(2.0, body_stroke),
    ));
    if searching {
        painter.text(
            center + Vec2::new(0.0, -half - 14.0),
            Align2::CENTER_CENTER,
            "🔍",
            FontId::proportional(14.0),
            Color32::from_rgb(215, 140, 255),
        );
    }

    // Indicatore ShootMove — cerchio rosso pulsante attorno al robot
    if app.sim.state == crate::simulation::SimState::ShootMove {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis() as f32 / 1000.0;
        let pulse = (t * std::f32::consts::PI * 2.0).sin() * 0.5 + 0.5;
        let alpha = (100.0 + pulse * 120.0) as u8;
        painter.circle_stroke(
            center,
            half * 1.3 + pulse * 8.0,
            Stroke::new(2.5, Color32::from_rgba_unmultiplied(255, 60, 60, alpha)),
        );
        painter.text(
            center + Vec2::new(0.0, -half - 14.0),
            Align2::CENTER_CENTER,
            "🎯",
            FontId::proportional(14.0),
            Color32::from_rgb(255, 80, 80),
        );
    }

    // Freccia bianca = dove guarda il robot (heading)
    let front = rotate(0.0, -half * 0.8);
    painter.arrow(center, front - center, Stroke::new(2.5, Color32::WHITE));

    // Freccia verde = direzione di movimento (tangente al percorso)
    // Visibile solo durante la simulazione quando heading ≠ travel dir
    if app.sim.is_running() {
        if let Some(seg) = app.segments.get(app.sim.seg_index.min(app.segments.len().saturating_sub(1))) {
            let from = if app.sim.seg_index == 0 { app.start }
                       else { app.segments[app.sim.seg_index.saturating_sub(1)].end() };
            let pts = seg.sample(from, 120);
            let travel_deg = crate::simulation::tangent_at_fraction(&pts, app.sim.t);
            let travel_rad = -(travel_deg - 90.0) * PI / 180.0;
            let travel_dir = Vec2::new(travel_rad.cos(), travel_rad.sin());
            let travel_tip = center + travel_dir * half * 0.8;
            painter.arrow(center, travel_tip - center,
                Stroke::new(2.0, Color32::from_rgb(0, 220, 80)));
        }
    }

    painter.text(center, Align2::CENTER_CENTER, "R", FontId::proportional(13.0), Color32::WHITE);
    painter.text(
        center + Vec2::new(half + 4.0, 0.0),
        Align2::LEFT_CENTER,
        format!("👁 {:.0}°", app.robot_heading),
        FontId::proportional(11.0),
        Color32::from_rgb(0, 255, 255),
    );
}


/// Preview del prossimo nodo mentre il cursore si muove sul campo.
fn draw_preview(painter: &Painter, field: &FieldView, app: &HudApp, hover: Option<egui::Pos2>, shift: bool) {
    // Disegna sempre le linee focali
    draw_focal_lines(painter, field, app);

    let pos = match hover { Some(p) => p, None => return };
    let raw = field.to_field(pos);

    // Calcola snap con feedback
    let ortho_ref = match app.mode {
        EditMode::AddBarrier => app.barriers.last().and_then(|b| {
            if b.closed { None } else { b.vertices.last().copied() }
        }),
        EditMode::AddLine | EditMode::AddCurve | EditMode::AddSearch => Some(last_point(app)),
        EditMode::AddFocalLine => app.focal_line_first,
        _ => None,
    };
    let (fp, snap_hint) = apply_snap_full(
        raw, ortho_ref, !app.hide_grid,
        shift,
        &app.focal_lines,
    );

    if point_blocked_by_barrier(fp, app) { return; }

    // Feedback snap visivo
    match &snap_hint {
        SnapHint::Grid => {
            // Croce verde sul punto snappato
            let sp = field.to_screen(fp);
            let r = 8.0;
            let sc = Stroke::new(1.5, Color32::from_rgba_unmultiplied(100, 255, 100, 200));
            painter.line_segment([sp + Vec2::new(-r,0.0), sp + Vec2::new(r,0.0)], sc);
            painter.line_segment([sp + Vec2::new(0.0,-r), sp + Vec2::new(0.0,r)], sc);
        }
        SnapHint::Ortho => {
            // Linea guida blu dal reference al punto
            if let Some(r) = ortho_ref {
                let rs = field.to_screen(r);
                let fs = field.to_screen(fp);
                painter.line_segment([rs, fs],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 150)));
                // Simbolo angolo retto
                painter.circle_filled(fs, 4.0, Color32::from_rgb(100, 180, 255));
            }
        }
        SnapHint::Focal(i) => {
            // Evidenzia la linea focale attiva
            if let Some(fl) = app.focal_lines.get(*i) {
                let pa = field.to_screen(extend_line(fl.a, fl.b, -2000.0));
                let pb = field.to_screen(extend_line(fl.a, fl.b,  2000.0));
                painter.line_segment([pa, pb],
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 200, 50, 180)));
            }
            painter.circle_filled(field.to_screen(fp), 5.0, Color32::from_rgb(255, 200, 50));
        }
        SnapHint::None => {}
    }

    let ghost_color  = Color32::from_rgba_unmultiplied(255, 255, 255, 60);
    let ghost_stroke = Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 80));
    let dot_color    = Color32::from_rgba_unmultiplied(255, 255, 255, 140);

    match app.mode {
        EditMode::AddLine => {
            let from = last_point(app);
            // Linea opaca dal punto precedente al cursore
            painter.line_segment(
                [field.to_screen(from), field.to_screen(fp)],
                ghost_stroke,
            );
            // Pallino sul cursore
            painter.circle_filled(field.to_screen(fp), 5.0, dot_color);
        }
        EditMode::AddCurve => {
            let from = last_point(app);
            // Control point stimato (stesso calcolo di quando si piazza)
            let ctrl = [
                (from[0] + fp[0]) / 2.0,
                (from[1] + fp[1]) / 2.0 + 30.0,
            ];
            // Curva preview campionata come Bezier
            let p0 = from;
            let p1 = ctrl;
            let p2 = ctrl;
            let p3 = fp;
            let pts: Vec<egui::Pos2> = (0..=20).map(|i| {
                let t = i as f32 / 20.0;
                let u = 1.0 - t;
                let x = u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0];
                let y = u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1];
                field.to_screen([x, y])
            }).collect();
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], ghost_stroke);
            }
            painter.circle_filled(field.to_screen(fp), 5.0, dot_color);
        }
        EditMode::AddBarrier => {
            // Linea dal punto precedente della barriera al cursore
            if let Some(b) = app.barriers.last() {
                if !b.closed {
                    if let Some(last_v) = b.vertices.last() {
                        painter.line_segment(
                            [field.to_screen(*last_v), field.to_screen(fp)],
                            Stroke::new(1.5, Color32::from_rgba_unmultiplied(220, 60, 60, 120)),
                        );
                        // Linea tratteggiata di chiusura dal cursore al primo vertice
                        if b.vertices.len() >= 2 {
                            draw_dashed_line(
                                painter,
                                field.to_screen(fp),
                                field.to_screen(b.vertices[0]),
                                5.0, 4.0,
                                Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 60, 60, 80)),
                            );
                        }
                    }
                }
            }
            painter.circle_filled(field.to_screen(fp), 5.0,
                Color32::from_rgba_unmultiplied(220, 60, 60, 180));
        }
        EditMode::AddFocalLine => {
            // Mostra linea dal primo punto al cursore
            if let Some(first) = app.focal_line_first {
                let pa = field.to_screen(first);
                let pb = field.to_screen(fp);
                painter.line_segment([pa, pb],
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 200, 50, 180)));
                painter.circle_filled(pa, 5.0, Color32::from_rgb(255, 200, 50));
            }
            painter.circle_filled(field.to_screen(fp), 5.0, Color32::from_rgb(255, 200, 50));
        }
        EditMode::AddSearch => {
            let from = last_point(app);
            let w = crate::segment::SearchParams::default().width;
            if let Some(c) = search_rect(from, fp, w) {
                let sc: Vec<Pos2> = c.iter().map(|p| field.to_screen(*p)).collect();
                painter.add(egui::Shape::convex_polygon(
                    sc,
                    Color32::from_rgba_unmultiplied(170, 90, 255, 25),
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(190, 120, 255, 120)),
                ));
            }
            painter.line_segment(
                [field.to_screen(from), field.to_screen(fp)],
                Stroke::new(1.5, Color32::from_rgba_unmultiplied(190, 120, 255, 160)),
            );
            painter.circle_filled(field.to_screen(fp), 5.0, Color32::from_rgb(190, 120, 255));
        }
        EditMode::Select => {}
    }
}

/// Angoli (in cm) del rettangolo di ricerca: `from` e `to` sono i punti medi
/// dei due lati corti, `width` è la dimensione perpendicolare alla retta from→to.
fn search_rect(from: [f32; 2], to: [f32; 2], width: f32) -> Option<[[f32; 2]; 4]> {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 { return None; }
    let h = width / 2.0;
    let nx = -dy / len * h;
    let ny =  dx / len * h;
    Some([
        [from[0] + nx, from[1] + ny],
        [to[0]   + nx, to[1]   + ny],
        [to[0]   - nx, to[1]   - ny],
        [from[0] - nx, from[1] - ny],
    ])
}

fn draw_focal_lines(painter: &Painter, field: &FieldView, app: &HudApp) {
    for fl in app.focal_lines.iter().filter(|f| f.visible) {
        let pa = field.to_screen(extend_line(fl.a, fl.b, -2000.0));
        let pb = field.to_screen(extend_line(fl.a, fl.b,  2000.0));
        draw_dashed_line(painter, pa, pb, 8.0, 5.0,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 50, 80)));
    }
}

/// Estende una retta a→b di `dist` cm oltre b (negativo = oltre a).
fn extend_line(a: [f32; 2], b: [f32; 2], dist: f32) -> [f32; 2] {
    let dx = b[0]-a[0]; let dy = b[1]-a[1];
    let len = (dx*dx+dy*dy).sqrt().max(0.001);
    [b[0] + dx/len*dist, b[1] + dy/len*dist]
}

fn draw_mouse_coords(painter: &Painter, response: &Response, field: &FieldView) {
    if let Some(pos) = response.hover_pos() {
        let f = field.to_field(pos);
        painter.text(
            response.rect.right_top() + Vec2::new(-8.0, 8.0),
            Align2::RIGHT_TOP,
            format!("x={:.1}  y={:.1} cm", f[0], f[1]),
            FontId::monospace(13.0),
            Color32::WHITE,
        );
    }
}

// ════════════════════════════════════════════════════════════
// Hit-test
// ════════════════════════════════════════════════════════════

fn waypoint_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<usize> {
    if field.to_screen(app.start).distance(pos) <= WAYPOINT_RADIUS + HIT_EXTRA {
        return Some(0);
    }
    for (i, seg) in app.segments.iter().enumerate() {
        if field.to_screen(seg.end()).distance(pos) <= WAYPOINT_RADIUS + HIT_EXTRA {
            return Some(i + 1);
        }
    }
    None
}

fn control_point_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<(usize, usize)> {
    for (si, seg) in app.segments.iter().enumerate() {
        if let Segment::Curve { controls, .. } = seg {
            for (ci, cp) in controls.iter().enumerate() {
                if field.to_screen(*cp).distance(pos) <= CTRL_RADIUS + HIT_EXTRA {
                    return Some((si, ci));
                }
            }
        }
    }
    None
}

fn barrier_vertex_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<(usize, usize)> {
    for (bi, b) in app.barriers.iter().enumerate() {
        for (vi, v) in b.vertices.iter().enumerate() {
            if field.to_screen(*v).distance(pos) <= VERTEX_RADIUS + HIT_EXTRA {
                return Some((bi, vi));
            }
        }
    }
    None
}

/// Restituisce l'indice della barriera chiusa di cui pos è dentro il bounding box.
fn barrier_body_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<usize> {
    for (bi, b) in app.barriers.iter().enumerate() {
        if !b.closed || b.vertices.len() < 3 { continue; }
        let screen_verts: Vec<Pos2> = b.vertices.iter().map(|v| field.to_screen(*v)).collect();
        if point_in_polygon(pos, &screen_verts) {
            return Some(bi);
        }
    }
    None
}

/// Ray-casting per punto in poligono.
fn point_in_polygon(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Controlla se un punto in coordinate campo [cm] cade dentro o troppo vicino
/// a qualsiasi barriera chiusa (distanza < EXCLUSION_CM dai lati).
fn point_blocked_by_barrier(p: [f32; 2], app: &HudApp) -> bool {
    use crate::barrier::{dist_point_to_segment, min_dist_to_barriers};
    for b in &app.barriers {
        if !b.closed || b.vertices.len() < 3 { continue; }
        // Dentro al poligono?
        let verts: Vec<[f32;2]> = b.vertices.clone();
        if point_in_polygon_field(p, &verts) { return true; }
        // Troppo vicino ai lati?
        let d = b.edges().iter()
            .map(|(a, bb)| dist_point_to_segment(p, *a, *bb))
            .fold(f32::INFINITY, f32::min);
        if d < EXCLUSION_CM { return true; }
    }
    false
}

/// Ray-casting in coordinate campo (y non invertita).
fn point_in_polygon_field(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi[1] > p[1]) != (pj[1] > p[1]))
            && (p[0] < (pj[0] - pi[0]) * (p[1] - pi[1]) / (pj[1] - pi[1]) + pi[0])
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn is_near_robot(pos: Pos2, field: &FieldView, app: &HudApp) -> bool {
    let center = field.to_screen(app.robot_pos);
    let scale = field.rect.width() / FIELD_CM;
    let half = (ROBOT_SIZE_CM / 2.0) * scale;
    center.distance(pos) <= half * 1.1
}

// ════════════════════════════════════════════════════════════
// Utility
// ════════════════════════════════════════════════════════════

/// Disegna il punto di inizio frenata per ogni segmento.
fn draw_brake_points(painter: &Painter, field: &FieldView, app: &HudApp) {
    let mut prev = app.start;
    for (i, seg) in app.segments.iter().enumerate() {
        // La ricerca ha velocità costante: niente punto di frenata
        if matches!(seg, Segment::Search { .. }) { prev = seg.end(); continue; }
        let pts = seg.sample(prev, 120);
        let t   = seg.opts().speed.brake_point;
        let bp  = sample_at_t_field(&pts, t);
        let sp  = field.to_screen(bp);

        // Triangolino verso il basso = punto di frenata
        let r = 7.0_f32;
        let tri = vec![
            Pos2::new(sp.x, sp.y + r),
            Pos2::new(sp.x - r * 0.866, sp.y - r * 0.5),
            Pos2::new(sp.x + r * 0.866, sp.y - r * 0.5),
        ];
        painter.add(egui::Shape::convex_polygon(
            tri,
            Color32::from_rgb(255, 80, 80),
            Stroke::new(1.5, Color32::WHITE),
        ));
        painter.text(
            sp + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            format!("t={:.2}", t),
            FontId::proportional(10.0),
            Color32::from_rgb(255, 150, 150),
        );
        prev = seg.end();
    }
}

fn sample_at_t_field(pts: &[[f32; 2]], t: f32) -> [f32; 2] {
    // Stessa parametrizzazione (lunghezza d'arco) usata dalla simulazione
    crate::simulation::point_at_fraction(pts, t)
}

/// Ritorna il t ∈ [0,1] del punto più vicino a `pos` sulla polilinea campionata.
fn closest_t_on_path(pts: &[[f32; 2]], field: &FieldView, pos: Pos2) -> f32 {
    let fr = crate::simulation::fractions(pts);
    let mut best_t = 0.0_f32;
    let mut best_d = f32::INFINITY;
    for (i, p) in pts.iter().enumerate() {
        let d = field.to_screen(*p).distance(pos);
        if d < best_d {
            best_d = d;
            best_t = fr[i];
        }
    }
    best_t
}

/// Hit-test sul triangolino del brake_point.
fn brake_point_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<usize> {
    let mut prev = app.start;
    for (i, seg) in app.segments.iter().enumerate() {
        // La ricerca ha velocità costante: niente punto di frenata
        if matches!(seg, Segment::Search { .. }) { prev = seg.end(); continue; }
        let pts = seg.sample(prev, 120);
        let t   = seg.opts().speed.brake_point;
        let bp  = sample_at_t_field(&pts, t);
        let sp  = field.to_screen(bp);
        if sp.distance(pos) <= 12.0 {
            return Some(i);
        }
        prev = seg.end();
    }
    None
}

/// Dopo aver chiuso una barriera, sposta tutti i waypoint che sono finiti
/// dentro o troppo vicino alla zona di esclusione verso fuori.
fn push_out_all_waypoints(app: &mut HudApp) {
    use crate::barrier::push_out_of_barriers;
    let barriers = app.barriers.clone();

    // Start
    if point_blocked_by_barrier(app.start, app) {
        app.start = push_out_of_barriers(app.start, &barriers, EXCLUSION_CM);
    }

    // Waypoint di ogni segmento
    for seg in app.segments.iter_mut() {
        let end = seg.end();
        if point_blocked_by_barrier_list(end, &barriers) {
            let fixed = push_out_of_barriers(end, &barriers, EXCLUSION_CM);
            seg.set_end(fixed);
        }
    }
}

/// Versione di point_blocked_by_barrier che prende la lista direttamente.
fn point_blocked_by_barrier_list(p: [f32; 2], barriers: &[crate::barrier::Barrier]) -> bool {
    use crate::barrier::dist_point_to_segment;
    for b in barriers {
        if !b.closed || b.vertices.len() < 3 { continue; }
        if point_in_polygon_field(p, &b.vertices) { return true; }
        let d = b.edges().iter()
            .map(|(a, bb)| dist_point_to_segment(p, *a, *bb))
            .fold(f32::INFINITY, f32::min);
        if d < EXCLUSION_CM { return true; }
    }
    false
}

pub const GRID_SNAP_STEP: f32 = TILE_CM / 4.0; // griglia ogni quarto di tile (~15cm)
const SNAP_THRESHOLD_CM: f32 = 8.0;           // entro 8cm si attrae

fn clamp_field(fp: [f32; 2]) -> [f32; 2] {
    let m = EXCLUSION_CM;
    [fp[0].clamp(m, FIELD_CM - m), fp[1].clamp(m, FIELD_CM - m)]
}

/// Snap automatico: griglia (se griglia ON), ortho rispetto a reference, linee focali.
/// Restituisce (punto_snappato, tipo_snap) per il feedback visivo.
#[derive(Clone, Debug, PartialEq)]
pub enum SnapHint {
    None,
    Grid,
    Ortho,
    Focal(usize), // indice linea focale
}

pub fn apply_snap_full(
    fp: [f32; 2],
    reference: Option<[f32; 2]>,
    grid_on: bool,
    shift: bool,               // snap ortho solo con Shift
    focal_lines: &[crate::app::FocalLine],
) -> ([f32; 2], SnapHint) {
    let mut best = clamp_field(fp);
    let mut best_dist = f32::INFINITY;
    let mut hint = SnapHint::None;

    // 1. Snap griglia — solo se griglia ON
    if grid_on {
        let m = EXCLUSION_CM;
        let step = GRID_SNAP_STEP;
        let gx = m + ((fp[0] - m) / step).round() * step;
        let gy = m + ((fp[1] - m) / step).round() * step;
        let dx = (fp[0] - gx).abs();
        let dy = (fp[1] - gy).abs();
        let d = dx.min(dy);
        if d < SNAP_THRESHOLD_CM && d < best_dist {
            best_dist = d;
            best = clamp_field([gx, gy]);
            hint = SnapHint::Grid;
        }
    }

    // 2. Snap ortho — solo tenendo Shift (indipendente dalla griglia)
    if shift {
        if let Some(r) = reference {
            let dx = (fp[0] - r[0]).abs();
            let dy = (fp[1] - r[1]).abs();
            let dh = dy;
            if dh < SNAP_THRESHOLD_CM * 3.0 && dh < best_dist {
                best_dist = dh;
                best = clamp_field([fp[0], r[1]]);
                hint = SnapHint::Ortho;
            }
            let dv = dx;
            if dv < SNAP_THRESHOLD_CM * 3.0 && dv < best_dist {
                best_dist = dv;
                best = clamp_field([r[0], fp[1]]);
                hint = SnapHint::Ortho;
            }
        }
    }

    // 3. Snap linee focali — sempre attivo indipendentemente dalla griglia
    for (i, fl) in focal_lines.iter().enumerate() {
        if !fl.visible { continue; }
        let d = fl.dist(fp);
        if d < fl.snap_radius_cm && d < best_dist {
            best_dist = d;
            best = clamp_field(fl.project(fp));
            hint = SnapHint::Focal(i);
        }
    }

    (best, hint)
}

fn apply_snap(
    fp: [f32; 2],
    reference: Option<[f32; 2]>,
    grid_on: bool,
    shift: bool,
    focal_lines: &[crate::app::FocalLine],
) -> [f32; 2] {
    apply_snap_full(fp, reference, grid_on, shift, focal_lines).0
}

fn last_point(app: &HudApp) -> [f32; 2] {
    app.segments.last().map(|s| s.end()).unwrap_or(app.start)
}

fn remove_node(app: &mut HudApp, i: usize) {
    if i > 0 && i <= app.segments.len() {
        app.segments.remove(i - 1);
    }
}
