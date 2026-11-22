//! Zone del campo PERMANENTEMENTE vietate al robot (POLIGONI).
//!
//! Le disegni col tool "⛔ Zone vietate" (tasto 9) e si salvano da sole in `zone_vietate.json`
//! (cartella di lavoro): vengono ricaricate ad ogni avvio e valgono per qualsiasi percorso
//! (non fanno parte del progetto). Il robot (45×45 cm, con la sua larghezza) non deve MAI
//! toccarle: nemmeno a 2 cm di distanza. Effetti:
//!  - nodi / start / parcheggio non si possono piazzare troppo vicino (vengono spinti fuori);
//!  - se il corpo del robot tocca (o sta a ≤ 2 cm da) una zona, in simulazione il robot diventa
//!    ROSSO e compare un avviso; i segmenti dove succede sono rossi;
//!  - il tragitto di emergenza verso il parcheggio le aggira (vedi `route_avoiding`).
//!
//! Una zona è un poligono semplice (convesso o concavo) dato dalla lista dei suoi vertici in cm.
//! Vivono in uno stato globale, così le funzioni di controllo si possono chiamare da qualsiasi
//! punto del programma.

use std::f32::consts::{PI, TAU};
use std::sync::RwLock;

use crate::barrier::dist_point_to_segment;
use crate::field::FIELD_CM;

/// Lato del robot (cm). Deve coincidere con `ROBOT_SIZE_CM` in `field_canvas.rs`.
pub const ROBOT_SIZE_CM: f32 = 45.0;
/// Distanza di sicurezza: a 2 cm o meno dalla zona il robot è già considerato "sopra".
pub const CLEARANCE_CM: f32 = 2.0;

const ROBOT_HALF_CM: f32 = ROBOT_SIZE_CM / 2.0;
const ROBOT_HALF_DIAG_CM: f32 = ROBOT_HALF_CM * 1.414_213_5;
/// Il centro del robot non può avvicinarsi ai bordi del campo più di così (cm).
const FIELD_MARGIN: f32 = 23.5;

/// Distanza minima del CENTRO del robot dal poligono, sotto la quale è impossibile
/// non toccarlo (qualunque sia l'heading): mezzo lato + sicurezza.
pub const KEEP_OUT_CM: f32 = ROBOT_HALF_CM + CLEARANCE_CM;

/// File in cui le zone si salvano da sole (cartella di lavoro).
const FILE: &str = "zone_vietate.json";

#[derive(Clone, Debug)]
pub struct ForbiddenZone {
    /// Vertici del poligono (cm), in ordine (orario o antiorario, indifferente). Minimo 3.
    pub vertices: Vec<[f32; 2]>,
}

impl ForbiddenZone {
    /// Poligono regolare a 16 lati che contiene il cerchio (centro, raggio):
    /// serve per convertire le vecchie zone circolari salvate in `zone_vietate.json`.
    pub fn from_circle(center: [f32; 2], radius: f32) -> Self {
        const N: usize = 16;
        let r = radius.max(1.0) / (PI / N as f32).cos();
        let vertices = (0..N).map(|i| {
            let a = TAU * i as f32 / N as f32;
            [center[0] + r * a.cos(), center[1] + r * a.sin()]
        }).collect();
        Self { vertices }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        for v in &mut self.vertices {
            v[0] += dx;
            v[1] += dy;
        }
    }

    /// Rettangolo che contiene la zona: (minimo, massimo).
    pub fn bbox(&self) -> ([f32; 2], [f32; 2]) {
        if self.vertices.is_empty() { return ([0.0; 2], [0.0; 2]); }
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        for v in &self.vertices {
            lo[0] = lo[0].min(v[0]);
            lo[1] = lo[1].min(v[1]);
            hi[0] = hi[0].max(v[0]);
            hi[1] = hi[1].max(v[1]);
        }
        (lo, hi)
    }

    /// Area (cm²), sempre positiva.
    pub fn area(&self) -> f32 {
        signed_area(&self.vertices).abs()
    }

