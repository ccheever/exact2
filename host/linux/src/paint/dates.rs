//! A date or time control's value as the device writes one (LLP 1115.001
//! D9: "the value shown in the locale's format"). The value is HTML's
//! (`2026-10-23`, `14:05`, `2026-10-23T14:05`); what is shown is the
//! locale's medium date and its clock, 12 or 24-hour as the person set it.
//!
//! Nothing is tabled here. The shell tells, with the device's settings
//! (`Device::dates`), the patterns Android's own formatter chooses for the
//! locale (`DateFormat.getBestDateTimePattern`, Unicode's date field
//! symbols: <https://unicode.org/reports/tr35/tr35-dates.html#Date_Field_Symbol_Table>),
//! the month names and the weekdays', short and full, the eras,
//! the two day periods and the zero digit, and they
//! are filled in here (the painter keeps them, `Painter::dates`). A host
//! whose shell tells none shows HTML's value.
/// The device's date and time patterns and names, as its shell told them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dates {
    date: String,
    time: String,
    time_seconds: String,
    date_time: String,
    date_time_seconds: String,
    periods: [String; 2],
    zero: char,
    months: [String; 12],
    months_full: [String; 12],
    /// Monday first, short then full.
    weekdays: [String; 7],
    weekdays_full: [String; 7],
    /// Before and after the common era.
    eras: [String; 2],
}

impl Dates {
    /// The shell's lines: the date, time, time with seconds, date and time,
    /// and date and time with seconds patterns, the AM and PM names, the
    /// zero digit, the twelve months' short names and their full ones, the
    /// seven weekdays' from Monday likewise, and the two eras. `None` for
    /// anything else.
    pub(crate) fn from_lines(told: &str) -> Option<Dates> {
        let lines: Vec<&str> = told.split('\n').collect();
        let [date, time, time_seconds, date_time, date_time_seconds, am, pm, zero, names @ ..] =
            lines.as_slice()
        else {
            return None;
        };
        fn take<const N: usize>(names: &[&str], at: usize) -> Option<[String; N]> {
            let names = names.get(at..at + N)?.iter().map(|m| m.to_string());
            names.collect::<Vec<_>>().try_into().ok()
        }
        if names.len() != 12 + 12 + 7 + 7 + 2 {
            return None;
        }
        let (months, months_full) = (take(names, 0)?, take(names, 12)?);
        let (weekdays, weekdays_full, eras) =
            (take(names, 24)?, take(names, 31)?, take(names, 38)?);
        let mut zero = zero.chars();
        let digit = zero.next().filter(|_| zero.next().is_none())?;
        Some(Dates {
            date: date.to_string(),
            time: time.to_string(),
            time_seconds: time_seconds.to_string(),
            date_time: date_time.to_string(),
            date_time_seconds: date_time_seconds.to_string(),
            periods: [am.to_string(), pm.to_string()],
            zero: digit,
            months,
            months_full,
            weekdays,
            weekdays_full,
            eras,
        })
    }

    /// `value`, a `kind` control's (`date`, `time`, `datetime-local`), as
    /// the locale writes it; `None` when it is not a valid value of that
    /// kind, which is then shown as it is.
    pub(crate) fn show(&self, kind: &str, value: &str) -> Option<String> {
        let (day, time) = match kind {
            "date" => (Some(day(value)?), None),
            "time" => (None, Some(time(value)?)),
            _ => {
                let (d, t) = value.split_once('T')?;
                (Some(day(d)?), Some(time(t)?))
            }
        };
        let seconds = time.is_some_and(|t| t.3);
        let pattern = match (day.is_some(), time.is_some(), seconds) {
            (true, false, _) => &self.date,
            (false, _, false) => &self.time,
            (false, _, true) => &self.time_seconds,
            (true, true, false) => &self.date_time,
            (true, true, true) => &self.date_time_seconds,
        };
        Some(self.fill(
            pattern,
            day.unwrap_or((1970, 1, 1)),
            time.unwrap_or_default(),
        ))
    }

    /// Unicode's pattern syntax: a run of one ASCII letter is a field, text
    /// in single quotes is literal (`''` a quote), anything else is itself.
    fn fill(&self, pattern: &str, (y, mo, d): (i32, u32, u32), (h, mi, s, _): Time) -> String {
        let mut out = String::new();
        let mut chars = pattern.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    out.push('\'');
                    continue;
                }
                while let Some(q) = chars.next() {
                    match q {
                        '\'' if chars.peek() == Some(&'\'') => {
                            chars.next();
                            out.push('\'');
                        }
                        '\'' => break,
                        _ => out.push(q),
                    }
                }
                continue;
            }
            if !c.is_ascii_alphabetic() {
                out.push(c);
                continue;
            }
            let mut n = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                n += 1;
            }
            let number = |out: &mut String, v: i64, width: usize| {
                for digit in format!("{v:0width$}").chars() {
                    out.push(self.digit(digit));
                }
            };
            match c {
                'y' | 'u' if n == 2 => number(&mut out, i64::from(y.rem_euclid(100)), 2),
                'y' | 'u' => number(&mut out, i64::from(y), n),
                'M' | 'L' if n >= 4 => out.push_str(&self.months_full[mo as usize - 1]),
                'M' | 'L' if n == 3 => out.push_str(&self.months[mo as usize - 1]),
                'M' | 'L' => number(&mut out, i64::from(mo), n),
                'd' => number(&mut out, i64::from(d), n),
                'H' => number(&mut out, i64::from(h), n),
                'k' => number(&mut out, i64::from(if h == 0 { 24 } else { h }), n),
                'h' => number(
                    &mut out,
                    i64::from(if h % 12 == 0 { 12 } else { h % 12 }),
                    n,
                ),
                'K' => number(&mut out, i64::from(h % 12), n),
                'm' => number(&mut out, i64::from(mi), n),
                's' => number(&mut out, i64::from(s), n),
                'a' | 'b' | 'B' => out.push_str(&self.periods[usize::from(h >= 12)]),
                'E' | 'c' | 'e' if n >= 4 => out.push_str(&self.weekdays_full[weekday(y, mo, d)]),
                'E' => out.push_str(&self.weekdays[weekday(y, mo, d)]),
                'c' | 'e' if n >= 3 => out.push_str(&self.weekdays[weekday(y, mo, d)]),
                'G' => out.push_str(&self.eras[usize::from(y > 0)]),
                // A field these patterns do not carry (a zone, a week of
                // the year) is left out rather than guessed.
                _ => {}
            }
        }
        out
    }

    fn digit(&self, c: char) -> char {
        match c.to_digit(10) {
            Some(n) => char::from_u32(self.zero as u32 + n).unwrap_or(c),
            None => c,
        }
    }
}

