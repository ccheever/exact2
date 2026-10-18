//! Live HTML time presentations and their English fallback. @ref LLP 1126 D6
use crate::{AxisOffer, PropId, TextMeasureRequest, TextMeasurer, TextMetrics, TextRun};

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
            let base = if r.format == "time-short" {
                format!("{h}:{:02} {period}", s / 60 % 60)
            } else {
                format!("{h}:{:02}:{:02} {period}", s / 60 % 60, s % 60)
            };
            base
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
            if let Some(v) = format(&q) {
                values.push(v);
            }
        }
        for amount in [
            59.,
            3599.,
            86399.,
            2_591_999.,
            31_535_999.,
            315_359_999_999.,
        ] {
            for sign in [-1., 1.] {
                let q = TimeRequest {
                    datetime: 0.,
                    now: sign * amount * 1000.,
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
        let mut values: Vec<_> = (0..=9)
            .map(|n| format!("{n}{n}{n}{n}{n}{n}{n}{n}{n}{n}:{n}{n}:{n}{n}"))
            .collect();
        let q = TimeRequest {
            datetime: 0.,
            now: 7_199_999_999_955_000.,
            format: "timer",
            ..*r
        };
        if let Some(model) = format(&q) {
            let digits: Vec<_> = (0..=9)
                .filter_map(|d| {
                    format(&TimeRequest {
                        now: f64::from(d) * 1000.,
                        ..q
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
        let runs = [TextRun {
            text: value.into(),
            style,
        }];
        let request = TextMeasureRequest {
            runs: &runs,
            paragraph: arena.paragraph(slot),
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        };
        let candidate = m.measure(&request);
        metrics.width = metrics.width.max(candidate.width);
        metrics.height = metrics.height.max(candidate.height);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Padding {
    pub(crate) text: String,
    pub(crate) spacing: f32,
}

// Inline time is an atomic run followed by invisible advance. Both measuring
// and painting read these same runs; only the time's glyphs change on a tick.
pub(crate) fn prepare_padding(arena: &mut crate::arena::NodeArena, m: &mut dyn TextMeasurer) {
    let slots: Vec<_> = arena.time_slots.iter().collect();
    for slot in slots {
        arena.time_padding.remove(&slot);
        if !arena.is_inline_run(slot) {
            continue;
        }
        let mut actual = Vec::new();
        arena.text_runs(slot, &mut actual);
        let Some(run) = actual.first() else {
            continue;
        };
        let style = run.style;
        let measure = |m: &mut dyn TextMeasurer, runs: &[TextRun<'_>]| {
            m.measure(&TextMeasureRequest {
                runs,
                paragraph: arena.paragraph(slot),
                width: AxisOffer::MaxContent,
                height: AxisOffer::MaxContent,
                exclusions: &[],
            })
            .width
        };
        let current = measure(m, &actual);
        let mut widest = current;
        for value in arena
            .props(slot)
            .str(PropId::TextReservation)
            .unwrap_or("")
            .lines()
        {
            widest = widest.max(measure(
                m,
                &[TextRun {
                    text: value.into(),
                    style,
                }],
            ));
        }
        let gap = widest - current;
        // A shaper may hang trailing spaces, even a nonbreaking one. Measure
        // them between visible marks; averaging avoids rounded glyph metrics.
        let padded = format!("|{}|", "\u{a0}".repeat(1024));
        let space = (measure(
            m,
            &[TextRun {
                text: padded.into(),
                style,
            }],
        ) - measure(
            m,
            &[TextRun {
                text: "||".into(),
                style,
            }],
        )) / 1024.0;
        if gap > 0.01 && space > 0.0 && gap.is_finite() && space.is_finite() {
            let count = (gap / space).ceil().clamp(1.0, 4096.0) as usize;
            arena.time_padding.insert(
                slot,
                Padding {
                    text: "\u{a0}".repeat(count),
                    spacing: style.letter_spacing + gap / count as f32 - space,
                },
            );
        }
    }
}

/// English month names shared by Contract date functions.
pub const MONTHS: [&str; 12] = [
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
/// English weekday names shared by Contract date functions.
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Proleptic Gregorian civil date, from days since 1970-01-01 (Hinnant).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}
