//! Schedule summaries and the positional values declared by app.contract.

use crate::dates;
use exact_plan::Value;
use serde_json::{json, Value as Json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Event,
    Plan,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Plan => "plan",
        }
    }

    pub fn read(text: &str) -> Result<Self, String> {
        match text {
            "event" => Ok(Self::Event),
            "plan" => Ok(Self::Plan),
            _ => Err("Invalid calendar item type in storage.".into()),
        }
    }
}

pub(crate) const STICKERS: [&str; 15] = [
    "sunshine", "coffee", "cake", "heart", "sparkle", "flower", "book", "workout", "travel",
    "rest", "bunny", "paris", "daisy", "moon", "picnic",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Schedule {
    pub id: String,
    pub title: String,
    pub color: String,
    pub all_day: bool,
    pub start: i32,
    pub end: i32,
    pub start_time: u16,
    pub end_time: u16,
    pub kind: Kind,
}

impl Schedule {
    pub fn last_day(&self) -> i32 {
        self.end
    }

    pub fn occurs(&self, day: i32) -> bool {
        self.start <= day && day <= self.last_day()
    }

    pub fn moved(&self, day: i32) -> Result<Self, String> {
        let end = day
            .checked_add(self.end - self.start)
            .ok_or("The moved schedule is outside the calendar range.")?;
        if !dates::in_range(day) || !dates::in_range(end) {
            return Err("The complete schedule must stay between 1900 and 2100.".into());
        }
        Ok(Self {
            start: day,
            end,
            ..self.clone()
        })
    }

    pub fn time_label(&self) -> String {
        if self.all_day {
            "All day".into()
        } else {
            dates::display_time(self.start_time)
        }
    }

    pub fn range_label(&self) -> String {
        if self.all_day {
            if self.start == self.end {
                "All day".into()
            } else {
                format!(
                    "{} – {} · All day",
                    dates::short_date(self.start),
                    dates::short_date(self.end)
                )
            }
        } else if self.start == self.end {
            format!(
                "{} – {}",
                dates::display_time(self.start_time),
                dates::display_time(self.end_time)
            )
        } else {
            format!(
                "{}, {} – {}, {}",
                dates::short_date(self.start),
                dates::display_time(self.start_time),
                dates::short_date(self.end),
                dates::display_time(self.end_time)
            )
        }
    }

    pub fn value(&self, notes: &str) -> Value {
        Value::record(vec![
            Value::str(&self.id),
            Value::str(&self.title),
            Value::str(notes),
            Value::str(&self.color),
            Value::Bool(self.all_day),
            Value::Number(self.start.into()),
            Value::Number(self.end.into()),
            Value::str(&dates::iso(self.start)),
            Value::str(&dates::iso(self.end)),
            Value::str(&dates::time(self.start_time)),
            Value::str(&dates::time(self.end_time)),
            Value::str(&self.range_label()),
            Value::str(self.kind.name()),
        ])
    }

    pub fn params(&self, notes: &str) -> Json {
        json!([
            self.id,
            self.title,
            notes,
            self.color,
            self.all_day as u8,
            self.start,
            self.end,
            self.start_time,
            self.end_time,
            self.kind.name()
        ])
    }

    pub fn read(row: &Json) -> Result<Self, String> {
        let text = |i| {
            row[i]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "Invalid calendar text in storage.".to_string())
        };
        let number = |i| integer(&row[i]);
        let start = i32::try_from(number(4)?).map_err(|_| "Invalid stored start date.")?;
        let end = i32::try_from(number(5)?).map_err(|_| "Invalid stored end date.")?;
        let start_time = u16::try_from(number(6)?).map_err(|_| "Invalid stored start time.")?;
        let end_time = u16::try_from(number(7)?).map_err(|_| "Invalid stored end time.")?;
        let all_day = match number(3)? {
            0 => false,
            1 => true,
            _ => return Err("Invalid stored all-day value.".into()),
        };
        let schedule = Self {
            id: text(0)?,
            title: text(1)?,
            color: text(2)?,
            all_day,
            start,
            end,
            start_time,
            end_time,
            kind: Kind::read(&text(8)?)?,
        };
        schedule.validate()?;
        Ok(schedule)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.len() > 192 {
            return Err("The schedule identifier is invalid.".into());
        }
        if self.title.trim().is_empty() || self.title.chars().count() > 160 {
            return Err("Enter a title with 1 to 160 characters.".into());
        }
        validate_color(&self.color)?;
        if !dates::in_range(self.start) || !dates::in_range(self.end) || self.end < self.start {
            return Err(
                "The end date must be on or after the start date, between 1900 and 2100.".into(),
            );
        }
        if self.start_time >= 1440
            || self.end_time >= 1440
            || (!self.all_day && self.start == self.end && self.end_time <= self.start_time)
        {
            return Err("The end time must be after the start time.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Todo {
    pub id: String,
    pub title: String,
    pub color: String,
    pub created_at: i64,
    pub completed_at: i64,
}

impl Todo {
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::str(&self.id),
            Value::str(&self.title),
            Value::str(&self.color),
            Value::Bool(self.completed_at != 0),
            Value::Number(self.created_at as f64),
            Value::Number(self.completed_at as f64),
        ])
    }

    pub fn read(row: &Json) -> Result<Self, String> {
        let text = |i| {
            row[i]
                .as_str()
                .map(str::to_owned)
                .ok_or("Invalid todo text in storage.")
        };
        let todo = Self {
            id: text(0)?,
            title: text(1)?,
            color: text(2)?,
            created_at: integer(&row[3])?,
            completed_at: integer(&row[4])?,
        };
        todo.validate()?;
        Ok(todo)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.len() > 192 {
            return Err("The todo identifier is invalid.".into());
        }
        if self.title.trim().is_empty() || self.title.chars().count() > 160 {
            return Err("Enter a title with 1 to 160 characters.".into());
        }
        if self.created_at <= 0
            || self.created_at > 9_007_199_254_740_991
            || self.completed_at < 0
            || self.completed_at > 9_007_199_254_740_991
        {
            return Err("The todo time is invalid.".into());
        }
        validate_color(&self.color)
    }
}

