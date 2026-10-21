//! The canvas row (SPEC 4) as a Canvas 2D drawing (LLP 1056): `canvas surface=card(c)` in
//! app.contract, drawn here in the SPEC's order with the web's `CanvasRenderingContext2D` calls.
//! The node's CSS `border-radius=12` clips the bitmap.

use exact_runner::exact_canvas::{Context2d, DrawError, Frame};
use exact_runner::Value;
use std::f64::consts::TAU;

fn fields(v: &Value) -> &[Value] {
    match v {
        Value::List(x) | Value::Record(x) => x,
        _ => &[],
    }
}
fn num(v: &Value) -> f64 {
    v.as_number().unwrap_or(0.0)
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// `shape CanvasK`: title, image, bg, band0, band1, strokes (color, width, p), dots (x, y, r, color).
pub fn draw(args: &[Value], ctx: &Context2d, frame: &Frame) -> Result<bool, DrawError> {
    let [card] = args else { return Err("card(c): one argument".into()) };
    let f = fields(card);
    if f.len() < 7 {
        return Err("card: not a CanvasK".into());
    }
    let (w, h) = (frame.width, frame.height);
    // The bitmap keeps its pixels across redraws (args, image decode): start clean.
    ctx.reset();
    ctx.set_fill_style_str(text(&f[2]));
    ctx.fill_rect(0.0, 0.0, w, h);

    // 1. The band: a rounded rect, a left-to-right gradient.
    let (bx, by, bw, bh) = (16.0, h - 60.0, 0.4 * w, 44.0);
    let g = ctx.create_linear_gradient(bx, 0.0, bx + bw, 0.0)?;
    g.add_color_stop(0.0, text(&f[3]))?;
    g.add_color_stop(1.0, text(&f[4]))?;
    ctx.set_fill_style_canvas_gradient(&g);
    ctx.begin_path();
    ctx.round_rect_with_f64(bx, by, bw, bh, 10.0)?;
    ctx.fill();

    // 2. Six cubic Béziers, round caps and joins.
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    for st in fields(&f[5]) {
        let st = fields(st);
        let p: Vec<f64> = fields(&st[2]).iter().map(num).collect();
        if p.len() < 8 {
            continue;
        }
        ctx.set_stroke_style_str(text(&st[0]));
        ctx.set_line_width(num(&st[1]));
        ctx.begin_path();
        ctx.move_to(p[0] * w, p[1] * h);
        ctx.bezier_curve_to(p[2] * w, p[3] * h, p[4] * w, p[5] * h, p[6] * w, p[7] * h);
        ctx.stroke();
    }

    // 3. Fourteen dots at 60 % opacity.
    ctx.save();
    ctx.set_global_alpha(0.6);
    for d in fields(&f[6]) {
        let d = fields(d);
        ctx.set_fill_style_str(text(&d[3]));
        ctx.begin_path();
        ctx.arc(num(&d[0]) * w, num(&d[1]) * h, num(&d[2]), 0.0, TAU)?;
        ctx.fill();
    }
    ctx.restore();

    // 4. The image, 64 × 64 at (W − 80, 16), in a circle. The source is square, so the
    //    aspect-fill is the whole image.
    ctx.save();
    ctx.begin_path();
    ctx.arc(w - 80.0 + 32.0, 16.0 + 32.0, 32.0, 0.0, TAU)?;
    ctx.clip();
    ctx.draw_image_with_image_handle_and_dw_and_dh(text(&f[1]), w - 80.0, 16.0, 64.0, 64.0)?;
    ctx.restore();

    // 5. The title: system 20 semibold #1C1C1E, its box's top-left at (16, 16).
    ctx.set_font("600 20px system-ui");
    ctx.set_text_baseline("top");
    ctx.set_text_align("left");
    ctx.set_fill_style_str("#1C1C1E");
    ctx.fill_text(text(&f[0]), 16.0, 16.0)?;
    Ok(false)
}
