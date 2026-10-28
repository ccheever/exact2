use crate::atlas::{Atlas, Entry, Stats};
use crate::device::{Device, Result};
use std::collections::HashMap;
use swash::scale::{image::Content, Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Angle, Format, Transform, Vector};

pub struct Font<'a> {
    pub id: [u64; 2],
    pub data: &'a [u8],
    pub index: u32,
    pub coords: &'a [i16],
    pub skew: bool,
}

#[derive(Clone, Copy)]
pub struct Glyph {
    pub entry: Entry,
    pub left: i32,
    pub top: i32,
    pub size: [u32; 2],
    pub color: bool,
}

pub struct Glyphs {
    alpha: Atlas,
    color: Atlas,
    scale: ScaleContext,
    placements: HashMap<Vec<u32>, Glyph>,
    empty: std::collections::HashSet<Vec<u32>>,
}

impl Default for Glyphs {
    fn default() -> Self {
        Self {
            alpha: Atlas::glyphs(),
            color: Atlas::colors(),
            scale: ScaleContext::new(),
            placements: HashMap::new(),
            empty: Default::default(),
        }
    }
}

impl Glyphs {
    pub fn begin(&mut self, device: &mut impl Device) -> Result<()> {
        self.alpha.begin(device)?;
        self.color.begin(device)?;
        self.placements.retain(|k, g| {
            if g.color {
                self.color.contains(k)
            } else {
                self.alpha.contains(k)
            }
        });
        if self.empty.len() > 4096 {
            self.empty.clear();
        }
        Ok(())
    }

    pub fn stats(&self) -> (Stats, Stats) {
        (self.alpha.stats(), self.color.stats())
    }

    pub fn get(
        &mut self,
        device: &mut impl Device,
        font: &Font<'_>,
        glyph: u16,
        size: f32,
        phase: u8,
        sigma: f32,
    ) -> Result<Option<Glyph>> {
        let mut key = vec![
            font.id[0] as u32,
            (font.id[0] >> 32) as u32,
            font.index,
            u32::from(glyph),
            size.to_bits(),
            u32::from(phase),
            u32::from(font.skew),
            sigma.to_bits(),
        ];
        key.extend(font.coords.iter().map(|c| *c as u16 as u32));
        if self.empty.contains(&key) {
            return Ok(None);
        }
        if let Some(g) = self.placements.get(&key).copied() {
            let atlas = if g.color {
                &mut self.color
            } else {
                &mut self.alpha
            };
            if atlas.get(&key).is_some() {
                return Ok(Some(g));
            }
        }
        let Some(face) = swash::FontRef::from_index(font.data, font.index as usize) else {
            return Ok(None);
        };
        let mut scaler = self
            .scale
            .builder_with_id(face, font.id)
            .size(size)
            .hint(true)
            .normalized_coords(font.coords.iter().copied())
            .build();
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(f32::from(phase) * 0.25, 0.))
        .transform(
            font.skew
                .then(|| Transform::skew(Angle::from_degrees(14.), Angle::from_degrees(0.))),
        )
        .render(&mut scaler, glyph);
        let Some(image) = image.filter(|i| i.placement.width > 0 && i.placement.height > 0) else {
            self.empty.insert(key);
            return Ok(None);
        };
        let color = image.content == Content::Color;
        let channels = if color { 4 } else { 1 };
        let mut pixels = match image.content {
            Content::Mask => image.data,
            Content::SubpixelMask => image
                .data
                .chunks_exact(4)
                .map(|p| p[0].max(p[1]).max(p[2]))
                .collect(),
            Content::Color => image
                .data
                .chunks_exact(4)
                .flat_map(|p| {
                    let a = u16::from(p[3]);
                    [
                        (u16::from(p[0]) * a / 255) as u8,
                        (u16::from(p[1]) * a / 255) as u8,
                        (u16::from(p[2]) * a / 255) as u8,
                        p[3],
                    ]
                })
                .collect(),
        };
        let mut size = [image.placement.width, image.placement.height];
        let pad = if sigma.is_finite() && sigma > 0. {
            (sigma * 3.).ceil().min(256.) as u32
        } else {
            0
        };
        if pad > 0 {
            let expanded = [size[0] + 2 * pad, size[1] + 2 * pad];
            let mut next = vec![0; expanded[0] as usize * expanded[1] as usize * channels];
            for (y, row) in pixels.chunks_exact(size[0] as usize * channels).enumerate() {
                let at = ((y + pad as usize) * expanded[0] as usize + pad as usize) * channels;
                next[at..at + row.len()].copy_from_slice(row);
            }
            pixels = gaussian(&next, expanded, channels, sigma, pad as i32);
            size = expanded;
        }
        let atlas = if color {
            &mut self.color
        } else {
            &mut self.alpha
        };
        // A placement may outlive the page that held it until this frame's first use.
        if !self.placements.contains_key(&key) {
            let _ = atlas.get(&key);
        }
        let entry = atlas.insert(device, key.clone(), size, &pixels)?;
        let result = Glyph {
            entry,
            left: image.placement.left - pad as i32,
            top: image.placement.top + pad as i32,
            size,
            color,
        };
        self.placements.insert(key, result);
        Ok(Some(result))
    }
}

