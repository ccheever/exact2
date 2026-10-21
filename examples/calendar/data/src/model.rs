//! Schedule summaries and the positional values declared by app.contract.

use crate::dates;
use exact_plan::Value;
use serde_json::{json, Value as Json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Event,
    Plan,
    Todo,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Plan => "plan",
            Self::Todo => "todo",
        }
    }

    pub fn read(text: &str) -> Result<Self, String> {
        match text {
            "event" => Ok(Self::Event),
            "plan" => Ok(Self::Plan),
            "todo" => Ok(Self::Todo),
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
    pub completed: bool,
}

impl Schedule {
    pub fn last_day(&self) -> i32 {
        if self.kind == Kind::Todo {
            self.start
        } else {
            self.end
        }
    }

    pub fn occurs(&self, day: i32) -> bool {
        self.start <= day && day <= self.last_day()
    }

    pub fn moved(&self, day: i32) -> Result<Self, String> {
        let end = day
            .checked_add(self.last_day() - self.start)
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
        if self.all_day || self.kind == Kind::Todo {
            "All day".into()
        } else {
            dates::display_time(self.start_time)
        }
    }

    pub fn range_label(&self) -> String {
        if self.kind == Kind::Todo {
            return "All day".into();
        }
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
            Value::Bool(self.completed),
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
            self.kind.name(),
            self.completed as u8
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
            completed: match number(9)? {
                0 => false,
                1 => true,
                _ => return Err("Invalid stored completion value.".into()),
            },
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
        if self.kind == Kind::Todo && self.end != self.start {
            return Err("A todo belongs to one date.".into());
        }
        if self.start_time >= 1440
            || self.end_time >= 1440
            || (self.kind != Kind::Todo
                && !self.all_day
                && self.start == self.end
                && self.end_time <= self.start_time)
        {
            return Err("The end time must be after the start time.".into());
        }
        Ok(())
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
        completed: false,
    }
}

pub(crate) fn original_samples(today: i32) -> Vec<(Schedule, String)> {
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
            all_day, start: (base + start).min(last), end: (base + end).min(last), start_time, end_time, kind: Kind::Event, completed: false }, notes.into())
    }).collect()
}

