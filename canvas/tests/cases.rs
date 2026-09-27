//! `cases.txt` against the Rust recorder: the same calls, the same getters
//! and throws Chrome gave (LLP 1056 §4). The TypeScript recorder runs the
//! same file through `cases.mjs`.

use exact_canvas::{list, CanvasGradient, CanvasWindingRule, Context2d, DomMatrix, Radius, Style};

#[derive(Debug, Clone)]
enum V {
    B(bool),
    N(f64),
    S(String),
    L(Vec<V>),
    P(f64, f64),
    G,
}

fn parse_value(s: &str) -> (V, &str) {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"').unwrap();
        return (V::S(rest[..end].into()), &rest[end + 1..]);
    }
    if let Some(mut rest) = s.strip_prefix('[') {
        let mut items = Vec::new();
        loop {
            rest = rest.trim_start();
            if let Some(r) = rest.strip_prefix(']') {
                return (V::L(items), r);
            }
            let (v, r) = parse_value(rest);
            items.push(v);
            rest = r.trim_start().strip_prefix(',').unwrap_or(r);
        }
    }
    if let Some(rest) = s.strip_prefix('{') {
        let end = rest.find('}').unwrap();
        let mut x = 0.0;
        let mut y = 0.0;
        for kv in rest[..end].split(',') {
            let (k, v) = kv.split_once(':').unwrap();
            let v: f64 = v.trim().parse().unwrap();
            if k.trim() == "x" {
                x = v
            } else {
                y = v
            }
        }
        return (V::P(x, y), &rest[end + 1..]);
    }
    for (word, v) in [("true", V::B(true)), ("false", V::B(false))] {
        if let Some(rest) = s.strip_prefix(word) {
            return (v, rest);
        }
    }
    if let Some(rest) = s.strip_prefix('g') {
        return (V::G, rest);
    }
    let end = s.find([',', ')', ']']).unwrap_or(s.len());
    let t = s[..end].trim();
    let n = match t {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        _ => t.parse().unwrap_or_else(|_| panic!("number `{t}`")),
    };
    (V::N(n), &s[end..])
}

fn args(s: &str) -> Vec<V> {
    let mut rest = s.trim();
    let mut out = Vec::new();
    while !rest.is_empty() {
        let (v, r) = parse_value(rest);
        out.push(v);
        rest = r.trim_start().strip_prefix(',').unwrap_or(r).trim();
    }
    out
}

fn n(v: &V) -> f64 {
    match v {
        V::N(x) => *x,
        other => panic!("not a number: {other:?}"),
    }
}

fn s(v: &V) -> &str {
    match v {
        V::S(x) => x,
        other => panic!("not a string: {other:?}"),
    }
}

type R = Result<(), exact_canvas::DomException>;

