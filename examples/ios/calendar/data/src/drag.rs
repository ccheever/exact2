//! The calendar's private input-only module protocol. Contract paints the drag.
//! Coordinates are CSS pixels in the permanently mounted input root's local box.

use exact_plan::Value;
use serde_json::Value as Json;

const MAX_SERIAL: u64 = 9_007_199_254_740_991;

/// Malformed or unsupported input is inert, rather than poisoning the UI runner.
/// Field order is the `DragInput` shape in the app's Contract.
pub(crate) fn decode(raw: &str) -> Value {
    read(raw).unwrap_or_else(|| {
        Value::record(vec![
            Value::Bool(false),
            Value::str(""),
            Value::str(""),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::str(""),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
        ])
    })
}

fn read(raw: &str) -> Option<Value> {
    if raw.len() > 4096 {
        return None;
    }
    let input: Json = serde_json::from_str(raw).ok()?;
    if input.get("v")?.as_u64()? != 1 {
        return None;
    }
    let phase = input.get("phase")?.as_str()?;
    if !matches!(phase, "begin" | "move" | "end" | "cancel") {
        return None;
    }
    let id = input.get("id")?.as_str()?;
    if id.is_empty() || id.len() > 192 {
        return None;
    }
    let serial = input.get("serial")?.as_u64()?;
    if serial == 0 || serial > MAX_SERIAL {
        return None;
    }
    let sequence = input.get("seq")?.as_u64()?;
    if sequence == 0 || sequence > MAX_SERIAL {
        return None;
    }
    let coordinate = |key| {
        let value = input.get(key)?.as_f64()?;
        (value.is_finite() && value.abs() <= f64::from(f32::MAX)).then_some(value)
    };
    let x = coordinate("x")?;
    let y = coordinate("y")?;
    let rx = coordinate("rx")?;
    let ry = coordinate("ry")?;
    let rw = coordinate("rw")?;
    let rh = coordinate("rh")?;
    if rw <= 0.0 || rh <= 0.0 {
        return None;
    }
    let reason = input.get("reason")?.as_str()?;
    if phase == "cancel" {
        if !matches!(
            reason,
            "cancelled"
                | "source-removed"
                | "disabled"
                | "viewport-changed"
                | "interrupted"
                | "superseded"
        ) {
            return None;
        }
    } else if !reason.is_empty() {
        return None;
    }
    Some(Value::record(vec![
        Value::Bool(true),
        Value::str(phase),
        Value::str(id),
        Value::Number(serial as f64),
        Value::Number(sequence as f64),
        Value::Number(x),
        Value::Number(y),
        Value::str(reason),
        Value::Number(rx),
        Value::Number(ry),
        Value::Number(rw),
        Value::Number(rh),
    ]))
}

const NO_DAY: f64 = -1_000_000.0;

fn record(value: &Value) -> Option<&[Value]> {
    match value {
        Value::Record(fields) => Some(fields),
        _ => None,
    }
}

fn list(value: &Value) -> Option<&[Value]> {
    match value {
        Value::List(items) => Some(items),
        _ => None,
    }
}

fn hit_value(day: f64, x: f64, y: f64, width: f64, height: f64, max_scroll: f64) -> Value {
    Value::record(
        [day, x, y, width, height, max_scroll]
            .into_iter()
            .map(Value::Number)
            .collect(),
    )
}

/// The visible month uses natural, variable week heights. Its scroll container
/// clips dates at the final week, matching `MonthPage` without trailing padding.
/// Args: month, revision, pointer x/y, port x/y/width/height, scrollTop,
/// minimum week height, and the moved event's inclusive endpoint difference.
pub(crate) fn target(page: &Value, args: &[Value]) -> Value {
    target_inner(page, args).unwrap_or_else(|| hit_value(NO_DAY, 0.0, 0.0, 0.0, 0.0, 0.0))
}

