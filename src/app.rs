use eframe::egui;
use crate::{assets, ui};
use crate::barrier::Barrier;
use crate::segment::{Segment, HeadingMode, SegmentOptions};
use crate::simulation::{Simulation, SimState};

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
    /// Trascina il brake_point del segmento i lungo il segmento stesso
    BrakePoint(usize),
}

pub struct HudApp {
    pub start:         [f32; 2],
    pub segments:      Vec<Segment>,
    pub robot_pos:     [f32; 2],
    pub robot_heading: f32,
    pub barriers:      Vec<Barrier>,
    pub mode:          EditMode,
    pub dragging:      Dragging,
    pub hide_grid:     bool,
    pub field_texture: Option<egui::TextureHandle>,
    pub sim:           Simulation,
    // Stato corrente intake/flywheel (aggiornato dalla sim)
    pub intake_active:   bool,
    pub flywheel_active: bool,
}

impl Default for HudApp {
    fn default() -> Self {
        use crate::field::FIELD_CM;
        let center = FIELD_CM / 2.0;
        Self {
            start:           [center, center],
            segments:        Vec::new(),
            robot_pos:       [center, center],
            robot_heading:   90.0,
            barriers:        Vec::new(),
            mode:            EditMode::default(),
            dragging:        Dragging::default(),
            hide_grid:       false,
            field_texture:   None,
            sim:             Simulation::default(),
            intake_active:   false,
            flywheel_active: false,
        }
    }
}

impl eframe::App for HudApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.field_texture.is_none() {
            self.field_texture = Some(assets::load_field_texture(ctx));
        }

        // Aggiorna simulazione
        if self.sim.is_running() {
            let dt    = ctx.input(|i| i.unstable_dt).min(0.05);
            let start = self.start;
            let segs  = &self.segments;
            if let Some((pos, heading, intake, flywheel)) = self.sim.advance(dt, start, segs) {
                self.robot_pos       = pos;
                self.robot_heading   = heading;
                self.intake_active   = intake;
                self.flywheel_active = flywheel;
            }
            if self.sim.state == SimState::Finished {
                self.intake_active   = false;
                self.flywheel_active = false;
            }
            ctx.request_repaint();
        }

        egui::SidePanel::left("side").default_width(180.0).resizable(true).show(ctx, |ui| {
            ui::side_panel::show(ui, self);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui::field_canvas::show(ui, self);
        });
    }
}
