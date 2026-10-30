// Il campo FTC: dimensioni e conversione tra coordinate campo (cm) e schermo (pixel).
// Origine (0,0) in basso a sinistra, +Y verso l'alto.

use eframe::egui::{Pos2, Rect};

pub const FIELD_CM: f32 = 365.76; // 144" = 6 tile
pub const TILE_CM: f32 = 60.96; // 24"

pub struct FieldView {
    pub rect: Rect, // il quadrato sullo schermo in cui e' disegnato il campo
}

impl FieldView {
    // Lo schermo ha la Y verso il basso, il campo verso l'alto: la Y va invertita.
    pub fn to_screen(&self, p: [f32; 2]) -> Pos2 {
        let scale = self.rect.width() / FIELD_CM;
        Pos2::new(
            self.rect.left() + p[0] * scale,
            self.rect.bottom() - p[1] * scale,
        )
    }

    pub fn to_field(&self, pos: Pos2) -> [f32; 2] {
        let scale = FIELD_CM / self.rect.width();
        [
            (pos.x - self.rect.left()) * scale,
            (self.rect.bottom() - pos.y) * scale,
        ]
    }
}