fn gaussian(input: &[u8], size: [u32; 2], channels: usize, sigma: f32, radius: i32) -> Vec<u8> {
    let weights: Vec<f32> = (-radius..=radius)
        .map(|i| (-0.5 * (i as f32 / sigma).powi(2)).exp())
        .collect();
    let sum: f32 = weights.iter().sum();
    let (w, h) = (size[0] as usize, size[1] as usize);
    let mut source: Vec<f32> = input.iter().map(|v| f32::from(*v)).collect();
    for vertical in [false, true] {
        let mut dest = vec![0.; source.len()];
        for y in 0..h {
            for x in 0..w {
                for c in 0..channels {
                    let mut value = 0.;
                    for (i, weight) in weights.iter().enumerate() {
                        let delta = i as i32 - radius;
                        let (sx, sy) = if vertical {
                            (x as i32, y as i32 + delta)
                        } else {
                            (x as i32 + delta, y as i32)
                        };
                        if sx >= 0 && sy >= 0 && sx < w as i32 && sy < h as i32 {
                            value +=
                                source[(sy as usize * w + sx as usize) * channels + c] * weight;
                        }
                    }
                    dest[(y * w + x) * channels + c] = value / sum;
                }
            }
        }
        source = dest;
    }
    source
        .into_iter()
        .map(|v| v.round().clamp(0., 255.) as u8)
        .collect()
}

/// The oracle's signed quarter-pixel bins, including the negative half-ties.
pub fn subpixel(pos: f32) -> (i32, u8) {
    let trunc = pos as i32;
    let fract = pos - trunc as f32;
    if pos.is_sign_negative() {
        if fract > -0.125 {
            (trunc, 0)
        } else if fract > -0.375 {
            (trunc - 1, 3)
        } else if fract > -0.625 {
            (trunc - 1, 2)
        } else if fract > -0.875 {
            (trunc - 1, 1)
        } else {
            (trunc - 1, 0)
        }
    } else if fract < 0.125 {
        (trunc, 0)
    } else if fract < 0.375 {
        (trunc, 1)
    } else if fract < 0.625 {
        (trunc, 2)
    } else if fract < 0.875 {
        (trunc, 3)
    } else {
        (trunc + 1, 0)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::instance::Bindings;
    use crate::{Brush, Encoder, Instance, Kind, Wgpu};

    #[test]
    fn phases_skew_and_blur_have_distinct_resident_glyphs() {
        let mut device = match Wgpu::new() {
            Ok(d) => d,
            Err(crate::device::Error::NoAdapter(e)) => {
                eprintln!("no GPU: {e}");
                return;
            }
            Err(e) => panic!("{e}"),
        };
        let data = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");
        let face = swash::FontRef::from_index(data, 0).unwrap();
        let id = face.charmap().map('A');
        let mut font = Font {
            id: [1, 0],
            data,
            index: 0,
            coords: &[],
            skew: false,
        };
        let mut glyphs = Glyphs::default();
        let target = device.create_target(160, 48).unwrap();
        let mut previous = Vec::new();
        for frame in 0..2 {
            glyphs.begin(&mut device).unwrap();
            let mut encoder = Encoder::new(160, 48);
            let mut bounds = Vec::new();
            for n in 0..6 {
                font.skew = n == 4;
                let g = glyphs
                    .get(
                        &mut device,
                        &font,
                        id,
                        24.,
                        n.min(3),
                        if n == 5 { 1.5 } else { 0. },
                    )
                    .unwrap()
                    .unwrap();
                bounds.push((g.left, g.top, g.size));
                let mut i = Instance::new(
                    Kind::Glyph,
                    Brush::Solid,
                    [
                        n as f32 * 26. + g.left as f32 + 4.,
                        32. - g.top as f32,
                        g.size[0] as f32,
                        g.size[1] as f32,
                    ],
                );
                i.uv = g.entry.uv;
                encoder
                    .push(
                        i,
                        Bindings {
                            glyph: g.entry.texture,
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            device
                .render(&encoder.finish(), target, Some([0.; 4]))
                .unwrap();
            let pixels = device.readback(target).unwrap();
            assert!(pixels.iter().any(|v| *v > 0));
            assert!(bounds[5].2[0] > bounds[3].2[0] && bounds[5].2[1] > bounds[3].2[1]);
            assert_eq!(glyphs.stats().0.entries, 6);
            assert_eq!(glyphs.stats().0.misses, if frame == 0 { 6 } else { 0 });
            if frame == 1 {
                assert_eq!(pixels, previous);
            }
            previous = pixels;
        }
        assert_eq!(subpixel(-0.125), (-1, 3));
        assert_eq!(subpixel(0.875), (1, 0));
    }
}
