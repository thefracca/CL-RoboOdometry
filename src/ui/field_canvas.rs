use eframe::egui::{
    self, pos2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense,
    Stroke, Vec2,
};
use std::f32::consts::PI;

use crate::app::{Dragging, EditMode, HudApp};
use crate::barrier::Barrier;
use crate::field::{FieldView, FIELD_CM, TILE_CM};
use crate::segment::{HeadingMode, Segment};

const WAYPOINT_RADIUS: f32 = 12.0;
/// Margine di esclusione intorno alle barriere: metà robot + 1 cm di errore
const EXCLUSION_CM: f32 = ROBOT_SIZE_CM / 2.0 + 1.0;
const CTRL_RADIUS: f32 = 10.0;
const VERTEX_RADIUS: f32 = 10.0;
const HIT_EXTRA: f32 = 8.0;
const ROBOT_SIZE_CM: f32 = 45.0;

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::CentralPanel::default().show(ui, |ui| {
        let avail = ui.available_size();
        let side = avail.x.min(avail.y);
        let (response, painter) =
            ui.allocate_painter(Vec2::splat(side), Sense::click_and_drag());
        let field = FieldView { rect: response.rect };

        draw_field(&painter, &field, app);
        handle_input(&response, &field, app, ui.ctx());
        draw_path(&painter, &field, app);
        draw_barriers(&painter, &field, app);
        draw_barrier_exclusion_zones(&painter, &field, app);
        draw_robot(&painter, &field, app);
        draw_mouse_coords(&painter, &response, &field);
    });
}

// ════════════════════════════════════════════════════════════
// Input
// ════════════════════════════════════════════════════════════

fn handle_input(response: &Response, field: &FieldView, app: &mut HudApp, ctx: &egui::Context) {
    let shift = ctx.input(|i| i.modifiers.shift);

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
            let clamped = clamp_field(fp);
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
                        match seg {
                            Segment::Line { to, .. } => *to = clamped,
                            Segment::Curve { to, .. } => *to = clamped,
                        }
                    }
                }
                Dragging::ControlPoint(si, ci) => {
                    if let Some(Segment::Curve { controls, .. }) = app.segments.get_mut(si) {
                        if let Some(cp) = controls.get_mut(ci) {
                            *cp = clamped;
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
        app.dragging = Dragging::Nothing;
    }

    // ── Click sinistro ──────────────────────────────────────
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = clamp_field(fp);

            let on_existing = is_near_robot(pos, field, app)
                || waypoint_at(pos, field, app).is_some()
                || control_point_at(pos, field, app).is_some()
                || barrier_vertex_at(pos, field, app).is_some();

            if !on_existing {
                match app.mode {
                    EditMode::AddLine => {
                        app.segments.push(Segment::Line {
                            to: clamped,
                            heading: HeadingMode::Tangent,
                        });
                    }
                    EditMode::AddCurve => {
                        let prev = last_point(app);
                        let ctrl = [
                            (prev[0] + clamped[0]) / 2.0,
                            (prev[1] + clamped[1]) / 2.0 + 30.0,
                        ];
                        app.segments.push(Segment::Curve {
                            controls: vec![ctrl],
                            to: clamped,
                            heading: HeadingMode::Tangent,
                        });
                    }
                    EditMode::AddBarrier => {
                        // Se c'è una barriera aperta in cima, aggiungi vertice lì
                        if let Some(b) = app.barriers.last_mut() {
                            if !b.closed {
                                b.vertices.push(clamped);
                                return;
                            }
                        }
                        // Altrimenti inizia una nuova barriera
                        let mut b = Barrier::new();
                        b.vertices.push(clamped);
                        app.barriers.push(b);
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
            let clamped = clamp_field(fp);

            match app.mode {
                EditMode::AddBarrier => {
                    // Chiudi la barriera aperta
                    if let Some(b) = app.barriers.last_mut() {
                        if !b.closed && b.vertices.len() >= 3 {
                            b.closed = true;
                        }
                    }
                }
                EditMode::AddCurve => {
                    // Aggiungi control point extra alla prima curva trovata
                    for seg in app.segments.iter_mut() {
                        if let Segment::Curve { controls, .. } = seg {
                            controls.push(clamped);
                            break;
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
                    // Rimuovi ultimo vertice dalla barriera aperta
                    if let Some(b) = app.barriers.last_mut() {
                        if !b.closed {
                            b.vertices.pop();
                            if b.vertices.is_empty() {
                                app.barriers.pop();
                            }
                        }
                    }
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
                    } else if let Some((bi, _)) = barrier_vertex_at(pos, field, app) {
                        // Tasto destro su vertice barriera in Select = elimina tutta la barriera
                        app.barriers.remove(bi);
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

    if app.hide_grid { return; }

    let grid = Stroke::new(1.0, Color32::from_white_alpha(60));
    for i in 0..=6 {
        let v = i as f32 * TILE_CM;
        painter.line_segment(
            [field.to_screen([v, 0.0]), field.to_screen([v, FIELD_CM])],
            grid,
        );
        painter.line_segment(
            [field.to_screen([0.0, v]), field.to_screen([FIELD_CM, v])],
            grid,
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
        for w in pts.windows(2) {
            painter.line_segment(
                [field.to_screen(w[0]), field.to_screen(w[1])],
                Stroke::new(2.5, Color32::LIGHT_BLUE),
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

    painter.add(egui::Shape::convex_polygon(
        corners,
        Color32::from_rgba_unmultiplied(0, 220, 220, 120),
        Stroke::new(2.0, Color32::CYAN),
    ));

    let front = rotate(0.0, -half * 0.8);
    painter.arrow(center, front - center, Stroke::new(2.5, Color32::WHITE));

    painter.text(center, Align2::CENTER_CENTER, "R", FontId::proportional(13.0), Color32::WHITE);
    painter.text(
        center + Vec2::new(half + 4.0, 0.0),
        Align2::LEFT_CENTER,
        format!("{:.0}°", app.robot_heading),
        FontId::proportional(11.0),
        Color32::CYAN,
    );
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

fn is_near_robot(pos: Pos2, field: &FieldView, app: &HudApp) -> bool {
    let center = field.to_screen(app.robot_pos);
    let scale = field.rect.width() / FIELD_CM;
    let half = (ROBOT_SIZE_CM / 2.0) * scale;
    center.distance(pos) <= half * 1.1
}

// ════════════════════════════════════════════════════════════
// Utility
// ════════════════════════════════════════════════════════════

fn clamp_field(fp: [f32; 2]) -> [f32; 2] {
    [fp[0].clamp(0.0, FIELD_CM), fp[1].clamp(0.0, FIELD_CM)]
}

fn last_point(app: &HudApp) -> [f32; 2] {
    app.segments.last().map(|s| s.end()).unwrap_or(app.start)
}

fn remove_node(app: &mut HudApp, i: usize) {
    if i > 0 && i <= app.segments.len() {
        app.segments.remove(i - 1);
    }
}
