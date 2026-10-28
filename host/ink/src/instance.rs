//! Paint order, batching and clip assignment. @ref LLP 1149 D1.1, D2.4.

use bytemuck::{Pod, Zeroable};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    RoundedRect,
    Ring,
    Shadow,
    Glyph,
    Coverage,
    Layer,
    Blur,
    ColorPass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Brush {
    Solid,
    Gradient,
    Image,
    Pattern,
}

pub mod flags {
    pub const TINT: u32 = 1;
    pub const INSET: u32 = 1 << 1;
    pub const EVEN_ODD: u32 = 1 << 2;
    pub const REPEAT: u32 = 1 << 3;
    pub const NEAREST: u32 = 1 << 4;
    pub const COLOR_GLYPH: u32 = 1 << 5;
    pub const CLIP0: u32 = 1 << 6;
    pub const CLIP1: u32 = 1 << 7;
    pub const MASK: u32 = 1 << 8;
    pub const RADIAL: u32 = 1 << 9;
    pub const CONIC: u32 = 2 << 9;
    /// CSS blend index (0 source-over, 1 multiply, ... 15 luminosity).
    /// A nonzero blend needs a compose target in the layer renderer (D2.5).
    pub const BLEND_SHIFT: u32 = 11;
    pub const BLEND_MASK: u32 = 15 << BLEND_SHIFT;
    pub const RAMP_SHIFT: u32 = 16;
    pub const RAMP_MASK: u32 = 1023 << RAMP_SHIFT;
    pub const SHADOW_LOCAL: u32 = 1 << 26;
    pub const SVG_RADIAL: u32 = 1 << 27;
    pub const REFLECT: u32 = 1 << 28;
    pub const LAYER_MASK: u32 = 1 << 29;
    pub const REPLACE: u32 = 1 << 30;
}

/// Twelve 16-byte vertex attributes, with no storage-buffer dependency.
///
/// D1.1's unpacked field table totals 224 bytes. To meet its 192-byte stride,
/// the affine has no padding, kind/brush share a word, and clip radii use u16
/// at 1/16 device pixel (maximum error 1/32 px). Larger radii require a layer.
/// All positions, shape radii, colours and brush parameters remain f32.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Instance {
    /// Coverage quads use device bounds; their affine still maps brush space
    /// to device space, so a path mask and its gradient share one instance.
    pub rect: [f32; 4],
    /// [a, b, c, d, tx, ty]: (a*x + c*y + tx, b*x + d*y + ty).
    pub xform: [f32; 6],
    /// Kind in bits 0..15, brush in bits 16..31.
    pub kind_brush: u32,
    pub flags: u32,
    /// Top-left, top-right, bottom-right, bottom-left; CSS-reduced by the caller.
    pub radii: [f32; 4],
    /// Encoded-sRGB premultiplied RGBA. @ref LLP 1149 D2.7.
    pub color: [f32; 4],
    pub clip0: [f32; 4],
    pub clip1: [f32; 4],
    /// Two packed words per clip, in the same corner order as `radii`.
    pub clip_radii: [u32; 4],
    pub mask_rect: [f32; 4],
    pub mask_uv: [f32; 4],
    /// Normalized x/y/width/height; Ring: inner rect; Shadow: unblurred rect.
    pub uv: [f32; 4],
    /// Ring: inner radii; Shadow: sigma; Layer: opacity in x.
    /// Gradient: linear endpoints, radial center/radii, or conic center/angle.
    /// Pattern: tile x/y/width/height in local coordinates.
    pub param: [f32; 4],
}

const _: () = assert!(size_of::<Instance>() == 192);

impl Instance {
    pub fn new(kind: Kind, brush: Brush, rect: [f32; 4]) -> Self {
        Self {
            rect,
            xform: [1., 0., 0., 1., 0., 0.],
            kind_brush: kind as u32 | ((brush as u32) << 16),
            color: [1.; 4],
            uv: [0., 0., 1., 1.],
            param: [1., 0., 0., 0.],
            ..Self::zeroed()
        }
    }

    pub fn rounded(rect: [f32; 4], radii: [f32; 4], color: [f32; 4]) -> Self {
        Self {
            radii,
            color,
            ..Self::new(Kind::RoundedRect, Brush::Solid, rect)
        }
    }

    pub fn set_clip(&mut self, slot: usize, clip: RoundedClip) -> Result<(), NeedsLayer> {
        let packed = pack_radii(clip.radii)?;
        match slot {
            0 => {
                self.clip0 = clip.rect;
                self.flags |= flags::CLIP0;
            }
            1 => {
                self.clip1 = clip.rect;
                self.flags |= flags::CLIP1;
            }
            _ => return Err(NeedsLayer),
        }
        self.clip_radii[slot * 2..slot * 2 + 2].copy_from_slice(&packed);
        Ok(())
    }

    pub fn set_mask(&mut self, clip: CoverageClip) {
        self.mask_rect = clip.rect;
        self.mask_uv = clip.uv;
        self.flags |= flags::MASK;
    }
}

