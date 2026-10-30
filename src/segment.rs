// Tipi di segmento del percorso.
// Un Line è un segmento dritto da un punto a un altro.
// Un Curve è una Bezier cubica: "controls" sono i punti di controllo
// intermedi che l'utente sposta per piegare la curva.

#[derive(Clone, Debug)]
pub enum Segment {
    Line {
        to: [f32; 2],
        heading: HeadingMode,
    },
    Curve {
        /// Punti di controllo Bezier: il primo e l'ultimo sono i "manici"
        /// che l'utente trascina perpendicolarmente alla retta.
        controls: Vec<[f32; 2]>,
        to: [f32; 2],
        heading: HeadingMode,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadingMode {
    Tangent,        // il robot punta nella direzione di moto
    Fixed(f32),     // angolo fisso in gradi (0 = destra, 90 = su)
}

impl Default for HeadingMode {
    fn default() -> Self { HeadingMode::Tangent }
}

impl Segment {
    /// Punto di arrivo del segmento.
    pub fn end(&self) -> [f32; 2] {
        match self {
            Segment::Line { to, .. } => *to,
            Segment::Curve { to, .. } => *to,
        }
    }

    /// Campiona la curva Bezier cubica in `n` punti per disegnarla.
    /// Per un Line restituisce solo start e end.
    pub fn sample(&self, from: [f32; 2], n: usize) -> Vec<[f32; 2]> {
        match self {
            Segment::Line { to, .. } => vec![from, *to],
            Segment::Curve { controls, to, .. } => {
                // Bezier cubica: p0=from, p1=controls[0], p2=controls[last], p3=to
                // Se c'è un solo control point lo usiamo sia come p1 che come p2.
                let p0 = from;
                let (p1, p2) = if controls.is_empty() {
                    (from, *to)
                } else if controls.len() == 1 {
                    (controls[0], controls[0])
                } else {
                    (controls[0], controls[controls.len() - 1])
                };
                let p3 = *to;

                (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        [
                            u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0],
                            u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1],
                        ]
                    })
                    .collect()
            }
        }
    }
}
