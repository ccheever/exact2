//! List Bench data: the Expo PR 49975 screen's 10,000 messages, their order
//! in two sections (Inbox, Archive), the bookmark set and the edit-mode
//! selection. Deterministic; no I/O.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// Messages in all: the Inbox is the first half, the Archive the second.
pub const COUNT: usize = 10_000;
/// Where the Archive begins.
pub const HALF: usize = COUNT / 2;

const TEXTS: [&str; 4] = [
    "Are we still meeting for coffee?",
    "Yes! I found a place near the park. We can walk over afterward if the weather holds.",
    "Things to bring:\nCamera\nA warm jacket\nSomething for the picnic",
    "Sounds good. See you there ☕️",
];

/// The source.
/// - `messages()` answers the Feed (both sections, current order);
/// - `feed(op, id, n)` edits the order and answers the Feed: `reverse`
///   reverses each section, `delete` removes `id`, `move` moves `id` by `n`
///   places within its section (Expo's `onDelete` / `onMove`);
/// - `place(id, before)` moves `id` before `before` within its section, the
///   List's `reorderdrop`: a `before` in the other section, or none, is the
///   section's edge the drag crossed (Expo's `onMove` never leaves a section);
/// - `bookmark(id)` toggles one id's saved state and answers the set;
/// - `select(op, id)` toggles (`toggle`) or empties (`clear`) the edit-mode
///   selection and answers it.
pub struct ListBench {
    inbox: Vec<usize>,
    archive: Vec<usize>,
    saved: BTreeSet<usize>,
    selected: BTreeSet<usize>,
}

impl Default for ListBench {
    fn default() -> Self {
        Self {
            inbox: (0..HALF).collect(),
            archive: (HALF..COUNT).collect(),
            saved: BTreeSet::new(),
            selected: BTreeSet::new(),
        }
    }
}

/// `1234` → `"1,234"`, as `toLocaleString()` writes it in en-US.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn message_index(id: &str) -> Option<usize> {
    let n: usize = id.strip_prefix("message-")?.parse().ok()?;
    (n < COUNT && id == format!("message-{n}")).then_some(n)
}

fn ids(set: &BTreeSet<usize>) -> Value {
    let mut ids = String::from("|");
    for n in set {
        ids.push_str(&format!("message-{n}|"));
    }
    Value::record(vec![Value::str(&ids), Value::Number(set.len() as f64)])
}

impl ListBench {
    /// One row. Field order is `shape Message` in app.contract.
    fn row(i: usize, at: usize, len: usize, archive: bool) -> Value {
        let first = at == 0;
        let last = at + 1 == len;
        Value::record(vec![
            Value::str(&format!("message-{i}")),
            Value::Number(i as f64),
            Value::str(&format!("Message {}", i + 1)),
            Value::str(TEXTS[i % 4]),
            Value::Bool(i % 2 == 1),
            Value::Bool(i % 5 == 0),
            Value::Bool(first),
            Value::Bool(last),
            // The section decorations ride on the section's first and last rows.
            Value::Bool(!archive && first),
            Value::Bool(!archive && last),
            Value::Bool(archive && first),
            Value::Bool(archive && last),
        ])
    }

    fn feed(&self) -> Value {
        let (a, b) = (self.inbox.len(), self.archive.len());
        let mut rows = Vec::with_capacity(a + b);
        rows.extend(self.inbox.iter().enumerate().map(|(k, &i)| Self::row(i, k, a, false)));
        rows.extend(self.archive.iter().enumerate().map(|(k, &i)| Self::row(i, k, b, true)));
        Value::record(vec![
            Value::list(rows),
            Value::str(&format!("{} messages", grouped(a + b))),
            Value::str(&format!("Inbox · {}", grouped(a))),
            Value::str(&format!("Archive · {}", grouped(b))),
            Value::Bool(a == 0),
            Value::Bool(b == 0),
        ])
    }