fn target_inner(page: &Value, args: &[Value]) -> Option<Value> {
    let number = |at: usize| {
        let value = args.get(at)?.as_number()?;
        value.is_finite().then_some(value)
    };
    let x = number(2)?;
    let y = number(3)?;
    let left = number(4)?;
    let top = number(5)?;
    let width = number(6)?;
    let height = number(7)?;
    let scroll = number(8)?;
    let minimum = number(9)?;
    let span = number(10)?;
    if width <= 0.0 || height <= 0.0 || minimum < 0.0 || span < 0.0 || span.fract() != 0.0 {
        return None;
    }
    let weeks = list(record(page)?.get(3)?)?;
    let rows: Vec<(&[Value], f64)> = weeks
        .iter()
        .map(|week| {
            let fields = record(week)?;
            let lanes = fields.get(3)?.as_number()?;
            Some((fields, minimum.max(34.0 + lanes * 24.0 + 14.0)))
        })
        .collect::<Option<_>>()?;
    let max_scroll = (rows.iter().map(|(_, h)| h).sum::<f64>() - height).max(0.0);
    let outside = || hit_value(NO_DAY, 0.0, 0.0, 0.0, 0.0, max_scroll);
    if x < left || x >= left + width || y < top || y >= top + height {
        return Some(outside());
    }
    let content_y = y - top + scroll;
    let column = ((x - left) / (width / 7.0)).floor() as usize;
    let mut row_top = 0.0;
    for (week, row_height) in rows {
        if content_y >= row_top && content_y < row_top + row_height {
            let day = record(list(week.get(1)?)?.get(column)?)?
                .get(1)?
                .as_number()?;
            let first = f64::from(crate::dates::month_first(crate::dates::FIRST_MONTH));
            let last = f64::from(crate::dates::month_first(crate::dates::LAST_MONTH + 1) - 1);
            if day < first || day + span > last {
                return Some(outside());
            }
            return Some(hit_value(
                day,
                left + column as f64 * width / 7.0,
                top + row_top - scroll,
                width / 7.0,
                row_height,
                max_scroll,
            ));
        }
        row_top += row_height;
    }
    Some(outside())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Json {
        json!({ "v": 1, "phase": "begin", "id": "schedule-🍀", "serial": 7, "seq": 19,
            "x": 103.25, "y": 601.5, "reason": "", "rx": 20.0, "ry": 570.0,
            "rw": 350.0, "rh": 66.0 })
    }

    fn valid(input: &Json) -> bool {
        let Value::Record(fields) = decode(&input.to_string()) else {
            panic!("input decoder must always return a record")
        };
        fields[0] == Value::Bool(true)
    }

    #[test]
    fn root_coordinates_and_source_geometry_keep_fractional_values() {
        let Value::Record(fields) = decode(&sample().to_string()) else {
            panic!("expected record")
        };
        assert_eq!(fields[0], Value::Bool(true));
        assert_eq!(fields[2].as_str(), Some("schedule-🍀"));
        assert_eq!(fields[3], Value::Number(7.0));
        assert_eq!(fields[4], Value::Number(19.0));
        assert_eq!(fields[5], Value::Number(103.25));
        assert_eq!(fields[6], Value::Number(601.5));
        assert_eq!(fields[9], Value::Number(570.0));
        let mut outside = sample();
        outside["phase"] = json!("move");
        outside["x"] = json!(-50.0);
        assert!(valid(&outside), "a finger outside the root still moves");
    }

    #[test]
    fn malformed_messages_cannot_begin_or_complete_a_move() {
        for (key, value) in [
            ("v", json!(2)),
            ("phase", json!("drop")),
            ("id", json!("")),
            ("serial", json!(0)),
            ("serial", json!(1.5)),
            ("serial", json!(MAX_SERIAL + 1)),
            ("seq", json!(0)),
            ("seq", json!(1.5)),
            ("x", json!("NaN")),
            ("y", json!(1e100)),
            ("rw", json!(0)),
            ("rh", json!(-1)),
            ("reason", json!("cancelled")),
        ] {
            let mut input = sample();
            input[key] = value;
            assert!(!valid(&input), "unexpectedly accepted {input}");
        }
        for raw in ["", "{", "null", "[]", "{\"x\":NaN}"] {
            let Value::Record(fields) = decode(raw) else {
                panic!("expected inert record")
            };
            assert_eq!(fields[0], Value::Bool(false));
        }
    }

    #[test]
    fn cancellation_has_an_explicit_reason_and_keeps_contact_identity() {
        let mut input = sample();
        input["phase"] = json!("cancel");
        assert!(!valid(&input));
        for reason in [
            "cancelled",
            "source-removed",
            "disabled",
            "viewport-changed",
            "interrupted",
            "superseded",
        ] {
            input["reason"] = json!(reason);
            assert!(valid(&input));
        }
        input["reason"] = json!("unknown");
        assert!(!valid(&input));
    }

    fn geometry(month: i32, x: f64, y: f64, scroll: f64, span: f64) -> Vec<Value> {
        [
            f64::from(month),
            1.0,
            x,
            y,
            20.0,
            100.0,
            350.0,
            300.0,
            scroll,
            76.0,
            span,
        ]
        .into_iter()
        .map(Value::Number)
        .collect()
    }

    #[test]
    fn hit_testing_uses_variable_week_heights_and_rechecks_a_stationary_pointer() {
        let month = 2026 * 12 + 8;
        let first = crate::dates::month_first(month);
        let events: Vec<_> = (0..10)
            .map(|n| {
                let mut event = crate::model::samples(first)[0].0.clone();
                event.id = format!("dense-{n}");
                event.start = first;
                event.end = first;
                event
            })
            .collect();
        let refs = events.iter().collect::<Vec<_>>();
        let page = crate::layout::month(month, &refs);
        let hit = target(&page, &geometry(month, 145.0, 250.0, 0.0, 0.0));
        assert_eq!(record(&hit).unwrap()[0], Value::Number(f64::from(first)));
        assert_eq!(record(&hit).unwrap()[4], Value::Number(288.0));
        assert_eq!(record(&hit).unwrap()[5], Value::Number(292.0));

        let scrolled = target(&page, &geometry(month, 145.0, 250.0, 150.0, 0.0));
        assert_eq!(
            record(&scrolled).unwrap()[0],
            Value::Number(f64::from(first + 7))
        );
        assert_eq!(record(&scrolled).unwrap()[2], Value::Number(238.0));

        let empty = crate::layout::month(month, &[]);
        let empty_hit = target(&empty, &geometry(month, 145.0, 250.0, 0.0, 0.0));
        assert_eq!(
            record(&empty_hit).unwrap()[0],
            Value::Number(f64::from(first + 7))
        );
    }

    #[test]
    fn clipping_stops_at_the_last_week_and_rejects_dates_outside_the_calendar_range() {
        let month = 2026 * 12 + 8;
        let page = crate::layout::month(month, &[]);
        for (x, y) in [(19.0, 150.0), (370.0, 150.0), (145.0, 99.0), (145.0, 400.0)] {
            let answer = target(&page, &geometry(month, x, y, 0.0, 0.0));
            assert_eq!(record(&answer).unwrap()[0], Value::Number(NO_DAY));
        }
        let first = target(&page, &geometry(month, 145.0, 101.0, 0.0, 0.0));
        let max_scroll = record(&first).unwrap()[5].as_number().unwrap();
        assert_eq!(
            max_scroll, 80.0,
            "five 76 px weeks extend 80 px past the port"
        );
        let bottom = target(&page, &geometry(month, 145.0, 399.0, max_scroll, 0.0));
        assert_eq!(
            record(&bottom).unwrap()[0],
            Value::Number(f64::from(crate::dates::month_first(month) + 28))
        );
        assert_eq!(record(&bottom).unwrap()[2], Value::Number(324.0));
        assert_eq!(record(&bottom).unwrap()[4], Value::Number(76.0));
        let past_last_week = target(
            &page,
            &geometry(month, 145.0, 399.0, max_scroll + 76.0, 0.0),
        );
        assert_eq!(record(&past_last_week).unwrap()[0], Value::Number(NO_DAY));

        let last_month = crate::dates::LAST_MONTH;
        let page = crate::layout::month(last_month, &[]);
        let last = crate::dates::month_first(last_month + 1) - 1;
        let from = crate::dates::month_first(last_month)
            - crate::dates::weekday(crate::dates::month_first(last_month)) as i32;
        let cell = last - from;
        let x = 20.0 + f64::from(cell % 7) * 50.0 + 25.0;
        let scroll = f64::from(cell / 7) * 76.0;
        let fits = target(&page, &geometry(last_month, x, 101.0, scroll, 0.0));
        assert_eq!(record(&fits).unwrap()[0], Value::Number(f64::from(last)));
        let crosses = target(&page, &geometry(last_month, x, 101.0, scroll, 1.0));
        assert_eq!(record(&crosses).unwrap()[0], Value::Number(NO_DAY));
    }
}
