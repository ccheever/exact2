//! Deterministic game state, ordered entities, and a scene without a host.
#![deny(missing_docs)]
#![deny(unsafe_code)]

extern crate self as exact_game;

mod agent;
mod args;
pub mod data;
mod environment;
mod input;
pub mod math;
mod rng;
mod scene;
mod sim;
mod spatial;
mod spring;
#[allow(unsafe_code)]
mod storage;
mod values;
mod world;

pub use args::{Arg, Args};
pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
pub use environment::{Bloom, Environment, Fog};
pub use exact_game_derive::{Component, Data, Resource};
pub use exact_motion::spring::SpringConfig;
pub use exact_plan::Value;
pub use glam::{Affine3A, Quat, Vec2, Vec3, Vec4};
pub use input::{Actions, Input, InputEvent, PointerPhase, PointerState, Region, Stick};
pub use rng::{RangeValue, Rng};
pub use scene::*;
pub use sim::{Clock, Game, Now, Sim};
pub use spring::Spring;
pub use storage::{Page, Pages, Plain, Query, QueryBorrow, QueryIter, Ref, RefMut, PAGE};
pub use values::Published;
pub use world::{Bundle, Component, Entity, Event, Resource, World};

pub mod audio;
