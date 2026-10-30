// Area centrale: disegno del campo, input del mouse, punti e linee.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Response, Sense, Stroke, Vec2};

use crate::app::HudApp;
use crate::field::{FieldView, FIELD_CM, TILE_CM};

const POINT_RADIUS: f32 = 7.0; // in pixel

pub fn show(ui: &mut egui::Ui, app: &mut HudApp) {
    egui::CentralPanel::default().show(ui, |ui| {
        // il campo e' un quadrato grande quanto il lato piu' corto disponibile
        let avail = ui.available_size();
        let side = avail.x.min(avail.y);
        let (response, painter) = ui.allocate_painter(Vec2::splat(side), Sense::click_and_drag());
        let field = FieldView { rect: response.rect };

        draw_field(&painter, &field);
        handle_input(&response, &field, app);
        draw_points(&painter, &field, app);
        draw_mouse_coords(&painter, &response, &field);
    });
}

// Indice del punto sotto la posizione del mouse, se c'e'.
fn point_at(points: &[[f32; 2]], field: &FieldView, pos: Pos2) -> Option<usize> {
    points
        .iter()
        .position(|p| field.to_screen(*p).distance(pos) <= POINT_RADIUS + 4.0)
}

// Sfondo e griglia a tile da 24" (7 linee per direzione, bordi inclusi).
fn draw_field(painter: &Painter, field: &FieldView) {
    painter.rect_filled(field.rect, 0.0, Color32::from_gray(30));

    let grid = Stroke::new(1.0, Color32::from_gray(70));
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

fn handle_input(response: &Response, field: &FieldView, app: &mut HudApp) {
    // Inizio drag: se parti da un punto, lo selezioni
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            app.selected = point_at(&app.points, field, pos);
        }
    }
    // Durante il drag: il punto selezionato segue il mouse (dentro il campo)
    if response.dragged() {
        if let (Some(i), Some(pos)) = (app.selected, response.interact_pointer_pos()) {
            let f = field.to_field(pos);
            app.points[i] = [f[0].clamp(0.0, FIELD_CM), f[1].clamp(0.0, FIELD_CM)];
        }
    }
    if response.drag_stopped() {
        app.selected = None;
    }
    // Click (senza trascinare): aggiunge un punto se non ce n'e' gia' uno li'
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if point_at(&app.points, field, pos).is_none() {
                app.points.push(field.to_field(pos));
            }
        }
    }
    // Tasto destro: elimina il punto sotto il mouse
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(i) = point_at(&app.points, field, pos) {
                app.points.remove(i);
            }
        }
    }
}

fn draw_points(painter: &Painter, field: &FieldView, app: &HudApp) {
    for w in app.points.windows(2) {
        painter.line_segment(
            [field.to_screen(w[0]), field.to_screen(w[1])],
            Stroke::new(2.0, Color32::LIGHT_BLUE),
        );
    }
    for (i, p) in app.points.iter().enumerate() {
        let pos = field.to_screen(*p);
        let color = if Some(i) == app.selected {
            Color32::YELLOW
        } else {
            Color32::from_rgb(255, 140, 0)
        };
        painter.circle_filled(pos, POINT_RADIUS, color);
        painter.text(
            pos + Vec2::new(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            i.to_string(),
            FontId::proportional(14.0),
            Color32::WHITE,
        );
    }
}

// Coordinate del mouse in alto a sinistra (utile per controllare la scala).
fn draw_mouse_coords(painter: &Painter, response: &Response, field: &FieldView) {
    if let Some(pos) = response.hover_pos() {
        let f = field.to_field(pos);
        painter.text(
            response.rect.left_top() + Vec2::new(8.0, 8.0),
            Align2::LEFT_TOP,
            format!("x={:.1}  y={:.1} cm", f[0], f[1]),
            FontId::monospace(14.0),
            Color32::WHITE,
        );
    }
}
