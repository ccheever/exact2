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
pub mod collect;
mod color;
pub mod data;
pub mod diagnostics;
pub mod emitter;
mod environment;
mod events;
pub use events::TypedEvent;
mod input;
pub mod math;
pub mod motion;
mod node_offsets;
mod particle_look;
mod placed;
mod present;
pub use node_offsets::{NodeName, NodeOffset, NodeOffsets};
pub mod recipes;
pub mod rig;
mod rng;
pub mod scene;
mod sim;
mod spatial;
pub mod spawn;
mod spring;
pub mod sprite;
pub mod steer;
#[allow(unsafe_code)]
mod storage;
mod tween;
mod values;
mod world;

pub use animation::{Animation, Animator, Blend, Cmp, Condition, Ik, Play, SocketFollow, State};
pub use args::{Args, ArgumentKind};
pub use asset::pose::Pose;
pub use color::Color;
pub use data::{bin, hash, json, Data, DataError, Number, Reader, Writer};
pub use emitter::Emitter;
pub use environment::{
    AmbientOcclusion, AoQuality, Bloom, Caustics, Environment, EnvironmentMap, Fog, Quality,
    SceneSamples, ShadowBox, ShadowFilter, Shadows, Shafts, ToneMapping, Water, Waves,
    AUTO_RENDER_SCALE,
};
pub use exact_game_derive::{Args, Component, Data, Event, Presentation, Resource};
pub use exact_motion::spring::SpringConfig;
pub use exact_plan::Value;
pub use glam::{Affine3A, Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4};
pub use input::{
    tilt_stick, Actions, Binding, Bindings, Curve, Input, InputContexts, InputEvent, InputState,
    Pad, PadState, PointerPhase, PointerState, Region, SensorKind, SensorSample, SensorState,
    Stick, Tilt, MAX_PADS, MOUSE_BUTTONS, PAD_BUTTONS,
};
pub use particle_look::ParticleLook;
pub use placed::{CanvasChild, Facing, Placed, PlacedPlane};
pub use present::{Present, PresentRead, PresentationComponent};
pub use rng::{RangeValue, Rng};
pub use scene::*;
pub use sim::{Clock, Game, Now, Paranoid, Sim};
pub use spring::Spring;
pub use sprite::{Sprite, SpriteAnimation};
pub use storage::{
    Page, Pages, Plain, Query, QueryBorrow, QueryIter, QueryRows, Ref, RefMut, PAGE,
};
pub use tween::Tween;
pub use values::Published;
pub use world::{
    Bundle, Component, Entity, Event, Near, PropBuilder, Resource, SpriteBuilder, Target, World,
    WorldId,
};

pub mod audio;

/// Baked GPU-ready assets, independent of the renderer.
pub mod asset;

/// Typed spatial results shared with the agent geometry.
pub use spatial::{EntityLayout, PickHit, ScreenRect};
