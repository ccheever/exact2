//! @ref LLP 1043.000 §3 D1/D8 — identical filled and excluded polygon vertices.
use exact_plan::Value;
use std::f64::consts::{PI, TAU};
type Point = (f64, f64);
const COLORS: [&str; 6] = [
    "light-dark(#bf684a, #db9776)",
    "light-dark(#769383, #9bb7a2)",
    "light-dark(#c4a769, #cdb77e)",
    "light-dark(#486b68, #7ea6a0)",
    "light-dark(#a58179, #cda499)",
    "light-dark(#869caa, #a3bbc8)",
];
fn part(id: &str, bounds: [f64; 4], shape: String, clip: String, color: &str) -> Value {
    let [x, y, w, h] = bounds;
    Value::record(vec![
        Value::str(id),
        Value::Number(x),
        Value::Number(y),
        Value::Number(w),
        Value::Number(h),
        Value::str(&shape),
        Value::str(&clip),
        Value::str(color),
    ])
}
fn circle(id: &str, x: f64, y: f64, r: f64, color: &str) -> Value {
    part(
        id,
        [x - r, y - r, r * 2., r * 2.],
        "circle()".into(),
        "none".into(),
        color,
    )
}
fn polygon(id: &str, points: &[Point], scale: f64, offset: Point, color: &str) -> Value {
    let p: Vec<_> = points
        .iter()
        .map(|&(x, y)| (x * scale + offset.0, y * scale + offset.1))
        .collect();
    let x = p.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let y = p.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let w = p.iter().map(|p| p.0 - x).fold(0., f64::max);
    let h = p.iter().map(|p| p.1 - y).fold(0., f64::max);
    let points: Vec<_> = p
        .iter()
        .map(|p| format!("{:.3}px {:.3}px", p.0 - x, p.1 - y))
        .collect();
    let path: Vec<_> = p
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "{} {:.3} {:.3}",
                if i == 0 { "M" } else { "L" },
                p.0 - x,
                p.1 - y
            )
        })
        .collect();
    part(
        id,
        [x, y, w, h],
        format!("polygon({})", points.join(", ")),
        format!("path('{} Z')", path.join(" ")),
        color,
    )
}
// Twelve vertices, including both semicircular ends. Shared endpoints overlap.
fn capsule(a: Point, b: Point, r: f64) -> Vec<Point> {
    let angle = (b.1 - a.1).atan2(b.0 - a.0);
    [(b, angle - PI / 2.), (a, angle + PI / 2.)]
        .into_iter()
        .flat_map(|(p, t)| {
            (0..6).map(move |i| {
                let t = t + PI * i as f64 / 5.;
                (p.0 + r * t.cos(), p.1 + r * t.sin())
            })
        })
        .collect()
}
fn joint(p: Point, length: f64, angle: f64) -> Point {
    (p.0 + length * angle.sin(), p.1 + length * angle.cos())
}

