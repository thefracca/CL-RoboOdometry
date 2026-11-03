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
///
/// Cinematica (usata identica da simulazione e tempo stimato):
///  - parte da `v_start` e accelera a `accel` fino a `v_max` (poi mantiene v_max)
///  - al `brake_point` (frazione della lunghezza del segmento) decelera a `decel`
///    ma mai sotto `v_min`
///  - al punto di arrivo il robot si ferma sul posto (anche se v > 0)
#[derive(Clone, Debug)]
pub struct SpeedProfile {
    pub v_start:     f32,  // cm/s — velocità iniziale
    pub v_max:       f32,  // cm/s — velocità massima
    pub v_min:       f32,  // cm/s — velocità minima in frenata
    pub accel:       f32,  // cm/s² — accelerazione
    pub decel:       f32,  // cm/s² — decelerazione
    /// Frazione [0,1] della lunghezza del segmento dove inizia la frenata
    pub brake_point: f32,
}

impl Default for SpeedProfile {
    fn default() -> Self {
        Self { v_start: 0.0, v_max: 80.0, v_min: 25.0, accel: 120.0, decel: 200.0, brake_point: 0.75 }
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
    /// Velocità di rotazione (°/s) usata per ruotare dall'heading_end del segmento
    /// precedente all'heading_start di questo segmento (fase Rotating tra segmenti).
    pub rotation_speed: f32,
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
            rotation_speed:      180.0, // °/s default
        }
    }
}

/// Parametri della "ricerca palline": il robot va in linea retta dal punto
/// precedente a `to` a velocità costante `v`, cercando con la camera dentro
/// un rettangolo largo `width` il cui asse è la retta partenza→arrivo
/// (i punti medi dei due lati corti sono partenza e arrivo).
#[derive(Clone, Debug)]
pub struct SearchParams {
    pub v:           f32,  // cm/s — velocità costante
    pub width:       f32,  // cm — larghezza dell'area di ricerca
    pub time_limit:  f32,  // s — oltre questo tempo va diretto al punto finale
    pub shoot_after: bool, // spara fermo appena finita la ricerca
    pub shoot_secs:  f32,  // s di sparo (se shoot_after)
}

impl Default for SearchParams {
    fn default() -> Self {
        Self { v: 60.0, width: 60.0, time_limit: 5.0, shoot_after: false, shoot_secs: 1.0 }
    }
}

#[derive(Clone, Debug)]
pub enum Segment {
    Line   { to: [f32; 2], opts: SegmentOptions },
    Curve  { controls: Vec<[f32; 2]>, to: [f32; 2], opts: SegmentOptions },
    /// Ricerca palline con camera: linea retta a velocità costante.
    /// `opts` serve solo per gli heading del robot (start/end/rotation_speed).
    Search { to: [f32; 2], search: SearchParams, opts: SegmentOptions },
}

impl Segment {
    pub fn end(&self) -> [f32; 2] {
        match self {
            Segment::Line { to, .. } | Segment::Curve { to, .. } | Segment::Search { to, .. } => *to
        }
    }

    pub fn set_end(&mut self, p: [f32; 2]) {
        match self {
            Segment::Line { to, .. } | Segment::Curve { to, .. } | Segment::Search { to, .. } => *to = p,
        }
    }

    pub fn opts(&self) -> &SegmentOptions {
        match self {
            Segment::Line { opts, .. } | Segment::Curve { opts, .. } | Segment::Search { opts, .. } => opts
        }
    }

    pub fn opts_mut(&mut self) -> &mut SegmentOptions {
        match self {
            Segment::Line { opts, .. } | Segment::Curve { opts, .. } | Segment::Search { opts, .. } => opts
        }
    }

    /// Campiona il segmento in n punti uniformi in t — anche le linee
    /// vengono campionate densamente per avere movimento fluido.
    pub fn sample(&self, from: [f32; 2], n: usize) -> Vec<[f32; 2]> {
        match self {
            Segment::Line { to, .. } | Segment::Search { to, .. } => {
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
