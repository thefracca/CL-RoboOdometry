// Area centrale: campo, percorso, robot, input mouse.

use eframe::egui::{
    self, pos2, Align2, Color32, FontId, Key, Painter, Pos2, Rect, Response, Sense,
    Stroke, Vec2,
};
use std::f32::consts::PI;

use crate::app::{Dragging, EditMode, HudApp};
use crate::field::{FieldView, FIELD_CM, TILE_CM};
use crate::segment::{HeadingMode, Segment};

// ---------- Costanti di disegno ----------
const WAYPOINT_RADIUS: f32 = 7.0;
const CTRL_RADIUS: f32 = 5.0;
const ROBOT_SIZE_CM: f32 = 45.0; // larghezza del quadratino robot (cm)

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
        draw_robot(&painter, &field, app);
        draw_mouse_coords(&painter, &response, &field);
    });
}

// ════════════════════════════════════════════════════════════
// Input mouse
// ════════════════════════════════════════════════════════════

fn handle_input(response: &Response, field: &FieldView, app: &mut HudApp, ctx: &egui::Context) {
    let shift = ctx.input(|i| i.modifiers.shift);

    // ── Inizio del drag ────────────────────────────────────
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);

            // Robot? Il drag va al robot (rotate se Shift)
            if is_near_robot(pos, field, app) {
                if shift {
                    app.dragging = Dragging::RotateRobot {
                        start_x: pos.x,
                        start_heading: app.robot_heading,
                    };
                } else {
                    app.dragging = Dragging::Robot;
                }
            } else if let Some((si, ci)) = control_point_at(pos, field, app) {
                // Control point di una curva
                app.dragging = Dragging::ControlPoint(si, ci);
            } else if let Some(ni) = waypoint_at(pos, field, app) {
                // Waypoint (start o end di segmento)
                app.dragging = Dragging::Node(ni);
            } else {
                app.dragging = Dragging::Nothing;
            }
        }
    }

    // ── Durante il drag ────────────────────────────────────
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = [fp[0].clamp(0.0, FIELD_CM), fp[1].clamp(0.0, FIELD_CM)];

            match app.dragging.clone() {
                Dragging::Robot => {
                    app.robot_pos = clamped;
                }
                Dragging::RotateRobot { start_x, start_heading } => {
                    // 1 pixel orizzontale → 1° di rotazione
                    let delta = pos.x - start_x;
                    app.robot_heading = (start_heading - delta).rem_euclid(360.0);
                }
                Dragging::Node(0) => {
                    // Nodo 0 = start del percorso
                    app.start = clamped;
                }
                Dragging::Node(i) => {
                    // Nodo i = end del segmento i-1
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
                Dragging::Nothing => {}
            }
        }
    }

    if response.drag_stopped() {
        app.dragging = Dragging::Nothing;
    }

    // ── Click sinistro: aggiunge nodo ────────────────────────
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = [fp[0].clamp(0.0, FIELD_CM), fp[1].clamp(0.0, FIELD_CM)];

            // Non aggiungere se hai cliccato su un waypoint/robot esistente
            let on_existing = is_near_robot(pos, field, app)
                || waypoint_at(pos, field, app).is_some()
                || control_point_at(pos, field, app).is_some();

            if !on_existing {
                match app.mode {
                    EditMode::AddLine => {
                        app.segments.push(Segment::Line {
                            to: clamped,
                            heading: HeadingMode::Tangent,
                        });
                    }
                    EditMode::AddCurve => {
                        // Il control point inizia a metà tra l'ultimo punto e il nuovo
                        let prev = last_point(app);
                        let ctrl = [
                            (prev[0] + clamped[0]) / 2.0,
                            (prev[1] + clamped[1]) / 2.0 + 30.0, // leggermente spostato
                        ];
                        app.segments.push(Segment::Curve {
                            controls: vec![ctrl],
                            to: clamped,
                            heading: HeadingMode::Tangent,
                        });
                    }
                    EditMode::Select => {} // click su niente in modalità Select: ignora
                }
            }
        }
    }

    // ── Tasto destro: elimina waypoint ───────────────────────
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(ni) = waypoint_at(pos, field, app) {
                remove_node(app, ni);
            } else if let Some((si, ci)) = control_point_at(pos, field, app) {
                // Rimuovi il control point dalla curva
                if let Some(Segment::Curve { controls, .. }) = app.segments.get_mut(si) {
                    if controls.len() > 1 {
                        controls.remove(ci);
                    }
                }
            }
        }
    }

    // ── Doppio click su curva: aggiunge control point ────────
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fp = field.to_field(pos);
            let clamped = [fp[0].clamp(0.0, FIELD_CM), fp[1].clamp(0.0, FIELD_CM)];
            // Cerca se siamo vicini a una curva (approssimativo: usa il centro)
            for seg in app.segments.iter_mut() {
                if let Segment::Curve { controls, .. } = seg {
                    // Aggiungi il nuovo control point al volo
                    controls.push(clamped);
                    break; // solo primo match
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════
// Disegno
// ════════════════════════════════════════════════════════════

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

    if app.hide_grid {
        return;
    }
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

fn draw_path(painter: &Painter, field: &FieldView, app: &HudApp) {
    let mut prev = app.start;

    // Disegna il punto di start
    draw_waypoint(painter, field, app.start, 0, false);

    for (i, seg) in app.segments.iter().enumerate() {
        // Curva/linea campionata
        let pts = seg.sample(prev, 40);
        for w in pts.windows(2) {
            painter.line_segment(
                [field.to_screen(w[0]), field.to_screen(w[1])],
                Stroke::new(2.5, Color32::LIGHT_BLUE),
            );
        }

        // Control points delle curve
        if let Segment::Curve { controls, .. } = seg {
            for (ci, cp) in controls.iter().enumerate() {
                let sp = field.to_screen(*cp);
                // Linea tratteggiata dal punto prima/dopo al control point
                let from_s = field.to_screen(prev);
                let to_s = field.to_screen(seg.end());
                // Solo linea di riferimento al control point
                painter.line_segment(
                    [from_s, sp],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 0, 100)),
                );
                painter.line_segment(
                    [to_s, sp],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 0, 100)),
                );

                // Cerchietto del control point
                painter.circle_stroke(sp, CTRL_RADIUS, Stroke::new(1.5, Color32::GOLD));
                painter.circle_filled(sp, CTRL_RADIUS - 1.5, Color32::from_rgb(80, 60, 0));
                painter.text(
                    sp + Vec2::new(7.0, -7.0),
                    Align2::LEFT_BOTTOM,
                    format!("c{}", ci),
                    FontId::proportional(11.0),
                    Color32::GOLD,
                );
            }
        }

        // Waypoint di arrivo
        draw_waypoint(painter, field, seg.end(), i + 1, false);
        prev = seg.end();
    }
}

