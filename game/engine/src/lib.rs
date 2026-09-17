//! Deterministic game state, ordered entities, and a scene without a host.
#![deny(missing_docs)]
#![deny(unsafe_code)]

extern crate self as exact_game;

pub mod data;
pub mod math;
mod rng;
mod scene;
mod spring;
#[allow(unsafe_code)]
mod storage;
mod world;

pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
pub use exact_game_derive::{Component, Data};
pub use exact_motion::spring::SpringConfig;
pub use exact_plan::Value;
pub use glam::{Affine3A, Quat, Vec2, Vec3, Vec4};
pub use rng::{RangeValue, Rng};
pub use scene::*;
pub use spring::Spring;
pub use storage::{Query, QueryIter, Ref, RefMut};
pub use world::{Bundle, Component, Entity, Event, Resource, World};
