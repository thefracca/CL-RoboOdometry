//! Serializzazione del percorso nel formato JSON definito.

use crate::app::HudApp;
use crate::barrier::Barrier;
use crate::segment::{HeadingMode, RobotAction, Segment};
use crate::simulation::MATCH_DURATION;

pub fn to_json(app: &HudApp) -> String {
    let start = format!(
        r#"{{"x":{:.2},"y":{:.2},"heading":{:.2}}}"#,
        app.start[0], app.start[1], app.robot_heading
    );

    let segments: Vec<String> = app.segments.iter().enumerate().map(|(i, seg)| {
        let opts = seg.opts();

        let type_str = match seg {
            Segment::Line  { .. } => "line",
            Segment::Curve { .. } => "curve",
        };

        let to = match seg {
            Segment::Line  { to, .. } | Segment::Curve { to, .. } =>
                format!(r#"{{"x":{:.2},"y":{:.2}}}"#, to[0], to[1]),
        };

        let heading = match &opts.heading {
            HeadingMode::Tangent    => r#""tangent""#.to_string(),
            HeadingMode::Fixed(deg) => format!(r#"{{"fixed":{:.2}}}"#, deg),
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

        let action = match &opts.action {
            RobotAction::Move             => r#""move""#.to_string(),
            RobotAction::Wait(secs)       => format!(r#"{{"wait":{:.2}}}"#, secs),
            RobotAction::ShootStop(secs)  => format!(r#"{{"shoot_stop":{:.2}}}"#, secs),
            RobotAction::ShootMove(secs)  => format!(r#"{{"shoot_move":{:.2}}}"#, secs),
        };

        let sp = &opts.speed;
        format!(
            r#"    {{
      "id":{i},
      "type":"{type_str}",
      "to":{to}{controls},
      "heading":{heading},
      "robot_heading":{{
        "start":{:.2},
        "end":{:.2}
      }},
      "speed":{{
        "v_start":{:.2},
        "v_max":{:.2},
        "accel":{:.2},
        "decel":{:.2},
        "brake_point":{:.4}
      }},
      "action":{action},
      "intake":{},
      "flywheel":{}
    }}"#,
            opts.robot_heading_start,
            opts.robot_heading_end,
            sp.v_start, sp.v_max, sp.accel, sp.decel, sp.brake_point,
            opts.intake, opts.flywheel,
        )
    }).collect();

    let barriers: Vec<String> = app.barriers.iter().enumerate()
        .filter(|(_, b)| b.closed)
        .map(|(i, b)| {
            let verts: Vec<String> = b.vertices.iter()
                .map(|v| format!(r#"{{"x":{:.2},"y":{:.2}}}"#, v[0], v[1]))
                .collect();
            format!(
                r#"    {{"id":{i},"vertices":[{}]}}"#,
                verts.join(",")
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
  "barriers":[
{}
  ]
}}"#,
        MATCH_DURATION as u32,
        segments.join(",\n"),
        barriers.join(",\n"),
    )
}
