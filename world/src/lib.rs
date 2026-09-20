//! Deterministic, nonspatial state and fixed ticks under a caller-owned clock.
#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
extern crate self as exact_world;
pub use exact_motion::SpringConfig;
pub use exact_world_derive::{Args, Component, Data, Resource};
pub mod data;
pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
#[allow(unsafe_code)]
pub mod storage;
pub use storage::{Page, Pages, Query, QueryBorrow, QueryIter, QueryRows, Ref, RefMut, PAGE};
mod world;
pub use world::inspect::{Ambient, Candidate, Readiness, Sample, Work};
pub use world::journal::{
    Change, ChangeConsumer, ChangeKind, Changes, Event, EventKind, LogCursor, Logs,
};
pub use world::ownership::Parent;
pub use world::{Bundle, Component, Entity, Resource, Target, World, WorldId};
pub mod args;
pub use args::{Args, ArgumentKind};
mod values;
pub use values::Published;
pub mod math;
mod rng;
pub use rng::Rng;
mod spring;
pub use spring::Spring;
mod tween;
pub use tween::Tween;
mod input;
pub use input::{stick_axis, Action, Input, InputEvent};
mod sim;
pub use sim::{Game, Now, Paranoid, Sim};
/// Bound for entity-indexed work, including dead slots.
pub const MAX_ENTITIES: usize = 200_000;

#[cfg(test)]
#[allow(dead_code, unsafe_code)]
#[path = "storage/counting.rs"]
mod counting;
