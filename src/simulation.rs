use crate::segment::{Mechanism, ParkPoint, RobotAction, Segment, SpeedProfile};

pub const MATCH_DURATION: f32 = 30.0;
const SAMPLES: usize = 120;
/// Velocità di rotazione sul posto (°/s) usata per stime e simulazione. Non è regolabile:
/// col follower reale la rotazione la gestisce lui.
pub const ROTATION_SPEED_DEG_S: f32 = 180.0;

#[derive(Default, Clone, Debug, PartialEq)]
pub enum SimState {
    #[default]
    Idle,
    Running,
    /// Robot fermo, sta ruotando per allinearsi all'heading del prossimo segmento
    Rotating { from: f32, to: f32, current: f32, speed: f32 },
    /// Robot fermo in attesa
    Waiting { remaining: f32 },
    /// Robot fermo, sta sparando per `remaining` secondi
    ShootStop { remaining: f32 },
    /// Robot in ricerca palline (camera): linea retta a velocità costante
    Searching,
    /// Robot fermo, a fine segmento gira su se stesso fino all'heading `to`
    Aligning { to: f32, current: f32, speed: f32 },
    /// Failsafe: ultimi secondi, lascia tutto e va dritto al punto di parcheggio
    Parking,
    /// Azioni puntuali su un nodo (sparo / intake / flywheel con ritardo e durata):
    /// il robot è fermo a fine segmento, poi esegue l'azione del segmento.
    NodeHold { remaining: f32 },
    Finished,
}

/// Azione puntuale su un nodo: `kind` attivo da `delay` a `delay + secs` secondi dopo l'arrivo.
#[derive(Clone, Debug)]
pub struct NodeAction {
    pub node:  usize, // nodo = arrivo del segmento node-1
    pub kind:  Mechanism,
    pub delay: f32,
    pub secs:  f32,
}

/// Tempo di sosta su un nodo = fine dell'ultima azione puntuale (ritardo + durata).
fn node_hold_time(actions: &[NodeAction], node: usize) -> f32 {
    actions.iter().filter(|a| a.node == node).map(|a| a.delay + a.secs).fold(0.0, f32::max)
}

#[derive(Clone, Debug, Default)]
pub struct Simulation {
    pub state:     SimState,
    pub elapsed:   f32,
    pub seg_index: usize,
    /// Frazione di lunghezza d'arco percorsa nel segmento corrente [0,1]
    pub t:         f32,
    /// Distanza percorsa nel segmento corrente (cm)
    pub s:         f32,
    /// Velocità istantanea (cm/s)
    pub v:         f32,
    /// Tempo massimo di simulazione (s)
    pub limit:     f32,
    /// Tempo trascorso nel segmento corrente (s) — usato dalla ricerca per il time limit
    pub seg_time:  f32,
    /// Parcheggio d'emergenza (aggiornato dall'app ad ogni frame)
    pub park:      Option<ParkPoint>,
    /// Tragitto di emergenza verso il parcheggio (aggira le zone vietate)
    pub park_route: Vec<[f32; 2]>,
    pub park_from_heading: f32,
    /// Azioni puntuali sui nodi (aggiornate dall'app ad ogni frame)
    pub node_actions: Vec<NodeAction>,
    /// Tempo trascorso nella sosta su un nodo (s)
    pub hold_time: f32,
    /// Nella simulazione in corso il corpo del robot ha toccato una zona vietata (aggiornato dall'app)
    pub zone_hit: bool,
}

// ════════════════════════════════════════════════════════════
// Cinematica condivisa (simulazione + tempo stimato)
// ════════════════════════════════════════════════════════════

/// Un passo di integrazione: accelera fino a v_max, dal brake_point decelera
/// fino a v_min. Restituisce (nuova_v, nuova_s).
pub fn kin_step(v: f32, s: f32, seg_len: f32, sp: &SpeedProfile, dt: f32) -> (f32, f32) {
    let s_brake = sp.brake_point.clamp(0.0, 1.0) * seg_len;
    let floor = sp.v_min.max(1.0).min(sp.v_max.max(1.0));
    let mut nv = if s < s_brake {
        (v + sp.accel * dt).min(sp.v_max)
    } else {
        (v - sp.decel * dt).max(floor)
    };
    nv = nv.max(1.0); // il robot non si blocca mai a metà segmento
    (nv, s + nv * dt)
}

