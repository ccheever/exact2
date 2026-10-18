//! Live HTML time presentations and their English fallback. @ref LLP 1126 D6
use crate::{AxisOffer, PropId, TextMeasureRequest, TextMeasurer, TextMetrics, TextRun};
use exact_num::date::{civil, MONTHS, WEEKDAYS};

/// Named presentations of an instant; date and time styles use the viewer's zone.
pub const FORMATS: &[&str] = &[
    "relative",
    "timer",
    "countdown",
    "date-short",
    "date-medium",
    "date-long",
    "date-full",
    "time-short",
    "time-medium",
];

/// A formatter always receives the runner's date, including under the agent.
#[derive(Clone, Copy, Debug)]
pub struct TimeRequest<'a> {
    /// Unix milliseconds of the instant.
    pub datetime: f64,
    /// Unix milliseconds on the runner clock.
    pub now: f64,
    /// One of FORMATS.
    pub format: &'a str,
    /// Viewer BCP 47 locale.
    pub locale: &'a str,
    /// Viewer IANA time zone.
    pub time_zone: &'a str,
    /// Minutes east of UTC, for the English fallback.
    pub utc_offset: f64,
}

/// Contract dates have the Gregorian years 1 through 9999.
pub fn instant(n: f64) -> bool {
    n.is_finite() && (-62_135_596_800_000.0..=253_402_300_799_999.0).contains(&n)
}

/// The displayed duration, in whole seconds. A timer counts either side of
/// its instant; a countdown stops at zero.
pub fn seconds(r: &TimeRequest<'_>) -> u64 {
    let delta = r.datetime - r.now;
    if r.format == "countdown" {
        (delta.max(0.0) / 1000.0).ceil() as u64
    } else {
        (delta.abs() / 1000.0).floor() as u64
    }
}

/// English `Intl.RelativeTimeFormat`'s short, numeric presentation.
pub fn relative(delta: f64) -> (i64, &'static str) {
    let minutes = (delta / 60_000.0).ceil();
    let (divisor, unit) = if minutes.abs() < 60.0 {
        (1.0, "minute")
    } else if minutes.abs() < 1440.0 {
        (60.0, "hour")
    } else if minutes.abs() < 43_200.0 {
        (1440.0, "day")
    } else if minutes.abs() < 525_600.0 {
        (43_200.0, "month")
    } else {
        (525_600.0, "year")
    };
    ((minutes / divisor).round() as i64, unit)
}

/// Duration field shape: minutes/seconds, or the number of hour digits.
pub fn duration_shape(r: &TimeRequest<'_>) -> u32 {
    let hours = seconds(r) / 3600;
    if hours == 0 {
        0
    } else {
        hours.ilog10() + 1
    }
}

/// Next relative value boundary, with a minute as the minimum cadence.
pub fn relative_delay(datetime: f64, now: f64) -> f64 {
    let minutes = ((datetime - now) / 60_000.).ceil();
    let current = relative(datetime - now);
    let mut lo = 1;
    let mut hi = 525_600;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if relative((minutes - f64::from(mid)) * 60_000.) == current {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    60_000. - (now - datetime).rem_euclid(60_000.) + f64::from(lo - 1) * 60_000.
}

/// One English table for Linux and the terminal, and the unsupported-host fallback.
pub fn english(r: &TimeRequest<'_>) -> Option<String> {
    if r.format == "relative-reservation" {
        return Some(
            if r.now > r.datetime {
                "59 min, 59 sec ago"
            } else {
                "in 59 min, 59 sec"
            }
            .into(),
        );
    }
    if !instant(r.datetime) || !FORMATS.contains(&r.format) {
        return None;
    }
    if matches!(r.format, "timer" | "countdown") {
        let s = seconds(r);
        return Some(if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        } else {
            format!("{:02}:{:02}", s / 60, s % 60)
        });
    }
    if r.format == "relative" {
        let (n, unit) = relative(r.datetime - r.now);
        if n == 0 {
            return Some("now".into());
        }
        let unit = match unit {
            "minute" => "min.",
            "hour" => "hr.",
            "month" => "mo.",
            "year" => "yr.",
            _ => {
                if n.abs() == 1 {
                    "day"
                } else {
                    "days"
                }
            }
        };
        return Some(if n < 0 {
            format!("{} {unit} ago", n.abs())
        } else {
            format!("in {n} {unit}")
        });
    }
    let wall = r.datetime + r.utc_offset * 60_000.0;
    let days = (wall / 86_400_000.0).floor() as i64;
    let (year, month, day) = civil(days);
    let name = MONTHS[month as usize - 1];
    Some(match r.format {
        "date-short" => format!("{month}/{day}/{:02}", year % 100),
        "date-medium" => format!("{} {day}, {year}", &name[..3]),
        "date-long" => format!("{name} {day}, {year}"),
        "date-full" => format!(
            "{}, {name} {day}, {year}",
            WEEKDAYS[(days + 4).rem_euclid(7) as usize]
        ),
        _ => {
            let s = (wall.rem_euclid(86_400_000.0) / 1000.0) as u64;
            let h = s / 3600;
            let period = if h < 12 { "AM" } else { "PM" };
            let h = if h.is_multiple_of(12) { 12 } else { h % 12 };
            if r.format == "time-short" {
                format!("{h}:{:02} {period}", s / 60 % 60)
            } else {
                format!("{h}:{:02}:{:02} {period}", s / 60 % 60, s % 60)
            }
        }
    })
}

