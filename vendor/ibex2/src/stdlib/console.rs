//! `console` — pure, ungated (LLP 0059.000 §3.1).
//!
//! Formatting happens in Rust so output is identical across engines.
//!
//! **Non-blocking by construction.** 214 uses across 89 modules makes `console`
//! the most frequently called API in the system, and a synchronous flush per
//! call is a startup regression on its own. Writes enqueue; the embedder drains
//! on a timer or at a size threshold.
//!
//! Out of v1: `table`, `group`, `time`, `trace`, `dir`, `count`, `assert`.

use crate::boundary::HostArg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Log,
    Info,
    Debug,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Log => "log",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub level: Level,
    pub message: String,
}

/// A bounded queue of console records.
///
/// Bounded on purpose: an unbounded buffer turns a runaway logging loop into
/// unbounded memory growth, which is a worse failure than dropping output. The
/// drop is counted and reported so it is never silent.
#[derive(Debug)]
pub struct Console {
    pending: Vec<Record>,
    capacity: usize,
    dropped: usize,
}

impl Console {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pending: Vec::new(),
            capacity,
            dropped: 0,
        }
    }

    /// Enqueue one call. Returns immediately; nothing is written here.
    pub fn write(&mut self, level: Level, args: &[HostArg]) {
        if self.pending.len() >= self.capacity {
            self.dropped += 1;
            return;
        }
        self.pending.push(Record {
            level,
            message: format_args_list(args),
        });
    }

    /// Drain everything queued. If records were dropped, a synthetic record
    /// says so rather than letting the gap pass unnoticed.
    pub fn drain(&mut self) -> Vec<Record> {
        let mut records = std::mem::take(&mut self.pending);
        if self.dropped > 0 {
            records.push(Record {
                level: Level::Warn,
                message: format!("[ibex2] {} console records dropped", self.dropped),
            });
            self.dropped = 0;
        }
        records
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Join arguments the way `console.log` does: single space, no trailing space.
pub fn format_args_list(args: &[HostArg]) -> String {
    args.iter().map(format_value).collect::<Vec<_>>().join(" ")
}

/// Render one value as JavaScript would.
pub fn format_value(value: &HostArg) -> String {
    match value {
        HostArg::Undefined => "undefined".to_string(),
        HostArg::Null => "null".to_string(),
        HostArg::Bool(true) => "true".to_string(),
        HostArg::Bool(false) => "false".to_string(),
        HostArg::Number(n) => format_number(*n),
        // A bare string logs without quotes, as the first argument does in a
        // browser console.
        HostArg::Str(s) => (*s).to_string(),
        HostArg::Bytes(bytes) => format!("Uint8Array({})", bytes.len()),
    }
}

/// Format a number the way JavaScript's `String(n)` does.
///
/// Rust and JavaScript disagree in three places that all show up immediately in
/// logs: `1.0` prints as `1` not `1.0`, non-finite values are `Infinity`/`NaN`
/// rather than `inf`/`NaN`, and negative zero prints as `0`.
pub fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e21 {
        return format!("{}", value as i64);
    }
    let mut text = format!("{value}");
    if text.contains('e') && !text.contains("e-") {
        text = text.replace('e', "e+");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> HostArg<'_> {
        HostArg::Str(text)
    }

    #[test]
    fn arguments_join_with_single_spaces() {
        assert_eq!(
            format_args_list(&[s("a"), HostArg::Number(1.0), HostArg::Bool(true)]),
            "a 1 true"
        );
        assert_eq!(format_args_list(&[]), "");
        assert_eq!(format_args_list(&[s("solo")]), "solo");
    }

    #[test]
    fn numbers_format_as_javascript_does() {
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(1.5), "1.5");
        assert_eq!(format_number(f64::NAN), "NaN");
        assert_eq!(format_number(f64::INFINITY), "Infinity");
        assert_eq!(format_number(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(format_number(42.0), "42");
    }

    #[test]
    fn null_and_undefined_are_distinct() {
        assert_eq!(format_value(&HostArg::Null), "null");
        assert_eq!(format_value(&HostArg::Undefined), "undefined");
    }

    #[test]
    fn strings_log_without_quotes() {
        assert_eq!(format_value(&s("plain")), "plain");
    }

    #[test]
    fn writes_enqueue_and_do_not_flush() {
        let mut console = Console::with_capacity(8);
        console.write(Level::Log, &[s("one")]);
        console.write(Level::Error, &[s("two")]);
        assert_eq!(console.pending_len(), 2);

        let drained = console.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].level, Level::Log);
        assert_eq!(drained[0].message, "one");
        assert_eq!(drained[1].level, Level::Error);
        assert_eq!(console.pending_len(), 0);
    }

    #[test]
    fn overflow_drops_and_says_so_rather_than_growing_without_bound() {
        let mut console = Console::with_capacity(2);
        for i in 0..5 {
            console.write(Level::Log, &[HostArg::Number(f64::from(i))]);
        }
        let drained = console.drain();
        assert_eq!(drained.len(), 3, "two records plus the drop notice");
        assert_eq!(drained[2].level, Level::Warn);
        assert!(drained[2].message.contains("3 console records dropped"));

        // The notice is reported once, not on every later drain.
        console.write(Level::Log, &[s("after")]);
        let drained = console.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].message, "after");
    }
}
