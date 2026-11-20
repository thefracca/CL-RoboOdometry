use eframe::egui;
use crate::{assets, ui};
use crate::barrier::Barrier;
use crate::segment::{mechanism_active, Highlight, Mechanism, Segment};
use crate::simulation::{Simulation, SimState};

#[derive(Clone, Debug)]
pub struct FocalLine {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub snap_radius_cm: f32, // distanza entro cui il punto si attrae sulla linea
    pub visible: bool,       // se false: nascosta e senza snap
}

impl FocalLine {
    pub fn new(a: [f32; 2], b: [f32; 2]) -> Self {
        Self { a, b, snap_radius_cm: 15.0, visible: true }
    }

    /// Proietta il punto p sulla retta definita da a→b.
    pub fn project(&self, p: [f32; 2]) -> [f32; 2] {
        let ab = [self.b[0]-self.a[0], self.b[1]-self.a[1]];
        let ap = [p[0]-self.a[0],     p[1]-self.a[1]];
        let len2 = ab[0]*ab[0] + ab[1]*ab[1];
        if len2 < 0.001 { return p; }
        let t = (ap[0]*ab[0] + ap[1]*ab[1]) / len2;
        [self.a[0] + t*ab[0], self.a[1] + t*ab[1]]
    }

    pub fn dist(&self, p: [f32; 2]) -> f32 {
        let proj = self.project(p);
        let dx = p[0]-proj[0]; let dy = p[1]-proj[1];
        (dx*dx+dy*dy).sqrt()
    }
}

#[derive(Default, Clone, PartialEq, Debug)]
pub enum EditMode {
    #[default]
    Select,
    AddLine,
    AddCurve,
    AddBarrier,
    AddFocalLine, // primo click = punto A, secondo click = punto B
    AddSearch,    // click = punto di arrivo della ricerca palline
    AddHighlight, // trascina sul percorso per evidenziare un tratto intake/flywheel
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
    /// Sta trascinando un tratto evidenziato (bozza in `highlight_draft`)
    Highlighting,
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
    pub intake_active:   bool,
    pub flywheel_active: bool,
    /// Linee focali: coppie di punti che definiscono una retta infinita
    pub focal_lines: Vec<FocalLine>,
    pub focal_line_first: Option<[f32; 2]>, // primo punto mentre si disegna
    /// Tratti evidenziati dove intake / flywheel sono attivi
    pub highlights:      Vec<Highlight>,
    pub highlight_kind:  Mechanism,
    pub highlight_draft: Option<(f32, f32)>,
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
            focal_lines:      Vec::new(),
            focal_line_first: None,
            highlights:      Vec::new(),
            highlight_kind:  Mechanism::Intake,
            highlight_draft: None,
        }
    }
}

impl HudApp {
    /// Rimuove un segmento e riallinea i tratti evidenziati.
    pub fn remove_segment(&mut self, i: usize) {
        if i >= self.segments.len() { return; }
        self.segments.remove(i);
        let fi = i as f32;
        let map = |p: f32| if p >= fi + 1.0 { p - 1.0 } else if p > fi { fi } else { p };
        for h in &mut self.highlights {
            h.from = map(h.from);
            h.to   = map(h.to);
        }
        self.highlights.retain(|h| h.to - h.from > 0.01);
    }

    /// Cancella tutto il percorso (e i tratti evidenziati, che ne dipendono).
    pub fn clear_path(&mut self) {
        self.segments.clear();
        self.highlights.clear();
        self.highlight_draft = None;
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
                // Intake / flywheel: dai tratti evidenziati (+ sparo/ricerca della simulazione)
                let gp = self.sim.seg_index as f32 + self.sim.t;
                self.intake_active   = intake   || mechanism_active(&self.highlights, Mechanism::Intake,   gp);
                self.flywheel_active = flywheel || mechanism_active(&self.highlights, Mechanism::Flywheel, gp);
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
        egui::SidePanel::right("shortcuts").default_width(200.0).resizable(false).show(ctx, |ui| {
            ui::field_canvas::show_shortcuts(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui::field_canvas::show(ui, self);
        });
    }
}