/// Candidate strings are measured separately, at the element's font. Keeping
/// both directions and two units also fits SwiftUI's archived relative text.
pub fn reservation(
    r: &TimeRequest<'_>,
    mut format: impl FnMut(&TimeRequest<'_>) -> Option<String>,
) -> String {
    if r.format == "relative" {
        let mut values = Vec::new();
        for sign in [-1., 1.] {
            let q = TimeRequest {
                datetime: 0.,
                now: sign * 3_599_000.,
                format: "relative-reservation",
                ..*r
            };
            if let Some(v) = format(&q).or_else(|| english(&q)) {
                values.push(v);
            }
        }
        for minutes in [59., 23. * 60., 29. * 1440., 11. * 43_200., 9998. * 525_600.] {
            for sign in [-1., 1.] {
                let q = TimeRequest {
                    datetime: 0.,
                    now: sign * minutes * 60_000.,
                    ..*r
                };
                if let Some(v) = format(&q) {
                    values.push(v);
                }
            }
        }
        return values.join("\n");
    }
    if matches!(r.format, "timer" | "countdown") {
        let mut values = Vec::new();
        // An archived timer must fit the next hour-field shape without a runner.
        let model = if r.format == "timer" {
            format(&TimeRequest {
                datetime: 0.,
                now: 3_600_000. * 10_f64.powi(duration_shape(r) as i32),
                ..*r
            })
        } else {
            format(r)
        };
        if let Some(model) = model {
            let digits: Vec<_> = (0..=9)
                .filter_map(|d| {
                    format(&TimeRequest {
                        datetime: 0.,
                        now: f64::from(d) * 1000.,
                        format: "timer",
                        ..*r
                    })
                    .and_then(|s| s.chars().rev().find(|c| c.is_numeric()))
                })
                .collect();
            if digits.len() == 10
                && digits
                    .iter()
                    .enumerate()
                    .all(|(i, d)| !digits[..i].contains(d))
            {
                for digit in &digits {
                    values.push(
                        model
                            .chars()
                            .map(|c| if digits.contains(&c) { *digit } else { c })
                            .collect(),
                    );
                }
            }
            values.push(model);
        }
        return values.join("\n");
    }
    format(r).unwrap_or_default()
}

/// Grow an intrinsic text offer to the widest reserved candidate; the
/// candidate is never painted or used as the paragraph's line-break source.
pub(crate) fn reserve(
    arena: &crate::arena::NodeArena,
    slot: u32,
    m: &mut dyn TextMeasurer,
    metrics: &mut TextMetrics,
) {
    if arena.is_inline_run(slot) {
        return;
    }
    let Some(values) = arena.props(slot).str(PropId::TextReservation) else {
        return;
    };
    let mut actual = Vec::new();
    arena.text_runs(slot, &mut actual);
    let style = actual.first().map_or_else(
        || crate::TextStyle::from_style(arena.style(slot)),
        |run| run.style,
    );
    for value in values.lines() {
        let shown = arena.reservation_text(slot, value);
        let runs = [TextRun { text: shown, style }];
        let request = TextMeasureRequest {
            runs: &runs,
            paragraph: arena.paragraph(slot),
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        };
        let Some(candidate) = m.measure_reservation(&request) else {
            continue;
        };
        metrics.width = metrics.width.max(candidate.width);
        metrics.height = metrics.height.max(candidate.height);
    }
}
