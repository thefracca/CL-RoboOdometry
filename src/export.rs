//! Serializzazione del percorso nel formato JSON definito.

use crate::app::HudApp;
use crate::segment::{Mechanism, RobotAction, Segment};
use crate::simulation::MATCH_DURATION;

/// Posizione lungo il percorso (indice segmento + frazione) → {"segment":i,"t":f}
fn pos_json(p: f32, n_segments: usize) -> String {
    if n_segments == 0 {
        return r#"{"segment":0,"t":0.0000}"#.to_string();
    }
    let mut seg = p.floor().max(0.0) as usize;
    let mut t = p - seg as f32;
    if seg >= n_segments {
        seg = n_segments - 1;
        t = 1.0;
    }
    format!(r#"{{"segment":{},"t":{:.4}}}"#, seg, t.clamp(0.0, 1.0))
}

pub fn to_json(app: &HudApp) -> String {
    let start = format!(
        r#"{{"x":{:.2},"y":{:.2},"heading":{:.2}}}"#,
        app.start[0], app.start[1], app.robot_heading
    );

    let segments: Vec<String> = app.segments.iter().enumerate().map(|(i, seg)| {
        let opts = seg.opts();

        let type_str = match seg {
            Segment::Line   { .. } => "line",
            Segment::Curve  { .. } => "curve",
            Segment::Search { .. } => "search",
        };

        let to = match seg {
            Segment::Line  { to, .. } | Segment::Curve { to, .. } | Segment::Search { to, .. } =>
                format!(r#"{{"x":{:.2},"y":{:.2}}}"#, to[0], to[1]),
        };

        let controls = match seg {
            Segment::Curve { controls, .. } => {
                let pts: Vec<String> = controls.iter()
                    .map(|c| format!(r#"{{"x":{:.2},"y":{:.2}}}"#, c[0], c[1]))
                    .collect();
                format!(",\n      \"controls\":[{}]", pts.join(","))
            }
            _ => String::new(),
        };

        // Ricerca palline: solo i suoi parametri (v costante, area, time limit, sparo finale)
        let search = match seg {
            Segment::Search { search, .. } => format!(
                ",\n      \"search\":{{\n        \"v\":{:.2},\n        \"area_width\":{:.2},\n        \"time_limit\":{:.2},\n        \"shoot_after\":{},\n        \"shoot_time\":{:.2}\n      }}",
                search.v, search.width, search.time_limit, search.shoot_after, search.shoot_secs
            ),
            _ => String::new(),
        };

        // Velocità: linee = profilo completo, curve = senza frenata, ricerca = nessun blocco
        let sp = &opts.speed;
        let speed = match seg {
            Segment::Line { .. } => format!(
                ",\n      \"speed\":{{\n        \"v_start\":{:.2},\n        \"v_max\":{:.2},\n        \"v_min\":{:.2},\n        \"accel\":{:.2},\n        \"decel\":{:.2},\n        \"brake_point\":{:.4}\n      }}",
                sp.v_start, sp.v_max, sp.v_min, sp.accel, sp.decel, sp.brake_point
            ),
            Segment::Curve { .. } => format!(
                ",\n      \"speed\":{{\n        \"v_start\":{:.2},\n        \"v_max\":{:.2},\n        \"accel\":{:.2}\n      }}",
                sp.v_start, sp.v_max, sp.accel
            ),
            Segment::Search { .. } => String::new(),
        };

        let action = match &opts.action {
            RobotAction::Move             => r#""move""#.to_string(),
            RobotAction::Wait(secs)       => format!(r#"{{"wait":{:.2}}}"#, secs),
            RobotAction::ShootStop(secs)  => format!(r#"{{"shoot_stop":{:.2}}}"#, secs),
            RobotAction::ShootMove(secs)  => format!(r#"{{"shoot_move":{:.2}}}"#, secs),
        };

        format!(
            r#"    {{
      "id":{i},
      "type":"{type_str}",
      "to":{to}{controls}{search},
      "robot_heading":{{
        "start":{:.2},
        "end":{:.2},
        "rotation_speed":{:.2}
      }}{speed},
      "action":{action}
    }}"#,
            opts.robot_heading_start,
            opts.robot_heading_end,
            opts.rotation_speed,
        )
    }).collect();

    // Tratti evidenziati: intake / flywheel attivi tra due posizioni del percorso
    let n = app.segments.len();
    let mut hl = app.highlights.clone();
    hl.sort_by(|a, b| a.from.partial_cmp(&b.from).unwrap_or(std::cmp::Ordering::Equal));
    let mechanisms: Vec<String> = hl.iter().map(|h| {
        let kind = match h.kind {
            Mechanism::Intake   => "intake",
            Mechanism::Flywheel => "flywheel",
        };
        format!(
            r#"    {{"type":"{kind}","from":{},"to":{}}}"#,
            pos_json(h.from, n),
            pos_json(h.to, n),
        )
    }).collect();

    format!(
        r#"{{
  "units":"cm",
  "match_duration":{},
  "start":{start},
  "segments":[
{}
  ],
  "mechanisms":[
{}
  ]
}}"#,
        MATCH_DURATION as u32,
        segments.join(",\n"),
        mechanisms.join(",\n"),
    )
}
