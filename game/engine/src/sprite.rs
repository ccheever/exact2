//! Camera-facing textured quads and saved atlas animation; no second simulation.
use crate::{asset::AlphaMode, Component, Vec2, World};

/// A textured quad. Negative Transform scale changes geometry, not texture flip.
#[derive(Clone, Debug, Component)]
pub struct Sprite {
    /// Baked .tex asset name. Declare in Game::ASSETS when setup must wait for it.
    pub texture: String,
    /// Width and height in world units.
    pub size: Vec2,
    /// Pivot measured from the lower-left corner, normally within [0,1].
    pub anchor: Vec2,
    /// Flip texture coordinates horizontally and vertically.
    pub flip: [bool; 2],
    /// Linear RGBA tint.
    pub color: [f32; 4],
    /// Equal-depth translucent ordering; smaller layers draw first.
    pub layer: i32,
    /// Atlas rectangle [x, y, width, height] in texels from the upper-left.
    /// Zero width or height selects the entire texture.
    pub frame: [u16; 4],
    /// Opaque, alpha mask, or ordered alpha blending.
    pub alpha: AlphaMode,
    /// Alpha-mask threshold.
    pub cutoff: f32,
}
impl Default for Sprite {
    fn default() -> Self {
        Self {
            texture: String::new(),
            size: Vec2::ONE,
            anchor: Vec2::splat(0.5),
            flip: [false; 2],
            color: [1.; 4],
            layer: 0,
            frame: [0; 4],
            alpha: AlphaMode::Blend,
            cutoff: 0.5,
        }
    }
}
impl Sprite {
    /// A centered, alpha-blended quad with the given full dimensions.
    pub fn new(texture: impl Into<String>, size: impl Into<Vec2>) -> Self {
        Self {
            texture: texture.into(),
            size: size.into(),
            ..Self::default()
        }
    }
    /// Validate independent of asynchronous asset arrival.
    pub fn validate(&self) -> Result<(), String> {
        if self.texture.ends_with(".tex")
            && self.size.is_finite()
            && self.size.min_element() > 0.
            && self.anchor.is_finite()
            && self.color.iter().all(|n| n.is_finite())
            && self.cutoff.is_finite()
            && (0. ..=1.).contains(&self.cutoff)
        {
            Ok(())
        } else {
            Err(format!(
                "Sprite `{}`: expected .tex, finite positive size, anchor, color and cutoff",
                self.texture
            ))
        }
    }
}
/// Saved atlas animation. Call sprite::step in the game's tick.
#[derive(Clone, Debug, Component)]
pub struct SpriteAnimation {
    /// Atlas rectangles in playback order.
    pub frames: Vec<[u16; 4]>,
    /// Playback frames per second.
    pub fps: f32,
    /// Repeat after the last frame.
    pub looping: bool,
    /// Completed animation ticks, saved and hashed.
    pub age: u64,
    /// Current frame index, saved and inspectable.
    pub frame: u32,
}
impl Default for SpriteAnimation {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            fps: 12.,
            looping: true,
            age: 0,
            frame: 0,
        }
    }
}
impl SpriteAnimation {
    /// Build a looping animation from atlas rectangles.
    pub fn new(frames: impl IntoIterator<Item = [u16; 4]>, fps: f32) -> Self {
        Self {
            frames: frames.into_iter().collect(),
            fps,
            ..Self::default()
        }
    }
}
/// Advance atlas frames at the world's fixed rate; finite animations stop changing.
pub fn step(w: &World) {
    for (entity, a) in w.query::<&mut SpriteAnimation>().iter() {
        if a.frames.is_empty() || !a.fps.is_finite() || a.fps <= 0. {
            continue;
        }
        if !a.looping && a.frame as usize == a.frames.len() - 1 {
            continue;
        }
        a.age += 1;
        let frame = (a.age as f64 * a.fps as f64 / w.hz() as f64) as u64;
        a.frame = if a.looping {
            (frame % a.frames.len() as u64) as u32
        } else {
            frame.min(a.frames.len() as u64 - 1) as u32
        };
        if let Some(mut sprite) = w.get_mut::<Sprite>(entity) {
            sprite.frame = a.frames[a.frame as usize];
        }
    }
}