/// Heading reale a fine segmento (se c'è un Align è il suo heading di arrivo).
pub fn end_heading(seg: &Segment) -> f32 {
    let o = seg.opts();
    o.align.unwrap_or(o.robot_heading_end)
}

/// Differenza di heading con segno, percorso più corto (-180..180].
pub fn heading_delta(from: f32, to: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 { d -= 360.0; }
    d
}

fn interp_heading(start: f32, end: f32, t: f32) -> f32 {
    (start + heading_delta(start, end) * t).rem_euclid(360.0)
}

/// Profilo di velocità nel tempo dell'intero percorso: punti [tempo s, velocità cm/s].
/// Stessa cinematica della simulazione (passo fine).
pub fn speed_trace(start: [f32; 2], segments: &[Segment], node_actions: &[NodeAction]) -> Vec<[f32; 2]> {
    let mut out: Vec<[f32; 2]> = vec![[0.0, 0.0]];
    let mut total = 0.0_f32;
    let mut prev = start;
    let mut prev_heading_end: Option<f32> = None;

    for (seg_i, seg) in segments.iter().enumerate() {
        let opts = seg.opts();

        // Rotazione sul posto tra la fine del segmento precedente e l'inizio di questo
        if let Some(pe) = prev_heading_end {
            let d = heading_delta(pe, opts.robot_heading_start).abs();
            if d > 1.0 {
                out.push([total, 0.0]);
                total += d / ROTATION_SPEED_DEG_S;
                out.push([total, 0.0]);
            }
        }

        let pts = seg.sample(prev, SAMPLES);
        let len = *cumulative(&pts).last().unwrap_or(&0.0);
        if len > 0.001 {
            if let Segment::Search { search, .. } = seg {
                // Ricerca: velocità costante, si ferma sul posto all'arrivo
                let v = search.v.max(1.0);
                out.push([total, v]);
                total += len / v;
                out.push([total, v]);
            } else {
                let sp = seg.speed_profile();
                let dt = 0.005;
                let (mut v, mut s, mut t) = (sp.v_start, 0.0_f32, 0.0_f32);
                let mut next_rec = 0.0_f32;
                let mut guard = 0;
                while s < len && guard < 400_000 {
                    if t >= next_rec {
                        out.push([total + t, v]);
                        next_rec = t + 0.05;
                    }
                    let r = kin_step(v, s, len, &sp, dt);
                    v = r.0; s = r.1; t += dt; guard += 1;
                }
                total += t;
                out.push([total, v]);
            }
            out.push([total, 0.0]);
        }

        // Align: a fine segmento si ferma e ruota sul posto (prima dell'azione)
        if let Some(target) = opts.align {
            let d = heading_delta(opts.robot_heading_end, target).abs();
            if d > 1.0 {
                out.push([total, 0.0]);
                total += d / ROTATION_SPEED_DEG_S;
                out.push([total, 0.0]);
            }
        }

        // Azioni puntuali sul nodo di arrivo: sosta fino alla fine dell'ultima azione
        let hold = node_hold_time(node_actions, seg_i + 1);
        if hold > 0.0 {
            out.push([total, 0.0]);
            total += hold;
            out.push([total, 0.0]);
        }

        match opts.action {
            RobotAction::Wait(x) => {
                out.push([total, 0.0]);
                total += x;
                out.push([total, 0.0]);
            }
            _ => {}
        }
        if let Segment::Search { search, .. } = seg {
            if search.shoot_after {
                out.push([total, 0.0]);
                total += search.shoot_secs;
                out.push([total, 0.0]);
            }
        }
        prev = seg.end();
        prev_heading_end = Some(end_heading(seg));
    }
    out
}