    fn section(&mut self, n: usize) -> Option<&mut Vec<usize>> {
        if self.inbox.contains(&n) {
            Some(&mut self.inbox)
        } else if self.archive.contains(&n) {
            Some(&mut self.archive)
        } else {
            None
        }
    }

    fn edit(&mut self, op: &str, id: &str, by: f64) -> Result<Value, DataError> {
        let bad = || DataError::BadArguments("feed(op, id, n): id is a present message-<0..9999>".into());
        match op {
            "reverse" => {
                self.inbox.reverse();
                self.archive.reverse();
            }
            "delete" => {
                let n = message_index(id).ok_or_else(bad)?;
                let section = self.section(n).ok_or_else(bad)?;
                section.retain(|&m| m != n);
            }
            "move" => {
                let n = message_index(id).ok_or_else(bad)?;
                if !by.is_finite() || by.fract() != 0.0 {
                    return Err(DataError::BadArguments("feed(\"move\", id, n): n is a whole number".into()));
                }
                let section = self.section(n).ok_or_else(bad)?;
                let from = section.iter().position(|&m| m == n).ok_or_else(bad)?;
                let to = (from as f64 + by).clamp(0.0, (section.len() - 1) as f64) as usize;
                let item = section.remove(from);
                section.insert(to, item);
            }
            _ => return Err(DataError::BadArguments(format!("feed: unknown op {op:?}"))),
        }
        Ok(self.feed())
    }

    fn place(&mut self, id: &str, before: Option<&str>) -> Result<Value, DataError> {
        let bad = || DataError::BadArguments("place(id, before): ids are present message-<0..9999>".into());
        let n = message_index(id).ok_or_else(bad)?;
        let before = before.map(|b| message_index(b).ok_or_else(bad)).transpose()?;
        let archive = self.archive.contains(&n);
        let section = if archive { &mut self.archive } else { &mut self.inbox };
        let from = section.iter().position(|&m| m == n).ok_or_else(bad)?;
        section.remove(from);
        let to = match before.and_then(|b| section.iter().position(|&m| m == b)) {
            Some(at) => at,
            // Dragged up out of the Archive: its start; otherwise the end.
            None if archive && before.is_some() => 0,
            None => section.len(),
        };
        section.insert(to, n);
        Ok(self.feed())
    }
}

