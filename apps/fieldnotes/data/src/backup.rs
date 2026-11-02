//! The existing TypeScript backup format, including JavaScript's UTF-16 limit.
use serde::Serialize;

pub const LIMIT: usize = 4 * 1024 * 1024;
pub const TOO_LARGE: &str =
    "This backup exceeds 4 MB. Split or remove large notes before backing up.";

#[derive(Serialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
}

const PREFIX: &str = "{\n  \"version\": 1,\n  \"notes\": [\n";
const SUFFIX: &str = "\n  ]\n}";

#[derive(Default)]
pub struct Builder {
    parts: Vec<String>,
    units: usize,
}

impl Builder {
    pub fn count(&self) -> usize {
        self.parts.len()
    }

    pub fn push(&mut self, note: &Note) -> Result<(), String> {
        let note = serde_json::to_string_pretty(note).map_err(|error| error.to_string())?;
        let part = note
            .lines()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let units =
            self.units + part.encode_utf16().count() + usize::from(!self.parts.is_empty()) * 2;
        if PREFIX.len() + units + SUFFIX.len() > LIMIT {
            return Err(TOO_LARGE.into());
        }
        self.units = units;
        self.parts.push(part);
        Ok(())
    }

    pub fn finish(self) -> String {
        if self.parts.is_empty() {
            "{\n  \"version\": 1,\n  \"notes\": []\n}".into()
        } else {
            format!("{PREFIX}{}{SUFFIX}", self.parts.join(",\n"))
        }
    }
}
