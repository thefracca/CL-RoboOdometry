use crate::segment::{RobotAction, Segment};

pub const MATCH_DURATION: f32 = 30.0;

#[derive(Default, Clone, Debug, PartialEq)]
pub enum SimState {
    #[default]
    Idle,
    Running,
    Shooting { remaining: f32 }, // attende N secondi fermo
    Finished,
}

#[derive(Clone, Debug, Default)]
pub struct Simulation {
    pub state:     SimState,
    pub elapsed:   f32,
    pub seg_index: usize,
    pub t:         f32,     // parametro [0,1] dentro il segmento corrente
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
        matches!(self.state, SimState::Running | SimState::Shooting { .. })
    }

    /// Avanza di dt. Restituisce (pos, heading, intake, flywheel) se in movimento.
    pub fn advance(
        &mut self,
        dt: f32,
        start: [f32; 2],
        segments: &[Segment],
    ) -> Option<([f32; 2], f32, bool, bool)> {
        if segments.is_empty() { self.state = SimState::Finished; return None; }

        self.elapsed += dt;
        if self.elapsed >= MATCH_DURATION { self.state = SimState::Finished; return None; }

        // ── Fase Shoot (robot fermo) ──────────────────────────
        if let SimState::Shooting { remaining } = &mut self.state {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.next_segment(segments);
            }
            // Restituisce la posizione corrente (fine del segmento precedente)
            let pos = if self.seg_index == 0 { start }
                      else { segments[self.seg_index.saturating_sub(1)].end() };
            let opts = if self.seg_index > 0 {
                segments[self.seg_index - 1].opts()
            } else { segments[0].opts() };
            return Some((pos, 90.0, opts.intake, opts.flywheel));
        }

        if self.seg_index >= segments.len() { self.state = SimState::Finished; return None; }

        let seg   = &segments[self.seg_index];
        let opts  = seg.opts();
        let from  = prev_point(start, segments, self.seg_index);
        let pts   = seg.sample(from, 80);
        let seg_len = arc_length(&pts);

        if seg_len < 0.001 {
            self.finish_segment(segments);
            return self.advance(0.0, start, segments);
        }

        // Velocità istantanea in base al profilo
        let v = opts.speed.velocity_at(self.t, seg_len).max(1.0);
        let dt_t = (v * dt) / seg_len;
        self.t += dt_t;

        if self.t >= 1.0 {
            let overshoot = ((self.t - 1.0) * seg_len / v).min(dt);
            self.t = 1.0;
            let pos = sample_at_t(&pts, 1.0);
            let heading = tangent_at(&pts, 1.0);
            self.finish_segment(segments);
            // Se c'era overshoot e non siamo in Shoot, recupera il tempo
            if matches!(self.state, SimState::Running) {
                return self.advance(overshoot, start, segments);
            }
            return Some((pos, heading, opts.intake, opts.flywheel));
        }

        let pos     = sample_at_t(&pts, self.t);
        let heading = tangent_at(&pts, self.t);
        Some((pos, heading, opts.intake, opts.flywheel))
    }

    fn finish_segment(&mut self, segments: &[Segment]) {
        let seg = &segments[self.seg_index];
        // Controlla se c'è un'azione Shoot alla fine
        if let RobotAction::Shoot(secs) = seg.opts().action {
            self.state = SimState::Shooting { remaining: secs };
            self.seg_index += 1;
            self.t = 0.0;
        } else {
            self.next_segment(segments);
        }
    }

    fn next_segment(&mut self, segments: &[Segment]) {
        self.seg_index += 1;
        self.t = 0.0;
        if self.seg_index >= segments.len() {
            self.state = SimState::Finished;
        } else {
            self.state = SimState::Running;
        }
    }
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
    let idx = ((t * n as f32) as usize).min(n);
    pts[idx]
}

fn tangent_at(pts: &[[f32; 2]], t: f32) -> f32 {
    let n = pts.len();
    let idx = ((t * (n - 1) as f32) as usize).min(n - 2);
    let dx = pts[idx + 1][0] - pts[idx][0];
    let dy = pts[idx + 1][1] - pts[idx][1];
    dy.atan2(dx).to_degrees().rem_euclid(360.0)
}
