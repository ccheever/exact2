use exact_game::asset::*;
use std::collections::BTreeMap;

pub fn materials(
    doc: &gltf::Document,
    images: &[gltf::image::Data],
    out: &mut Model,
) -> Result<(), String> {
    let json = serde_json::to_value(doc.as_json()).map_err(|e| e.to_string())?;
    let mut cache = BTreeMap::new();
    for material in doc.materials() {
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
            let (s, c) = angle.sin_cos();
            m.uv_transforms[slot] = [
                c * scale[0],
                s * scale[0],
                -s * scale[1],
                c * scale[1],
                offset[0],
                offset[1],
            ];
            let srgb = slot == 0 || slot == 3;
            let key = (texture.index(), srgb);
            let index = if let Some(&i) = cache.get(&key) {
                i
            } else {
                let image = &images[texture.source().index()];
                let rgba = rgba(image)?;
                let sampler = texture.sampler();
                let wrap = |mode| match mode {
                    gltf::texture::WrappingMode::ClampToEdge => Wrap::Clamp,
                    gltf::texture::WrappingMode::MirroredRepeat => Wrap::Mirror,
                    _ => Wrap::Repeat,
                };
                let index = out.textures.len() as u32;
                out.textures.push(TextureData {
                    width: image.width,
                    height: image.height,
                    mips: mips(image.width, image.height, rgba, srgb),
                    srgb,
                    wrap: [wrap(sampler.wrap_s()), wrap(sampler.wrap_t())],
                    filter: Filter::Linear,
                });
                cache.insert(key, index);
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
    Ok(())
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
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
/// Box mip chain with linear-light colour filtering; alpha and data channels are linear.
pub fn mips(mut w: u32, mut h: u32, rgba: Vec<u8>, color: bool) -> Vec<Vec<u8>> {
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
                    for sy in y0..y1 {
                        for sx in x0..x1 {
                            let v = previous[((sy * w + sx) * 4 + c) as usize] as f32 / 255.;
                            sum += if color && c < 3 { linear(v) } else { v };
                        }
                    }
                    let v = sum / ((x1 - x0) * (y1 - y0)) as f32;
                    next[((y * nw + x) * 4 + c) as usize] =
                        ((if color && c < 3 { srgb(v) } else { v }) * 255.)
                            .round()
                            .clamp(0., 255.) as u8;
                }
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
            super::mips(2, 1, pixels.clone(), true)[1],
            [188, 188, 188, 255]
        );
        assert_eq!(super::mips(2, 1, pixels, false)[1], [128, 128, 128, 255]);
        assert_eq!(
            super::mips(
                3,
                1,
                vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255],
                false
            )[1],
            [85, 85, 85, 85]
        );
    }
}
