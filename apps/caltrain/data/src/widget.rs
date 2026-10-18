//! The widget refresh uses the app's schedule. The fixture repeats that
//! seeded day on the requested day; render receives only dated props.

use super::{departures, station, Value, DAY_START_MS};

pub(super) fn timeline(now: f64) -> Value {
    const DAY: f64 = 86_400_000.0;
    let today = now.div_euclid(DAY) * DAY;
    let station = station("mv").expect("fixture station");
    let mut trains = Vec::new();
    for day in [today, today + DAY] {
        for direction in ["north", "south"] {
            for departure in departures(station, direction) {
                let Value::Record(fields) = departure else {
                    unreachable!()
                };
                let mut fields = fields.to_vec();
                let Value::Number(at) = fields[4] else {
                    unreachable!()
                };
                let at = at - DAY_START_MS + day;
                fields[4] = Value::Number(at);
                if at >= now {
                    trains.push((at, Value::record(fields)));
                }
            }
        }
    }
    trains.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut dates = vec![now];
    dates.extend(trains.iter().map(|(at, _)| at + 1.0));
    dates.dedup();
    Value::list(
        dates
            .into_iter()
            .take(20)
            .map(|date| {
                let departures = trains
                    .iter()
                    .filter(|(at, _)| *at >= date)
                    .take(2)
                    .map(|(_, train)| train.clone())
                    .collect();
                Value::record(vec![
                    Value::Number(date),
                    Value::record(vec![Value::str(station.name), Value::list(departures)]),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::DataSource;

    #[test]
    fn refresh_returns_dated_props_and_removes_departed_trains() {
        let now = DAY_START_MS + 9.0 * 3_600_000.0;
        let value = super::super::Caltrain
            .query("nextTrains", &[Value::Number(now)])
            .unwrap();
        let Value::List(entries) = value else {
            panic!("timeline")
        };
        assert_eq!(entries.len(), 20);
        let mut previous = now - 1.0;
        for entry in entries.iter() {
            let Value::Record(row) = entry else {
                panic!("entry")
            };
            let Value::Number(date) = row[0] else {
                panic!("date")
            };
            assert!(date > previous);
            previous = date;
            let Value::Record(props) = &row[1] else {
                panic!("props")
            };
            assert_eq!(props[0].as_str(), Some("Mountain View"));
            let Value::List(trains) = &props[1] else {
                panic!("departures")
            };
            assert_eq!(trains.len(), 2);
            for train in trains.iter() {
                let Value::Record(fields) = train else {
                    panic!("departure")
                };
                assert!(matches!(fields[4], Value::Number(at) if at >= date));
            }
        }
    }
}