/// The weekday of a proleptic Gregorian date, Monday 0 (Howard Hinnant's
/// `days_from_civil`: <https://howardhinnant.github.io/date_algorithms.html>).
fn weekday(y: i32, m: u32, d: u32) -> usize {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    // 1970-01-01 was a Thursday.
    (days + 3).rem_euclid(7) as usize
}

/// An hour, minute and second, and whether the value wrote its seconds.
type Time = (u32, u32, u32, bool);

fn day(s: &str) -> Option<(i32, u32, u32)> {
    if !exact_kernel::ControlKind::Date.valid_value(s) || s.is_empty() {
        return None;
    }
    let mut parts = s.rsplitn(3, '-');
    let (d, m, y) = (parts.next()?, parts.next()?, parts.next()?);
    Some((y.parse().ok()?, m.parse().ok()?, d.parse().ok()?))
}

fn time(s: &str) -> Option<Time> {
    if !exact_kernel::ControlKind::Time.valid_value(s) || s.is_empty() {
        return None;
    }
    let mut parts = s.split(':');
    let (h, m) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    let seconds = parts.next();
    let s = seconds.map_or(Some(0.0), |s| s.parse::<f32>().ok())?;
    Some((h, m, s as u32, seconds.is_some()))
}

#[cfg(test)]
mod tests {
    use super::Dates;

    const MONTHS: &str = "Jan\nFeb\nMar\nApr\nMay\nJun\nJul\nAug\nSep\nOct\nNov\nDec\nJanuary\nFebruary\nMarch\nApril\nMay\nJune\nJuly\nAugust\nSeptember\nOctober\nNovember\nDecember\nMon\nTue\nWed\nThu\nFri\nSat\nSun\nMonday\nTuesday\nWednesday\nThursday\nFriday\nSaturday\nSunday\nBC\nAD";

    fn dates(patterns: [&str; 5], zero: char) -> Dates {
        let told = format!("{}\nAM\nPM\n{zero}\n{MONTHS}", patterns.join("\n"));
        Dates::from_lines(&told).unwrap()
    }

    #[test]
    fn a_value_is_written_as_the_locale_writes_it() {
        // en-US on a 12-hour clock, as Android's ICU answers the skeletons
        // yMMMd, hm, hms, yMMMdhm and yMMMdhms.
        let us = dates(
            [
                "MMM d, y",
                "h:mm a",
                "h:mm:ss a",
                "MMM d, y, h:mm a",
                "MMM d, y, h:mm:ss a",
            ],
            '0',
        );
        assert_eq!(
            us.show("date", "2026-10-03").as_deref(),
            Some("Oct 3, 2026")
        );
        assert_eq!(us.show("time", "00:05").as_deref(), Some("12:05 AM"));
        assert_eq!(us.show("time", "13:05:09").as_deref(), Some("1:05:09 PM"));
        assert_eq!(
            us.show("datetime-local", "2026-10-23T12:01").as_deref(),
            Some("Oct 23, 2026, 12:01 PM")
        );
        // de on a 24-hour clock, with a quoted literal.
        let de = dates(
            [
                "d. MMM y",
                "HH:mm",
                "HH:mm:ss",
                "d. MMM y, HH:mm",
                "d. MMM y, HH:mm:ss",
            ],
            '0',
        );
        assert_eq!(de.show("time", "09:30").as_deref(), Some("09:30"));
        assert_eq!(
            de.show("date", "2026-01-31").as_deref(),
            Some("31. Jan 2026")
        );
        let quoted = dates(["d 'de' MMM 'o''clock'", "", "", "", ""], '0');
        assert_eq!(
            quoted.show("date", "2026-05-02").as_deref(),
            Some("2 de May o'clock")
        );
        // A locale's own digits.
        let arabic = dates(["d/M/y", "", "", "", ""], '\u{660}');
        assert_eq!(
            arabic.show("date", "2026-10-03").as_deref(),
            Some("\u{663}/\u{661}\u{660}/\u{662}\u{660}\u{662}\u{666}")
        );
        // What is not a value of its kind is shown as it is.
        assert_eq!(us.show("date", ""), None);
        assert_eq!(us.show("date", "2026-02-30"), None);
        assert_eq!(us.show("datetime-local", "2026-10-23"), None);
        assert!(Dates::from_lines("MMM d, y").is_none());
        // A weekday and an era, where a locale's pattern has them.
        let full = dates(["EEEE, d MMMM y G", "", "", "", ""], '0');
        assert_eq!(
            full.show("date", "2026-10-16").as_deref(),
            Some("Friday, 16 October 2026 AD")
        );
        assert_eq!(
            dates(["EEE d", "", "", "", ""], '0')
                .show("date", "2000-02-29")
                .as_deref(),
            Some("Tue 29")
        );
    }
}