/// Tempo totale stimato (ultimo istante di `speed_trace`).
pub fn estimate_total_time(start: [f32; 2], segments: &[Segment], node_actions: &[NodeAction]) -> f32 {
    speed_trace(start, segments, node_actions).last().map(|p| p[0]).unwrap_or(0.0)
}

// ════════════════════════════════════════════════════════════
// Simulazione
// ════════════════════════════════════════════════════════════

impl Simulation {
    pub fn start(&mut self, segments: &[Segment]) {
        self.state     = SimState::Running;
        self.elapsed   = 0.0;
        self.zone_hit  = false;
        self.seg_index = 0;
        self.limit     = MATCH_DURATION;
        self.begin_segment(segments);
    }

    pub fn stop(&mut self) {
        self.state = SimState::Idle;
    }

    pub fn is_running(&self) -> bool {
        !matches!(self.state, SimState::Idle | SimState::Finished)
    }

    /// Il robot sta sparando in questo momento (sparo finale della ricerca o azione puntuale su un nodo).
    pub fn is_shooting(&self) -> bool {
        matches!(self.state, SimState::ShootStop { .. }) || self.node_hold_active(Mechanism::Shoot)
    }

    /// Durante la sosta su un nodo: l'azione `kind` è attiva adesso?
    /// (ritardo ≤ tempo dall'arrivo < ritardo + durata)
    pub fn node_hold_active(&self, kind: Mechanism) -> bool {
        if !matches!(self.state, SimState::NodeHold { .. }) { return false; }
        let node = self.seg_index + 1;
        self.node_actions.iter().any(|a| {
            a.node == node && a.kind == kind
                && self.hold_time >= a.delay && self.hold_time < a.delay + a.secs
        })
    }

