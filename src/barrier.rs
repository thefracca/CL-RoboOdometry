/// Barriere: zone non accessibili al robot.

#[derive(Clone, Debug)]
pub struct Barrier {
    pub vertices: Vec<[f32; 2]>,
    pub closed: bool,
}

impl Barrier {
    pub fn new() -> Self {
        Self { vertices: Vec::new(), closed: false }
    }

    pub fn centroid(&self) -> [f32; 2] {
        if self.vertices.is_empty() { return [0.0, 0.0]; }
        let n = self.vertices.len() as f32;
        let sx: f32 = self.vertices.iter().map(|v| v[0]).sum();
        let sy: f32 = self.vertices.iter().map(|v| v[1]).sum();
        [sx / n, sy / n]
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        for v in &mut self.vertices {
            v[0] += dx;
            v[1] += dy;
        }
    }

    /// Restituisce tutti i segmenti del poligono chiuso come coppie di vertici.
    pub fn edges(&self) -> Vec<([f32; 2], [f32; 2])> {
        if !self.closed || self.vertices.len() < 2 { return vec![]; }
        let n = self.vertices.len();
        (0..n).map(|i| (self.vertices[i], self.vertices[(i + 1) % n])).collect()
    }
}

// ─── Geometria ────────────────────────────────────────────────

/// Distanza dal punto `p` al segmento `(a, b)`.
pub fn dist_point_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 == 0.0 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0);
    let closest = [a[0] + t * ab[0], a[1] + t * ab[1]];
    ((p[0] - closest[0]).powi(2) + (p[1] - closest[1]).powi(2)).sqrt()
}

/// Distanza minima dal punto `p` a qualsiasi lato di qualsiasi barriera chiusa.
pub fn min_dist_to_barriers(p: [f32; 2], barriers: &[Barrier]) -> f32 {
    barriers.iter()
        .flat_map(|b| b.edges())
        .map(|(a, b)| dist_point_to_segment(p, a, b))
        .fold(f32::INFINITY, f32::min)
}

/// Sposta `p` verso l'esterno finché la distanza minima da tutte le barriere è >= `min_dist`.
/// Usa un approccio iterativo: trova il lato più vicino e sposta lungo la normale.
pub fn push_out_of_barriers(mut p: [f32; 2], barriers: &[Barrier], min_dist: f32) -> [f32; 2] {
    for _ in 0..20 {
        // Trova il lato più vicino
        let closest_edge = barriers.iter()
            .flat_map(|b| b.edges())
            .min_by(|(a1, b1), (a2, b2)| {
                dist_point_to_segment(p, *a1, *b1)
                    .partial_cmp(&dist_point_to_segment(p, *a2, *b2))
                    .unwrap()
            });

        if let Some((a, b)) = closest_edge {
            let d = dist_point_to_segment(p, a, b);
            if d >= min_dist { break; }

            // Normale al segmento (perpendicolare, verso l'esterno)
            let ab = [b[0] - a[0], b[1] - a[1]];
            let len = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
            if len == 0.0 { break; }
            // Normale: ruota ab di 90°
            let normal = [-ab[1] / len, ab[0] / len];

            // Decidi il verso corretto (allontanati)
            let push = min_dist - d + 0.5;
            // Prova entrambi i versi e scegli quello che aumenta la distanza
            let p1 = [p[0] + normal[0] * push, p[1] + normal[1] * push];
            let p2 = [p[0] - normal[0] * push, p[1] - normal[1] * push];
            let d1 = dist_point_to_segment(p1, a, b);
            let d2 = dist_point_to_segment(p2, a, b);
            p = if d1 >= d2 { p1 } else { p2 };
        } else {
            break;
        }
    }
    p
}