/// Exact previous starter contents, used only to preserve edits during refresh.
pub(crate) fn previous_samples(today: i32) -> Vec<(Schedule, String)> {
    let year = dates::civil(today).0.clamp(1900, 2100);
    use Kind::{Event, Plan, Todo};
    let items = [
        (
            9,
            0,
            "Flight to San Francisco",
            Event,
            false,
            8,
            8,
            10 * 60,
            18 * 60,
            "#747AFF",
            false,
            "Check in online and leave time for the airport.",
        ),
        (
            9,
            1,
            "Ferry Building",
            Event,
            false,
            10,
            10,
            10 * 60,
            12 * 60,
            "#F39B65",
            false,
            "Coffee, the farmers market, and a walk along the Embarcadero.",
        ),
        (
            9,
            2,
            "Book Alcatraz tickets",
            Todo,
            true,
            3,
            3,
            0,
            0,
            "#70B8A2",
            true,
            "Reserve the morning ferry for September 18.",
        ),
        (
            9,
            3,
            "Pack for San Francisco",
            Todo,
            true,
            6,
            6,
            0,
            0,
            "#EE8192",
            false,
            "Layers for the fog, comfortable shoes, and a travel adapter.",
        ),
        (
            9,
            4,
            "Dinner in North Beach",
            Event,
            false,
            13,
            13,
            19 * 60,
            20 * 60 + 30,
            "#F39B65",
            false,
            "A table for two after exploring the neighborhood.",
        ),
        (
            9,
            5,
            "Golden Gate walk",
            Event,
            false,
            16,
            16,
            10 * 60,
            12 * 60,
            "#70B8A2",
            false,
            "Start at Crissy Field and walk toward the bridge.",
        ),
        (
            9,
            6,
            "San Francisco",
            Plan,
            true,
            8,
            20,
            0,
            0,
            "#70B8A2",
            false,
            "Time to explore San Francisco, from arrival through the flight home.",
        ),
        (
            9,
            7,
            "Download boarding pass",
            Todo,
            true,
            7,
            7,
            0,
            0,
            "#747AFF",
            true,
            "Keep an offline copy for the flight.",
        ),
        (
            9,
            8,
            "Alcatraz morning",
            Event,
            false,
            18,
            18,
            9 * 60,
            12 * 60,
            "#747AFF",
            false,
            "Ferry from Pier 33, then lunch by the waterfront.",
        ),
        (
            9,
            9,
            "Choose Japan hotels",
            Todo,
            true,
            29,
            29,
            0,
            0,
            "#EE8192",
            false,
            "Compare stays near the station in Tokyo and Kyoto.",
        ),
        (
            9,
            10,
            "Travel journal",
            Plan,
            true,
            22,
            25,
            0,
            0,
            "#F39B65",
            false,
            "A few quiet evenings to write down favorite places and sort photos.",
        ),
        (
            9,
            11,
            "Photo evening",
            Event,
            false,
            27,
            27,
            18 * 60,
            20 * 60,
            "#747AFF",
            false,
            "Share favorite San Francisco photos over dinner.",
        ),
        (
            10,
            0,
            "Japan trip",
            Plan,
            true,
            10,
            22,
            0,
            0,
            "#EE8192",
            false,
            "Tokyo, Kyoto, and a final evening in Osaka.",
        ),
        (
            10,
            1,
            "Flight to Tokyo",
            Event,
            false,
            10,
            10,
            9 * 60,
            15 * 60,
            "#747AFF",
            false,
            "Passport, boarding pass, and the hotel address are ready.",
        ),
        (
            10,
            2,
            "Reserve Kyoto stay",
            Todo,
            true,
            2,
            2,
            0,
            0,
            "#EE8192",
            true,
            "Booked a small hotel near Kyoto Station.",
        ),
        (
            10,
            3,
            "Set up Japan eSIM",
            Todo,
            true,
            8,
            8,
            0,
            0,
            "#70B8A2",
            false,
            "Install before departure and enable data on arrival.",
        ),
        (
            10,
            4,
            "Tokyo neighborhoods",
            Plan,
            true,
            11,
            15,
            0,
            0,
            "#747AFF",
            false,
            "Asakusa, quiet coffee stops, and an evening in Shinjuku.",
        ),
        (
            10,
            5,
            "Shinkansen to Kyoto",
            Event,
            false,
            16,
            16,
            9 * 60,
            11 * 60 + 15,
            "#F39B65",
            false,
            "Pick up breakfast before boarding.",
        ),
        (
            10,
            6,
            "Kyoto stay",
            Plan,
            true,
            16,
            20,
            0,
            0,
            "#70B8A2",
            false,
            "An early start at Fushimi Inari, then a slow walk through Higashiyama.",
        ),
        (
            10,
            7,
            "Reserve teamLab tickets",
            Todo,
            true,
            5,
            5,
            0,
            0,
            "#747AFF",
            true,
            "The afternoon entry time is confirmed.",
        ),
        (
            10,
            8,
            "Osaka food walk",
            Event,
            false,
            21,
            21,
            17 * 60,
            20 * 60,
            "#F39B65",
            false,
            "Try takoyaki and explore Dotonbori after sunset.",
        ),
        (
            10,
            9,
            "Send Japan postcards",
            Todo,
            true,
            18,
            18,
            0,
            0,
            "#EE8192",
            false,
            "Find stamps and write a note home.",
        ),
        (
            10,
            10,
            "Japan trip review",
            Event,
            false,
            26,
            26,
            18 * 60,
            19 * 60,
            "#70B8A2",
            false,
            "Share the best moments and save favorite places for next time.",
        ),
        (
            10,
            11,
            "Print favorite photos",
            Todo,
            true,
            30,
            30,
            0,
            0,
            "#F39B65",
            false,
            "Choose a small set for the travel album.",
        ),
    ];
    items
        .into_iter()
        .map(
            |(
                month,
                i,
                title,
                kind,
                all_day,
                start,
                end,
                start_time,
                end_time,
                color,
                completed,
                notes,
            )| {
                (
                    Schedule {
                        id: format!("sample-{year:04}-{month:02}-{i}"),
                        title: title.into(),
                        color: color.into(),
                        all_day,
                        start: dates::ordinal(year, month, start),
                        end: dates::ordinal(year, month, end),
                        start_time,
                        end_time,
                        kind,
                        completed,
                    },
                    notes.into(),
                )
            },
        )
        .collect()
}