    /// Baricentro dell'area (se il poligono è degenere: media dei vertici).
    pub fn centroid(&self) -> [f32; 2] {
        let v = &self.vertices;
        let n = v.len();
        if n == 0 { return [0.0, 0.0]; }
        let (mut a2, mut cx, mut cy) = (0.0_f32, 0.0_f32, 0.0_f32);
        for i in 0..n {
            let p = v[i];
            let q = v[(i + 1) % n];
            let w = p[0] * q[1] - q[0] * p[1];
            a2 += w;
            cx += (p[0] + q[0]) * w;
            cy += (p[1] + q[1]) * w;
        }
        if a2.abs() < 1e-3 {
            let sx: f32 = v.iter().map(|p| p[0]).sum();
            let sy: f32 = v.iter().map(|p| p[1]).sum();
            return [sx / n as f32, sy / n as f32];
        }
        [cx / (3.0 * a2), cy / (3.0 * a2)]
    }

    /// Il punto è dentro il poligono? (ray casting)
    pub fn contains(&self, p: [f32; 2]) -> bool {
        let v = &self.vertices;
        let n = v.len();
        if n < 3 { return false; }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (a, b) = (v[i], v[j]);
            if (a[1] > p[1]) != (b[1] > p[1])
                && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Punto del bordo più vicino a `p` e relativa distanza.
    pub fn nearest_boundary(&self, p: [f32; 2]) -> ([f32; 2], f32) {
        let v = &self.vertices;
        let n = v.len();
        let mut best = (p, f32::INFINITY);
        for i in 0..n {
            let q = closest_on_segment(p, v[i], v[(i + 1) % n]);
            let d = dist(p, q);
            if d < best.1 { best = (q, d); }
        }
        best
    }

    /// Distanza di `p` dalla zona (0 se è dentro).
    pub fn dist_to(&self, p: [f32; 2]) -> f32 {
        if self.vertices.len() < 3 { return f32::INFINITY; }
        if self.contains(p) { return 0.0; }
        self.nearest_boundary(p).1
    }
}

static ZONES: RwLock<Vec<ForbiddenZone>> = RwLock::new(Vec::new());

// ── Stato e salvataggio ──────────────────────────────────────

/// Copia delle zone attuali.
pub fn zones() -> Vec<ForbiddenZone> {
    ZONES.read().map(|z| z.clone()).unwrap_or_default()
}

/// Sostituisce le zone (non salva: chiama `save()` dopo).
pub fn set_zones(v: Vec<ForbiddenZone>) {
    if let Ok(mut z) = ZONES.write() {
        *z = v;
    }
}

/// Salva le zone in `zone_vietate.json`.
/// Formato: {"zones":[{"v":[[x,y],[x,y],...]}, ...]}
pub fn save() -> Result<(), String> {
    let list: Vec<String> = zones().iter()
        .map(|z| {
            let pts: Vec<String> = z.vertices.iter()
                .map(|p| format!("[{:.2},{:.2}]", p[0], p[1]))
                .collect();
            format!(r#"    {{"v":[{}]}}"#, pts.join(","))
        })
        .collect();
    let json = format!("{{\n  \"zones\":[\n{}\n  ]\n}}\n", list.join(",\n"));
    std::fs::write(FILE, json).map_err(|e| e.to_string())
}

/// Carica le zone da `zone_vietate.json` (se il file non c'è: nessuna zona).
/// Compatibile con il vecchio formato a cerchi {"x","y","r"}: diventano poligoni a 16 lati.
pub fn load() {
    let Ok(text) = std::fs::read_to_string(FILE) else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let mut out = Vec::new();
    if let Some(arr) = v["zones"].as_array() {
        for z in arr {
            if let Some(pts) = z["v"].as_array() {
                let vertices: Vec<[f32; 2]> = pts.iter()
                    .filter_map(|p| Some([p[0].as_f64()? as f32, p[1].as_f64()? as f32]))
                    .collect();
                if vertices.len() >= 3 {
                    out.push(ForbiddenZone { vertices });
                }
            } else if let (Some(x), Some(y), Some(r)) =
                (z["x"].as_f64(), z["y"].as_f64(), z["r"].as_f64())
            {
                out.push(ForbiddenZone::from_circle([x as f32, y as f32], r as f32));
            }
        }
    }
    set_zones(out);
}

// ── Geometria di base ────────────────────────────────────────

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn clamp_field(p: [f32; 2]) -> [f32; 2] {
    [p[0].clamp(FIELD_MARGIN, FIELD_CM - FIELD_MARGIN),
     p[1].clamp(FIELD_MARGIN, FIELD_CM - FIELD_MARGIN)]
}

/// Orientamento di (a, b, c): > 0 se antiorario.
fn orient(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Area con segno (> 0 se i vertici sono in senso antiorario).
fn signed_area(v: &[[f32; 2]]) -> f32 {
    let n = v.len();
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (v[i], v[(i + 1) % n]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

fn closest_on_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1e-9 { return a; }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    [a[0] + t * ab[0], a[1] + t * ab[1]]
}

/// I segmenti ab e cd si incrociano (in modo proprio)?
fn segments_cross(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let o1 = orient(a, b, c);
    let o2 = orient(a, b, d);
    let o3 = orient(c, d, a);
    let o4 = orient(c, d, b);
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

/// Distanza minima tra due segmenti.
fn seg_seg_dist(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> f32 {
    if segments_cross(a, b, c, d) { return 0.0; }
    dist_point_to_segment(a, c, d)
        .min(dist_point_to_segment(b, c, d))
        .min(dist_point_to_segment(c, a, b))
        .min(dist_point_to_segment(d, a, b))
}

/// Distanza minima tra il segmento (a, b) e la zona (0 se la tocca o ci entra).
fn seg_zone_dist(a: [f32; 2], b: [f32; 2], z: &ForbiddenZone) -> f32 {
    let v = &z.vertices;
    let n = v.len();
    if n < 3 { return f32::INFINITY; }
    if z.contains(a) || z.contains(b) { return 0.0; }
    (0..n).map(|i| seg_seg_dist(a, b, v[i], v[(i + 1) % n])).fold(f32::INFINITY, f32::min)
}

// ── Controlli di collisione ──────────────────────────────────

/// Il centro del robot in `p` toccherebbe di sicuro una zona (con qualsiasi heading)?
pub fn in_zone(p: [f32; 2]) -> bool {
    zones().iter().any(|z| z.dist_to(p) < KEEP_OUT_CM)
}

/// Se il punto è troppo vicino a una zona (o dentro) lo sposta sul limite, verso l'esterno
/// (restando nel campo).
pub fn push_out(mut p: [f32; 2]) -> [f32; 2] {
    let zs = zones();
    for _ in 0..6 {
        let mut moved = false;
        for z in zs.iter() {
            if z.vertices.len() < 3 { continue; }
            let inside = z.contains(p);
            let (q, d) = z.nearest_boundary(p);
            if !inside && d >= KEEP_OUT_CM { continue; }
            // direzione verso l'esterno
            let (mut ux, mut uy) = if d > 1e-3 {
                if inside { ((q[0] - p[0]) / d, (q[1] - p[1]) / d) }
                else      { ((p[0] - q[0]) / d, (p[1] - q[1]) / d) }
            } else {
                // esattamente sul bordo: allontanati dal baricentro
                let c = z.centroid();
                let dc = dist(q, c);
                if dc > 1e-3 { ((q[0] - c[0]) / dc, (q[1] - c[1]) / dc) } else { (1.0, 0.0) }
            };
            let l = (ux * ux + uy * uy).sqrt();
            if l > 1e-6 { ux /= l; uy /= l; }
            p = clamp_field([
                q[0] + ux * (KEEP_OUT_CM + 0.5),
                q[1] + uy * (KEEP_OUT_CM + 0.5),
            ]);
            moved = true;
        }
        if !moved { break; }
    }
    p
}

/// Angoli del quadrato del robot (45×45 cm) centrato in `pos`, ruotato di `heading_deg`.
fn robot_corners(pos: [f32; 2], heading_deg: f32) -> [[f32; 2]; 4] {
    let (s, c) = heading_deg.to_radians().sin_cos();
    let h = ROBOT_HALF_CM;
    let f = [c * h, s * h];   // avanti
    let l = [-s * h, c * h];  // sinistra
    [
        [pos[0] + f[0] + l[0], pos[1] + f[1] + l[1]],
        [pos[0] - f[0] + l[0], pos[1] - f[1] + l[1]],
        [pos[0] - f[0] - l[0], pos[1] - f[1] - l[1]],
        [pos[0] + f[0] - l[0], pos[1] + f[1] - l[1]],
    ]
}

/// Il punto è dentro (o sul bordo) del quadrato del robot?
fn in_robot_square(p: [f32; 2], pos: [f32; 2], s: f32, c: f32) -> bool {
    let vx = p[0] - pos[0];
    let vy = p[1] - pos[1];
    let lx = vx * c + vy * s;
    let ly = -vx * s + vy * c;
    lx.abs() <= ROBOT_HALF_CM && ly.abs() <= ROBOT_HALF_CM
}

fn robot_hits_in(zs: &[ForbiddenZone], pos: [f32; 2], heading_deg: f32) -> bool {
    if zs.is_empty() { return false; }
    let (s, c) = heading_deg.to_radians().sin_cos();
    let corners = robot_corners(pos, heading_deg);
    let reach = ROBOT_HALF_DIAG_CM + CLEARANCE_CM;
    zs.iter().any(|z| {
        let v = &z.vertices;
        let n = v.len();
        if n < 3 { return false; }
        // scarto rapido: il robot è troppo lontano dal rettangolo che contiene la zona
        let (lo, hi) = z.bbox();
        if pos[0] < lo[0] - reach || pos[0] > hi[0] + reach
            || pos[1] < lo[1] - reach || pos[1] > hi[1] + reach { return false; }
        // robot dentro la zona
        if corners.iter().any(|k| z.contains(*k)) { return true; }
        // zona (o un suo vertice) dentro il robot
        if v.iter().any(|p| in_robot_square(*p, pos, s, c)) { return true; }
        // bordi a ≤ 2 cm l'uno dall'altro
        for i in 0..n {
            let (a, b) = (v[i], v[(i + 1) % n]);
            for k in 0..4 {
                if seg_seg_dist(a, b, corners[k], corners[(k + 1) % 4]) <= CLEARANCE_CM {
                    return true;
                }
            }
        }
        false
    })
}

/// Il CORPO del robot (quadrato 45×45 cm ruotato di `heading_deg`, centrato in `pos`)
/// tocca una zona o sta a ≤ 2 cm da essa?
pub fn robot_hits(pos: [f32; 2], heading_deg: f32) -> bool {
    robot_hits_in(&zones(), pos, heading_deg)
}

/// Un segmento campionato (`pts`), percorso col robot che ruota da `h0` a `h1`
/// (heading iniziale/finale del segmento), fa toccare una zona al corpo del robot?
pub fn path_hits(pts: &[[f32; 2]], h0: f32, h1: f32) -> bool {
    let n = pts.len();
    let zs = zones();
    if n == 0 || zs.is_empty() { return false; }
    let mut d = (h1 - h0).rem_euclid(360.0);
    if d > 180.0 { d -= 360.0; }
    pts.iter().enumerate().any(|(i, p)| {
        let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
        robot_hits_in(&zs, *p, h0 + d * t)
    })
}

/// Tragitto da `from` a `to` che evita le zone vietate: lista di punti (parte da `from`,
/// finisce in `to`). Se la retta diretta non taglia nessuna zona (con il robot di qualsiasi
/// heading) è semplicemente `[from, to]`; altrimenti inserisce un punto di aggiramento oltre
/// il poligono, sul lato della retta rispetto al baricentro della zona.
pub fn route_avoiding(from: [f32; 2], to: [f32; 2]) -> Vec<[f32; 2]> {
    let zs = zones();
    // distanza dal bordo oltre la quale il robot è al sicuro con QUALSIASI heading (semi-diagonale)
    let safe = ROBOT_HALF_DIAG_CM + CLEARANCE_CM;
    let mut route = vec![from, to];
    for _ in 0..6 {
        let mut inserted = false;
        for i in 0..route.len() - 1 {
            let (a, b) = (route[i], route[i + 1]);
            // zona più vicina ad `a` tra quelle tagliate da questa tratta
            let mut hit: Option<(usize, f32)> = None;
            for (zi, z) in zs.iter().enumerate() {
                if z.vertices.len() < 3 { continue; }
                if seg_zone_dist(a, b, z) < safe {
                    let d = dist(a, z.centroid());
                    if hit.map_or(true, |h| d < h.1) { hit = Some((zi, d)); }
                }
            }
            if let Some((zi, _)) = hit {
                let z = &zs[zi];
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-3 { continue; }
                let dir = [dx / len, dy / len];
                // normale alla tratta; lato = da che parte del baricentro passa la retta
                let n = [-dir[1], dir[0]];
                let c = z.centroid();
                let side = (a[0] - c[0]) * n[0] + (a[1] - c[1]) * n[1];
                let sgn = if side >= 0.0 { 1.0 } else { -1.0 };
                // quanto il poligono sporge da questo lato della retta
                let extent = z.vertices.iter()
                    .map(|v| sgn * ((v[0] - a[0]) * n[0] + (v[1] - a[1]) * n[1]))
                    .fold(0.0_f32, f32::max);
                let along = ((c[0] - a[0]) * dir[0] + (c[1] - a[1]) * dir[1]).clamp(0.0, len);
                let off = sgn * (extent + safe * 1.2 + 3.0);
                let detour = clamp_field([
                    a[0] + dir[0] * along + n[0] * off,
                    a[1] + dir[1] * along + n[1] * off,
                ]);
                route.insert(i + 1, detour);
                inserted = true;
                break;
            }
        }
        if !inserted { break; }
    }
    route
}

// ── Disegno ──────────────────────────────────────────────────

/// Contorno (chiuso) del limite per il CENTRO del robot: il poligono "gonfiato" di
/// `KEEP_OUT_CM`, con gli angoli arrotondati. Solo grafico.
pub fn keep_out_outline(z: &ForbiddenZone) -> Vec<[f32; 2]> {
    let n = z.vertices.len();
    if n < 3 { return vec![]; }
    let mut v = z.vertices.clone();
    if signed_area(&v) < 0.0 { v.reverse(); } // ora antiorario
    let k = KEEP_OUT_CM;
    // normale uscente di un lato (poligono antiorario)
    let out_normal = |p: [f32; 2], q: [f32; 2]| -> [f32; 2] {
        let dx = q[0] - p[0];
        let dy = q[1] - p[1];
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-6 { [0.0, 0.0] } else { [dy / l, -dx / l] }
    };
    let mut out = Vec::new();
    for i in 0..n {
        let p0 = v[(i + n - 1) % n];
        let p1 = v[i];
        let p2 = v[(i + 1) % n];
        let n0 = out_normal(p0, p1);
        let n1 = out_normal(p1, p2);
        let rot = n0[0] * n1[1] - n0[1] * n1[0];
        if rot > 1e-4 {
            // vertice convesso: arco di raggio k da n0 a n1
            let a0 = n0[1].atan2(n0[0]);
            let mut da = n1[1].atan2(n1[0]) - a0;
            while da < 0.0 { da += TAU; }
            let steps = ((da / (PI / 12.0)).ceil() as usize).max(1);
            for s in 0..=steps {
                let a = a0 + da * s as f32 / steps as f32;
                out.push([p1[0] + k * a.cos(), p1[1] + k * a.sin()]);
            }
        } else {
            // vertice riflesso (o dritto): punto d'incontro dei due lati spostati
            let m = [n0[0] + n1[0], n0[1] + n1[1]];
            let ml = (m[0] * m[0] + m[1] * m[1]).sqrt();
            if ml < 1e-3 {
                out.push([p1[0] + n0[0] * k, p1[1] + n0[1] * k]);
            } else {
                let s = k / (ml / 2.0).max(0.2);
                out.push([p1[0] + m[0] / ml * s, p1[1] + m[1] / ml * s]);
            }
        }
    }
    out
}

/// Divide il poligono in triangoli (ear clipping) per poterlo riempire anche se è concavo.
/// Restituisce terne di indici nella lista dei vertici.
pub fn triangulate(v: &[[f32; 2]]) -> Vec<[usize; 3]> {
    let n = v.len();
    if n < 3 { return vec![]; }
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(v) < 0.0 { idx.reverse(); } // antiorario
    let in_tri = |p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        orient(a, b, p) >= 0.0 && orient(b, c, p) >= 0.0 && orient(c, a, p) >= 0.0
    };
    let mut tris = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let mut ear: Option<usize> = None;
        for k in 0..m {
            let (i0, i1, i2) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (v[i0], v[i1], v[i2]);
            if orient(a, b, c) <= 1e-6 { continue; } // vertice riflesso o degenere
            let blocked = idx.iter().any(|&j| {
                j != i0 && j != i1 && j != i2 && in_tri(v[j], a, b, c)
            });
            if !blocked { ear = Some(k); break; }
        }
        match ear {
            Some(k) => {
                tris.push([idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]]);
                idx.remove(k);
            }
            None => {
                // poligono degenere o che si interseca: riempimento a ventaglio di emergenza
                for k in 1..idx.len() - 1 {
                    tris.push([idx[0], idx[k], idx[k + 1]]);
                }
                return tris;
            }
        }
    }
    tris.push([idx[0], idx[1], idx[2]]);
    tris
}
