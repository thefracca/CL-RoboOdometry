// Stato dell'app e ciclo di aggiornamento.
// In egui TUTTO lo stato sta qui: i widget non ricordano nulla.

use eframe::egui;

use crate::ui;

// #[derive(Default)] e' come un costruttore vuoto: Vec vuoto, Option = None.
// I campi sono `pub` perche' i moduli in ui/ devono poterli leggere e modificare.
#[derive(Default)]
pub struct HudApp {
    pub points: Vec<[f32; 2]>,   // punti in cm
    pub selected: Option<usize>, // indice del punto che stai trascinando
}

impl eframe::App for HudApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::side_panel::show(root, self);
        ui::field_canvas::show(root, self);
    }
}