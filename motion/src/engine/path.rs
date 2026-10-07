//! A path's `d` under `transition` (LLP 1055.000 D15).
//!
//! @ref CSS Transitions 1 §3 (starting, interrupting, reversing)
//!
//! A path is not a [`Value`], so its transition runs in two parts: the
//! node's [`Property::D`] slot runs the curve, a progress from 0 to 1 under
//! the matching declaration, and [`PathTrack`] holds the paths at its ends.
//! The presented path is the one between them at that progress. The slot
//! takes part in everything a curve does — the clock, `frame`,
//! `settle_time`, removal — so a host repaints the path as it would a radius.
//!
//! CSS's rules, by path: a node first seen takes its path with no
//! transition; a new path starts one from the path presented now, when the
//! two interpolate and a declaration covers `d`; otherwise the path changes
//! at once and anything running stops. A change back to where a running
//! transition began reverses it with CSS's shortening. `d` never springs as
//! physics: a `spring()` plays its curve from rest (LLP 1062 D3).

use super::{Engine, Live, Slot};
use crate::path::PathValue;
use crate::property::{Property, Value};
use crate::transition::{Curve, Running};
use std::sync::Arc;

/// One path's `d` and its transition's ends.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PathTrack {
    /// The path the row says now.
    target: Arc<PathValue>,
    /// The path the running transition starts from.
    from: Arc<PathValue>,
    /// CSS §3.2: the path a reversal is measured against.
    reversing_adjusted_start: Arc<PathValue>,
}

impl PathTrack {
    fn settled(path: Arc<PathValue>) -> PathTrack {
        PathTrack {
            from: path.clone(),
            reversing_adjusted_start: path.clone(),
            target: path,
        }
    }
}

impl Engine {
    /// A committed path's `d`: `None` when it is not a path that moves (its
    /// `transition` covers no `d`, or the data has an error), which forgets
    /// the path and stops its transition. Otherwise CSS Transitions §3, as
    /// [`Engine::observe`] for a value.
    pub fn observe_path(&mut self, node: u64, path: Option<PathValue>) {
        let key = (node, Property::D);
        let Some(path) = path.filter(PathValue::is_finite) else {
            if self.paths.remove(&node).is_some() {
                self.running.remove(&key);
                self.slots.remove(&key);
                self.dirty.remove(&key);
            }
            return;
        };
        let Some(track) = self.paths.get(&node) else {
            self.paths.insert(node, PathTrack::settled(Arc::new(path)));
            return;
        };
        if *track.target == path {
            return;
        }
        let now = self.now;
        let running = self.slots.get(&key).and_then(Slot::running).cloned();
        let current = match &running {
            Some(r) => Arc::new(track.from.lerp(&track.target, r.sample(now).value.x)),
            None => track.target.clone(),
        };
        let declaration = self
            .transitions
            .get(&node)
            .and_then(|t| t.matching(Property::D))
            .filter(|t| t.starts())
            .map(|t| t.governing(Property::D));
        let path = Arc::new(path);
        let Some(declaration) =
            declaration.filter(|_| current.interpolable(&path) && *current != *path)
        else {
            // Not interpolable, or nothing covers `d`: at once.
            self.paths.insert(node, PathTrack::settled(path));
            self.running.remove(&key);
            if self.slots.remove(&key).is_some() {
                self.dirty.insert(key);
            }
            return;
        };
        let reverses = running
            .as_ref()
            .filter(|r| matches!(r.curve, Curve::Easing { .. }))
            .filter(|_| *path == *track.reversing_adjusted_start);
        let (adjusted, factor) = match reverses {
            // CSS §3.2, the reversing case.
            Some(r) => (
                track.target.clone(),
                (r.easing_progress(now) * r.reversing_shortening + (1.0 - r.reversing_shortening))
                    .abs()
                    .clamp(0.0, 1.0),
            ),
            None => (current.clone(), 1.0),
        };
        let curve = Running::start(
            &declaration,
            Value::scalar(0.0),
            Value::scalar(1.0),
            Value::ZERO,
            now,
            Value::scalar(0.0),
            factor,
        );
        self.paths.insert(
            node,
            PathTrack {
                target: path,
                from: current,
                reversing_adjusted_start: adjusted,
            },
        );
        let presented = curve.sample(now).value;
        self.slots.insert(
            key,
            Slot {
                target: Value::scalar(1.0),
                live: Some(Box::new(Live {
                    presented,
                    running: Some(curve),
                    owner: None,
                })),
            },
        );
        self.running.insert(key);
        self.dirty.insert(key);
    }

    /// The path a host draws for `node` now: the one between a running
    /// transition's ends, or `None` while nothing runs (the row shows).
    pub fn presented_path(&self, node: u64) -> Option<PathValue> {
        let slot = self.slots.get(&(node, Property::D))?;
        slot.running()?;
        let track = self.paths.get(&node)?;
        Some(track.from.lerp(&track.target, slot.presented().x))
    }
}
