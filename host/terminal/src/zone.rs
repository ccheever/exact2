//! Interactive launch samples the machine; headless launch uses the agent facts.
use chrono::{Offset, TimeZone};
use exact_runner::time::{Launch, Place, WallTime};
use std::time::{SystemTime, UNIX_EPOCH};

const AGENT_EPOCH: f64 = 1_767_225_600_000.;

fn locale(value: &str) -> String {
    let tag = value.split(['.', '@']).next().unwrap_or("");
    if tag.is_empty() || matches!(tag, "C" | "POSIX") {
        "en-US".into()
    } else {
        tag.replace('_', "-")
    }
}

pub(crate) fn launch(
    interactive: bool,
    env: impl Fn(&str) -> Option<String>,
) -> (Launch, Vec<String>) {
    let mut notes = Vec::new();
    let mut place = if interactive {
        Place {
            locale: locale(
                &["LC_ALL", "LC_TIME", "LANG"]
                    .iter()
                    .find_map(|key| env(key).filter(|s| !s.is_empty()))
                    .unwrap_or_default(),
            ),
            time_zone: env("TZ")
                .or_else(|| iana_time_zone::get_timezone().ok())
                .unwrap_or_else(|| "UTC".into()),
            seed: 0.,
        }
    } else {
        Place {
            locale: env("EXACT_AGENT_LOCALE").unwrap_or_else(|| "en-US".into()),
            time_zone: env("EXACT_AGENT_TIME_ZONE").unwrap_or_else(|| "UTC".into()),
            seed: 1.,
        }
    };
    let zone = match (place.validate(), place.time_zone.parse::<chrono_tz::Tz>()) {
        (Ok(()), Ok(zone)) => zone,
        _ => {
            notes.push(format!(
                "terminal place refused: {}/{}; using en-US/UTC",
                place.locale, place.time_zone
            ));
            place = Place {
                seed: place.seed,
                ..Place::default()
            };
            chrono_tz::UTC
        }
    };
    let epoch = if interactive {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64
    } else {
        env("EXACT_AGENT_EPOCH").map_or(AGENT_EPOCH, |s| {
            s.parse::<u64>().map_or(f64::NAN, |n| n as f64)
        })
    };
    let offset = zone
        .timestamp_millis_opt(epoch as i64)
        .single()
        .map_or(0., |at| {
            f64::from(at.offset().fix().local_minus_utc()) / 60.
        });
    let mut time = WallTime {
        epoch_at_zero: epoch,
        utc_offset: offset,
    };
    if time.validate().is_err() {
        notes.push("terminal time refused; using the agent epoch in UTC".into());
        time = WallTime {
            epoch_at_zero: AGENT_EPOCH,
            utc_offset: 0.,
        };
        place.time_zone = "UTC".into();
    }
    (
        Launch {
            time: Some(time),
            place: Some(place),
        },
        notes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_locales_drop_codesets_and_modifiers() {
        for (value, expected) in [
            ("sr_RS@latin", "sr-RS"),
            ("ca_ES@valencia", "ca-ES"),
            ("de_DE.UTF-8@euro", "de-DE"),
            ("C@calendar=gregorian", "en-US"),
            ("POSIX", "en-US"),
        ] {
            assert_eq!(locale(value), expected);
        }
    }

    #[test]
    fn headless_facts_use_the_agent_epoch_and_zone_without_reading_the_machine() {
        let (facts, notes) = launch(false, |key| match key {
            "EXACT_AGENT_EPOCH" => Some("1790000000000".into()),
            "EXACT_AGENT_TIME_ZONE" => Some("America/Toronto".into()),
            "EXACT_AGENT_LOCALE" => Some("fr-CA".into()),
            _ => panic!("headless launch read {key}"),
        });
        assert!(notes.is_empty());
        let time = facts.time.unwrap();
        assert_eq!(
            (time.epoch_at_zero, time.utc_offset),
            (1_790_000_000_000., -240.)
        );
        assert_eq!(facts.place.unwrap().locale, "fr-CA");
        let (default, _) = launch(false, |_| None);
        assert_eq!(default.time.unwrap().epoch_at_zero, AGENT_EPOCH);
    }

    #[test]
    fn refused_places_and_times_fall_back_to_valid_facts_with_a_note() {
        for (key, value) in [
            ("EXACT_AGENT_LOCALE", "bad@locale"),
            ("EXACT_AGENT_TIME_ZONE", "Bad/Zone"),
            ("EXACT_AGENT_EPOCH", "not a date"),
        ] {
            let (facts, notes) = launch(false, |k| (k == key).then(|| value.into()));
            assert_eq!(notes.len(), 1);
            assert!(facts.place.unwrap().validate().is_ok());
            assert!(facts.time.unwrap().validate().is_ok());
        }
    }
}
