//! Bounded host string journal with line-index cursors, outside saved state.
use exact_world::{json, Data, DataError, LogCursor, World, Writer};
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

impl History {
    pub(crate) fn read(&mut self, world: &World, since: u64) -> Result<String, DataError> {
        if since > self.next {
            return Err(DataError::new("future log cursor"));
        }
        // The kernel returns at most 512 events / 64 KiB per read. Host lines
        // are JSON strings containing each event, not objects coerced by JS.
        let page = world.logs(self.cursor)?;
        let lines = event_lines(&page.entries)?
            .into_iter()
            .map(|entry| {
                let line = entry.to_string();
                let size = json::to_string(&line)?.len() + 1;
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
        let mut out = json::Encoder::default();
        out.begin_struct();
        out.field("tick");
        world.tick().write(&mut out);
        out.field("from");
        from.write(&mut out);
        out.field("next");
        self.next.write(&mut out);
        out.field("lines");
        out.begin_seq((self.next - from) as usize);
        for (line, _) in self.lines.iter().skip((from - first) as usize) {
            out.item();
            out.string(line);
        }
        out.end_seq();
        out.field("reset");
        out.boolean(self.reset_at.is_some_and(|at| since <= at));
        out.field("truncated");
        out.boolean(self.truncated || from != since);
        out.end_struct();
        out.finish()
    }
}

// Split the kernel's already valid, bounded JSON event array. Do not decode or
// reconstruct event objects merely to carry their existing text as host lines.
fn event_lines(text: &str) -> Result<Vec<&str>, DataError> {
    let text = text
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| DataError::new("kernel log array expected"))?;
    let (mut start, mut depth, mut quoted, mut escape) = (0, 0u32, false, false);
    let mut lines = Vec::new();
    for (i, b) in text.bytes().enumerate() {
        if quoted {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    lines.push(
                        text.get(start..i)
                            .ok_or_else(|| DataError::new("invalid log boundary"))?,
                    );
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    if let Some(last) = text.get(start..).filter(|s| !s.trim().is_empty()) {
        lines.push(last);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_lines_preserve_kernel_events_with_delimiters_escapes_and_unicode() {
        let world = World::new(60, 0);
        world.log("quotes \" slash \\ newline\n ,{}[] 🌕").unwrap();
        let kernel: Vec<serde_json::Value> =
            serde_json::from_str(&world.logs(LogCursor::default()).unwrap().entries).unwrap();
        let page: serde_json::Value =
            serde_json::from_str(&History::default().read(&world, 0).unwrap()).unwrap();
        let host: Vec<serde_json::Value> = page["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| serde_json::from_str(line.as_str().unwrap()).unwrap())
            .collect();
        assert!(!host.is_empty());
        assert_eq!(host, kernel);
    }
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
