//! The illustrated manuscript's dragon: one silhouette polygon that is both
//! the visible clip and the exclusion outline, so the words meet the ink.
//! @ref LLP 1043.000 §3 D1 — identical filled and excluded vertices.
use crate::prose::DRAGON;
use exact_plan::Value;

/// Design-space silhouette, 400 × 250, clockwise from the snout, facing left.
const BODY: [(f32, f32); 62] = [
    (14.0, 118.0),
    (30.0, 104.0),
    (52.0, 92.0),
    (66.0, 80.0),
    (72.0, 56.0),
    (82.0, 76.0),
    (94.0, 52.0),
    (100.0, 78.0),
    (118.0, 88.0),
    (130.0, 98.0),
    (136.0, 84.0),
    (144.0, 102.0),
    (158.0, 108.0),
    (164.0, 94.0),
    (172.0, 112.0),
    (186.0, 116.0),
    (196.0, 100.0),
    (222.0, 60.0),
    (252.0, 24.0),
    (286.0, 6.0),
    (302.0, 8.0),
    (318.0, 50.0),
    (304.0, 72.0),
    (346.0, 92.0),
    (320.0, 108.0),
    (348.0, 132.0),
    (308.0, 138.0),
    (334.0, 150.0),
    (362.0, 164.0),
    (384.0, 184.0),
    (396.0, 206.0),
    (386.0, 226.0),
    (364.0, 220.0),
    (372.0, 246.0),
    (346.0, 236.0),
    (350.0, 214.0),
    (330.0, 196.0),
    (312.0, 182.0),
    (300.0, 190.0),
    (312.0, 224.0),
    (300.0, 244.0),
    (280.0, 244.0),
    (282.0, 226.0),
    (276.0, 200.0),
    (250.0, 198.0),
    (226.0, 200.0),
    (214.0, 202.0),
    (222.0, 238.0),
    (206.0, 246.0),
    (190.0, 244.0),
    (192.0, 222.0),
    (184.0, 198.0),
    (160.0, 186.0),
    (136.0, 170.0),
    (116.0, 156.0),
    (96.0, 152.0),
    (62.0, 158.0),
    (34.0, 152.0),
    (14.0, 142.0),
    (36.0, 133.0),
    (26.0, 126.0),
    (14.0, 118.0),
];
/// The lighter belly, drawn over the body inside the same clip.
const BELLY: [(f32, f32); 12] = [
    (118.0, 150.0),
    (160.0, 178.0),
    (200.0, 192.0),
    (250.0, 190.0),
    (300.0, 182.0),
    (330.0, 190.0),
    (330.0, 202.0),
    (300.0, 198.0),
    (250.0, 204.0),
    (200.0, 206.0),
    (160.0, 192.0),
    (116.0, 162.0),
];
const DESIGN: (f32, f32) = (400.0, 250.0);

fn polygon(points: &[(f32, f32)], scale: f32) -> String {
    let parts: Vec<String> = points
        .iter()
        .map(|&(x, y)| format!("{:.2}px {:.2}px", x * scale, y * scale))
        .collect();
    format!("polygon({})", parts.join(", "))
}

fn path(points: &[(f32, f32)], scale: f32) -> String {
    let parts: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            format!(
                "{} {:.2} {:.2}",
                if i == 0 { "M" } else { "L" },
                x * scale,
                y * scale
            )
        })
        .collect();
    format!("path('{} Z')", parts.join(" "))
}

/// `dragon(width, dx, dy, boxHeight)`: the beast scaled to the page, offset by
/// the reader's drag and kept inside the paragraph box.
pub fn dragon(width: f32, dx: f32, dy: f32, box_height: f32) -> Value {
    let scale = ((width - 24.0) / DESIGN.0).clamp(0.5, 1.0);
    let (w, h) = (DESIGN.0 * scale, DESIGN.1 * scale);
    let x = ((width - w) / 2.0 + dx)
        .clamp(0.0, (width - w).max(0.0))
        .round();
    let y = (150.0 * scale + dy)
        .clamp(0.0, (box_height - h).max(0.0))
        .round();
    Value::record(vec![
        Value::str(DRAGON),
        Value::Number(x as f64),
        Value::Number(y as f64),
        Value::Number(w.round() as f64),
        Value::Number(h.round() as f64),
        Value::str(&polygon(&BODY, scale)),
        Value::str(&path(&BODY, scale)),
        Value::str(&path(&BELLY, scale)),
        Value::Number((54.0 * scale).round() as f64),
        Value::Number((94.0 * scale).round() as f64),
        Value::Number((5.0 * scale).max(3.0).round() as f64),
    ])
}