/// Exact two-day starter contents from the previous installed sample revision.
pub(crate) fn previous_samples_v4(today: i32) -> Vec<(Schedule, String)> {
    let edits = [
        ("Fly to SF", 8, 9),
        ("Ferry market", 10, 11),
        ("Book tickets", 3, 4),
        ("Pack bags", 6, 7),
        ("Food tour", 13, 14),
        ("Coast hike", 16, 17),
        ("San Francisco", 8, 19),
        ("Check in", 7, 8),
        ("Photo walk", 18, 19),
        ("Book hotels", 29, 30),
        ("Travel journal", 22, 25),
        ("Photo class", 27, 28),
        ("Japan trip", 9, 22),
        ("Fly to Tokyo", 9, 10),
        ("Kyoto hotel", 2, 3),
        ("Set up eSIM", 8, 9),
        ("Tokyo stay", 11, 15),
        ("Kyoto arrival", 16, 17),
        ("Kyoto stay", 16, 20),
        ("Book teamLab", 5, 6),
        ("Osaka tour", 21, 22),
        ("Postcards", 18, 19),
        ("Travel class", 26, 27),
        ("Print photos", 30, 31),
    ];
    previous_samples(today)
        .into_iter()
        .zip(edits)
        .enumerate()
        .map(|(index, ((mut event, mut notes), (title, start, end)))| {
            let (year, month, _) = dates::civil(event.start);
            event.title = title.into();
            event.start = dates::ordinal(year, month, start);
            event.end = dates::ordinal(year, month, end);
            if event.end == event.start + 1 && dates::weekday(event.start) == 6 {
                event.start -= 1;
                event.end -= 1;
            }
            if matches!(index, 0 | 13) {
                event.start_time = 23 * 60;
                event.end_time = 15 * 60;
                notes = "An overnight flight, with time to settle in on arrival.".into();
            } else if event.kind == Kind::Event {
                event.all_day = true;
                event.start_time = 0;
                event.end_time = 0;
                notes = match index {
                    1 => "Two days of market stops, coffee, and waterfront exploring.",
                    4 => "A two-day neighborhood food tour, with a table booked each evening.",
                    5 => "An overnight coastal hike, with a slow morning by the water.",
                    8 => "A two-day photography walk through favorite San Francisco neighborhoods.",
                    11 => "A weekend photography class, using pictures from the trip.",
                    17 => "The train to Kyoto, hotel check-in, and a first day exploring.",
                    20 => "Two days tasting local favorites and exploring Osaka.",
                    22 => {
                        "A two-day travel workshop to turn favorite places into the next itinerary."
                    }
                    _ => unreachable!(),
                }
                .into();
            }
            (event, notes)
        })
        .collect()
}

