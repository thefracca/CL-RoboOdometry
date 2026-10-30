// Stato dell'app e ciclo di aggiornamento.

use eframe::egui;
use crate::{assets, ui};
use crate::segment::{Segment, HeadingMode};

/// Cosa si sta facendo con il mouse sul campo
#[derive(Default, Clone, PartialEq, Debug)]
pub enum EditMode {
    #[default]
    Select,        // trascina punti/robot/control-point
    AddLine,       // il prossimo click aggiunge un nodo Line
    AddCurve,      // il prossimo click aggiunge un nodo Curve (con 1 control point)
}

/// Cosa si sta trascinando in questo momento
#[derive(Default, Clone, Debug)]
pub enum Dragging {
    #[default]
    Nothing,
    /// Indice del nodo (waypoint) finale di un segmento
    Node(usize),
    /// (indice segmento, indice control-point dentro quel segmento)
    ControlPoint(usize, usize),
    /// Il robot: si trascina la posizione
    Robot,
    /// Il robot: si ruota con drag orizzontale (Shift premuto)
    RotateRobot { start_x: f32, start_heading: f32 },
}

pub struct HudApp {
    // ---- Percorso ----
    /// Punto di partenza del percorso in cm (origine in basso a sinistra)
    pub start: [f32; 2],
    /// Sequenza di segmenti. Ogni segmento conosce il proprio punto di arrivo.
    pub segments: Vec<Segment>,

    // ---- Robot fisico ----
    /// Posizione del robot in cm
    pub robot_pos: [f32; 2],
    /// Heading del robot in gradi (0 = destra, 90 = su, 180 = sinistra, 270 = giù)
    pub robot_heading: f32,

    // ---- UI ----
    pub mode: EditMode,
    pub dragging: Dragging,

    pub hide_grid: bool,
    pub field_texture: Option<egui::TextureHandle>,
}

impl Default for HudApp {
    fn default() -> Self {
        use crate::field::FIELD_CM;
        let center = FIELD_CM / 2.0;
        Self {
            start: [center, center],
            segments: Vec::new(),
            robot_pos: [center, center],
            robot_heading: 90.0,
            mode: EditMode::default(),
            dragging: Dragging::default(),
            hide_grid: false,
            field_texture: None,
        }
    }
}

impl eframe::App for HudApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.field_texture.is_none() {
            self.field_texture = Some(assets::load_field_texture(root.ctx()));
        }
        ui::side_panel::show(root, self);
        ui::field_canvas::show(root, self);
    }
}
