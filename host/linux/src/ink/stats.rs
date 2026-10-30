//! Completed-frame counters; a reader never borrows the painter. @ref LLP 1149 D8.
use super::*;
use exact_ink::atlas::Stats;
use std::{cell::Cell, rc::Rc};

/// Timestamped cumulative observations and high-water marks for the agent.
pub fn counter_state() -> serde_json::Value {
    let Some(s) = exact_ink::counters::snapshot() else {
        return serde_json::Value::Null;
    };
    let names = exact_ink::counters::NAMES;
    let values: serde_json::Map<_, _> = names
        .iter()
        .zip(s.values)
        .map(|(n, v)| ((*n).into(), v.into()))
        .collect();
    let high: serde_json::Map<_, _> = names
        .iter()
        .zip(s.high_water)
        .map(|(n, v)| ((*n).into(), v.into()))
        .collect();
    let displayed = if cfg!(target_os = "macos") {
        serde_json::json!(s.displayed)
    } else {
        serde_json::Value::Null
    };
    serde_json::json!({"timeUnixMs": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64() * 1000.,
        "values": values, "highWater": high, "updates": s.updates, "displayedFrames": displayed})
}

/// Retained atlas storage and work performed in the last completed frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Statistics {
    /// Completed painter snapshots, including frames whose present was refused.
    pub snapshots: u64,
    /// Grayscale coverage, one byte per atlas pixel.
    pub glyphs: Stats,
    /// Premultiplied colour glyphs, four bytes per atlas pixel.
    pub color_glyphs: Stats,
    /// Geometry and coverage clips share this atlas.
    pub paths: Stats,
    /// Gradient stop rows, independent of their placement.
    pub ramps: Stats,
    /// Viewport targets used this frame and retained at the pool's high-water mark.
    pub layers: usize,
    /// Number of retained viewport targets.
    pub layer_targets: usize,
    /// Retained viewport-target storage, excluding the final readback target.
    pub layer_bytes: u64,
}

/// A snapshot handle remains readable while the presenter owns the backend.
#[derive(Clone, Default)]
pub struct StatisticsReader(Rc<Cell<Statistics>>);

impl StatisticsReader {
    /// Copy the last completed frame's counters.
    pub fn read(&self) -> Statistics {
        self.0.get()
    }
}

impl Ink {
    /// Observe counters after ownership passes to a presenter.
    pub fn statistics(&self) -> StatisticsReader {
        self.statistics.clone()
    }

    pub(super) fn update_statistics(&self) {
        let (glyphs, color_glyphs) = self.glyphs.stats();
        self.statistics.0.set(Statistics {
            snapshots: self.statistics.read().snapshots + 1,
            glyphs,
            color_glyphs,
            paths: self.paths.stats(),
            ramps: self.ramps.stats(),
            layers: self.pool.used(),
            layer_targets: self.pool.retained(),
            layer_bytes: self.pool.bytes(),
        });
    }
}

impl Drop for Ink {
    fn drop(&mut self) {
        if std::env::var("EXACT_AGENT_WINDOW_PROBE").as_deref() == Ok("on") {
            eprintln!(
                "exact-ink: painter snapshots {}",
                self.statistics.read().snapshots
            );
        }
    }
}