fn call(ctx: &Context2d, g: &mut Option<CanvasGradient>, line: &str) -> R {
    if let Some((lhs, rhs)) = line.split_once(" = ") {
        let lhs = lhs.trim();
        if lhs == "g" {
            let (name, a) = rhs.split_once('(').unwrap();
            let a = args(a.strip_suffix(')').unwrap());
            let f = |i: usize| n(&a[i]);
            *g = Some(match name {
                "createLinearGradient" => ctx.create_linear_gradient(f(0), f(1), f(2), f(3))?,
                "createRadialGradient" => {
                    ctx.create_radial_gradient(f(0), f(1), f(2), f(3), f(4), f(5))?
                }
                other => panic!("{other}"),
            });
            return Ok(());
        }
        let (v, _) = parse_value(rhs);
        match (lhs, &v) {
            ("lineWidth", v) => ctx.set_line_width(n(v)),
            ("miterLimit", v) => ctx.set_miter_limit(n(v)),
            ("lineCap", v) => ctx.set_line_cap(s(v)),
            ("lineJoin", v) => ctx.set_line_join(s(v)),
            ("lineDashOffset", v) => ctx.set_line_dash_offset(n(v)),
            ("globalAlpha", v) => ctx.set_global_alpha(n(v)),
            ("globalCompositeOperation", v) => ctx.set_global_composite_operation(s(v))?,
            ("fillStyle", V::G) => ctx.set_fill_style_canvas_gradient(g.as_ref().unwrap()),
            ("strokeStyle", V::G) => ctx.set_stroke_style_canvas_gradient(g.as_ref().unwrap()),
            ("fillStyle", v) => ctx.set_fill_style_str(s(v)),
            ("strokeStyle", v) => ctx.set_stroke_style_str(s(v)),
            (other, _) => panic!("attribute {other}"),
        }
        return Ok(());
    }
    let (name, a) = line.split_once('(').unwrap();
    let a = args(a.strip_suffix(')').unwrap());
    let f = |i: usize| n(&a[i]);
    let b = |i: usize| matches!(&a[i], V::B(true));
    let rule = |v: &V| {
        if s(v) == "evenodd" {
            CanvasWindingRule::Evenodd
        } else {
            CanvasWindingRule::Nonzero
        }
    };
    match name {
        "g.addColorStop" => g.as_ref().unwrap().add_color_stop_f64(f(0), s(&a[1]))?,
        "save" => ctx.save(),
        "restore" => ctx.restore(),
        "reset" => ctx.reset(),
        "translate" => ctx.translate(f(0), f(1))?,
        "scale" => ctx.scale(f(0), f(1))?,
        "rotate" => ctx.rotate(f(0))?,
        "setTransform" => ctx.set_transform(f(0), f(1), f(2), f(3), f(4), f(5))?,
        "transform" => ctx.transform(f(0), f(1), f(2), f(3), f(4), f(5))?,
        "resetTransform" => ctx.reset_transform()?,
        "setLineDash" => {
            let V::L(items) = &a[0] else { panic!() };
            ctx.set_line_dash(&items.iter().map(n).collect::<Vec<_>>())?
        }
        "arc" if a.len() == 6 => ctx.arc_with_anticlockwise(f(0), f(1), f(2), f(3), f(4), b(5))?,
        "arc" => ctx.arc(f(0), f(1), f(2), f(3), f(4))?,
        "ellipse" if a.len() == 8 => {
            ctx.ellipse_with_anticlockwise(f(0), f(1), f(2), f(3), f(4), f(5), f(6), b(7))?
        }
        "ellipse" => ctx.ellipse(f(0), f(1), f(2), f(3), f(4), f(5), f(6))?,
        "beginPath" => ctx.begin_path(),
        "moveTo" => ctx.move_to(f(0), f(1)),
        "lineTo" => ctx.line_to(f(0), f(1)),
        "quadraticCurveTo" => ctx.quadratic_curve_to(f(0), f(1), f(2), f(3)),
        "bezierCurveTo" => ctx.bezier_curve_to(f(0), f(1), f(2), f(3), f(4), f(5)),
        "closePath" => ctx.close_path(),
        "rect" => ctx.rect(f(0), f(1), f(2), f(3)),
        "fill" if a.is_empty() => ctx.fill(),
        "fill" => ctx.fill_with_canvas_winding_rule(rule(&a[0])),
        "stroke" => ctx.stroke(),
        "clip" if a.is_empty() => ctx.clip(),
        "clip" => ctx.clip_with_canvas_winding_rule(rule(&a[0])),
        "fillRect" => ctx.fill_rect(f(0), f(1), f(2), f(3)),
        "strokeRect" => ctx.stroke_rect(f(0), f(1), f(2), f(3)),
        "clearRect" => ctx.clear_rect(f(0), f(1), f(2), f(3)),
        "arcTo" => ctx.arc_to(f(0), f(1), f(2), f(3), f(4))?,
        "roundRect" if matches!(a[4], V::N(_)) => {
            ctx.round_rect_with_f64(f(0), f(1), f(2), f(3), f(4))?
        }
        "roundRect" => {
            let V::L(items) = &a[4] else { panic!() };
            let radii: Vec<Radius> = items
                .iter()
                .map(|v| match v {
                    V::N(r) => Radius { x: *r, y: *r },
                    V::P(x, y) => Radius { x: *x, y: *y },
                    other => panic!("{other:?}"),
                })
                .collect();
            ctx.round_rect_with_radii(f(0), f(1), f(2), f(3), &radii)?
        }
        other => panic!("method {other}"),
    }
    Ok(())
}

fn query(ctx: &Context2d, q: &str) -> String {
    let style = |st: Style| match st {
        Style::Color(c) => c,
        Style::Gradient(_) => "[object CanvasGradient]".into(),
    };
    let num = |v: f64| format!("{v}");
    match q {
        "lineWidth" => num(ctx.line_width()),
        "miterLimit" => num(ctx.miter_limit()),
        "lineCap" => ctx.line_cap(),
        "lineJoin" => ctx.line_join(),
        "lineDashOffset" => num(ctx.line_dash_offset()),
        "globalAlpha" => num(ctx.global_alpha()),
        "globalCompositeOperation" => ctx.global_composite_operation(),
        "fillStyle" => style(ctx.fill_style()),
        "strokeStyle" => style(ctx.stroke_style()),
        "getLineDash()" => ctx
            .get_line_dash()
            .iter()
            .map(|v| num(*v))
            .collect::<Vec<_>>()
            .join(","),
        "getTransform()" => {
            let m: DomMatrix = ctx.get_transform().unwrap();
            m.operands()
                .iter()
                .map(|v| num(*v))
                .collect::<Vec<_>>()
                .join(",")
        }
        other => panic!("query {other}"),
    }
}

