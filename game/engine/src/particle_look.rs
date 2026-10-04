//! How an emitter's particles look beyond its colour and size: an atlas texture
//! or flipbook, and velocity stretch. Presentation only; saved like any component.
use crate::Component;

/// Attach beside an `Emitter`. Without it particles are round soft dots.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct ParticleLook {
    /// A `.tex` asset (declare it in `Game::ASSETS`); empty draws soft dots.
    /// The particle colour multiplies it; alpha or additive follows the emitter.
    pub texture: String,
    /// Atlas columns and rows, read left to right then top to bottom.
    pub atlas: [u32; 2],
    /// Flipbook frames per second; zero spreads the frames over each lifetime.
    pub fps: f32,
    /// Seconds of velocity added to the length along the motion: sparks and
    /// rain streaks. Zero keeps camera-facing squares.
    pub stretch: f32,
}
impl Default for ParticleLook {
    fn default() -> Self {
        Self {
            texture: String::new(),
            atlas: [1, 1],
            fps: 0.0,
            stretch: 0.0,
        }
    }
}
impl ParticleLook {
    /// The atlas frame for a particle `age` seconds into `lifetime`.
    pub fn frame(&self, age: f32, lifetime: f32) -> u32 {
        let frames = self.atlas[0].max(1) * self.atlas[1].max(1);
        let at = if self.fps > 0.0 {
            (age * self.fps) as u32
        } else {
            (age / lifetime.max(1e-6) * frames as f32) as u32
        };
        if self.fps > 0.0 {
            at % frames
        } else {
            at.min(frames - 1)
        }
    }
    /// The frame's texture rectangle: x, y, width, height in 0..1.
    pub fn uv(&self, frame: u32) -> [f32; 4] {
        let [cols, rows] = self.atlas.map(|n| n.max(1));
        let (w, h) = (1.0 / cols as f32, 1.0 / rows as f32);
        [
            (frame % cols) as f32 * w,
            (frame / cols % rows) as f32 * h,
            w,
            h,
        ]
    }
}
