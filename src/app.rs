use eframe::egui;
use crate::{assets, ui};
use crate::barrier::Barrier;
use crate::segment::{Segment, HeadingMode};
use crate::simulation::Simulation;

#[derive(Default, Clone, PartialEq, Debug)]
pub enum EditMode {
    #[default]
    Select,
    AddLine,
    AddCurve,
    AddBarrier,
}

#[derive(Default, Clone, Debug)]
pub enum Dragging {
    #[default]
    Nothing,
    Node(usize),
    ControlPoint(usize, usize),
    Robot,
    RotateRobot { start_x: f32, start_heading: f32 },
    Barrier(usize),
    BarrierVertex(usize, usize),
}

pub struct HudApp {
    pub start: [f32; 2],
    pub segments: Vec<Segment>,
    pub robot_pos: [f32; 2],
    pub robot_heading: f32,
    pub barriers: Vec<Barrier>,
    pub mode: EditMode,
    pub dragging: Dragging,
    pub hide_grid: bool,
    pub field_texture: Option<egui::TextureHandle>,
    pub sim: Simulation,
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
            barriers: Vec::new(),
            mode: EditMode::default(),
            dragging: Dragging::default(),
            hide_grid: false,
            field_texture: None,
            sim: Simulation::default(),
        }
    }
}

impl eframe::App for HudApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.field_texture.is_none() {
            self.field_texture = Some(assets::load_field_texture(root.ctx()));
        }

        // Aggiorna simulazione
        if self.sim.state == crate::simulation::SimState::Running {
            let dt = root.ctx().input(|i| i.unstable_dt).min(0.05);
            let start = self.start;
            let segs = &self.segments;
            if let Some((pos, heading)) = self.sim.advance(dt, start, segs) {
                self.robot_pos = pos;
                self.robot_heading = heading;
            }
            root.ctx().request_repaint();
        }

        ui::side_panel::show(root, self);
        ui::field_canvas::show(root, self);
    }
}