/// Trips and events span multiple days; todos always belong to one date.
pub(crate) fn samples(today: i32) -> Vec<(Schedule, String)> {
    let mut samples = previous_samples_v4(today);
    let year = dates::civil(today).0.clamp(1900, 2100);
    use Kind::{Event, Plan, Todo};
    let additions = [
        (
            9,
            12,
            "Check passport",
            Todo,
            1,
            1,
            "#747AFF",
            true,
            "Passport and travel details are ready.",
        ),
        (
            9,
            13,
            "City workshop",
            Event,
            1,
            2,
            "#F39B65",
            false,
            "A two-day workshop on exploring a city through its neighborhoods.",
        ),
        (
            9,
            14,
            "Trip prep",
            Plan,
            2,
            5,
            "#70B8A2",
            false,
            "A few days to organize the San Francisco trip.",
        ),
        (
            9,
            15,
            "Save maps",
            Todo,
            5,
            5,
            "#747AFF",
            true,
            "Download a map for offline walks.",
        ),
        (
            9,
            16,
            "Coffee crawl",
            Event,
            4,
            5,
            "#EE8192",
            false,
            "Two mornings trying local coffee stops before the trip.",
        ),
        (
            9,
            17,
            "Hotel check-in",
            Todo,
            9,
            9,
            "#F39B65",
            false,
            "Confirm the room and leave the bags.",
        ),
        (
            9,
            18,
            "Art studios",
            Event,
            14,
            15,
            "#747AFF",
            false,
            "A two-day open-studio visit in the city.",
        ),
        (
            9,
            19,
            "Send a photo",
            Todo,
            15,
            15,
            "#EE8192",
            true,
            "Share a favorite view with friends.",
        ),
        (
            9,
            20,
            "Bay cruise",
            Event,
            20,
            21,
            "#70B8A2",
            false,
            "An overnight bay cruise to finish the San Francisco stay.",
        ),
        (
            9,
            21,
            "Laundry",
            Todo,
            23,
            23,
            "#F39B65",
            false,
            "Unpack and refresh the travel clothes.",
        ),
        (
            9,
            22,
            "Travel meetup",
            Event,
            25,
            26,
            "#EE8192",
            false,
            "A two-day gathering to share travel stories and photo books.",
        ),
        (
            9,
            23,
            "Plan October",
            Todo,
            30,
            30,
            "#747AFF",
            false,
            "Review the Japan itinerary for next month.",
        ),
        (
            10,
            12,
            "Check passport",
            Todo,
            1,
            1,
            "#747AFF",
            true,
            "Passport and hotel details are ready for Japan.",
        ),
        (
            10,
            13,
            "Language class",
            Event,
            1,
            2,
            "#F39B65",
            false,
            "Two evenings learning useful Japanese phrases.",
        ),
        (
            10,
            14,
            "Trip prep",
            Plan,
            4,
            8,
            "#70B8A2",
            false,
            "Set aside a few days for the Japan travel checklist.",
        ),
        (
            10,
            15,
            "Save addresses",
            Todo,
            6,
            6,
            "#EE8192",
            false,
            "Keep hotel names and addresses available offline.",
        ),
        (
            10,
            16,
            "Travel fair",
            Event,
            4,
            5,
            "#747AFF",
            false,
            "A two-day travel fair for food, books, and local tips.",
        ),
        (
            10,
            17,
            "Hotel check-in",
            Todo,
            11,
            11,
            "#F39B65",
            true,
            "The Tokyo room is ready.",
        ),
        (
            10,
            18,
            "Tokyo art",
            Event,
            13,
            14,
            "#EE8192",
            false,
            "Two days visiting galleries and design shops.",
        ),
        (
            10,
            19,
            "Buy train pass",
            Todo,
            15,
            15,
            "#747AFF",
            true,
            "Keep the rail reservation ready for Kyoto.",
        ),
        (
            10,
            20,
            "Tea workshop",
            Event,
            18,
            19,
            "#70B8A2",
            false,
            "A two-day introduction to tea in Kyoto.",
        ),
        (
            10,
            21,
            "Back up photos",
            Todo,
            24,
            24,
            "#F39B65",
            false,
            "Save a second copy of the Japan photos.",
        ),
        (
            10,
            22,
            "Photo show",
            Event,
            28,
            29,
            "#EE8192",
            false,
            "A two-day photo show with favorite moments from Japan.",
        ),
        (
            10,
            23,
            "Next trip",
            Todo,
            31,
            31,
            "#747AFF",
            false,
            "Write down one place to visit next.",
        ),
    ];
    samples.extend(additions.into_iter().map(
        |(month, index, title, kind, start, end, color, completed, notes)| {
            let mut event = Schedule {
                id: format!("sample-{year:04}-{month:02}-{index}"),
                title: title.into(),
                color: color.into(),
                all_day: true,
                start: dates::ordinal(year, month, start),
                end: dates::ordinal(year, month, end),
                start_time: 0,
                end_time: 0,
                kind,
                completed,
            };
            if kind != Todo && event.end == event.start + 1 && dates::weekday(event.start) == 6 {
                event.start -= 1;
                event.end -= 1;
            }
            (event, notes.into())
        },
    ));
    for (event, _) in &mut samples {
        if event.kind == Todo {
            event.end = event.start;
        }
    }
    samples
}

pub(crate) fn sample_stickers(today: i32) -> Vec<(i32, &'static str)> {
    let year = dates::civil(today).0.clamp(1900, 2100);
    [
        (9, 2, "coffee"),
        (9, 8, "travel"),
        (9, 14, "sunshine"),
        (9, 17, "workout"),
        (9, 23, "book"),
        (9, 28, "heart"),
        (10, 3, "coffee"),
        (10, 9, "travel"),
        (10, 14, "flower"),
        (10, 19, "moon"),
        (10, 24, "rest"),
        (10, 29, "sparkle"),
    ]
    .into_iter()
    .map(|(month, day, sticker)| (dates::ordinal(year, month, day), sticker))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_preserves_inclusive_days_and_both_wall_clocks() {
        let mut event = original_samples(dates::ordinal(2026, 9, 1))[6].0.clone();
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
