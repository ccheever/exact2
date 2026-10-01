//! Calendar labels for the session list (local_time.rs, #269): which day group a session belongs to
//! and the short time beside it. The labels follow the Mac's time zone, so the
//! split between "Today" and "Yesterday" falls at local midnight.

/// One instant broken down in local time.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    /// Days since 1970-01-01 in local time; consecutive calendar days differ by one.
    pub day: i64,
    pub year: i32,
    /// 1–12.
    pub month: u32,
    pub month_day: u32,
    /// 0 is Sunday.
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
}

/// `seconds` (Unix time) in the local time zone, `offset` seconds east of
/// UTC. Upstream asks `localtime_r` for each instant; the data source has no
/// time zone database (it also runs as wasm), so the host sends its current
/// offset with every reply and an instant across a DST change from now is
/// an hour off.
pub fn local(seconds: f64, offset: f64) -> LocalTime {
    utc(seconds + offset)
}

/// The same breakdown in UTC; also what tests use so they do not depend on the
/// machine's time zone.
pub fn utc(seconds: f64) -> LocalTime {
    let whole = seconds.floor() as i64;
    let day = whole.div_euclid(86_400);
    let rem = whole.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days, as in `backend::rfc3339`.
    let z = day + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month_day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as i32;
    LocalTime {
        day,
        year,
        month,
        month_day,
        // 1970-01-01 was a Thursday.
        weekday: (day + 4).rem_euclid(7) as u32,
        hour: (rem / 3_600) as u32,
        minute: (rem % 3_600 / 60) as u32,
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// The day group a session last active at `then` falls under.
pub fn day_group(now: LocalTime, then: LocalTime) -> String {
    match now.day - then.day {
        i64::MIN..=0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => "Previous 7 days".into(),
        _ if then.year == now.year => MONTHS[then.month as usize - 1].into(),
        _ => format!("{} {}", MONTHS[then.month as usize - 1], then.year),
    }
}

/// The short time beside a row: minutes or hours today, the clock time
/// yesterday, the weekday this week, and the date before that.
pub fn activity_label(
    now_seconds: f64,
    then_seconds: f64,
    now: LocalTime,
    then: LocalTime,
) -> String {
    let elapsed = (now_seconds - then_seconds).max(0.0);
    match now.day - then.day {
        i64::MIN..=0 if elapsed < 60.0 => "now".into(),
        i64::MIN..=0 if elapsed < 3_600.0 => format!("{}m", (elapsed / 60.0) as u64),
        i64::MIN..=0 => format!("{}h", (elapsed / 3_600.0) as u64),
        1 => {
            let hour = match then.hour % 12 {
                0 => 12,
                hour => hour,
            };
            let half = if then.hour < 12 { "AM" } else { "PM" };
            format!("{hour}:{:02} {half}", then.minute)
        }
        2..=6 => WEEKDAYS[then.weekday as usize].into(),
        _ if then.year == now.year => {
            format!(
                "{} {}",
                &MONTHS[then.month as usize - 1][..3],
                then.month_day
            )
        }
        _ => format!(
            "{} {}, {}",
            &MONTHS[then.month as usize - 1][..3],
            then.month_day,
            then.year
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::epoch_seconds;

    fn at(stamp: &str) -> (f64, LocalTime) {
        let seconds = epoch_seconds(stamp);
        (seconds, utc(seconds))
    }

    #[test]
    fn utc_breakdown_matches_the_calendar() {
        let (_, t) = at("2026-10-01T18:05:00Z");
        assert_eq!((t.year, t.month, t.month_day), (2026, 10, 1));
        assert_eq!(
            (t.hour, t.minute, WEEKDAYS[t.weekday as usize]),
            (18, 5, "Thu")
        );
        let (_, t) = at("2024-02-29T00:00:00Z");
        assert_eq!((t.year, t.month, t.month_day), (2024, 2, 29));
    }

    #[test]
    fn groups_follow_calendar_days() {
        let (_, now) = at("2026-10-01T09:00:00Z");
        let group = |stamp| day_group(now, at(stamp).1);
        assert_eq!(group("2026-10-01T00:00:01Z"), "Today");
        assert_eq!(group("2026-09-30T23:59:00Z"), "Yesterday");
        assert_eq!(group("2026-09-25T12:00:00Z"), "Previous 7 days");
        assert_eq!(group("2026-09-24T12:00:00Z"), "September");
        assert_eq!(group("2025-12-24T12:00:00Z"), "December 2025");
        // A clock a little ahead on a remote machine still reads as today.
        assert_eq!(group("2026-10-01T09:00:30Z"), "Today");
    }

    #[test]
    fn labels_shorten_with_age() {
        let (now_s, now) = at("2026-10-01T09:00:00Z");
        let label = |stamp| {
            let (s, t) = at(stamp);
            activity_label(now_s, s, now, t)
        };
        assert_eq!(label("2026-10-01T08:59:30Z"), "now");
        assert_eq!(label("2026-10-01T08:48:00Z"), "12m");
        assert_eq!(label("2026-10-01T06:00:00Z"), "3h");
        assert_eq!(label("2026-09-30T18:40:00Z"), "6:40 PM");
        assert_eq!(label("2026-09-30T00:15:00Z"), "12:15 AM");
        assert_eq!(label("2026-09-28T12:00:00Z"), "Mon");
        assert_eq!(label("2026-09-24T12:00:00Z"), "Sep 24");
        assert_eq!(label("2025-12-24T12:00:00Z"), "Dec 24, 2025");
    }
}
