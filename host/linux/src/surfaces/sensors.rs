//! A package feeds a canvas (LLP 1133.013 D6) on Android: its Kotlin writes
//! each sample into the surface's input (`SurfaceInput`, `Native.sensorInput`)
//! from whatever thread its listener runs on, without waiting for the host's;
//! it waits here until the host's next display frame, which delivers every
//! sample since the last, in order, to the live canvas of that surface with
//! the lowest view id, before the frame's render ticks its world. A sample
//! with no live canvas to take it is gone, as a touch over nothing is.
//!
//! Android runs one session a process, so the queue is the process's.

use super::super::Surfaces;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Mutex;

/// About a second at the fastest rate a phone samples (Android's 200 Hz), a
/// surface: a world queues at most 1,024 events a frame
/// (`game/engine/src/sim/queue.rs`).
const BOUND: usize = 256;

struct Sample {
    sensor: String,
    values: [f64; 4],
    dropped: u32,
    at: f64,
    ended: bool,
}

#[derive(Default)]
struct Waiting {
    by_surface: BTreeMap<String, VecDeque<Sample>>,
    /// Samples the bound let go, by surface and sensor: counted into the
    /// next of that sensor delivered.
    lost: BTreeMap<(String, String), u32>,
}

static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);
/// The page's surface names as of the host's last turn: what a start asks.
static NAMES: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// The surfaces whose samples went to no canvas, told once each.
static TOLD: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// A sample of `sensor` written into `surface`'s input, or its end. Any
/// thread. Whether none waited before it: the writer then wakes the host.
pub fn write(
    surface: &str,
    sensor: &str,
    values: &[f64],
    dropped: u32,
    at: f64,
    ended: bool,
) -> bool {
    let mut v = [0.0; 4];
    for (to, from) in v.iter_mut().zip(values) {
        *to = if from.is_finite() { *from } else { 0.0 };
    }
    let mut guard = WAITING.lock().unwrap_or_else(|e| e.into_inner());
    let waiting = guard.get_or_insert_with(Waiting::default);
    let first = waiting.by_surface.values().all(VecDeque::is_empty);
    let queue = waiting.by_surface.entry(surface.to_owned()).or_default();
    if queue.len() >= BOUND {
        if let Some(i) = queue.iter().position(|s| !s.ended) {
            let gone = queue.remove(i).expect("a sample");
            let key = (surface.to_owned(), gone.sensor);
            *waiting.lost.entry(key).or_default() += 1 + gone.dropped;
        }
    }
    let sensor = sensor.to_owned();
    queue.push_back(Sample {
        sensor,
        values: v,
        dropped,
        at,
        ended,
    });
    first
}

/// Whether samples wait for a display frame: the host asks for one.
pub(crate) fn waiting() -> bool {
    let guard = WAITING.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .is_some_and(|w| w.by_surface.values().any(|q| !q.is_empty()))
}

/// Whether the page placed a canvas of surface `name` at the host's last turn. Any thread.
pub fn has(name: &str) -> bool {
    NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(name)
}

impl Surfaces {
    /// The host's turn ended: the names a start reads.
    pub(crate) fn publish_names(&self) {
        let names = self.canvases.values().map(|c| c.name.clone()).collect();
        *NAMES.lock().unwrap_or_else(|e| e.into_inner()) = names;
    }

    /// At a display frame, before its render: what packages wrote since the
    /// last, as the input events every host writes (`{"t":"sensor"|"sensorEnded", …}`).
    /// The surfaces no live canvas took samples for, each the first time.
    pub(crate) fn drain_sensors(&mut self) -> Vec<String> {
        let Some(mut waiting) = WAITING.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return Vec::new();
        };
        let mut gone = Vec::new();
        for (surface, queue) in std::mem::take(&mut waiting.by_surface) {
            let view = (self.canvases.iter())
                .filter(|(_, c)| c.name == surface)
                .map(|(view, _)| *view)
                .min();
            for s in queue {
                let event = if s.ended {
                    json!({ "t": "sensorEnded", "sensor": s.sensor, "at": s.at })
                } else {
                    let key = (surface.clone(), s.sensor.clone());
                    let dropped = s.dropped + waiting.lost.remove(&key).unwrap_or(0);
                    json!({ "t": "sensor", "sensor": s.sensor, "values": s.values, "dropped": dropped, "at": s.at })
                };
                if !view.is_some_and(|view| self.input(view, event))
                    && !gone.contains(&surface)
                    && TOLD
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(surface.clone())
                {
                    gone.push(surface.clone());
                }
            }
        }
        // What the bound let go waits for its sensor's next sample.
        let mut guard = WAITING.lock().unwrap_or_else(|e| e.into_inner());
        let now = guard.get_or_insert_with(Waiting::default);
        for (key, n) in waiting.lost {
            *now.lost.entry(key).or_default() += n;
        }
        gone
    }
}