fn pack_radii(radii: [f32; 4]) -> Result<[u32; 2], NeedsLayer> {
    if radii
        .iter()
        .any(|r| !r.is_finite() || *r < 0. || *r > 65535. / 16.)
    {
        return Err(NeedsLayer);
    }
    let r = radii.map(|r| (r * 16.).round() as u32);
    Ok([r[0] | (r[1] << 16), r[2] | (r[3] << 16)])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

/// Zero selects the device's white fallback for that slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bindings {
    pub glyph: TextureId,
    pub color: TextureId,
    pub path: TextureId,
    pub ramp: TextureId,
    pub image: TextureId,
    pub mask: TextureId,
    pub destination: TextureId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scissor {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Scissor {
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self
            .x
            .saturating_add(self.width)
            .min(other.x.saturating_add(other.width));
        let bottom = self
            .y
            .saturating_add(self.height)
            .min(other.y.saturating_add(other.height));
        Self {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    pub instances: Range<u32>,
    pub scissor: Scissor,
    pub bindings: Bindings,
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub instances: Vec<Instance>,
    pub batches: Vec<Batch>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedClip {
    pub rect: [f32; 4],
    pub radii: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverageClip {
    pub texture: TextureId,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub enum Clip {
    Scissor(Scissor),
    Rounded(RoundedClip),
    Coverage(CoverageClip),
    /// A rotated or skewed clip must be applied when a layer is composed.
    Transformed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeedsLayer;

#[derive(Clone, Copy)]
struct ClipState {
    scissor: Scissor,
    rounded: [Option<RoundedClip>; 2],
    coverage: Option<CoverageClip>,
    needs_layer: bool,
}

pub struct Encoder {
    frame: Frame,
    state: ClipState,
    stack: Vec<ClipState>,
}

impl Encoder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            frame: Frame::default(),
            state: ClipState {
                scissor: Scissor {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                rounded: [None; 2],
                coverage: None,
                needs_layer: false,
            },
            stack: Vec::new(),
        }
    }

    /// A refusal remains on the stack; drawing refuses until the caller handles
    /// the layer or pops the clip. It never silently drops an overflowing clip.
    pub fn push_clip(&mut self, clip: Clip) -> Result<(), NeedsLayer> {
        self.stack.push(self.state);
        match clip {
            Clip::Scissor(s) => self.state.scissor = self.state.scissor.intersect(s),
            Clip::Rounded(c) => {
                if self.state.rounded[1].is_some() || pack_radii(c.radii).is_err() {
                    self.state.needs_layer = true;
                } else {
                    self.state.rounded[1] = self.state.rounded[0];
                    self.state.rounded[0] = Some(c);
                }
            }
            Clip::Coverage(c) => {
                if self.state.coverage.is_some() {
                    self.state.needs_layer = true;
                } else {
                    self.state.coverage = Some(c);
                }
            }
            Clip::Transformed => self.state.needs_layer = true,
        }
        if self.state.needs_layer {
            Err(NeedsLayer)
        } else {
            Ok(())
        }
    }

    pub fn pop_clip(&mut self) {
        self.state = self.stack.pop().expect("balanced clip stack");
    }

    /// Re-clips an instance, including clips recorded inside a kept row (D4).
    pub fn push(
        &mut self,
        mut instance: Instance,
        mut bindings: Bindings,
    ) -> Result<(), NeedsLayer> {
        if self.state.needs_layer {
            return Err(NeedsLayer);
        }
        for clip in self.state.rounded.into_iter().flatten() {
            let slot = if instance.flags & flags::CLIP0 == 0 {
                0
            } else if instance.flags & flags::CLIP1 == 0
                && instance.kind_brush & 65535 != Kind::Shadow as u32
            {
                1
            } else {
                return Err(NeedsLayer);
            };
            instance.set_clip(slot, clip)?;
        }
        if let Some(clip) = self.state.coverage {
            if instance.flags & flags::MASK != 0 {
                return Err(NeedsLayer);
            }
            instance.set_mask(clip);
            bindings.mask = clip.texture;
        }
        let scissor = self.state.scissor;
        if scissor.width == 0 || scissor.height == 0 {
            return Ok(());
        }
        let index = u32::try_from(self.frame.instances.len()).expect("instance count fits u32");
        if let Some(batch) = self
            .frame
            .batches
            .last_mut()
            .filter(|b| b.scissor == scissor && b.bindings == bindings)
        {
            batch.instances.end = index + 1;
        } else {
            self.frame.batches.push(Batch {
                instances: index..index + 1,
                scissor,
                bindings,
            });
        }
        self.frame.instances.push(instance);
        Ok(())
    }

    pub fn take_frame(&mut self) -> Frame {
        std::mem::take(&mut self.frame)
    }

    pub fn finish(self) -> Frame {
        self.frame
    }
}

#[cfg(test)]
mod tests;