// Six equal-density discs; fixed 16 ms steps, elastic impulses and wall reflection.
// Work is bounded by 1,875 steps × 15 pairs. A smooth 60-second clock
// traverses the same 30-second physical trajectory forwards and backwards.
// The turn has zero speed: no reset jump, and no state/history-dependent seeks.
fn orbs(ms: f64, width: f64) -> Vec<Value> {
    let radii = [43., 31., 24., 36., 20., 28.];
    let mut p: [Point; 6] = [
        (160., 74.),
        (426., 116.),
        (315., 248.),
        (540., 300.),
        (92., 330.),
        (440., 383.),
    ];
    let mut v: [Point; 6] = [
        (1.1, 0.68),
        (-0.85, 0.72),
        (1.02, -0.64),
        (-0.62, -0.83),
        (1.22, -0.45),
        (-0.82, -0.72),
    ];
    let phase = 15_000. + 15_000. * (ms.max(0.) % 60_000. * TAU / 60_000.).sin();
    for _ in 0..(phase / 16.) as usize {
        for i in 0..6 {
            p[i].0 += v[i].0;
            p[i].1 += v[i].1;
            for (value, velocity, edge) in [
                (&mut p[i].0, &mut v[i].0, 656.),
                (&mut p[i].1, &mut v[i].1, 440.),
            ] {
                if *value < radii[i] {
                    *value = 2. * radii[i] - *value;
                    *velocity = velocity.abs();
                }
                if *value > edge - radii[i] {
                    *value = 2. * (edge - radii[i]) - *value;
                    *velocity = -velocity.abs();
                }
            }
        }
        for i in 0..6 {
            for j in i + 1..6 {
                let (dx, dy) = (p[j].0 - p[i].0, p[j].1 - p[i].1);
                let distance = dx.hypot(dy);
                if distance >= radii[i] + radii[j] || distance < 0.0001 {
                    continue;
                }
                let (nx, ny) = (dx / distance, dy / distance);
                let (m1, m2) = (radii[i] * radii[i], radii[j] * radii[j]);
                let overlap = radii[i] + radii[j] - distance;
                p[i].0 -= nx * overlap * m2 / (m1 + m2);
                p[i].1 -= ny * overlap * m2 / (m1 + m2);
                p[j].0 += nx * overlap * m1 / (m1 + m2);
                p[j].1 += ny * overlap * m1 / (m1 + m2);
                let closing = (v[j].0 - v[i].0) * nx + (v[j].1 - v[i].1) * ny;
                if closing < 0. {
                    let impulse = 2. * closing / (m1 + m2);
                    v[i].0 += impulse * m2 * nx;
                    v[i].1 += impulse * m2 * ny;
                    v[j].0 -= impulse * m1 * nx;
                    v[j].1 -= impulse * m1 * ny;
                }
            }
        }
    }
    let s = width / 656.;
    p.iter()
        .enumerate()
        .map(|(i, p)| {
            circle(
                &format!("drift-{}", i + 1),
                p.0 * s,
                p.1 * s,
                radii[i] * s,
                COLORS[i],
            )
        })
        .collect()
}
fn dancer(ms: f64, width: f64) -> Vec<Value> {
    let t = ms.max(0.) % 24_000. / 1000.;
    let beat = t * TAU / 6.;
    let breath = beat.sin();
    let bend = (1. - (beat * 0.5).cos()) * 0.5;
    let lean = 0.10 * (beat - 0.4).sin();
    let hip = (0., 221. + 10. * bend);
    let shoulder = (-15. * breath, 141. + 10. * bend);
    let neck = (shoulder.0 - 3. * breath, 120. + 10. * bend);
    let head = (neck.0 - 3. * breath, 99. + 10. * bend);
    // 38 px head; neutral crown-to-toe about 304 px, the eight-head figure canon.
    let ls = (shoulder.0 - 20., shoulder.1);
    let rs = (shoulder.0 + 20., shoulder.1);
    let le = joint(ls, 52., -1.65 - 0.8 * (beat - 0.5).sin());
    let re = joint(rs, 52., 1.9 + 0.75 * (beat + 0.4).sin());
    let lw = joint(le, 47., -1.1 - 1.1 * (beat - 0.9).sin());
    let rw = joint(re, 47., 2.65 + 0.75 * (beat - 0.2).sin());
    let lh = (hip.0 - 13., hip.1);
    let rh = (hip.0 + 13., hip.1);
    let lk = joint(lh, 77., -0.16 - 0.48 * bend + lean);
    let rk = joint(rh, 77., 0.16 + 0.48 * bend + lean);
    let la = joint(lk, 76., 0.10 + 0.48 * bend - lean);
    let ra = joint(rk, 76., -0.10 - 0.48 * bend - lean);
    let s = (width / 656.).sqrt().clamp(0.58, 1.);
    let offset = (width / 2., -40. * s);
    let ink = "light-dark(#873e48, #d897a2)";
    let mut out = vec![circle(
        "dancer-head",
        head.0 * s + offset.0,
        head.1 * s + offset.1,
        19. * s,
        ink,
    )];
    let segments = [
        ("neck", neck, shoulder, 8.),
        ("upper-arm-left", ls, le, 8.),
        ("forearm-left", le, lw, 6.),
        ("upper-arm-right", rs, re, 8.),
        ("forearm-right", re, rw, 6.),
        ("thigh-left", lh, lk, 12.),
        ("calf-left", lk, la, 8.),
        ("thigh-right", rh, rk, 12.),
        ("calf-right", rk, ra, 8.),
        ("foot-left", la, (la.0 - 20., la.1 + 5.), 5.),
        ("foot-right", ra, (ra.0 + 20., ra.1 + 5.), 5.),
    ];
    for (id, a, b, r) in segments {
        out.push(polygon(
            &format!("dancer-{id}"),
            &capsule(a, b, r),
            s,
            offset,
            ink,
        ));
    }
    out.push(polygon(
        "dancer-bodice",
        &capsule((shoulder.0, shoulder.1 + 3.), (hip.0, hip.1 - 13.), 17.),
        s,
        offset,
        ink,
    ));
    let swing = 12. * (beat - 0.5).sin();
    out.push(polygon(
        "dancer-skirt",
        &[
            (hip.0 - 13., hip.1 - 18.),
            (hip.0 + 13., hip.1 - 18.),
            (hip.0 + 52. + swing, hip.1 + 38.),
            (hip.0 + 25. + swing, hip.1 + 45.),
            (hip.0 + swing, hip.1 + 41.),
            (hip.0 - 48. + swing, hip.1 + 34.),
        ],
        s,
        offset,
        "light-dark(#b66268, #e1a7ae)",
    ));
    out
}
pub(super) fn scene(name: &str, ms: f64, width: f64) -> Value {
    let width = width.clamp(240., 900.);
    Value::list(match name {
        "orbs" => orbs(ms, width),
        "dancer" => dancer(ms, width),
        _ => vec![],
    })
}