/// Every case's lists from the Rust recorder, by case name.
fn rust_lists(text: &str) -> Vec<(String, Vec<Vec<u8>>)> {
    let mut out: Vec<(String, Vec<Vec<u8>>)> = Vec::new();
    let mut ctx = Context2d::new();
    let mut g = None;
    for raw in text.lines().chain(std::iter::once("## end")) {
        let line = raw.trim();
        if let Some(n) = line.strip_prefix("## ") {
            if let Some(last) = out.last_mut() {
                last.1 = ctx.take_lists();
            }
            out.push((n.to_string(), Vec::new()));
            ctx = Context2d::new();
            g = None;
            continue;
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with("? ") {
            continue;
        }
        let stmt = line.rsplit_once(" ! ").map_or(line, |(c, _)| c.trim());
        let _ = call(&ctx, &mut g, stmt);
    }
    out.pop();
    out
}

/// The TypeScript recorder's lists for the same cases (`bun cases.mjs
/// --lists`) equal the Rust recorder's: the same records, operands equal to
/// 1e-9 (the two languages' `sin`/`cos` may differ in the last bit).
#[test]
fn the_typescript_recorder_writes_the_rust_recorders_lists() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
        .arg(dir.join("cases.mjs"))
        .arg("--lists")
        .output()
        .expect("bun runs the TypeScript recorder (the repo pins it)");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let js = String::from_utf8(out.stdout).unwrap();
    let mut failures = Vec::new();
    for (name, lists) in rust_lists(include_str!("cases.txt")) {
        let key = format!("{:?}:", name);
        let Some(at) = js.find(&key) else {
            failures.push(format!("{name}: missing from the TypeScript run"));
            continue;
        };
        let rest = &js[at + key.len()..];
        let array = &rest[..rest.find(']').unwrap() + 1];
        let hexes: Vec<&str> = array
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_matches('"'))
            .collect();
        if hexes.len() != lists.len() {
            failures.push(format!(
                "{name}: {} lists in TypeScript, {} in Rust",
                hexes.len(),
                lists.len()
            ));
            continue;
        }
        for (hex, rust) in hexes.iter().zip(&lists) {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let (a, b) = (list::records(&bytes).unwrap(), list::records(rust).unwrap());
            if a.len() != b.len() {
                failures.push(format!(
                    "{name}: {} records in TypeScript, {} in Rust",
                    a.len(),
                    b.len()
                ));
                continue;
            }
            for (i, (x, y)) in a.iter().zip(&b).enumerate() {
                let close = x.op == y.op
                    && x.len() == y.len()
                    && x.operands()
                        .zip(y.operands())
                        .all(|(p, q)| (p - q).abs() <= 1e-9 * (1.0 + q.abs()));
                if !close {
                    failures.push(format!("{name}: record {i}: TypeScript {x:?}, Rust {y:?}"));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_shared_cases_hold_for_the_rust_recorder() {
    let text = include_str!("cases.txt");
    let mut failures = Vec::new();
    let mut ctx = Context2d::new();
    let mut g = None;
    let mut name = "";
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(n) = line.strip_prefix("## ") {
            name = n;
            ctx = Context2d::new();
            g = None;
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(q) = line.strip_prefix("? ") {
            let (q, want) = q.split_once("=>").unwrap();
            let got = query(&ctx, q.trim());
            if got != want.trim() {
                failures.push(format!(
                    "{name}: {} => {got}, want {}",
                    q.trim(),
                    want.trim()
                ));
            }
            continue;
        }
        let (stmt, throws) = match line.rsplit_once(" ! ") {
            Some((c, t)) => (c.trim(), Some(t.trim())),
            None => (line, None),
        };
        match (call(&ctx, &mut g, stmt), throws) {
            (Ok(()), None) => {}
            (Err(e), Some(t)) if e.name == t => {}
            (Ok(()), Some(t)) => failures.push(format!("{name}: {stmt} did not throw {t}")),
            (Err(e), _) => failures.push(format!("{name}: {stmt} threw {}", e.name)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