fn validate_color(color: &str) -> Result<(), String> {
    let color = color.as_bytes();
    if color.len() != 7 || color[0] != b'#' || !color[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err("Choose a valid color.".into());
    }
    Ok(())
}

pub(crate) fn integer(value: &Json) -> Result<i64, String> {
    value["integer"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| "Invalid calendar number in storage.".into())
}

pub(crate) fn empty_schedule() -> Schedule {
    Schedule {
        id: String::new(),
        title: String::new(),
        color: "#747AFF".into(),
        all_day: false,
        start: 0,
        end: 0,
        start_time: 9 * 60,
        end_time: 10 * 60,
        kind: Kind::Event,
    }
}

pub(crate) fn samples(today: i32) -> Vec<(Schedule, String)> {
    let month = dates::month_of(today).clamp(dates::FIRST_MONTH, dates::LAST_MONTH);
    let base = dates::month_first(month);
    let last = dates::month_first(dates::LAST_MONTH + 1) - 1;
    [
        ("Design review", "#747AFF", false, 2, 2, 10 * 60, 11 * 60, "Review the new calendar flow with the team."),
        ("Coffee with Alex", "#F39B65", false, 5, 5, 9 * 60, 9 * 60 + 45, "Meet at the neighborhood café."),
        ("Focus time", "#70B8A2", false, 8, 8, 14 * 60, 16 * 60, "An afternoon for uninterrupted work."),
        ("Weekend getaway", "#EE8192", true, 11, 13, 0, 0, "A few days away. Long-press this item to move the whole trip."),
        ("Product workshop", "#747AFF", true, 16, 18, 0, 0, "Planning, ideas, and a working prototype."),
        ("Dinner reservation", "#F39B65", false, 20, 20, 19 * 60, 20 * 60 + 30, "A table for two."),
        ("Summer in Seoul", "#70B8A2", true, 24, 29, 0, 0, "The 25th through the 30th, inclusive. Move it to the 20th to keep the full range through the 25th."),
    ].into_iter().enumerate().map(|(i, (title, color, all_day, start, end, start_time, end_time, notes))| {
        (Schedule { id: format!("sample-{}-{i}", dates::month_id(month)), title: title.into(), color: color.into(),
            all_day, start: (base + start).min(last), end: (base + end).min(last), start_time, end_time, kind: Kind::Event }, notes.into())
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_preserves_inclusive_days_and_both_wall_clocks() {
        let mut event = samples(dates::ordinal(2026, 9, 1))[6].0.clone();
        let moved = event.moved(dates::ordinal(2026, 9, 20)).unwrap();
        assert_eq!(dates::iso(moved.end), "2026-09-25");
        assert_eq!(moved.end - moved.start + 1, 6);
        event.all_day = false;
        event.start = dates::ordinal(2026, 3, 6);
        event.end = dates::ordinal(2026, 3, 9);
        event.start_time = 9 * 60 + 30;
        event.end_time = 17 * 60;
        let moved = event.moved(dates::ordinal(2026, 10, 31)).unwrap();
        assert_eq!(dates::iso(moved.end), "2026-11-03");
        assert_eq!((moved.start_time, moved.end_time), (570, 1020));
        assert_eq!(event.moved(event.start).unwrap(), event);
    }

    #[test]
    fn all_schedule_end_dates_are_inclusive_even_at_midnight() {
        let mut event = samples(0)[0].0.clone();
        event.end = event.start + 1;
        event.end_time = 0;
        assert!(event.occurs(event.start));
        assert!(event.occurs(event.end));
        event.all_day = true;
        assert!(event.occurs(event.end));
        assert!(event.moved(dates::ordinal(2100, 12, 31)).is_err());
    }
}
