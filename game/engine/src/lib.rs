#![doc = include_str!("../../README.md")]
//! Deterministic game state, ordered entities, and a scene without a host.
#![deny(missing_docs)]
#![deny(unsafe_code)]

extern crate self as exact_game;

mod agent;
#[doc(hidden)]
pub mod args;
mod capture;
pub mod character;
pub mod data;
mod environment;
mod input;
pub mod math;
pub mod motion;
mod rng;
pub mod scene;
mod sim;
mod spatial;
mod spring;
#[allow(unsafe_code)]
mod storage;
mod tween;
mod values;
mod world;

pub use args::{Args, ArgumentKind};
pub use capture::{Capture, CaptureLimits};
pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
pub use environment::{Bloom, Environment, Fog};
pub use exact_game_derive::{Args, Component, Data, Resource};
pub use exact_motion::spring::SpringConfig;
pub use exact_plan::Value;
pub use glam::{Affine3A, Quat, Vec2, Vec3, Vec4};
pub use input::{Actions, Input, InputEvent, PointerPhase, PointerState, Region, Stick};
pub use rng::{RangeValue, Rng};
pub use scene::*;
pub use sim::{Clock, Game, Now, Sim};
pub use spring::Spring;
pub use storage::{
    Page, Pages, Plain, Query, QueryBorrow, QueryIter, QueryRows, Ref, RefMut, PAGE,
};
pub use tween::Tween;
pub use values::Published;
pub use world::{Bundle, Component, Entity, Event, Resource, Target, World, WorldId};

pub mod audio;

/// Baked GPU-ready assets, independent of the renderer.
pub mod asset;
