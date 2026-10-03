//! A development display's presented frames on Linux (LLP 1079 D3–D5).
//!
//! @ref LLP 1079 D3 (segments, the period, the record), D4 (the reply and
//! the journal line), D5 (`SIGUSR1`'s trace)
//!
//! The display loop's own page flips are the samples: there is no compositor
//! to observe beside it. Each frame remembers when it was first wanted (the
//! presenter went dirty) and the runner's `seq` when it was submitted, so a
//! flip claims only the transactions its picture carries. A frame wanted
//! before the previous flip completed is continuous presentation: `missed`
//! is the vblank-sequence gap − 1, never a division. A frame wanted after an
//! idle gap starts a segment and is measured from when it was wanted, so a
//! slow render is late and an idle gap never is. The period is the mode's
//! refresh. The headless agent presents no frame: its `perf frames` is
//! virtual (agent.rs).

use crate::presenter::Presenter;
use exact_runner::DataSource;
use serde_json::{json, Value};
use std::collections::VecDeque;

const RING: usize = 600;
const LATE: usize = 100;

#[derive(Debug, Clone, PartialEq)]
struct Record {
    t: f64,
    interval: f64,
    missed: u32,
    seq: Option<(u64, u64)>,
    paint: Option<f64>,
}

impl Record {
    fn json(&self) -> Value {
        let mut o = json!({ "t": self.t, "interval": self.interval, "missed": self.missed,
            "seq": self.seq.map(|(a, b)| json!([a, b])) });
        if let Some(paint) = self.paint {
            o["paint"] = json!(paint);
        }
        o
    }
}

/// The sampler over the display loop's flips.
#[derive(Debug)]
pub struct Frames {
    period_ms: f64,
    ring: VecDeque<Record>,
    late: VecDeque<Record>,
    dropped: u64,
    presented: u64,
    late_count: u64,
    missed: u64,
    segments: u64,
    /// The previous flip: its vblank sequence and when it completed.
    last: Option<(u32, f64)>,
    /// When the next frame was first wanted.
    wanted: Option<f64>,
    /// The frame awaiting its flip: when it was wanted, and the runner's
    /// `seq` when it was submitted.
    pending: Option<(f64, u64)>,
    /// The runner's `seq` at the previous submitted frame.
    seq_mark: u64,
    /// The presenter's host these samples are of ([`Frames::submitted`]).
    host: u64,
}

impl Frames {
    /// A sampler for a mode refreshing `refresh_hz` times a second.
    pub fn new(refresh_hz: u32) -> Frames {
        Frames {
            period_ms: 1000.0 / f64::from(refresh_hz.max(1)),
            ring: VecDeque::new(),
            late: VecDeque::new(),
            dropped: 0,
            presented: 0,
            late_count: 0,
            missed: 0,
            segments: 0,
            last: None,
            wanted: None,
            pending: None,
            seq_mark: 0,
            host: 0,
        }
    }

    /// The presenter has something to show at `t` (the loop's clock,
    /// milliseconds): the next frame is wanted from the first such call.
    pub fn dirty(&mut self, t: f64) {
        self.wanted.get_or_insert(t);
    }

    /// The loop submitted a frame at `t`, the runner's `seq` then, under
    /// the presenter's `host`th host. Another host (a reload, an update) is a
    /// runner numbering its transactions afresh: the samples start over.
    pub fn submitted(&mut self, t: f64, seq: u64, host: u64) {
        self.host(host);
        self.pending = Some((self.wanted.take().unwrap_or(t), seq));
    }

    /// The presenter's `host`th host is current: under another, every
    /// sample and the flip awaited (the old runner's picture) are dropped.
    pub fn host(&mut self, host: u64) {
        if host != self.host {
            let wanted = self.wanted;
            *self = Frames::new(0).with_period(self.period_ms);
            (self.host, self.wanted) = (host, wanted);
        }
    }

    fn with_period(mut self, period_ms: f64) -> Frames {
        self.period_ms = period_ms;
        self
    }

    /// The submitted frame's flip completed: vblank `sequence` at `t`, with
    /// its paint time. The journal line when it was late.
    pub fn flipped(&mut self, sequence: u32, t: f64, paint: Option<f64>) -> Option<String> {
        let r2 = |x: f64| (x * 100.0).round() / 100.0;
        let (wanted, seq) = self.pending.take()?;
        let carried = (seq > self.seq_mark).then_some((self.seq_mark + 1, seq));
        self.seq_mark = seq;
        let (interval, missed) = match self.last.replace((sequence, t)) {
            Some((before, at)) if wanted <= at + self.period_ms / 2.0 => {
                (t - at, sequence.wrapping_sub(before).saturating_sub(1))
            }
            _ => {
                self.segments += 1;
                let since = t - wanted;
                (
                    since,
                    ((since / self.period_ms).floor() as u32).saturating_sub(1),
                )
            }
        };
        let record = Record {
            t: r2(t),
            interval: r2(interval),
            missed,
            seq: carried,
            paint: paint.map(r2),
        };
        self.ring.push_back(record.clone());
        if self.ring.len() > RING {
            self.ring.pop_front();
            self.dropped += 1;
        }
        self.presented += 1;
        if missed == 0 {
            return None;
        }
        self.late_count += 1;
        self.missed += u64::from(missed);
        self.late.push_back(record.clone());
        if self.late.len() > LATE {
            self.late.pop_front();
        }
        let range = carried.map_or(String::new(), |(a, b)| format!(" seq {a}..{b}"));
        let paint = record
            .paint
            .map_or(String::new(), |p| format!(" paint {p}"));
        Some(format!(
            "frame late at {}: {missed} missed ({} ms / {} refresh){range}{paint}",
            record.t,
            record.interval,
            r2(self.period_ms)
        ))
    }