fn draw_waypoint(painter: &Painter, field: &FieldView, p: [f32; 2], index: usize, selected: bool) {
    let pos = field.to_screen(p);
    let color = if selected {
        Color32::YELLOW
    } else if index == 0 {
        Color32::GREEN
    } else {
        Color32::from_rgb(255, 140, 0)
    };
    painter.circle_filled(pos, WAYPOINT_RADIUS, color);
    painter.circle_stroke(pos, WAYPOINT_RADIUS, Stroke::new(1.5, Color32::WHITE));
    painter.text(
        pos + Vec2::new(10.0, -10.0),
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

    // Heading: 0°=destra, 90°=su in coordinate campo → in screen la Y è invertita
    let angle_rad = -(app.robot_heading - 90.0) * PI / 180.0; // converte a "matematico" screen

    // I 4 angoli del robot ruotati
    let corners_local: [[f32; 2]; 4] = [
        [-half, -half],
        [ half, -half],
        [ half,  half],
        [-half,  half],
    ];

    let rotate = |lx: f32, ly: f32| {
        let cos = angle_rad.cos();
        let sin = angle_rad.sin();
        Pos2::new(
            center.x + lx * cos - ly * sin,
            center.y + lx * sin + ly * cos,
        )
    };

    let corners: Vec<Pos2> = corners_local.iter()
        .map(|c| rotate(c[0], c[1]))
        .collect();

    // Riempimento ciano semi-trasparente
    painter.add(egui::Shape::convex_polygon(
        corners.clone(),
        Color32::from_rgba_unmultiplied(0, 220, 220, 120),
        Stroke::new(2.0, Color32::CYAN),
    ));

    // Freccia di direzione (dal centro verso la parte frontale)
    let front = rotate(0.0, -half * 0.8);
    painter.arrow(
        center,
        front - center,
        Stroke::new(2.5, Color32::WHITE),
    );

    // Label "R"
    painter.text(
        center,
        Align2::CENTER_CENTER,
        "R",
        FontId::proportional(13.0),
        Color32::WHITE,
    );

    // Etichetta heading
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
        let hint = if is_near_robot_raw(pos, field, &[], [0.0,0.0], 0.0) {
            ""
        } else {
            ""
        };
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
// Hit-test helpers
// ════════════════════════════════════════════════════════════

fn waypoint_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<usize> {
    // Start = indice 0
    if field.to_screen(app.start).distance(pos) <= WAYPOINT_RADIUS + 4.0 {
        return Some(0);
    }
    for (i, seg) in app.segments.iter().enumerate() {
        if field.to_screen(seg.end()).distance(pos) <= WAYPOINT_RADIUS + 4.0 {
            return Some(i + 1);
        }
    }
    None
}

fn control_point_at(pos: Pos2, field: &FieldView, app: &HudApp) -> Option<(usize, usize)> {
    for (si, seg) in app.segments.iter().enumerate() {
        if let Segment::Curve { controls, .. } = seg {
            for (ci, cp) in controls.iter().enumerate() {
                if field.to_screen(*cp).distance(pos) <= CTRL_RADIUS + 4.0 {
                    return Some((si, ci));
                }
            }
        }
    }
    None
}

fn is_near_robot(pos: Pos2, field: &FieldView, app: &HudApp) -> bool {
    let center = field.to_screen(app.robot_pos);
    let scale = field.rect.width() / FIELD_CM;
    let half = (ROBOT_SIZE_CM / 2.0) * scale;
    // Approssimazione con cerchio
    center.distance(pos) <= half * 1.1
}

fn is_near_robot_raw(pos: Pos2, field: &FieldView, _pts: &[[f32;2]], robot_pos: [f32;2], _h: f32) -> bool {
    false // placeholder, non usata nel draw
}

// ════════════════════════════════════════════════════════════
// Utility
// ════════════════════════════════════════════════════════════

/// Ultimo punto del percorso (il cursore da cui partire con il prossimo segmento)
fn last_point(app: &HudApp) -> [f32; 2] {
    app.segments.last().map(|s| s.end()).unwrap_or(app.start)
}

/// Rimuove il nodo i dal percorso (0 = start, altrimenti segmento i-1)
fn remove_node(app: &mut HudApp, i: usize) {
    if i == 0 && !app.segments.is_empty() {
        // Rimuovi il primo segmento, il secondo (se esiste) collega al nuovo start
        let removed_end = app.segments[0].end();
        app.segments.remove(0);
        // Nuovo start = dove finiva il primo segmento — o lascio lo start dove sta?
        // Scelta: lo start diventa il vecchio end del primo segmento
        // (comportamento naturale per ricostruire il percorso)
    } else if i > 0 && i <= app.segments.len() {
        app.segments.remove(i - 1);
    }
}
