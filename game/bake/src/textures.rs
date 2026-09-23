use crate::compress::Channels;
use exact_game::asset::*;
use std::collections::BTreeMap;

/// Full-resolution RGBA8 chains by authored name, and the material slots
/// (0 base, 1 normal, 2 metallic-roughness, 3 emission, 4 occlusion) sampling
/// each; bit 5 marks base colour whose alpha a MASK/BLEND material reads.
/// MASK colour also carries its cut as an alpha byte.
pub type Sources = BTreeMap<String, (TextureData, u8, Option<u8>)>;

pub fn materials(
    doc: &gltf::Document,
    images: &[gltf::image::Data],
    out: &mut Model,
    stem: &str,
    used: &std::collections::BTreeSet<usize>,
) -> Result<Sources, String> {
    let json = serde_json::to_value(doc.as_json()).map_err(|e| e.to_string())?;
    let mut cache = BTreeMap::new();
    let mut payloads: Sources = BTreeMap::new();
    for material in doc.materials() {
        if !used.contains(&material.index().unwrap()) {
            out.materials.push(MaterialData::default());
            continue;
        }
        let pbr = material.pbr_metallic_roughness();
        let mut m = MaterialData {
            base_color: pbr.base_color_factor(),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            emissive: material
                .emissive_factor()
                .map(|v| v * material.emissive_strength().unwrap_or(1.0)),
            alpha_mode: match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask,
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            },
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
            double_sided: material.double_sided(),
            normal_scale: material.normal_texture().map_or(1.0, |t| t.scale()),
            occlusion_strength: material.occlusion_texture().map_or(1.0, |t| t.strength()),
            ..Default::default()
        };
        let textures = [
            pbr.base_color_texture().map(|i| i.texture()),
            material.normal_texture().map(|i| i.texture()),
            pbr.metallic_roughness_texture().map(|i| i.texture()),
            material.emissive_texture().map(|i| i.texture()),
            material.occlusion_texture().map(|i| i.texture()),
        ];
        let raw = &json["materials"][material.index().unwrap()];
        let infos = [
            &raw["pbrMetallicRoughness"]["baseColorTexture"],
            &raw["normalTexture"],
            &raw["pbrMetallicRoughness"]["metallicRoughnessTexture"],
            &raw["emissiveTexture"],
            &raw["occlusionTexture"],
        ];
        for (slot, texture) in textures.into_iter().enumerate() {
            let Some(texture) = texture else { continue };
            let ext = &infos[slot]["extensions"]["KHR_texture_transform"];
            let coord = ext["texCoord"]
                .as_u64()
                .or_else(|| infos[slot]["texCoord"].as_u64())
                .unwrap_or(0);
            if coord != 0 {
                return Err(format!(
                    "material {} texture {slot}: TEXCOORD_{coord} unsupported",
                    material.index().unwrap()
                ));
            }
            let scale = [0, 1].map(|i| ext["scale"][i].as_f64().unwrap_or(1.) as f32);
            let offset = [0, 1].map(|i| ext["offset"][i].as_f64().unwrap_or(0.) as f32);
            let angle = ext["rotation"].as_f64().unwrap_or(0.) as f32;
            let (s, c) = (exact_game::math::sin(angle), exact_game::math::cos(angle));
            m.uv_transforms[slot] = [
                c * scale[0],
                s * scale[0],
                -s * scale[1],
                c * scale[1],
                offset[0],
                offset[1],
            ];
            let srgb = slot == 0 || slot == 3;
            let cutoff = (slot == 0 && m.alpha_mode == AlphaMode::Mask)
                .then_some(m.alpha_cutoff / m.base_color[3].max(f32::MIN_POSITIVE));
            let uses_alpha = slot == 0 && m.alpha_mode != AlphaMode::Opaque;
            let key = (texture.index(), srgb, uses_alpha, cutoff.map(f32::to_bits));
            let bits = 1 << slot | u8::from(uses_alpha) << 5;
            let index = if let Some(&(i, ref name)) = cache.get(&key) {
                payloads.get_mut::<String>(name).unwrap().1 |= bits;
                i
            } else {
                let image = &images[texture.source().index()];
                let name = format!(
                    "{stem}/{}-{}{}{}.tex",
                    texture.index(),
                    if srgb { "srgb" } else { "linear" },
                    if uses_alpha { "-alpha" } else { "-straight" },
                    cutoff.map_or(String::new(), |c| format!("-mask{:08x}", c.to_bits()))
                );
                if !asset_name(&name) {
                    return Err(format!("texture `{name}`: invalid asset name"));
                }
                if image.width > BLOCK_TEXTURE_LIMIT || image.height > BLOCK_TEXTURE_LIMIT {
                    return Err(format!(
                        "texture `{name}`: {}x{} exceeds {BLOCK_TEXTURE_LIMIT}x{BLOCK_TEXTURE_LIMIT}",
                        image.width, image.height
                    ));
                }
                let rgba = rgba(image)?;
                let sampler = texture.sampler();
                let wrap = |mode| match mode {
                    gltf::texture::WrappingMode::ClampToEdge => Wrap::Clamp,
                    gltf::texture::WrappingMode::MirroredRepeat => Wrap::Mirror,
                    _ => Wrap::Repeat,
                };
                let index = out.textures.len() as u32;
                let data = TextureData {
                    width: image.width,
                    height: image.height,
                    mips: mips(image.width, image.height, rgba, srgb, uses_alpha, cutoff),
                    srgb,
                    wrap: [wrap(sampler.wrap_s()), wrap(sampler.wrap_t())],
                    filter: [
                        if sampler.mag_filter() == Some(gltf::texture::MagFilter::Nearest) {
                            Filter::Nearest
                        } else {
                            Filter::Linear
                        },
                        if matches!(
                            sampler.min_filter(),
                            Some(
                                gltf::texture::MinFilter::Nearest
                                    | gltf::texture::MinFilter::NearestMipmapNearest
                                    | gltf::texture::MinFilter::NearestMipmapLinear
                            )
                        ) {
                            Filter::Nearest
                        } else {
                            Filter::Linear
                        },
                        if matches!(
                            sampler.min_filter(),
                            Some(
                                gltf::texture::MinFilter::NearestMipmapNearest
                                    | gltf::texture::MinFilter::LinearMipmapNearest
                            )
                        ) {
                            Filter::Nearest
                        } else {
                            Filter::Linear
                        },
                    ],
                    format: TextureFormat::Rgba8,
                };
                out.textures.push(name.clone());
                // The same byte the coverage-preserving mips cut at.
                let cut = cutoff.map(|c| (c * 255.).ceil().clamp(1., 255.) as u8);
                payloads.insert(name.clone(), (data, bits, cut));
                cache.insert(key, (index, name));
                index
            };
            match slot {
                0 => m.base_color_texture = Some(index),
                1 => m.normal_texture = Some(index),
                2 => m.metallic_roughness_texture = Some(index),
                3 => m.emissive_texture = Some(index),
                _ => m.occlusion_texture = Some(index),
            }
        }
        out.materials.push(m);
    }
    out.materials.push(MaterialData::default());
    Ok(payloads)
}
/// What the sampling slots read, for choosing block formats. Colour slots
/// (sRGB) never share a payload with data slots (linear): srgb is in its key.
pub fn channels(slots: u8, cut: Option<u8>) -> Channels {
    if let Some(cut) = cut {
        return Channels::Mask(cut);
    }
    match slots {
        _ if slots & 1 << 5 != 0 => Channels::ColorAlpha,
        0b0_0010 => Channels::Normal,
        0b1_0000 => Channels::R,
        _ => Channels::Rgb,
    }
}
fn rgba(data: &gltf::image::Data) -> Result<Vec<u8>, String> {
    use gltf::image::Format;
    let raw = data.pixels.clone();
    let img = match data.format {
        Format::R8 => image::DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(data.width, data.height, raw).ok_or("invalid R8 image")?,
        ),
        Format::R8G8 => image::DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(data.width, data.height, raw)
                .ok_or("invalid RG8 image")?,
        ),
        Format::R8G8B8 => image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(data.width, data.height, raw).ok_or("invalid RGB8 image")?,
        ),
        Format::R8G8B8A8 => image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(data.width, data.height, raw)
                .ok_or("invalid RGBA8 image")?,
        ),
        other => {
            return Err(format!(
                "image format {other:?} unsupported (expected PNG/JPEG 8-bit)"
            ))
        }
    };
    Ok(img.into_rgba8().into_raw())
}
fn linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        exact_game::math::powf((v + 0.055) / 1.055, 2.4)
    }
}
fn srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * exact_game::math::powf(v, 1. / 2.4) - 0.055
    }
}
/// Box mip chain with linear-light colour filtering; alpha and data channels are linear.
pub fn mips(
    mut w: u32,
    mut h: u32,
    rgba: Vec<u8>,
    color: bool,
    uses_alpha: bool,
    cutoff: Option<f32>,
) -> Vec<Vec<u8>> {
    let coverage = cutoff.map(|c| {
        rgba.chunks_exact(4)
            .filter(|p| p[3] as f32 / 255. >= c)
            .count() as f32
            / (w * h) as f32
    });
    // Every sample is a byte: one table replaces a power per texel and channel.
    let table: Vec<f32> = (0..=255u8).map(|v| linear(f32::from(v) / 255.)).collect();
    let mut out = vec![rgba];
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0; (nw * nh * 4) as usize];
        let previous = out.last().unwrap();
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let (x0, x1, y0, y1) =
                        (x * w / nw, (x + 1) * w / nw, y * h / nh, (y + 1) * h / nh);
                    let mut sum = 0.;
                    let mut weight = 0.;
                    for sy in y0..y1 {
                        for sx in x0..x1 {
                            let byte = previous[((sy * w + sx) * 4 + c) as usize];
                            let v = byte as f32 / 255.;
                            let alpha = previous[((sy * w + sx) * 4 + 3) as usize] as f32 / 255.;
                            if color && c < 3 {
                                let weight_here = if uses_alpha { alpha } else { 1. };
                                sum += table[usize::from(byte)] * weight_here;
                                weight += weight_here;
                            } else {
                                sum += v;
                                weight += 1.;
                            }
                        }
                    }
                    let v = if weight > 0. { sum / weight } else { 0. };
                    next[((y * nw + x) * 4 + c) as usize] =
                        ((if color && c < 3 { srgb(v) } else { v }) * 255.)
                            .round()
                            .clamp(0., 255.) as u8;
                }
            }
        }
        if let (Some(cutoff), Some(coverage)) = (cutoff, coverage) {
            // Preserve coverage to the nearest representable texel count. Uniform
            // scaling retains alpha order; ties use row order at the threshold.
            let count = (coverage * (nw * nh) as f32).round() as usize;
            let threshold = (cutoff * 255.).ceil().clamp(1., 255.) as u8;
            let mut order: Vec<_> = (0..(nw * nh) as usize).collect();
            order.sort_by_key(|&i| std::cmp::Reverse(next[i * 4 + 3]));
            let boundary = order
                .get(count.saturating_sub(1))
                .map_or(255, |&i| next[i * 4 + 3]);
            let scale = threshold as f32 / f32::from(boundary.max(1));
            for (rank, &i) in order.iter().enumerate() {
                let a = (next[i * 4 + 3] as f32 * scale).round().clamp(0., 255.) as u8;
                next[i * 4 + 3] = if rank < count {
                    a.max(threshold)
                } else {
                    a.min(threshold - 1)
                };
            }
        }
        out.push(next);
        w = nw;
        h = nh;
    }
    out
}
#[cfg(test)]
mod tests {
    #[test]
    fn color_box_is_linear_light_and_odd_edges_are_included() {
        let pixels = vec![0, 0, 0, 255, 255, 255, 255, 255];
        assert_eq!(
            super::mips(2, 1, pixels.clone(), true, false, None)[1],
            [188, 188, 188, 255]
        );
        assert_eq!(
            super::mips(2, 1, pixels, false, false, None)[1],
            [128, 128, 128, 255]
        );
        assert_eq!(
            super::mips(
                3,
                1,
                vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255],
                false,
                false,
                None
            )[1],
            [85, 85, 85, 85]
        );
    }
}

#[cfg(test)]
mod alpha_tests {
    #[test]
    fn transparent_colour_cannot_bleed_and_mask_coverage_survives() {
        let mip = super::mips(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0], true, true, None);
        assert_eq!(mip[1], [255, 0, 0, 128]);
        let mut pixels = Vec::new();
        for a in [200, 100, 100, 100, 200, 100, 100, 100] {
            pixels.extend([255, 255, 255, a]);
        }
        let mips = super::mips(8, 1, pixels, true, true, Some(0.5));
        for mip in &mips[..3] {
            let covered = mip.chunks_exact(4).filter(|p| p[3] >= 128).count();
            assert_eq!(covered, (mip.len() as f32 / 4. * 0.25).round() as usize);
        }
    }
}

#[cfg(test)]
mod straight_tests {
    #[test]
    fn opaque_and_emissive_colour_ignore_alpha() {
        let mip = super::mips(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0], true, false, None);
        assert_eq!(mip[1], [188, 0, 188, 128]);
    }
}