    pub fn is_rotating(&self) -> bool {
        matches!(self.state, SimState::Rotating { .. })
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self.state, SimState::Waiting { .. })
    }

    fn begin_segment(&mut self, segments: &[Segment]) {
        self.s = 0.0;
        self.t = 0.0;
        self.seg_time = 0.0;
        self.v = segments.get(self.seg_index).map(|s| s.opts().speed.v_start).unwrap_or(0.0);
    }

    /// Pose (posizione, heading) corrente, usata quando il tempo scade.
    fn current_pose(&self, start: [f32; 2], segments: &[Segment]) -> ([f32; 2], f32) {
        let i = self.seg_index.min(segments.len() - 1);
        let from = prev_point(start, segments, i);
        let pts = segments[i].sample(from, SAMPLES);
        let cum = cumulative(&pts);
        let total = *cum.last().unwrap_or(&0.0);
        let pos = point_at_distance(&pts, &cum, self.t * total);
        let o = segments[i].opts();
        let heading = match &self.state {
            SimState::Rotating { current, .. } | SimState::Aligning { current, .. } => *current,
            // da fermo a fine segmento: l'heading è quello finale (anche dopo un Align)
            SimState::Waiting { .. } | SimState::ShootStop { .. }
            | SimState::NodeHold { .. } => end_heading(&segments[i]),
            _ => interp_heading(o.robot_heading_start, o.robot_heading_end, self.t),
        };
        (pos, heading)
    }

    /// Pose lungo il tragitto verso il parcheggio (solo in stato Parking): segue `park_route`.
    fn park_pose(&self) -> Option<([f32; 2], f32)> {
        let park = self.park.as_ref()?;
        if self.park_route.len() < 2 { return None; }
        let cum = cumulative(&self.park_route);
        let dist = *cum.last().unwrap_or(&0.0);
        let f = if dist > 0.001 { (self.s / dist).clamp(0.0, 1.0) } else { 1.0 };
        Some((
            point_at_distance(&self.park_route, &cum, self.s),
            interp_heading(self.park_from_heading, park.heading, f),
        ))
    }

    /// Pose da fermo: fine del segmento corrente.
    fn hold_pose(&self, segments: &[Segment], shooting: bool) -> ([f32; 2], f32, bool, bool) {
        let i = self.seg_index.min(segments.len() - 1);
        // Heading finale reale (se c'è un Align è quello di arrivo dell'Align)
        (segments[i].end(), end_heading(&segments[i]), false, shooting)
    }

    pub fn advance(
        &mut self,
        dt: f32,
        start: [f32; 2],
        segments: &[Segment],
    ) -> Option<([f32; 2], f32, bool, bool)> {
        if segments.is_empty() { self.state = SimState::Finished; return None; }

        self.elapsed += dt;

        // Timer scaduto: tutto si ferma dove sta
        if self.elapsed >= self.limit {
            let (pos, heading) = match (self.state == SimState::Parking, self.park_pose()) {
                (true, Some(p)) => p,
                _ => self.current_pose(start, segments),
            };
            self.state = SimState::Finished;
            return Some((pos, heading, false, false));
        }

        // ── Failsafe: negli ultimi secondi lascia perdere tutto e va al parcheggio ──
        if self.state != SimState::Parking {
            if let Some(park) = self.park.clone() {
                if park.enabled && self.elapsed >= self.limit - park.trigger_s {
                    let (pos, heading) = self.current_pose(start, segments);
                    self.park_route = crate::forbidden::route_avoiding(pos, park.pos);
                    self.park_from_heading = heading;
                    self.s = 0.0;
                    self.state = SimState::Parking;
                }
            }
        }
        if self.state == SimState::Parking {
            if let Some(park) = self.park.clone() {
                let dist = *cumulative(&self.park_route).last().unwrap_or(&0.0);
                self.s += park.v.max(1.0) * dt;
                if self.s >= dist {
                    self.state = SimState::Finished;
                    return Some((park.pos, park.heading, false, false));
                }
                if let Some((pos, heading)) = self.park_pose() {
                    return Some((pos, heading, false, false));
                }
            }
            self.state = SimState::Finished;
            return None;
        }

        // ── Rotating: ruota sul posto tra segmenti ────────────
        if let SimState::Rotating { to, current, speed, .. } = &mut self.state {
            let delta = heading_delta(*current, *to);
            let step = speed.abs() * dt;
            let pos = if self.seg_index == 0 { start }
                      else { segments[self.seg_index.saturating_sub(1)].end() };
            if delta.abs() <= step {
                let final_h = *to;
                self.state = SimState::Running;
                return Some((pos, final_h, false, false));
            }
            *current = (*current + step * delta.signum()).rem_euclid(360.0);
            return Some((pos, *current, false, false));
        }

        // ── Aligning: a fine segmento gira su se stesso ──────
        if let SimState::Aligning { to, current, speed } = &mut self.state {
            let delta = heading_delta(*current, *to);
            let step = speed.abs() * dt;
            let pos = segments[self.seg_index.min(segments.len() - 1)].end();
            if delta.abs() <= step {
                let final_h = *to;
                self.after_align(segments); // finito l'align: eventuale sparo puntuale, poi l'azione
                return Some((pos, final_h, false, false));
            }
            *current = (*current + step * delta.signum()).rem_euclid(360.0);
            return Some((pos, *current, false, false));
        }

        // ── NodeHold: fermo su un nodo, azioni puntuali con ritardo/durata ──
        if let SimState::NodeHold { remaining } = &mut self.state {
            *remaining -= dt;
            let done = *remaining <= 0.0;
            self.hold_time += dt;
            // intake / flywheel / sparo li accende l'app con `node_hold_active`
            let out = self.hold_pose(segments, false);
            if done { self.run_action(segments); } // poi l'azione del segmento (wait / avanti)
            return Some(out);
        }

        // ── Wait: fermo senza sparare ─────────────────────────
        if let SimState::Waiting { remaining } = &mut self.state {
            *remaining -= dt;
            let done = *remaining <= 0.0;
            let out = self.hold_pose(segments, false);
            if done { self.advance_to_next(segments); }
            return Some(out);
        }

        // ── ShootStop: fermo, spara ───────────────────────────
        if let SimState::ShootStop { remaining } = &mut self.state {
            *remaining -= dt;
            let done = *remaining <= 0.0;
            let out = self.hold_pose(segments, true);
            if done { self.advance_to_next(segments); }
            return Some(out);
        }

        if self.seg_index >= segments.len() { self.state = SimState::Finished; return None; }

        // ── Movimento lungo il segmento corrente ──────────────
        let seg  = &segments[self.seg_index];
        let opts = seg.opts();
        let from = prev_point(start, segments, self.seg_index);
        let pts  = seg.sample(from, SAMPLES);
        let cum  = cumulative(&pts);
        let seg_len = *cum.last().unwrap_or(&0.0);

        // Ricerca: velocità costante (None per Line/Curve)
        let search_v: Option<f32> = match seg {
            Segment::Search { search, .. } => Some(search.v.max(1.0)),
            _ => None,
        };
        let is_search = search_v.is_some();
        self.state = if is_search { SimState::Searching } else { SimState::Running };

        if seg_len < 0.001 {
            self.finish_segment(segments);
            return self.advance(0.0, start, segments);
        }

        let sp = seg.speed_profile();
        let (nv, ns) = match search_v {
            Some(v) => (v, self.s + v * dt),
            None    => kin_step(self.v, self.s, seg_len, &sp, dt),
        };
        self.seg_time += dt;
        self.v = nv;
        self.s = ns;

        if self.s >= seg_len {
            let overshoot = ((self.s - seg_len) / self.v.max(1.0)).min(dt);
            self.s = seg_len;
            self.t = 1.0;
            let pos = *pts.last().unwrap();
            let heading = opts.robot_heading_end;
            let intake = is_search;
            let flywheel = false;
            self.finish_segment(segments);
            if self.state == SimState::Running {
                // Recupera il tempo avanzato nel segmento successivo
                self.elapsed -= overshoot;
                return self.advance(overshoot, start, segments);
            }
            return Some((pos, heading, intake, flywheel));
        }

        self.t = (self.s / seg_len).clamp(0.0, 1.0);
        let pos = point_at_distance(&pts, &cum, self.s);
        let heading = interp_heading(opts.robot_heading_start, opts.robot_heading_end, self.t);
        Some((pos, heading, is_search, false))
    }

    /// Il robot è arrivato in fondo al segmento: si ferma sul posto.
    fn finish_segment(&mut self, segments: &[Segment]) {
        // Align: prima si ferma e gira sul posto, poi esegue l'azione del segmento
        let opts = segments[self.seg_index].opts();
        if let Some(target) = opts.align {
            let from = opts.robot_heading_end;
            if heading_delta(from, target).abs() > 1.0 {
                self.state = SimState::Aligning {
                    to: target, current: from, speed: ROTATION_SPEED_DEG_S,
                };
                return;
            }
        }
        self.after_align(segments);
    }

    /// Dopo l'eventuale align: se sul nodo ci sono azioni puntuali (sparo / intake / flywheel
    /// con ritardo e durata) si ferma per il tempo necessario, poi esegue l'azione del segmento.
    fn after_align(&mut self, segments: &[Segment]) {
        let node = self.seg_index + 1;
        let hold = node_hold_time(&self.node_actions, node);
        if hold > 0.0 {
            self.hold_time = 0.0;
            self.state = SimState::NodeHold { remaining: hold };
        } else {
            self.run_action(segments);
        }
    }

    /// Azione di fine segmento (dopo l'eventuale align).
    fn run_action(&mut self, segments: &[Segment]) {
        // Ricerca: all'arrivo si ferma; se richiesto spara fermo, poi riparte
        if let Segment::Search { search, .. } = &segments[self.seg_index] {
            if search.shoot_after {
                self.state = SimState::ShootStop { remaining: search.shoot_secs };
            } else {
                self.advance_to_next(segments);
            }
            return;
        }
        let opts = segments[self.seg_index].opts();
        match &opts.action {
            RobotAction::Wait(secs)      => self.state = SimState::Waiting { remaining: *secs },
            RobotAction::Move => self.advance_to_next(segments),
        }
    }

    fn advance_to_next(&mut self, segments: &[Segment]) {
        let prev_idx = self.seg_index;
        self.seg_index += 1;
        if self.seg_index >= segments.len() {
            self.state = SimState::Finished;
            return;
        }
        self.begin_segment(segments);

        let prev_end   = end_heading(&segments[prev_idx]);
        let next_start = segments[self.seg_index].opts().robot_heading_start;
        if heading_delta(prev_end, next_start).abs() > 1.0 {
            let speed = ROTATION_SPEED_DEG_S;
            self.state = SimState::Rotating {
                from: prev_end, to: next_start, current: prev_end, speed,
            };
        } else {
            self.state = SimState::Running;
        }
    }
}

