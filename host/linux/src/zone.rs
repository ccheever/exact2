//! Locale, IANA zone, launch seed and UTC offset (LLP 1027.000.000).
#![allow(unsafe_code)]

/// The local zone's current UTC offset in minutes east, from the C
/// library's zone database (`TZ`, else `/etc/localtime`); zero without one.
#[cfg(unix)]
pub fn local_offset_minutes() -> f64 {
    // SAFETY: `time(NULL)` reads the clock; `localtime_r` writes only the
    // `tm` it is given and returns null on failure.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return 0.0;
        }
        tm.tm_gmtoff as f64 / 60.0
    }
}

#[cfg(windows)]
pub fn local_offset_minutes() -> f64 {
    chrono::Local::now().offset().local_minus_utc() as f64 / 60.0
}

/// Under the agent, the date at the clock's zero (`EXACT_AGENT_EPOCH`, Unix
/// milliseconds; default 2026-01-01T00:00:00Z) and the agent zone's offset
/// at that instant; `None` reads the machine (LLP 1027.000.000 D3).
pub fn agent_time(
    env: impl Fn(&str) -> Option<String>,
    zone: &str,
) -> Result<Option<(f64, f64)>, String> {
    if env("EXACT_AGENT").as_deref() != Some("1") {
        return Ok(None);
    }
    let epoch = env("EXACT_AGENT_EPOCH").map_or(Ok(1_767_225_600_000.0), |s| {
        s.parse::<u64>()
            .map(|ms| ms as f64)
            .map_err(|e| format!("EXACT_AGENT_EPOCH: {e}"))
    })?;
    Ok(Some((epoch, offset_minutes_at(zone, epoch))))
}

/// A zone's UTC offset at a Unix instant, from the C library's database.
/// The agent's process adopts the agent zone as its `TZ` to ask.
#[cfg(unix)]
pub(crate) fn offset_minutes_at(zone: &str, epoch_ms: f64) -> f64 {
    if matches!(zone, "UTC" | "Etc/UTC") {
        return 0.0;
    }
    if std::env::var("TZ").ok().as_deref() != Some(zone) {
        std::env::set_var("TZ", zone);
    }
    // SAFETY: `tzset` rereads `TZ`; `localtime_r` writes only the `tm` it
    // is given and returns null on failure.
    unsafe {
        extern "C" {
            fn tzset();
        }
        tzset();
        let at = (epoch_ms / 1000.0).floor() as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&at, &mut tm).is_null() {
            return 0.0;
        }
        tm.tm_gmtoff as f64 / 60.0
    }
}

#[cfg(windows)]
pub(crate) fn offset_minutes_at(zone: &str, epoch_ms: f64) -> f64 {
    use chrono::{Offset, TimeZone};
    let Ok(zone) = zone.parse::<chrono_tz::Tz>() else {
        return 0.0;
    };
    zone.timestamp_millis_opt(epoch_ms as i64)
        .single()
        .map_or(0.0, |at| at.offset().fix().local_minus_utc() as f64 / 60.0)
}

/// Place and entropy are sampled once at launch. Agent mode bypasses every
/// platform input, including the system's entropy source.
pub fn launch_place(
    env: impl Fn(&str) -> Option<String>,
) -> Result<exact_runner::time::Place, String> {
    let place = if env("EXACT_AGENT").as_deref() == Some("1") {
        exact_runner::time::Place {
            locale: env("EXACT_AGENT_LOCALE").unwrap_or_else(|| "en-US".into()),
            time_zone: env("EXACT_AGENT_TIME_ZONE").unwrap_or_else(|| "UTC".into()),
            seed: agent_seed(env("EXACT_AGENT_SEED"))?,
            format_locale: env("EXACT_AGENT_FORMAT_LOCALE").unwrap_or_default(),
        }
    } else {
        #[cfg(windows)]
        crate::com::ensure_mta().map_err(|error| format!("launch COM: {error}"))?;
        // Below 2^32 (LLP 1116.005 D5): four bytes of entropy.
        let mut bytes = [0; 4];
        getrandom::fill(&mut bytes).map_err(|e| format!("launch seed: {e}"))?;
        exact_runner::time::Place {
            locale: locale(&env, "LC_MESSAGES"),
            time_zone: env("TZ")
                .and_then(|zone| zone_name(&zone))
                .or_else(|| iana_time_zone::get_timezone().ok())
                .unwrap_or_else(|| "UTC".into()),
            seed: f64::from(u32::from_ne_bytes(bytes)),
            // POSIX's category for dates and times (`LC_TIME`) beside the
            // messages' language; none of its variables set, none apart
            // from the locale.
            format_locale: if ["LC_ALL", "LC_TIME", "LANG"]
                .iter()
                .any(|key| env(key).is_some_and(|s| !s.is_empty()))
            {
                locale(&env, "LC_TIME")
            } else {
                Default::default()
            },
        }
    };
    place
        .validate()
        .map_err(|e| format!("launch place: {e:?}"))?;
    Ok(place)
}

