//! Simulazione del movimento del robot lungo il percorso.

use crate::field::FIELD_CM;
use crate::segment::Segment;

pub const MATCH_DURATION: f32 = 30.0; // secondi

/// Velocità per ogni segmento (cm/s). Indice i → segmento i.
/// Se manca l'elemento, si usa DEFAULT_SPEED.
pub const DEFAULT_SPEED: f32 = 60.0;

#[derive(Default, Clone, Debug, PartialEq)]
pub enum SimState {
    #[default]
    Idle,
    Running,
    Finished,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    pub state: SimState,
    /// Tempo trascorso dall'avvio della simulazione (secondi)
    pub elapsed: f32,
    /// Indice del segmento corrente
    pub seg_index: usize,
    /// Parametro t [0..1] dentro il segmento corrente
    pub t: f32,
    /// Velocità per segmento (cm/s)
    pub speeds: Vec<f32>,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            state: SimState::Idle,
            elapsed: 0.0,
            seg_index: 0,
            t: 0.0,
            speeds: Vec::new(),
        }
    }
}

impl Simulation {
    /// Sincronizza il vettore `speeds` con il numero di segmenti.
    pub fn sync_speeds(&mut self, n_segments: usize) {
        if self.speeds.len() < n_segments {
            self.speeds.resize(n_segments, DEFAULT_SPEED);
        }
    }

    /// Avvia la simulazione.
    pub fn start(&mut self) {
        self.state = SimState::Running;
        self.elapsed = 0.0;
        self.seg_index = 0;
        self.t = 0.0;
    }

    pub fn stop(&mut self) {
        self.state = SimState::Idle;
    }

    /// Avanza di `dt` secondi. Restituisce la posizione [x, y] e l'heading (gradi).
    /// `start` = punto di partenza del percorso, `segments` = lista segmenti.
    pub fn advance(
        &mut self,
        dt: f32,
        start: [f32; 2],
        segments: &[Segment],
    ) -> Option<([f32; 2], f32)> {
        if self.state != SimState::Running { return None; }
        if segments.is_empty() {
            self.state = SimState::Finished;
            return None;
        }

        self.elapsed += dt;
        if self.elapsed >= MATCH_DURATION {
            self.state = SimState::Finished;
        }

        if self.seg_index >= segments.len() {
            self.state = SimState::Finished;
            return None;
        }

        let seg = &segments[self.seg_index];
        let speed = self.speeds.get(self.seg_index).copied().unwrap_or(DEFAULT_SPEED);

        // Lunghezza approssimativa del segmento (campionamento)
        let from = prev_point(start, segments, self.seg_index);
        let pts = seg.sample(from, 40);
        let seg_len = arc_length(&pts);
        if seg_len < 0.001 {
            self.next_segment(segments);
            return self.advance(0.0, start, segments);
        }

        // Quanto t avanza in dt?
        let dt_t = (speed * dt) / seg_len;
        self.t += dt_t;

        if self.t >= 1.0 {
            let overshoot_dt = (self.t - 1.0) * seg_len / speed;
            self.t = 0.0;
            self.next_segment(segments);
            // Ricorsione con il tempo rimanente nel prossimo segmento
            return self.advance(overshoot_dt, start, segments);
        }

        let pos = sample_at(seg, from, self.t);
        let heading = tangent_heading(seg, from, self.t);
        Some((pos, heading))
    }

    fn next_segment(&mut self, segments: &[Segment]) {
        self.seg_index += 1;
        self.t = 0.0;
        if self.seg_index >= segments.len() {
            self.state = SimState::Finished;
        }
    }
}

// ─── helpers ──────────────────────────────────────────────────

/// Punto di partenza del segmento `i`.
fn prev_point(start: [f32; 2], segments: &[Segment], i: usize) -> [f32; 2] {
    if i == 0 { start } else { segments[i - 1].end() }
}

/// Lunghezza dell'arco approssimata da una polilinea campionata.
fn arc_length(pts: &[[f32; 2]]) -> f32 {
    pts.windows(2)
        .map(|w| {
            let dx = w[1][0] - w[0][0];
            let dy = w[1][1] - w[0][1];
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

/// Posizione sul segmento al parametro t ∈ [0,1].
fn sample_at(seg: &Segment, from: [f32; 2], t: f32) -> [f32; 2] {
    let pts = seg.sample(from, 80);
    let idx = ((t * (pts.len() - 1) as f32) as usize).min(pts.len() - 1);
    pts[idx]
}

/// Heading (gradi, 0=destra 90=su) dalla tangente al segmento in t.
fn tangent_heading(seg: &Segment, from: [f32; 2], t: f32) -> f32 {
    let pts = seg.sample(from, 80);
    let n = pts.len();
    let idx = ((t * (n - 1) as f32) as usize).min(n - 2);
    let dx = pts[idx + 1][0] - pts[idx][0];
    let dy = pts[idx + 1][1] - pts[idx][1];
    // In coordinate campo: y cresce verso l'alto → atan2(dy, dx) già corretto
    let rad = dy.atan2(dx);
    rad.to_degrees().rem_euclid(360.0)
}