// ════════════════════════════════════════════════════════════
// Geometria lungo la polilinea (parametrizzata per lunghezza d'arco)
// ════════════════════════════════════════════════════════════

fn prev_point(start: [f32; 2], segments: &[Segment], i: usize) -> [f32; 2] {
    if i == 0 { start } else { segments[i - 1].end() }
}

/// Lunghezze cumulative lungo la polilinea (cum[0] = 0).
pub fn cumulative(pts: &[[f32; 2]]) -> Vec<f32> {
    let mut cum = Vec::with_capacity(pts.len().max(1));
    let mut acc = 0.0_f32;
    cum.push(0.0);
    for w in pts.windows(2) {
        let dx = w[1][0] - w[0][0];
        let dy = w[1][1] - w[0][1];
        acc += (dx * dx + dy * dy).sqrt();
        cum.push(acc);
    }
    cum
}

/// Frazione di lunghezza d'arco [0,1] di ogni campione.
pub fn fractions(pts: &[[f32; 2]]) -> Vec<f32> {
    let cum = cumulative(pts);
    let total = *cum.last().unwrap_or(&0.0);
    cum.iter().map(|c| if total > 0.0 { c / total } else { 0.0 }).collect()
}

fn locate(cum: &[f32], d: f32) -> (usize, f32) {
    let total = *cum.last().unwrap_or(&0.0);
    let d = d.clamp(0.0, total);
    let i = cum.partition_point(|&c| c < d).clamp(1, cum.len() - 1);
    let c0 = cum[i - 1];
    let c1 = cum[i];
    let f = if c1 - c0 > 1e-6 { (d - c0) / (c1 - c0) } else { 0.0 };
    (i, f)
}

