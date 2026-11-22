//! Salvataggio e caricamento del progetto in formato JSON.

use std::path::Path;
use crate::app::HudApp;
use crate::barrier::Barrier;
use crate::segment::*;

// ── Serializzazione ───────────────────────────────────────────

pub fn save(app: &HudApp, path: &Path) -> Result<(), String> {
    let json = serialize(app);
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load(app: &mut HudApp, path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    deserialize(app, &text)
}

fn serialize(app: &HudApp) -> String {
    let segs: Vec<String> = app.segments.iter().map(|seg| {
        let opts = seg.opts();
        let (type_str, to, controls) = match seg {
            Segment::Line { to, .. } =>
                ("line", *to, vec![]),
            Segment::Curve { to, controls, .. } =>
                ("curve", *to, controls.clone()),
            Segment::Search { to, .. } =>
                ("search", *to, vec![]),
        };
        let ctrls = controls.iter()
            .map(|c| format!("[{:.4},{:.4}]", c[0], c[1]))
            .collect::<Vec<_>>().join(",");
        let action = match &opts.action {
            RobotAction::Move             => "\"move\"".to_string(),
            RobotAction::Wait(s)          => format!("\"wait:{:.4}\"", s),
        };
        let al = match opts.align {
            Some(d) => format!(",\"al\":{:.4}", d),
            None    => String::new(),
        };
        // Parametri ricerca: [v, larghezza, time_limit, shoot_after, shoot_secs]
        let sr = match seg {
            Segment::Search { search, .. } => format!(
                ",\"sr\":[{:.4},{:.4},{:.4},{},{:.4}]",
                search.v, search.width, search.time_limit, search.shoot_after, search.shoot_secs
            ),
            _ => String::new(),
        };
        format!(
            r#"{{"t":"{type_str}","to":[{:.4},{:.4}],"ctrl":[{ctrls}],"act":{action},"spd":[{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}],"rhs":{:.4},"rhe":{:.4}{sr}{al}}}"#,
            to[0], to[1],
            opts.speed.v_start, opts.speed.v_max, opts.speed.accel,
            opts.speed.decel, opts.speed.brake_point, opts.speed.v_min,
            opts.robot_heading_start, opts.robot_heading_end,
        )
    }).collect();

    let barriers: Vec<String> = app.barriers.iter().map(|b| {
        let verts = b.vertices.iter()
            .map(|v| format!("[{:.4},{:.4}]", v[0], v[1]))
            .collect::<Vec<_>>().join(",");
        format!(r#"{{"closed":{},"v":[{verts}]}}"#, b.closed)
    }).collect();

    let focal: Vec<String> = app.focal_lines.iter().map(|f| {
        format!("[{:.4},{:.4},{:.4},{:.4},{:.4},{}]",
            f.a[0], f.a[1], f.b[0], f.b[1], f.snap_radius_cm, f.visible)
    }).collect();

    let hl: Vec<String> = app.highlights.iter().map(|h| {
        let k = match h.kind { Mechanism::Intake => 0, Mechanism::Flywheel => 1, Mechanism::IntakeDown => 2, Mechanism::Shoot => 3, Mechanism::BarDown => 4 };
        format!("[{},{:.4},{:.4},{:.4},{:.4}]", k, h.from, h.to, h.secs, h.delay)
    }).collect();

    format!(
        r#"{{"ver":2,"start":[{:.4},{:.4}],"rh":{:.4},"segs":[{}],"barriers":[{}],"focal":[{}],"hl":[{}],"park":[{:.4},{:.4},{:.4},{:.4},{:.4},{}]}}"#,
        app.start[0], app.start[1], app.robot_heading,
        segs.join(","),
        barriers.join(","),
        focal.join(","),
        hl.join(","),
        app.park.pos[0], app.park.pos[1], app.park.heading,
        app.park.trigger_s, app.park.v, app.park.enabled,
    )
}

fn deserialize(app: &mut HudApp, text: &str) -> Result<(), String> {
    // Parser minimalista senza dipendenze esterne
    // Usiamo serde_json che è già nel Cargo.toml
    use serde_json::Value;
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;

    app.start = [
        v["start"][0].as_f64().unwrap_or(182.9) as f32,
        v["start"][1].as_f64().unwrap_or(182.9) as f32,
    ];
    app.robot_heading = v["rh"].as_f64().unwrap_or(90.0) as f32;

    app.segments.clear();
    if let Some(segs) = v["segs"].as_array() {
        for s in segs {
            let to = [
                s["to"][0].as_f64().unwrap_or(0.0) as f32,
                s["to"][1].as_f64().unwrap_or(0.0) as f32,
            ];
            let controls: Vec<[f32;2]> = s["ctrl"].as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|c| [c[0].as_f64().unwrap_or(0.0) as f32,
                           c[1].as_f64().unwrap_or(0.0) as f32])
                .collect();

            let act_str = s["act"].as_str().unwrap_or("move");
            // Compatibilità: vecchi salvataggi con "align:gradi" nell'azione
            let al_from_act = act_str.strip_prefix("align:").and_then(|r| r.parse::<f32>().ok());
            let action = if act_str == "move" {
                RobotAction::Move
            } else if let Some(rest) = act_str.strip_prefix("wait:") {
                RobotAction::Wait(rest.parse().unwrap_or(1.0))
            } else if let Some(rest) = act_str.strip_prefix("shoot_stop:") {
                // compatibilità: il vecchio Shoot+Stop diventa un Wait della stessa durata
                RobotAction::Wait(rest.parse().unwrap_or(1.0))
            } else {
                RobotAction::Move
            };

            let spd = &s["spd"];
            let speed = SpeedProfile {
                v_start:     spd[0].as_f64().unwrap_or(0.0)  as f32,
                v_max:       spd[1].as_f64().unwrap_or(80.0) as f32,
                accel:       spd[2].as_f64().unwrap_or(120.0)as f32,
                decel:       spd[3].as_f64().unwrap_or(200.0)as f32,
                brake_point: spd[4].as_f64().unwrap_or(0.75) as f32,
                v_min:       spd[5].as_f64().unwrap_or(25.0) as f32,
            };

            let opts = SegmentOptions {
                speed,
                action,
                robot_heading_start: s["rhs"].as_f64().unwrap_or(90.0) as f32,
                robot_heading_end:   s["rhe"].as_f64().unwrap_or(90.0) as f32,
                align:               s["al"].as_f64().map(|x| x as f32).or(al_from_act),
            };

            let seg = if s["t"] == "curve" {
                Segment::Curve { controls, to, opts }
            } else if s["t"] == "search" {
                let sr = &s["sr"];
                let search = SearchParams {
                    v:           sr[0].as_f64().unwrap_or(60.0) as f32,
                    width:       sr[1].as_f64().unwrap_or(60.0) as f32,
                    time_limit:  sr[2].as_f64().unwrap_or(5.0)  as f32,
                    shoot_after: sr[3].as_bool().unwrap_or(false),
                    shoot_secs:  sr[4].as_f64().unwrap_or(1.0)  as f32,
                };
                Segment::Search { to, search, opts }
            } else {
                Segment::Line { to, opts }
            };
            app.segments.push(seg);
        }
    }

    app.barriers.clear();
    if let Some(bars) = v["barriers"].as_array() {
        for b in bars {
            let mut barrier = Barrier::new();
            barrier.closed = b["closed"].as_bool().unwrap_or(false);
            if let Some(verts) = b["v"].as_array() {
                for v in verts {
                    barrier.vertices.push([
                        v[0].as_f64().unwrap_or(0.0) as f32,
                        v[1].as_f64().unwrap_or(0.0) as f32,
                    ]);
                }
            }
            app.barriers.push(barrier);
        }
    }

    app.focal_lines.clear();
    if let Some(fl) = v["focal"].as_array() {
        for f in fl {
            let g = |i: usize| f[i].as_f64().unwrap_or(0.0) as f32;
            let mut line = crate::app::FocalLine::new([g(0), g(1)], [g(2), g(3)]);
            line.snap_radius_cm = g(4).max(1.0);
            line.visible = f[5].as_bool().unwrap_or(true);
            app.focal_lines.push(line);
        }
    }

    app.highlights.clear();
    app.highlight_draft = None;
    if let Some(hl) = v["hl"].as_array() {
        for h in hl {
            let kind = match h[0].as_i64().unwrap_or(0) {
                1 => Mechanism::Flywheel,
                2 => Mechanism::IntakeDown,
                3 => Mechanism::Shoot,
                4 => Mechanism::BarDown,
                _ => Mechanism::Intake,
            };
            let from = h[1].as_f64().unwrap_or(0.0) as f32;
            let to   = h[2].as_f64().unwrap_or(0.0) as f32;
            let secs  = h[3].as_f64().unwrap_or(1.0) as f32;
            let delay = h[4].as_f64().unwrap_or(0.0) as f32;
            // le azioni puntuali su un nodo hanno from == to
            if to > from || (kind.is_point_kind() && to == from) {
                app.highlights.push(Highlight { kind, from, to, secs, delay });
            }
        }
    }

    let pk = &v["park"];
    let d = ParkPoint::default();
    app.park = ParkPoint {
        pos: [
            pk[0].as_f64().map(|x| x as f32).unwrap_or(d.pos[0]),
            pk[1].as_f64().map(|x| x as f32).unwrap_or(d.pos[1]),
        ],
        heading:   pk[2].as_f64().map(|x| x as f32).unwrap_or(d.heading),
        trigger_s: pk[3].as_f64().map(|x| x as f32).unwrap_or(d.trigger_s),
        v:         pk[4].as_f64().map(|x| x as f32).unwrap_or(d.v),
        enabled:   pk[5].as_bool().unwrap_or(false),
    };

    Ok(())
}
