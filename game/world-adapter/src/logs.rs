//! Bounded host string journal with line-index cursors, outside saved state.
use exact_world::{DataError, LogCursor, World};
use std::collections::VecDeque;
const BYTES: usize = 48 * 1024;
#[derive(Default)]
pub(crate) struct History {
    cursor: LogCursor,
    lines: VecDeque<(String, usize)>,
    bytes: usize,
    next: u64,
    reset_at: Option<u64>,
    truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn churn_returns_bounded_string_suffix_with_line_indices() {
        let world = World::new(60, 0);
        for _ in 0..5000 {
            world.log(&"\"\\\n".repeat(40)).unwrap();
        }
        let mut history = History::default();
        let mut next = 0;
        for _ in 0..100 {
            let text = history.read(&world, next).unwrap();
            assert!(text.len() <= 65_536);
            let page: serde_json::Value = serde_json::from_str(&text).unwrap();
            let from = page["from"].as_u64().unwrap();
            next = page["next"].as_u64().unwrap();
            let lines = page["lines"].as_array().unwrap();
            assert!(lines.len() <= 512);
            assert!(lines.iter().all(|line| line.is_string()));
            assert_eq!(next - from, lines.len() as u64);
        }
        assert!(next >= 4096);
        let page: serde_json::Value =
            serde_json::from_str(&history.read(&world, 0).unwrap()).unwrap();
        assert!(page["from"].as_u64().unwrap() > 0);
        assert_eq!(page["truncated"], true);
        let tail: serde_json::Value =
            serde_json::from_str(&history.read(&world, next - 1).unwrap()).unwrap();
        assert_eq!(tail["lines"].as_array().unwrap().len(), 1);
        assert!(history.read(&world, next + 1).is_err());
    }
}
impl History {
    pub(crate) fn read(&mut self, world: &World, since: u64) -> Result<String, DataError> {
        if since > self.next {
            return Err(DataError::new("future log cursor"));
        }
        // The kernel returns at most 512 events / 64 KiB per read. Host lines
        // are JSON strings containing each event, not objects coerced by JS.
        let page = world.logs(self.cursor)?;
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&page.entries).map_err(|e| DataError::new(e.to_string()))?;
        let lines = entries
            .into_iter()
            .map(|entry| {
                let line = entry.to_string();
                let size = serde_json::to_string(&line)
                    .map_err(|e| DataError::new(e.to_string()))?
                    .len()
                    + 1;
                if size > BYTES {
                    return Err(DataError::new("log line exceeds 49152 encoded bytes"));
                }
                Ok((line, size))
            })
            .collect::<Result<Vec<_>, DataError>>()?;
        let next = self
            .next
            .checked_add(lines.len() as u64)
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or_else(|| DataError::new("log cursor exhausted"))?;
        if page.reset {
            self.lines.clear();
            self.bytes = 0;
            self.reset_at = Some(self.next);
        }
        self.truncated |= page.truncated;
        self.cursor = page.next;
        self.next = next;
        for (line, size) in lines {
            while self.lines.len() == 512 || self.bytes + size > BYTES {
                if let Some((_, removed)) = self.lines.pop_front() {
                    self.bytes -= removed;
                }
            }
            self.bytes += size;
            self.lines.push_back((line, size));
        }
        let first = self.next - self.lines.len() as u64;
        let from = since.max(first);
        let lines: Vec<&str> = self
            .lines
            .iter()
            .skip((from - first) as usize)
            .map(|(line, _)| line.as_str())
            .collect();
        Ok(
            serde_json::json!({"tick":world.tick(), "from":from, "next":self.next,
            "lines":lines, "reset":self.reset_at.is_some_and(|at| since <= at),
            "truncated":self.truncated || from != since})
            .to_string(),
        )
    }
}