/// Punto a distanza `d` (cm) dall'inizio, interpolato linearmente.
pub fn point_at_distance(pts: &[[f32; 2]], cum: &[f32], d: f32) -> [f32; 2] {
    if pts.len() < 2 || cum.len() < 2 { return pts.first().copied().unwrap_or([0.0, 0.0]); }
    let (i, f) = locate(cum, d);
    [
        pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * f,
        pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * f,
    ]
}

/// Punto alla frazione `frac` ∈ [0,1] della lunghezza totale.
pub fn point_at_fraction(pts: &[[f32; 2]], frac: f32) -> [f32; 2] {
    let cum = cumulative(pts);
    let total = *cum.last().unwrap_or(&0.0);
    point_at_distance(pts, &cum, frac.clamp(0.0, 1.0) * total)
}

/// Direzione di marcia (gradi, 0 = destra, 90 = su) alla frazione `frac`.
pub fn tangent_at_fraction(pts: &[[f32; 2]], frac: f32) -> f32 {
    let cum = cumulative(pts);
    if pts.len() < 2 || cum.len() < 2 { return 0.0; }
    let total = *cum.last().unwrap_or(&0.0);
    let (i, _) = locate(&cum, frac.clamp(0.0, 1.0) * total);
    let dx = pts[i][0] - pts[i - 1][0];
    let dy = pts[i][1] - pts[i - 1][1];
    dy.atan2(dx).to_degrees().rem_euclid(360.0)
}