/// The drive's seed as given (default 1), refused unless it is a whole
/// number below 2^32, the range every host draws (LLP 1116.005 D5).
fn agent_seed(value: Option<String>) -> Result<f64, String> {
    let Some(value) = value else { return Ok(1.0) };
    value.parse::<u32>().map(f64::from).map_err(|_| {
        format!(
            "EXACT_AGENT_SEED: a whole number from 0 through 2^32 - 1 (4294967295), not {value:?}"
        )
    })
}

/// `category`'s locale as BCP 47: `LC_ALL`, else the category, else `LANG`.
fn locale(env: impl Fn(&str) -> Option<String>, category: &str) -> String {
    let value = ["LC_ALL", category, "LANG"]
        .into_iter()
        .find_map(|key| env(key).filter(|s| !s.is_empty()))
        .unwrap_or_default();
    let tag = value.split(['.', '@']).next().unwrap_or("");
    if tag.is_empty() || matches!(tag, "C" | "POSIX") {
        return "en-US".into();
    }
    let mut parts = tag.split('_');
    let language = parts.next().unwrap().to_ascii_lowercase();
    let region = parts.next().map(str::to_ascii_uppercase);
    let script = match value.split_once('@').map(|(_, modifier)| modifier) {
        Some("latin") => Some("Latn"),
        Some("cyrillic") => Some("Cyrl"),
        _ => None,
    };
    [Some(language.as_str()), script, region.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("-")
}

fn zone_name(value: &str) -> Option<String> {
    let name = value.strip_prefix(':').unwrap_or(value);
    let name = name.split_once("zoneinfo/").map_or(name, |(_, name)| name);
    let name = name
        .strip_prefix("posix/")
        .or_else(|| name.strip_prefix("right/"))
        .unwrap_or(name);
    if name.split('/').any(|part| matches!(part, "" | ".." | ".")) {
        return None;
    }
    // A POSIX rule such as PST8PDT,M3.2.0,M11.1.0 is not an IANA name.
    #[cfg(unix)]
    return std::fs::metadata(std::path::Path::new("/usr/share/zoneinfo").join(name))
        .ok()
        .filter(|meta| meta.is_file())
        .map(|_| name.into());
    #[cfg(windows)]
    name.parse::<chrono_tz::Tz>()
        .ok()
        .map(|zone| zone.name().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_environment_precedence_and_posix_normalization() {
        for (all, messages, lang, expected) in [
            ("fr_CA.UTF-8", "de_DE.UTF-8", "en_US.UTF-8", "fr-CA"),
            ("", "de_DE.UTF-8", "en_US.UTF-8", "de-DE"),
            ("", "", "en_US.UTF-8", "en-US"),
            ("C.UTF-8", "fr_FR", "en_US", "en-US"),
            ("POSIX", "", "", "en-US"),
            ("", "", "sr_RS.UTF-8@latin", "sr-Latn-RS"),
            ("", "", "", "en-US"),
        ] {
            assert_eq!(
                locale(
                    |key| Some(
                        match key {
                            "LC_ALL" => all,
                            "LC_MESSAGES" => messages,
                            "LANG" => lang,
                            _ => unreachable!(),
                        }
                        .into()
                    ),
                    "LC_MESSAGES"
                ),
                expected
            );
        }
    }

    #[test]
    fn the_format_locale_is_lc_time() {
        let place = launch_place(|key| match key {
            "LC_MESSAGES" | "LANG" => Some("en_US.UTF-8".into()),
            "LC_TIME" => Some("ko_KR.UTF-8".into()),
            "TZ" => Some("Asia/Seoul".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            (place.locale.as_str(), place.format_locale.as_str()),
            ("en-US", "ko-KR")
        );
        // With no LC_ALL, LC_TIME or LANG (an Android process), none apart
        // from the locale, which the runner then answers.
        let bare = launch_place(|key| (key == "LC_MESSAGES").then(|| "ko_KR".into())).unwrap();
        assert_eq!(
            (bare.locale.as_str(), bare.format_locale.as_str()),
            ("ko-KR", "")
        );
        let agent = launch_place(|key| match key {
            "EXACT_AGENT" => Some("1".into()),
            "EXACT_AGENT_FORMAT_LOCALE" => Some("en-KR".into()),
            key if key.starts_with("EXACT_AGENT_") => None,
            _ => panic!("agent read the machine's {key}"),
        })
        .unwrap();
        assert_eq!(
            (agent.locale.as_str(), agent.format_locale.as_str()),
            ("en-US", "en-KR")
        );
    }

    #[test]
    fn agent_place_defaults_and_overrides_ignore_the_machine() {
        let defaults = launch_place(|key| match key {
            "EXACT_AGENT" => Some("1".into()),
            key if key.starts_with("EXACT_AGENT_") => None,
            _ => panic!("agent read the machine's {key}"),
        })
        .unwrap();
        assert_eq!(
            defaults,
            exact_runner::time::Place {
                seed: 1.0,
                ..exact_runner::time::Place::default()
            }
        );
        let custom = launch_place(|key| {
            Some(
                match key {
                    "EXACT_AGENT" => "1",
                    "EXACT_AGENT_LOCALE" => "fr-CA",
                    "EXACT_AGENT_TIME_ZONE" => "America/Toronto",
                    "EXACT_AGENT_SEED" => "4294967295",
                    "EXACT_AGENT_FORMAT_LOCALE" => "fr-FR",
                    _ => panic!("agent read the machine's {key}"),
                }
                .into(),
            )
        })
        .unwrap();
        assert_eq!(custom.locale, "fr-CA");
        assert_eq!(custom.time_zone, "America/Toronto");
        assert_eq!(custom.seed, 4_294_967_295.0);
        assert_eq!(custom.format_locale, "fr-FR");
        // A seed past 2^32 is refused, not wrapped (LLP 1116.005 D5).
        for bad in ["4294967296", "9007199254740991", "-1", "1.5", "seven"] {
            let refused = launch_place(|key| match key {
                "EXACT_AGENT" => Some("1".into()),
                "EXACT_AGENT_SEED" => Some(bad.into()),
                _ => None,
            })
            .unwrap_err();
            assert!(refused.contains("0 through 2^32 - 1"), "{bad}: {refused}");
        }
        let agent = |key: &str| (key == "EXACT_AGENT").then(|| "1".into());
        assert_eq!(
            agent_time(agent, "UTC").unwrap(),
            Some((1_767_225_600_000.0, 0.0))
        );
        let at = |key: &str| match key {
            "EXACT_AGENT" => Some("1".into()),
            "EXACT_AGENT_EPOCH" => Some("1790000000000".into()),
            _ => None,
        };
        // 2026-09-21, daylight time in Toronto.
        assert_eq!(
            agent_time(at, "America/Toronto").unwrap(),
            Some((1_790_000_000_000.0, -240.0))
        );
        assert_eq!(agent_time(|_| None, "UTC").unwrap(), None);
    }

    #[test]
    fn ordinary_launch_uses_the_locale_zone_database_and_fresh_entropy() {
        let env = |key: &str| match key {
            "LANG" => Some("fr_FR.UTF-8".into()),
            "TZ" => Some(":/usr/share/zoneinfo/Europe/Paris".into()),
            _ => None,
        };
        let a = launch_place(env).unwrap();
        let b = launch_place(env).unwrap();
        assert_eq!(a.locale, "fr-FR");
        assert_eq!(a.time_zone, "Europe/Paris");
        assert_ne!(a.seed, b.seed);
        assert!(a.validate().is_ok() && b.validate().is_ok());
        // The drawn seed is a whole number below 2^32 (LLP 1116.005 D5).
        for _ in 0..64 {
            let seed = launch_place(env).unwrap().seed;
            assert!(
                seed < exact_runner::time::SEED_LIMIT && seed.fract() == 0.0,
                "{seed}"
            );
        }
        assert_eq!(zone_name("PST8PDT,M3.2.0,M11.1.0"), None);
    }
}
