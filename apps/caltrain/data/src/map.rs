//! The line map: a Canvas 2D surface (LLP 1056 §8 stage 1, the take in
//! `rules/DEFERRED.md`), drawn by the data crate with the web's canvas
//! calls. It was a wgpu surface (LLP 1009); the drawing is the same.
//!
//! Arguments, in `canvas surface=map(line, selectedId, board, nowMs, em)`
//! order: the stations in line order (records `[id, name, zone, distance]`),
//! the selected station's id, the northbound board (records `[id, train,
//! service, headsign, at]`), the clock, and the `em` the rows are sized in
//! (16 points, 24 on a TV). It draws the line and the next
//! northbound train as a square sliding toward the selected station as its
//! countdown runs. The stations — a dot and a name each — are the canvas's
//! children (LLP 1014), laid out by the kernel over the bitmap. The two
//! agree on where a station is by construction: the children are a column
//! with 16 points of padding and `1.25em` rows spread `space-between`, so
//! the first row's centre is 16 points and `0.625em` from the top and the
//! last as far from the bottom, and the train interpolates between the same
//! centres; the line and the train scale with `em`. `nowMs`
//! ticks once a second, so the map draws on `args` alone.

use exact_runner::exact_canvas::{Context2d, DrawError, Frame};
use exact_runner::Value;

fn fields(v: &Value) -> Option<&[Value]> {
    match v {
        Value::List(items) | Value::Record(items) => Some(items),
        _ => None,
    }
}

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => Some(*n),
        _ => None,
    }
}

fn text(v: Option<&Value>) -> Option<&str> {
    v?.as_str()
}

/// Progress of the next northbound train toward the selected station, 0–1:
/// the first departure not yet gone, over its last twenty minutes.
pub fn train_progress(board: &Value, now: f64) -> Option<f64> {
    fields(board)?.iter().find_map(|d| {
        let at = number(fields(d)?.get(4))?;
        (at >= now).then(|| 1.0 - (((at - now) / 60_000.0).max(0.0) / 20.0).min(1.0))
    })
}

/// Draw the map into `ctx` at `frame`'s size.
pub fn draw(args: &[Value], ctx: &Context2d, frame: &Frame) -> Result<bool, DrawError> {
    let [line, selected, board, now, em] = args else {
        return Err(format!("map: expected 5 arguments, got {}", args.len()).into());
    };
    let stations: Vec<&str> = fields(line)
        .ok_or("map: line is not a list")?
        .iter()
        .filter_map(|s| text(fields(s)?.first()))
        .collect();
    let selected = text(Some(selected)).ok_or("map: selectedId is not a string")?;
    let now = number(Some(now)).ok_or("map: nowMs is not a number")?;
    let em = number(Some(em)).ok_or("map: em is not a number")?;
    let k = em / 16.0;
    let (w, h) = (frame.width, frame.height);
    ctx.clear_rect(0.0, 0.0, w, h);
    ctx.set_fill_style_str("#f7f7f7");
    ctx.fill_rect(0.0, 0.0, w, h);
    let n = stations.len();
    if n == 0 {
        return Ok(false);
    }
    let x = 0.5 * w;
    let (top, bottom) = (16.0 + 0.625 * em, h - 16.0 - 0.625 * em);
    let y_of = |i: usize| top + (bottom - top) * (i as f64 / (n.max(2) - 1) as f64);
    ctx.set_fill_style_str("#bfbfbf");
    ctx.fill_rect(x - 1.5 * k, top, 3.0 * k, bottom - top);
    let sel = stations.iter().position(|id| *id == selected);
    if let (Some(sel), Some(progress)) = (sel, train_progress(board, now)) {
        // Northbound: from the next station south (the higher index) toward
        // the selected one; left of the line, as the names are on its right.
        let from = (sel + 1).min(n - 1);
        let y = y_of(from) + (y_of(sel) - y_of(from)) * progress;
        ctx.set_fill_style_str("#2980e6");
        ctx.fill_rect(x - 23.0 * k, y - 5.0 * k, 10.0 * k, 10.0 * k);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::exact_canvas::list;

    fn departure(id: &str, at: f64) -> Value {
        Value::record(vec![
            Value::str(id),
            Value::Number(1.0),
            Value::str("Local"),
            Value::str("San Francisco"),
            Value::Number(at),
        ])
    }

    #[test]
    fn the_first_not_yet_departed_train_is_twenty_minutes_out() {
        let board = Value::list(vec![
            departure("past", 1_000.0),
            departure("next", 1_800_000.0),
        ]);
        assert_eq!(train_progress(&board, 600_000.0), Some(0.0));
        assert_eq!(train_progress(&board, 1_800_000.0), Some(1.0));
    }

    #[test]
    fn the_map_draws_background_line_and_train() {
        let station = |id: &str| {
            Value::record(vec![
                Value::str(id),
                Value::str(id),
                Value::Number(1.0),
                Value::Number(0.0),
            ])
        };
        let ctx = Context2d::new();
        let frame = Frame {
            width: 300.0,
            height: 160.0,
            scale: 2.0,
            pixel_width: 600,
            pixel_height: 320,
            ..Default::default()
        };
        let args = [
            Value::list(vec![station("a"), station("b")]),
            Value::str("a"),
            Value::list(vec![departure("next", 1_200_000.0)]),
            Value::Number(600_000.0),
            Value::Number(16.0),
        ];
        assert_eq!(draw(&args, &ctx, &frame), Ok(false));
        let lists = ctx.take_lists();
        let names: Vec<_> = list::records(&lists[0])
            .unwrap()
            .iter()
            .map(|r| r.op.name())
            .collect();
        assert_eq!(
            names,
            [
                "ClearRect",
                "FillColor",
                "FillRect",
                "FillColor",
                "FillRect",
                "FillColor",
                "FillRect"
            ]
        );
    }
}
