// Immagini incorporate nell'eseguibile con include_bytes!:
// il file .exe finale e' uno solo, senza cartelle da copiare insieme.
// I percorsi sono relativi a QUESTO file (src/assets.rs).

use eframe::egui::{ColorImage, Context, IconData, TextureHandle, TextureOptions};

const FIELD_PNG: &[u8] = include_bytes!("../assets/field.png");
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

// Decodifica la PNG dell'icona nel formato che vuole la finestra.
pub fn load_window_icon() -> IconData {
    let img = image::load_from_memory(ICON_PNG)
        .expect("assets/icon.png non e' una PNG valida")
        .to_rgba8();
    let (width, height) = img.dimensions();
    IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

// Decodifica l'immagine del campo e la carica sulla GPU come texture.
pub fn load_field_texture(ctx: &Context) -> TextureHandle {
    let img = image::load_from_memory(FIELD_PNG)
        .expect("assets/field.png non e' un'immagine valida")
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture("field", color_image, TextureOptions::LINEAR)
}
