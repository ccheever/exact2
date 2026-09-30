//! Month geometry prepared ahead of scrolling; the Contract owns every view.

use crate::{dates, model::Schedule};
use exact_plan::Value;
use std::collections::{BTreeMap, HashMap};

pub(crate) fn month(month: i32, events: &[&Schedule], stickers: &BTreeMap<i32, String>) -> Value {
    let first = dates::month_first(month);
    let last = dates::month_first(month + 1) - 1;
    let from = first - dates::weekday(first) as i32;
    let weeks = (last - from + 7) / 7;
    let mut previous_lanes: HashMap<&str, usize> = HashMap::new();
    let rows = (0..weeks)
        .map(|week| {
            let start = from + week * 7;
            let cells = (0..7)
                .map(|column| {
                    let day = start + column;
                    let date = dates::iso(day);
                    Value::record(vec![
                        Value::str(&date),
                        Value::Number(day.into()),
                        Value::str(&date),
                        Value::Number(dates::civil(day).2.into()),
                        Value::Bool(first <= day && day <= last),
                        Value::str(stickers.get(&day).map_or("", String::as_str)),
                    ])
                })
                .collect();
            let mut touching: Vec<_> = events
                .iter()
                .copied()
                .filter(|e| e.start <= start + 6 && e.last_day() >= start)
                .collect();
            touching.sort_by(|a, b| {
                (
                    a.kind,
                    a.start,
                    std::cmp::Reverse(a.last_day()),
                    !a.all_day,
                    a.start_time,
                    &a.id,
                )
                    .cmp(&(
                        b.kind,
                        b.start,
                        std::cmp::Reverse(b.last_day()),
                        !b.all_day,
                        b.start_time,
                        &b.id,
                    ))
            });
            let mut occupied: Vec<u8> = Vec::new();
            let mut lanes = HashMap::new();
            let bars = touching
                .into_iter()
                .map(|event| {
                    let left = event.start.max(start) - start;
                    let span = event.last_day().min(start + 6) - (start + left) + 1;
                    let mask = (((1_u16 << span) - 1) << left) as u8;
                    let kept = previous_lanes
                        .get(event.id.as_str())
                        .copied()
                        .filter(|&lane| occupied.get(lane).copied().unwrap_or(0) & mask == 0);
                    let lane = kept
                        .or_else(|| occupied.iter().position(|used| used & mask == 0))
                        .unwrap_or(occupied.len());
                    occupied.resize(occupied.len().max(lane + 1), 0);
                    occupied[lane] |= mask;
                    lanes.insert(event.id.as_str(), lane);
                    Value::record(vec![
                        Value::str(&format!("{}-{start}", event.id)),
                        Value::str(&event.id),
                        Value::str(&event.title),
                        Value::str(&event.color),
                        Value::Number(left.into()),
                        Value::Number(span.into()),
                        Value::Number(lane as f64),
                        Value::Bool(event.start < start),
                        Value::Bool(event.last_day() > start + 6),
                        Value::Bool(event.all_day),
                        Value::str(&event.time_label()),
                        Value::str(event.kind.name()),
                        Value::Bool(event.completed),
                    ])
                })
                .collect();
            previous_lanes = lanes;
            Value::record(vec![
                Value::str(&dates::iso(start)),
                Value::list(cells),
                Value::list(bars),
                Value::Number(occupied.len() as f64),
                Value::Number(if stickers.range(start..=start + 6).next().is_some() {
                    40.0
                } else {
                    14.0
                }),
            ])
        })
        .collect();
    Value::record(vec![
        Value::str(&dates::month_id(month)),
        Value::Number(month.into()),
        Value::str(&dates::month_label(month)),
        Value::list(rows),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    fn fields(value: &Value) -> &[Value] {
        let Value::Record(fields) = value else {
            panic!("record")
        };
        fields
    }
    fn list(value: &Value) -> &[Value] {
        let Value::List(items) = value else {
            panic!("list")
        };
        items
    }
    fn num(value: &Value) -> i32 {
        value.as_number().unwrap() as i32
    }

    #[test]
    fn month_rows_have_all_days_and_overlapping_bars_never_share_a_lane() {
        let m = 2026 * 12 + 8;
        let base = dates::month_first(m);
        let seed = model::samples(base)[0].0.clone();
        let events: Vec<_> = (0..300)
            .map(|i| Schedule {
                id: format!("event-{i}"),
                all_day: true,
                start: base + i % 30,
                end: base + i % 30 + i % 12,
                ..seed.clone()
            })
            .collect();
        let refs: Vec<_> = events.iter().collect();
        let page = month(m, &refs, &BTreeMap::new());
        let mut in_month = 0;
        let mut covered = HashMap::<String, usize>::new();
        for week in list(&fields(&page)[3]) {
            let f = fields(week);
            assert_eq!(list(&f[1]).len(), 7);
            for cell in list(&f[1]) {
                in_month += usize::from(fields(cell)[4] == Value::Bool(true));
            }
            let mut masks = HashMap::<i32, u16>::new();
            for bar in list(&f[2]) {
                let b = fields(bar);
                let col = num(&b[4]);
                let span = num(&b[5]);
                assert!(col >= 0 && span > 0 && col + span <= 7);
                let mask = ((1_u16 << span) - 1) << col;
                let occupied = masks.entry(num(&b[6])).or_default();
                assert_eq!(*occupied & mask, 0);
                *occupied |= mask;
                *covered
                    .entry(b[1].as_str().unwrap().to_owned())
                    .or_default() += span as usize;
            }
        }
        assert_eq!(in_month, 30);
        let visible_end = base + 32; // This September's final row ends October 3.
        for event in &events {
            assert_eq!(
                covered[&event.id],
                (event.last_day().min(visible_end) - event.start + 1) as usize
            );
        }
    }

    #[test]
    fn four_five_and_six_week_months_fit_their_actual_dates() {
        for (month, weeks) in [(2026 * 12 + 1, 4), (2026 * 12 + 8, 5), (2026 * 12 + 7, 6)] {
            assert_eq!(
                list(&fields(&self::month(month, &[], &BTreeMap::new()))[3]).len(),
                weeks
            );
        }
    }
}
