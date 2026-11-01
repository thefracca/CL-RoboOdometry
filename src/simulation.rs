use crate::segment::{RobotAction, Segment};

pub const MATCH_DURATION: f32 = 30.0;

#[derive(Default, Clone, Debug, PartialEq)]
pub enum SimState {
    #[default]
    Idle,
    Running,
    /// Robot fermo in attesa
    Waiting { remaining: f32 },
    /// Robot fermo, sta sparando per `remaining` secondi
    ShootStop { remaining: f32 },
    /// Robot in movimento, sta sparando (nessuna pausa)
    ShootMove,
    Finished,
}

#[derive(Clone, Debug, Default)]
pub struct Simulation {
    pub state:     SimState,
    pub elapsed:   f32,
    pub seg_index: usize,
    pub t:         f32,
}

impl Simulation {
    pub fn start(&mut self) {
        self.state     = SimState::Running;
        self.elapsed   = 0.0;
        self.seg_index = 0;
        self.t         = 0.0;
    }

    pub fn stop(&mut self) {
        self.state = SimState::Idle;
    }

    pub fn is_running(&self) -> bool {
        !matches!(self.state, SimState::Idle | SimState::Finished)
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self.state, SimState::Waiting { .. })
    }

    pub fn advance(
        &mut self,
        dt: f32,
        start: [f32; 2],
        segments: &[Segment],
    ) -> Option<([f32; 2], f32, bool, bool)> {
        if segments.is_empty() { self.state = SimState::Finished; return None; }

        self.elapsed += dt;

        // ── Bug 2 fix: timer scaduto → Finished ma restituisce ultima pos ──
        if self.elapsed >= MATCH_DURATION {
            self.state = SimState::Finished;
            let pos = current_pos(start, segments, self.seg_index, self.t);
            return Some((pos.0, pos.1, false, false));
        }

        // ── Wait: fermo senza sparare ─────────────────────────
        if let SimState::Waiting { remaining } = &mut self.state {
            *remaining -= dt;
            let done = *remaining <= 0.0;
            // Leggi pos/opts PRIMA di incrementare seg_index
            let seg_i = self.seg_index.min(segments.len().saturating_sub(1));
            let pos   = segments[seg_i].end();
            let intake   = segments[seg_i].opts().intake;
            let flywheel = segments[seg_i].opts().flywheel;
            if done {
                self.seg_index += 1;
                self.t = 0.0;
                self.state = if self.seg_index >= segments.len() {
                    SimState::Finished
                } else {
                    SimState::Running
                };
            }
            return Some((pos, 90.0, intake, flywheel));
        }

        if let SimState::ShootStop { remaining } = &mut self.state {
            *remaining -= dt;
            let done = *remaining <= 0.0;
            // Leggi pos/opts PRIMA di incrementare seg_index
            let seg_i = self.seg_index.min(segments.len().saturating_sub(1));
            let pos   = segments[seg_i].end();
            let intake   = segments[seg_i].opts().intake;
            let flywheel = segments[seg_i].opts().flywheel;
            if done {
                self.seg_index += 1;
                self.t = 0.0;
                self.state = if self.seg_index >= segments.len() {
                    SimState::Finished
                } else {
                    SimState::Running
                };
            }
            return Some((pos, 90.0, intake, flywheel));
        }

        if self.seg_index >= segments.len() { self.state = SimState::Finished; return None; }

        let seg    = &segments[self.seg_index];
        let opts   = seg.opts();
        let from   = prev_point(start, segments, self.seg_index);
        let pts    = seg.sample(from, 80);
        let seg_len = arc_length(&pts);

        // Flywheel attivo se ShootMove o se impostato nel segmento
        let shooting_move = self.state == SimState::ShootMove;

        if seg_len < 0.001 {
            self.finish_segment(segments);
            return self.advance(0.0, start, segments);
        }

        let v    = opts.speed.velocity_at(self.t, seg_len).max(1.0);
        let dt_t = (v * dt) / seg_len;
        self.t  += dt_t;

        if self.t >= 1.0 {
            let overshoot = ((self.t - 1.0) * seg_len / v).min(dt);
            self.t = 1.0;
            let pos     = sample_at_t(&pts, 1.0);
            let heading = interp_heading(opts.robot_heading_start, opts.robot_heading_end, 1.0);
            self.finish_segment(segments);
            if matches!(self.state, SimState::Running | SimState::ShootMove) {
                return self.advance(overshoot, start, segments);
            }
            return Some((pos, heading, opts.intake, opts.flywheel));
        }

        let pos     = sample_at_t(&pts, self.t);
        let heading = interp_heading(opts.robot_heading_start, opts.robot_heading_end, self.t);
        Some((pos, heading, opts.intake, opts.flywheel || shooting_move))
    }

    fn finish_segment(&mut self, segments: &[Segment]) {
        let seg = &segments[self.seg_index];
        match seg.opts().action {
            RobotAction::Wait(secs) => {
                self.state = SimState::Waiting { remaining: secs };
                self.t = 0.0;
            }
            RobotAction::ShootStop(secs) => {
                // Bug 3 fix: NON incrementiamo seg_index qui,
                // lo fa ShootStop quando il timer scade
                self.state = SimState::ShootStop { remaining: secs };
                self.t = 0.0;
                // seg_index rimane invariato finché Shoot non finisce
            }
            RobotAction::ShootMove(_) => {
                // Spara mentre si muove: passa al prossimo segmento normalmente
                self.state = SimState::ShootMove;
                self.seg_index += 1;
                self.t = 0.0;
                if self.seg_index >= segments.len() {
                    self.state = SimState::Finished;
                }
            }
            RobotAction::Move => {
                self.seg_index += 1;
                self.t = 0.0;
                if self.seg_index >= segments.len() {
                    self.state = SimState::Finished;
                } else {
                    self.state = SimState::Running;
                }
            }
        }
    }
}

fn current_pos(start: [f32; 2], segments: &[Segment], seg_i: usize, t: f32) -> ([f32; 2], f32) {
    let i = seg_i.min(segments.len().saturating_sub(1));
    let from = if i == 0 { start } else { segments[i-1].end() };
    let pts = segments[i].sample(from, 80);
    (sample_at_t(&pts, t), tangent_at(&pts, t))
}

fn prev_point(start: [f32; 2], segments: &[Segment], i: usize) -> [f32; 2] {
    if i == 0 { start } else { segments[i - 1].end() }
}

fn arc_length(pts: &[[f32; 2]]) -> f32 {
    pts.windows(2).map(|w| {
        let dx = w[1][0] - w[0][0];
        let dy = w[1][1] - w[0][1];
        (dx*dx + dy*dy).sqrt()
    }).sum()
}

fn sample_at_t(pts: &[[f32; 2]], t: f32) -> [f32; 2] {
    let n = pts.len() - 1;
    pts[((t * n as f32) as usize).min(n)]
}

fn tangent_at(pts: &[[f32; 2]], t: f32) -> f32 {
    let n = pts.len();
    let idx = ((t * (n-1) as f32) as usize).min(n-2);
    let dx = pts[idx+1][0] - pts[idx][0];
    let dy = pts[idx+1][1] - pts[idx][1];
    dy.atan2(dx).to_degrees().rem_euclid(360.0)
}

/// Interpola l'heading del robot tra start e end lungo t ∈ [0,1].
/// Prende il percorso più corto attorno al cerchio (max 180°).
fn interp_heading(start: f32, end: f32, t: f32) -> f32 {
    let mut delta = (end - start).rem_euclid(360.0);
    if delta > 180.0 { delta -= 360.0; }
    (start + delta * t).rem_euclid(360.0)
}
