//! The Caltrain data source.
//!
//! @ref LLP 1004 D4 (app data logic is a Rust data source)
//!
//! What exact1's `data.ts` did in TypeScript: a seeded corridor model —
//! stations with positions, clock-face departures in both directions — and
//! the queries the app's `resource` declarations name: `stations(location)`,
//! `nearest(location, count)`, `station(id, location)`, `board(stationId,
//! direction)`, `search(query, location)`, `trip(id)`. Values cross the seam
//! as [`Value`]s shaped exactly as the app's `shape` declarations say; the
//! runner refuses anything else.
//!
//! Deterministic: no clock, no randomness. Departure times are minutes past
//! midnight UTC turned into epoch milliseconds on a fixed day, so a countdown
//! is a pure function of the clock the runner supplies.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use exact_runner::{DataError, DataSource, Value};

/// One station on the corridor.
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    /// Short id.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Fare zone.
    pub zone: u8,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Minutes from San Francisco on a local, for schedule generation.
    pub offset_min: f64,
}

/// The corridor, north to south (a slice; the real one has ~31).
pub const STATIONS: &[Station] = &[
    Station {
        id: "sf",
        name: "San Francisco",
        zone: 1,
        lat: 37.7765,
        lon: -122.3947,
        offset_min: 0.0,
    },
    Station {
        id: "22nd",
        name: "22nd Street",
        zone: 1,
        lat: 37.7573,
        lon: -122.3920,
        offset_min: 5.0,
    },
    Station {
        id: "millbrae",
        name: "Millbrae",
        zone: 2,
        lat: 37.6003,
        lon: -122.3868,
        offset_min: 20.0,
    },
    Station {
        id: "sanmateo",
        name: "San Mateo",
        zone: 2,
        lat: 37.5680,
        lon: -122.3240,
        offset_min: 27.0,
    },
    Station {
        id: "redwood",
        name: "Redwood City",
        zone: 3,
        lat: 37.4855,
        lon: -122.2310,
        offset_min: 38.0,
    },
    Station {
        id: "paloalto",
        name: "Palo Alto",
        zone: 3,
        lat: 37.4436,
        lon: -122.1647,
        offset_min: 45.0,
    },
    Station {
        id: "mv",
        name: "Mountain View",
        zone: 4,
        lat: 37.3947,
        lon: -122.0763,
        offset_min: 52.0,
    },
    Station {
        id: "sunnyvale",
        name: "Sunnyvale",
        zone: 4,
        lat: 37.3784,
        lon: -122.0312,
        offset_min: 57.0,
    },
    Station {
        id: "sj",
        name: "San Jose Diridon",
        zone: 4,
        lat: 37.3297,
        lon: -121.9023,
        offset_min: 70.0,
    },
];

/// The fixed day departures are placed on: 2026-08-28 00:00 UTC, in ms.
pub const DAY_START_MS: f64 = 1_787_875_200_000.0;

/// The seeded location when the host has none: Mountain View's platform.
pub const DEFAULT_LOCATION: (f64, f64) = (37.3947, -122.0763);

/// Service tiers, on a clock-face pattern.
const SERVICES: &[(&str, f64, f64)] = &[
    ("Local", 5.0, 1.0),
    ("Limited", 25.0, 0.8),
    ("Express", 45.0, 0.6),
];

fn station(id: &str) -> Option<&'static Station> {
    STATIONS.iter().find(|s| s.id == id)
}