impl DataSource for ListBench {
    fn app_id(&self) -> &str {
        "dev.exact.listbench.exact"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("messages", []) => Ok(self.feed()),
            ("feed", [op, id, n]) => {
                let (Some(op), Some(id), Some(n)) = (op.as_str(), id.as_str(), n.as_number()) else {
                    return Err(DataError::BadArguments("feed(op: string, id: string, n: number)".into()));
                };
                self.edit(op, id, n)
            }
            ("place", [id, before]) => {
                let before = match before {
                    Value::Option(Some(b)) => Some(b.as_str().ok_or_else(|| {
                        DataError::BadArguments("place(id, before: option<string>)".into())
                    })?),
                    _ => None,
                };
                let id = id.as_str().ok_or_else(|| DataError::BadArguments("place(id: string, before)".into()))?;
                self.place(id, before)
            }
            ("bookmark", [id]) => {
                let n = id.as_str().and_then(message_index).ok_or_else(|| {
                    DataError::BadArguments("bookmark(id): id is message-<0..9999>".into())
                })?;
                if !self.saved.remove(&n) {
                    self.saved.insert(n);
                }
                Ok(ids(&self.saved))
            }
            ("select", [op, id]) => match (op.as_str(), id.as_str()) {
                (Some("clear"), _) => {
                    self.selected.clear();
                    Ok(ids(&self.selected))
                }
                (Some("toggle"), Some(id)) => {
                    let n = message_index(id).ok_or_else(|| {
                        DataError::BadArguments("select(\"toggle\", id): id is message-<0..9999>".into())
                    })?;
                    if !self.selected.remove(&n) {
                        self.selected.insert(n);
                    }
                    Ok(ids(&self.selected))
                }
                _ => Err(DataError::BadArguments("select(op: \"toggle\" | \"clear\", id: string)".into())),
            },
            ("messages", _) => Err(DataError::BadArguments("messages()".into())),
            ("feed", _) => Err(DataError::BadArguments("feed(op, id, n)".into())),
            ("place", _) => Err(DataError::BadArguments("place(id, before)".into())),
            ("bookmark", _) => Err(DataError::BadArguments("bookmark(id: string)".into())),
            ("select", _) => Err(DataError::BadArguments("select(op, id)".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(s: &ListBench) -> (Vec<usize>, Vec<usize>) {
        (s.inbox.clone(), s.archive.clone())
    }

    #[test]
    fn reversal_reverses_each_section() {
        let mut s = ListBench::default();
        s.query("feed", &[Value::str("reverse"), Value::str(""), Value::Number(0.0)]).unwrap();
        let (a, b) = order(&s);
        assert_eq!((a[0], a[4999], b[0], b[4999]), (4999, 0, 9999, 5000));
    }

    #[test]
    fn place_moves_before_within_the_section_and_clamps_at_its_edges() {
        let mut s = ListBench::default();
        let place = |s: &mut ListBench, id: &str, before: Option<&str>| {
            let before = Value::Option(before.map(|b| std::rc::Rc::new(Value::str(b))));
            s.query("place", &[Value::str(id), before]).unwrap();
        };
        place(&mut s, "message-0", Some("message-3"));
        assert_eq!(&s.inbox[..4], &[1, 2, 0, 3]);
        place(&mut s, "message-2", Some("message-5000"));
        assert_eq!(*s.inbox.last().unwrap(), 2, "into the Archive's first gap is the Inbox's end");
        place(&mut s, "message-5003", Some("message-4"));
        assert_eq!(s.archive[0], 5003, "up out of the Archive is its start");
        place(&mut s, "message-5003", None);
        assert_eq!(*s.archive.last().unwrap(), 5003);
        assert_eq!((s.inbox.len(), s.archive.len()), (HALF, HALF));
    }

    #[test]
    fn delete_and_move_stay_in_their_section() {
        let mut s = ListBench::default();
        s.query("feed", &[Value::str("delete"), Value::str("message-0"), Value::Number(0.0)]).unwrap();
        assert_eq!(s.inbox.len(), 4999);
        assert_eq!(s.inbox[0], 1);
        s.query("feed", &[Value::str("move"), Value::str("message-1"), Value::Number(2.0)]).unwrap();
        assert_eq!(&s.inbox[..3], &[2, 3, 1]);
        s.query("feed", &[Value::str("move"), Value::str("message-5000"), Value::Number(-3.0)]).unwrap();
        assert_eq!(s.archive[0], 5000);
        s.query("feed", &[Value::str("move"), Value::str("message-4999"), Value::Number(9.0)]).unwrap();
        assert_eq!(*s.inbox.last().unwrap(), 4999);
        let feed = format!("{:?}", s.feed());
        assert!(feed.contains("9,999 messages") && feed.contains("Inbox · 4,999"), "{feed}");
    }

    #[test]
    fn bookmark_and_selection_toggle() {
        let mut s = ListBench::default();
        let a = s.query("bookmark", &[Value::str("message-3")]).unwrap();
        assert!(format!("{a:?}").contains("|message-3|"));
        s.query("bookmark", &[Value::str("message-3")]).unwrap();
        assert!(s.saved.is_empty());
        assert!(s.query("bookmark", &[Value::str("message-03")]).is_err());
        s.query("select", &[Value::str("toggle"), Value::str("message-7")]).unwrap();
        s.query("select", &[Value::str("toggle"), Value::str("message-8")]).unwrap();
        assert_eq!(s.selected.len(), 2);
        s.query("select", &[Value::str("clear"), Value::str("")]).unwrap();
        assert!(s.selected.is_empty());
    }

    #[test]
    fn grouping() {
        assert_eq!(grouped(10_000), "10,000");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }
}
