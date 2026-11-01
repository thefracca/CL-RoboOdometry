#[derive(Clone, Debug, PartialEq)]
pub enum HeadingMode {
    Tangent,
    Fixed(f32),
}

impl Default for HeadingMode {
    fn default() -> Self { HeadingMode::Tangent }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RobotAction {
    Move,
    Wait(f32),       // attendi N secondi fermo (senza sparare)
    ShootStop(f32),  // spara fermo per N secondi
    ShootMove(f32),  // spara mentre si muove
}

impl Default for RobotAction {
    fn default() -> Self { RobotAction::Move }
}

/// Profilo di velocità per un segmento.
#[derive(Clone, Debug)]
pub struct SpeedProfile {
    pub v_start:     f32,  // cm/s — velocità iniziale
    pub v_max:       f32,  // cm/s — velocità massima
    pub accel:       f32,  // cm/s² — accelerazione
    pub decel:       f32,  // cm/s² — decelerazione
    /// t ∈ [0,1] — dove inizia la frenata sul segmento
    pub brake_point: f32,
}

impl Default for SpeedProfile {
    fn default() -> Self {
        Self { v_start: 0.0, v_max: 80.0, accel: 120.0, decel: 200.0, brake_point: 0.75 }
    }
}

impl SpeedProfile {
    /// Velocità istantanea dato t ∈ [0,1] e la lunghezza del segmento.
    /// Fase 1: accelera da v_start verso v_max
    /// Fase 2: viaggia a v_max
    /// Fase 3: da brake_point decelera verso 0
    pub fn velocity_at(&self, t: f32, _seg_len: f32) -> f32 {
        let bp = self.brake_point.clamp(0.01, 0.99);

        if t >= bp {
            // Fase frenata: decelera linearmente da v_max a 0
            let frac = (t - bp) / (1.0 - bp);
            (self.v_max * (1.0 - frac)).max(1.0)
        } else {
            // Fase accelerazione: da v_start a v_max
            let frac = t / bp;
            (self.v_start + (self.v_max - self.v_start) * frac).max(1.0)
        }
    }
}

#[derive(Clone, Debug)]
pub struct SegmentOptions {
    pub heading:       HeadingMode,
    pub speed:         SpeedProfile,
    pub action:        RobotAction,
    pub intake:        bool,
    pub flywheel:      bool,
    /// Heading del robot (dove guarda) all'inizio del segmento, in gradi.
    /// Indipendente dalla direzione di movimento (mecanum).
    pub robot_heading_start: f32,
    /// Heading del robot alla fine del segmento. Viene interpolato linearmente.
    pub robot_heading_end:   f32,
}

impl Default for SegmentOptions {
    fn default() -> Self {
        Self {
            heading:             HeadingMode::default(),
            speed:               SpeedProfile::default(),
            action:              RobotAction::default(),
            intake:              false,
            flywheel:            false,
            robot_heading_start: 90.0,
            robot_heading_end:   90.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Segment {
    Line  { to: [f32; 2], opts: SegmentOptions },
    Curve { controls: Vec<[f32; 2]>, to: [f32; 2], opts: SegmentOptions },
}

impl Segment {
    pub fn end(&self) -> [f32; 2] {
        match self { Segment::Line { to, .. } | Segment::Curve { to, .. } => *to }
    }

    pub fn opts(&self) -> &SegmentOptions {
        match self { Segment::Line { opts, .. } | Segment::Curve { opts, .. } => opts }
    }

    pub fn opts_mut(&mut self) -> &mut SegmentOptions {
        match self { Segment::Line { opts, .. } | Segment::Curve { opts, .. } => opts }
    }

    /// Campiona il segmento in n punti uniformi in t — anche le linee
    /// vengono campionate densamente per avere movimento fluido.
    pub fn sample(&self, from: [f32; 2], n: usize) -> Vec<[f32; 2]> {
        match self {
            Segment::Line { to, .. } => {
                (0..=n).map(|i| {
                    let t = i as f32 / n as f32;
                    [from[0] + t * (to[0] - from[0]),
                     from[1] + t * (to[1] - from[1])]
                }).collect()
            }
            Segment::Curve { controls, to, .. } => {
                let p0 = from;
                let (p1, p2) = match controls.len() {
                    0 => (from, *to),
                    1 => (controls[0], controls[0]),
                    _ => (controls[0], controls[controls.len() - 1]),
                };
                let p3 = *to;
                (0..=n).map(|i| {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    [u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0],
                     u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1]]
                }).collect()
            }
        }
    }
}