/// Great-circle distance in meters (haversine).
pub fn distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let r = 6_371_000.0;
    let (lat1, lon1) = (a.0.to_radians(), a.1.to_radians());
    let (lat2, lon2) = (b.0.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

fn station_value(s: &Station, location: (f64, f64)) -> Value {
    Value::record(vec![
        Value::str(s.id),
        Value::str(s.name),
        Value::Number(s.zone as f64),
        Value::Number(distance_m(location, (s.lat, s.lon)).round()),
    ])
}

/// Departures from `s` heading `direction` ("north" or "south"), the whole
/// day, ordered by time. A train every N minutes per service tier, offset by
/// the station's position on the line.
fn departures(s: &Station, direction: &str) -> Vec<Value> {
    let mut out = Vec::new();
    // Nothing leaves a terminus in its own direction.
    let terminus = if direction == "north" {
        STATIONS.first()
    } else {
        STATIONS.last()
    };
    if terminus.is_some_and(|t| t.id == s.id) {
        return Vec::new();
    }
    let sign = if direction == "north" { -1.0 } else { 1.0 };
    for (si, (service, every, speed)) in SERVICES.iter().enumerate() {
        let mut minute = 5.0 * 60.0 + si as f64 * 7.0;
        let mut n = 0;
        while minute < 23.0 * 60.0 {
            let at = minute + sign * s.offset_min * speed;
            if (0.0..24.0 * 60.0).contains(&at) {
                let train =
                    100 + si as i64 * 100 + n * 2 + if direction == "north" { 1 } else { 0 };
                let headsign = if direction == "north" {
                    "San Francisco"
                } else {
                    "San Jose"
                };
                out.push((
                    at,
                    Value::record(vec![
                        Value::str(&format!("{}-{}-{train}", s.id, direction)),
                        Value::Number(train as f64),
                        Value::str(service),
                        Value::str(headsign),
                        Value::Number(DAY_START_MS + at * 60_000.0),
                    ]),
                ));
            }
            minute += every * 6.0;
            n += 1;
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out.into_iter().map(|(_, v)| v).collect()
}

/// The data source.
#[derive(Debug, Default)]
pub struct Caltrain;

fn loc(args: &[Value], i: usize) -> Result<(f64, f64), DataError> {
    match args.get(i) {
        Some(Value::Record(f)) if f.len() == 2 => match (&f[0], &f[1]) {
            (Value::Number(lat), Value::Number(lon)) => Ok((*lat, *lon)),
            _ => Err(DataError::BadArguments("location".into())),
        },
        _ => Err(DataError::BadArguments("location".into())),
    }
}

fn text(args: &[Value], i: usize) -> Result<&str, DataError> {
    args.get(i)
        .and_then(Value::as_str)
        .ok_or_else(|| DataError::BadArguments(format!("argument {i}")))
}

impl DataSource for Caltrain {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "defaultLocation" => Ok(Value::record(vec![
                Value::Number(DEFAULT_LOCATION.0),
                Value::Number(DEFAULT_LOCATION.1),
            ])),
            "stations" => {
                let location = loc(args, 0)?;
                Ok(Value::list(
                    STATIONS
                        .iter()
                        .map(|s| station_value(s, location))
                        .collect(),
                ))
            }
            "nearest" => {
                let location = loc(args, 0)?;
                let count = args.get(1).and_then(Value::as_number).unwrap_or(3.0) as usize;
                let mut all: Vec<&Station> = STATIONS.iter().collect();
                all.sort_by(|a, b| {
                    distance_m(location, (a.lat, a.lon))
                        .partial_cmp(&distance_m(location, (b.lat, b.lon)))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Ok(Value::list(
                    all.into_iter()
                        .take(count)
                        .map(|s| station_value(s, location))
                        .collect(),
                ))
            }
            "station" => {
                let id = text(args, 0)?;
                let location = loc(args, 1)?;
                let s =
                    station(id).ok_or_else(|| DataError::Unavailable(format!("station {id}")))?;
                Ok(station_value(s, location))
            }
            "board" => {
                let id = text(args, 0)?;
                let direction = text(args, 1)?;
                if direction != "north" && direction != "south" {
                    return Err(DataError::BadArguments("direction".into()));
                }
                let s =
                    station(id).ok_or_else(|| DataError::Unavailable(format!("station {id}")))?;
                Ok(Value::list(departures(s, direction)))
            }
            "search" => {
                let q = text(args, 0)?.to_lowercase();
                let location = loc(args, 1)?;
                Ok(Value::list(
                    STATIONS
                        .iter()
                        .filter(|s| s.name.to_lowercase().contains(&q))
                        .map(|s| station_value(s, location))
                        .collect(),
                ))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}
