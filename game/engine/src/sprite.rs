//! Camera-facing textured quads and saved atlas animation; no second simulation.
use crate::{asset::AlphaMode, Component, Vec2, World};

/// A textured quad. Transform scale is magnitude-only; Sprite.flip flips the texture.
#[derive(Debug, Component)]
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
    /// Translucency sorts back-to-front by depth first. At equal depth only,
    /// smaller layers draw first; this is not a 2D z-order.
    pub layer: i32,
    /// Atlas rectangle [x, y, width, height] in texels from the upper-left.
    /// Zero width or height selects the entire texture.
    pub frame: [u16; 4],
    /// Opaque, alpha mask, or ordered alpha blending.
    pub alpha: AlphaMode,
    /// Alpha-mask threshold.
    pub cutoff: f32,
}
impl Clone for Sprite {
    fn clone(&self) -> Self {
        Self {
            texture: self.texture.clone(),
            ..*self
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.texture.clone_from(&source.texture);
        self.size = source.size;
        self.anchor = source.anchor;
        self.flip = source.flip;
        self.color = source.color;
        self.layer = source.layer;
        self.frame = source.frame;
        self.alpha = source.alpha;
        self.cutoff = source.cutoff;
    }
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
    /// Horizontal atlas strip, with checked texel coordinates.
    pub fn strip(origin: [u16; 2], size: [u16; 2], frames: u16, fps: f32) -> Self {
        Self::new(
            (0..frames).map(|i| {
                [
                    origin[0]
                        .checked_add(size[0].checked_mul(i).expect("sprite strip width overflow"))
                        .expect("sprite strip coordinate overflow"),
                    origin[1],
                    size[0],
                    size[1],
                ]
            }),
            fps,
        )
    }
    /// Pair with a sprite, initializing its rectangle before the first tick.
    /// Works identically for strips and explicit, irregular frame lists.
    pub fn sprite(self, mut sprite: Sprite) -> (Sprite, Self) {
        if let Some(frame) = self.frames.first() {
            sprite.frame = *frame;
        }
        (sprite, self)
    }
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
        let frame = a.frames[a.frame as usize];
        if w.get::<Sprite>(entity).is_some_and(|s| s.frame != frame) {
            w.get_mut::<Sprite>(entity).unwrap().frame = frame;
        }
    }
}

pub(crate) fn texture_names_changed(w: &World, names: &mut Vec<(crate::Entity, String)>) -> bool {
    if w.query::<&Sprite>()
        .iter()
        .map(|(e, s)| (e, s.texture.as_str()))
        .eq(names.iter().map(|(e, n)| (*e, n.as_str())))
    {
        return false;
    }
    names.clear();
    names.extend(
        w.query::<&Sprite>()
            .iter()
            .map(|(e, s)| (e, s.texture.clone())),
    );
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_frames_do_not_invalidate_texture_roots() {
        let mut w = World::new(60, 0);
        let e = w.spawn(Sprite::new("one.tex", [1., 1.]));
        let mut names = Vec::new();
        assert!(texture_names_changed(&w, &mut names));
        let pointer = names[0].1.as_ptr();
        w.get_mut::<Sprite>(e).unwrap().frame = [16, 0, 16, 16];
        assert!(!texture_names_changed(&w, &mut names));
        assert_eq!(names[0].1.as_ptr(), pointer);
        w.get_mut::<Sprite>(e).unwrap().texture = "two.tex".into();
        assert!(texture_names_changed(&w, &mut names));
        assert_eq!(names[0].1, "two.tex");
    }
}

#[cfg(test)]
mod strip_tests {
    use super::*;
    #[test]
    fn strip_initializes_the_same_first_frame_as_explicit_frames() {
        let strip = SpriteAnimation::strip([4, 8], [16, 12], 2, 6.);
        assert_eq!(strip.frames, [[4, 8, 16, 12], [20, 8, 16, 12]]);
        let sprite = Sprite::new("strip.tex", [1., 1.]);
        let (sprite, animation) = strip.sprite(sprite);
        assert_eq!(sprite.frame, animation.frames[0]);
        let (explicit, _) =
            SpriteAnimation::new([[4, 8, 16, 12]], 6.).sprite(Sprite::new("strip.tex", [1., 1.]));
        assert_eq!(sprite.frame, explicit.frame);
    }
}
