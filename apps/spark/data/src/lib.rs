//! Spark's data: `profiles()` is the deck, eight people with two portraits
//! each, the same on every host. The portraits are randomuser.me's, loaded
//! from the network as a web page's `<img>` loads them.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Profiles;

struct Person {
    name: &'static str,
    age: u32,
    job: &'static str,
    miles: u32,
    bio: &'static str,
    likes_you: bool,
    /// randomuser.me's `women/N` or `men/N` portraits.
    portraits: [&'static str; 2],
    interests: &'static [&'static str],
}

const DECK: [Person; 8] = [
    Person {
        name: "Maya",
        age: 27,
        job: "Product designer at Figma",
        miles: 3,
        bio: "Weekend potter, weekday pixel pusher. Looking for someone to split a burrito and a sunset with.",
        likes_you: true,
        portraits: ["women/44", "women/68"],
        interests: &["Ceramics", "Hiking", "Tacos"],
    },
    Person {
        name: "Jordan",
        age: 30,
        job: "Line cook",
        miles: 1,
        bio: "I will judge your knife skills but never your taste in music. Currently perfecting a sourdough starter named Gerald.",
        likes_you: false,
        portraits: ["men/32", "men/75"],
        interests: &["Cooking", "Vinyl", "Cycling"],
    },
    Person {
        name: "Priya",
        age: 25,
        job: "Pediatric nurse",
        miles: 5,
        bio: "Night shifts, strong coffee, stronger opinions about dogs. Tell me your go-to karaoke song.",
        likes_you: true,
        portraits: ["women/65", "women/90"],
        interests: &["Dogs", "Karaoke", "Yoga"],
    },
    Person {
        name: "Sam",
        age: 29,
        job: "Climbing instructor",
        miles: 8,
        bio: "Half my week is spent upside down. The other half I'm planning the next road trip. Snacks provided.",
        likes_you: false,
        portraits: ["men/46", "men/22"],
        interests: &["Climbing", "Road trips", "Camping"],
    },
    Person {
        name: "Elena",
        age: 31,
        job: "Architect",
        miles: 2,
        bio: "I notice doorframes so you don't have to. Museum dates, bookstore detours and very long walks.",
        likes_you: true,
        portraits: ["women/12", "women/29"],
        interests: &["Museums", "Books", "Wine"],
    },
    Person {
        name: "Marcus",
        age: 28,
        job: "Software engineer",
        miles: 4,
        bio: "Builds apps by day, board game empires by night. Will absolutely let you win at Catan. Once.",
        likes_you: true,
        portraits: ["men/52", "men/85"],
        interests: &["Board games", "Running", "Ramen"],
    },
    Person {
        name: "Chloe",
        age: 26,
        job: "Marine biologist",
        miles: 12,
        bio: "Ask me about octopuses. No, really, ask me. I have slides. Beach cleanups count as a first date.",
        likes_you: false,
        portraits: ["women/33", "women/57"],
        interests: &["Diving", "Surfing", "Photography"],
    },
    Person {
        name: "Theo",
        age: 32,
        job: "Jazz pianist",
        miles: 6,
        bio: "Plays standards at a bar downtown on Thursdays. Come for the music, stay for the terrible puns.",
        likes_you: false,
        portraits: ["men/61", "men/11"],
        interests: &["Jazz", "Coffee", "Film"],
    },
];

fn strings(items: impl IntoIterator<Item = String>) -> Value {
    Value::list(items.into_iter().map(|s| Value::str(&s)).collect())
}

/// Profile `n`, fields in the order `shape Profile` declares them.
fn profile(n: usize, p: &Person) -> Value {
    Value::record(vec![
        Value::str(&format!("p{n}")),
        Value::Number(n as f64),
        Value::str(p.name),
        Value::Number(f64::from(p.age)),
        Value::str(p.job),
        Value::str(&format!(
            "{} mile{} away",
            p.miles,
            if p.miles == 1 { "" } else { "s" }
        )),
        Value::str(p.bio),
        Value::Bool(p.likes_you),
        strings(
            p.portraits
                .iter()
                .map(|s| format!("https://randomuser.me/api/portraits/{s}.jpg")),
        ),
        strings(p.interests.iter().map(|s| s.to_string())),
    ])
}

impl DataSource for Profiles {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "profiles" => Ok(Value::list(
                DECK.iter()
                    .enumerate()
                    .map(|(n, p)| profile(n, p))
                    .collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.ccheever.spark"
    }
}
