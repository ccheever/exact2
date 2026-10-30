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
        .map(|(n, v)| {
            (
                (*n).into(),
                observed_frame_value(n, v, s.submitted_available, s.displayed_available),
            )
        })
        .collect();
    let high: serde_json::Map<_, _> = names
        .iter()
        .zip(s.high_water)
        .map(|(n, v)| {
            (
                (*n).into(),
                observed_frame_value(n, v, s.submitted_available, s.displayed_available),
            )
        })
        .collect();
    let displayed = observed_frame_value(
        "displayed_frames",
        s.displayed,
        s.submitted_available,
        s.displayed_available,
    );
    serde_json::json!({"timeUnixMs": exact_ink::counters::timestamp(std::time::Instant::now()) / 1000.,
    "values": values, "highWater": high, "updates": s.updates, "displayedFrames": displayed,
    "poolGauges": {
        "ownership": "targets_retained, target_bytes, targets_reserved_this_frame: after each pool mutation; bytes are logical RGBA storage",
        "completion": "targets_incomplete, targets_idle, target_idle_bytes: last take partition, excluding targets reserved in that frame",
        "completionSample": "target_partition_unix_us: anchored timestamp; target_partition_completed: existing completion serial used by that take"
    }})
}

fn observed_frame_value(
    name: &str,
    value: u64,
    submitted: bool,
    displayed: bool,
) -> serde_json::Value {
    if (name == "submitted_frames" && !submitted) || (name == "displayed_frames" && !displayed) {
        serde_json::Value::Null
    } else {
        value.into()
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_path_without_present_hooks_reports_unavailable() {
        for name in ["submitted_frames", "displayed_frames"] {
            assert!(super::observed_frame_value(name, 0, false, false).is_null());
        }
        assert_eq!(
            super::observed_frame_value("submitted_frames", 3, true, false),
            3
        );
        assert!(super::observed_frame_value("displayed_frames", 0, true, false).is_null());
        assert_eq!(
            super::observed_frame_value("displayed_frames", 0, true, true),
            0
        );
    }
}
