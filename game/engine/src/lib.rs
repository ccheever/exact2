#![cfg_attr(doc, doc = include_str!("../../README.md"))]
//! Deterministic game state, ordered entities, and a scene without a host.
#![deny(missing_docs)]
#![deny(unsafe_code)]

extern crate self as exact_game;

mod agent;
pub mod animation;
#[doc(hidden)]
pub mod args;
mod assets_load;
pub mod character;
pub mod data;
pub mod emitter;
mod environment;
mod input;
pub mod math;
pub mod motion;
mod particle_look;
mod placed;
mod present;
pub mod rig;
mod rng;
pub mod scene;
mod sim;
mod spatial;
mod spring;
pub mod sprite;
#[allow(unsafe_code)]
mod storage;
mod tween;
mod values;
mod world;

pub use animation::{Animation, Animator, Blend, Cmp, Condition, Ik, Play, SocketFollow, State};
pub use args::{Args, ArgumentKind};
pub use asset::pose::Pose;
pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
pub use emitter::Emitter;
pub use environment::{AmbientOcclusion, AoQuality, Bloom, Environment, EnvironmentMap, Fog};
pub use exact_game_derive::{Args, Component, Data, Presentation, Resource};
pub use exact_motion::spring::SpringConfig;
pub use exact_plan::Value;
pub use glam::{Affine3A, Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4};
pub use input::{
    Actions, Input, InputEvent, PointerPhase, PointerState, Region, Stick, MOUSE_BUTTONS,
};
pub use particle_look::ParticleLook;
pub use placed::{CanvasChild, Facing, Placed, PlacedPlane};
pub use present::{Derived, Keys, Present, PresentationComponent};
pub use rng::{RangeValue, Rng};
pub use scene::*;
pub use sim::{Clock, Delivery, Game, Now, Paranoid, Sim};
pub use spring::Spring;
pub use sprite::{Sprite, SpriteAnimation};
pub use storage::{
    Page, Pages, Plain, Query, QueryBorrow, QueryIter, QueryRows, Ref, RefMut, PAGE,
};
pub use tween::Tween;
pub use values::Published;
pub use world::{Bundle, Component, Entity, Event, Registered, Resource, Target, World, WorldId};

pub mod audio;

/// Baked GPU-ready assets, independent of the renderer.
pub mod asset;

/// Typed spatial results shared with the agent geometry.
pub use spatial::{EntityLayout, PickHit, ScreenRect};