    /// `{"op":"perf","frames":true}`'s shape (D4); `all` adds every
    /// retained record (a trace).
    pub fn reply(&self, late: usize, all: bool) -> Value {
        let mut xs: Vec<f64> = self.ring.iter().map(|r| r.interval).collect();
        xs.sort_by(f64::total_cmp);
        let q = |f: f64| {
            xs.get(((f * xs.len() as f64) as usize).min(xs.len().max(1) - 1))
                .copied()
        };
        let mut out = json!({
            "period": { "ms": (self.period_ms * 100.0).round() / 100.0, "source": "refresh" },
            "covers": ["page-flips"],
            "lifetime": { "presented": self.presented, "late": self.late_count, "missed": self.missed, "segments": self.segments },
            "window": { "from": self.ring.front().map(|r| r.t), "to": self.ring.back().map(|r| r.t),
                "samples": self.ring.len(), "dropped": self.dropped,
                "p50": q(0.5), "p95": q(0.95), "p99": q(0.99), "max": xs.last() },
            "late": self.late.iter().rev().take(late.min(LATE)).rev().map(Record::json).collect::<Vec<_>>(),
        });
        if all {
            out["records"] = self.ring.iter().map(Record::json).collect();
        }
        out
    }
}

/// This binary's app: its name without the host suffix (`caltrain-linux`).
pub fn app_name() -> String {
    let exe = std::env::current_exe().ok();
    let stem = exe
        .as_deref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    stem.trim_end_matches("-linux").to_string()
}

/// `SIGUSR1`'s trace (D5): the journal, the frames, `perf` over every root,
/// who made them and each timing's proxy, as `trace-<unix ms>.json` in the
/// temporary directory; `agent.mjs trace <file>` reads it back. The path.
pub fn save_trace<D: DataSource>(
    p: &mut Presenter<D>,
    frames: &Frames,
    app: &str,
) -> Result<std::path::PathBuf, String> {
    let read = |request: &str| {
        serde_json::from_str::<Value>(&p.host().agent(request)).unwrap_or(Value::Null)
    };
    let perf = read(r#"{"op":"perf"}"#);
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let trace = json!({
        "identity": { "host": "linux", "app": app, "trust": "development",
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "proxies": { "period": "refresh: the display mode's refresh rate",
            "interval": "the gap between page-flip completions", "missed": "the vblank-sequence gap − 1",
            "covers": ["page-flips"], "loaf": "none on this host" },
        "plan": perf.get("plan").cloned().unwrap_or(Value::Null),
        "journal": read(r#"{"op":"logs","since":0}"#),
        "frames": frames.reply(LATE, true),
        "perf": perf,
    });
    let path = std::env::temp_dir().join(format!("trace-{wall}.json"));
    std::fs::write(&path, trace.to_string()).map_err(|e| format!("trace refused: {e}"))?;
    p.host_mut().log(format!("trace saved: {}", path.display()));
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::Frames;

    #[test]
    fn a_flip_is_late_by_its_vblank_gap_a_slow_start_by_its_wait_and_idle_never() {
        let mut f = Frames::new(60);
        let p = 1000.0 / 60.0;
        f.dirty(0.0);
        f.submitted(0.0, 1, 0);
        assert_eq!(
            f.flipped(100, p, None),
            None,
            "wanted at 0, shown at the next vblank"
        );
        f.dirty(p);
        f.submitted(p, 2, 0);
        assert_eq!(
            f.flipped(101, 2.0 * p, Some(3.0)),
            None,
            "continuous, on time"
        );
        // Continuous, and a render that took three more vblanks.
        f.dirty(2.0 * p);
        f.submitted(5.0 * p, 4, 0);
        let line = f
            .flipped(105, 6.0 * p, Some(40.0))
            .expect("three vblanks missed");
        assert!(
            line.starts_with(
                "frame late at 100: 3 missed (66.67 ms / 16.67 refresh) seq 3..4 paint 40"
            ),
            "{line}"
        );
        // Idle, then a frame wanted at 1000 ms that took 40 ms to render.
        f.dirty(1000.0);
        f.submitted(1040.0, 5, 0);
        let line = f
            .flipped(170, 1000.0 + 3.0 * p, None)
            .expect("a slow first frame is late");
        assert!(
            line.contains(": 2 missed (50 ms / 16.67 refresh) seq 5..5"),
            "{line}"
        );
        // Idle, then a quick frame: never late, though the vblank gap is large.
        f.dirty(3000.0);
        f.submitted(3001.0, 5, 0);
        assert_eq!(f.flipped(300, 3000.0 + p, None), None);
        let reply = f.reply(20, true);
        assert_eq!(reply["lifetime"]["presented"], 5);
        assert_eq!(reply["lifetime"]["late"], 2);
        assert_eq!(reply["lifetime"]["missed"], 5);
        assert_eq!(reply["lifetime"]["segments"], 3);
        assert_eq!(reply["late"][0]["seq"], serde_json::json!([3, 4]));
        assert_eq!(reply["records"].as_array().unwrap().len(), 5);
        assert_eq!(reply["period"]["source"], "refresh");
        // Another host while a flip is awaited: its picture is the old runner's.
        f.dirty(3050.0);
        f.submitted(3051.0, 6, 0);
        f.host(1);
        assert_eq!(f.flipped(310, 3060.0, None), None);
        assert_eq!(f.reply(20, false)["lifetime"]["presented"], 0);
        // Another host (a reload): the samples start over.
        f.submitted(3100.0, 1, 2);
        assert_eq!(f.reply(20, false)["lifetime"]["presented"], 0);
    }
}
