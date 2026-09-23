//! Bake-time GPU block encoding, one payload per device family.
//!
//! The baker never ships in a game, so it links native encoders: Intel's ISPC
//! texture compressor for BC4/BC5/BC7 (prebuilt kernels for Apple, Linux and
//! Windows on x86-64 and arm64; no ISPC toolchain) and ARM's reference
//! `astcenc` for ASTC (vendored C++, built by `cc`), which also decodes for
//! the quality gates. Both come from the `ctt` bindings. (`intel_tex_2`, the
//! usual ISPC binding, has an unimplemented ASTC entry point and prebuilt
//! objects Apple's linker warns about.) Encoded bytes are deterministic per
//! machine; SIMD variants may differ bit-wise, so tests pin quality, not bytes.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
use exact_game::asset::{TextureData, TextureFamily, TextureFormat, RGBA8_TEXTURE_LIMIT};

/// What a texture's channels mean, from every material slot that samples it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channels {
    /// Colour whose alpha is sampled: BLEND base colour and sprites.
    ColorAlpha,
    /// MASK base colour, cut at this alpha byte: blocks keep every texel's side.
    Mask(u8),
    /// Three channels; alpha unused (opaque base colour, emission, metallic-roughness).
    Rgb,
    /// Tangent-space normal XY in RG; the shader rebuilds Z from them.
    Normal,
    /// One channel in R (occlusion).
    R,
}
impl Channels {
    /// The channels a quality comparison reads.
    pub fn compared(self) -> &'static [usize] {
        match self {
            Self::ColorAlpha | Self::Mask(_) => &[0, 1, 2, 3],
            Self::Rgb => &[0, 1, 2],
            Self::Normal => &[0, 1],
            Self::R => &[0],
        }
    }
    fn bc(self) -> TextureFormat {
        match self {
            Self::Normal => TextureFormat::Bc5,
            Self::R => TextureFormat::Bc4,
            Self::ColorAlpha | Self::Mask(_) | Self::Rgb => TextureFormat::Bc7,
        }
    }
}

/// Alpha bytes kept between a MASK texel and its cut before block encoding.
const MASK_MARGIN: u8 = 8;

/// Drop leading levels until an RGBA8 chain fits its 2048² limit; the tail of
/// a full chain is itself a full chain.
pub fn capped(mut texture: TextureData) -> TextureData {
    while texture.width.max(texture.height) > RGBA8_TEXTURE_LIMIT {
        texture.mips.remove(0);
        texture.width = (texture.width / 2).max(1);
        texture.height = (texture.height / 2).max(1);
    }
    texture
}

/// The authored name's RGBA8 fallback and its `.bc.tex`/`.astc.tex` payloads.
/// A family whose blocks cannot represent the texture carries the fallback:
/// base dimensions that are not whole 4×4 blocks, or (`exact`) any texel the
/// decoded blocks would change — nearest-filtered sprites keep their palette.
pub fn variants(
    name: &str,
    full: &TextureData,
    channels: Channels,
    exact: bool,
) -> Result<Vec<(String, TextureData)>, String> {
    let fallback = capped(full.clone());
    fallback
        .validate()
        .map_err(|e| format!("texture `{name}`: {e}"))?;
    let mut out = vec![(name.to_owned(), fallback.clone())];
    for (family, format) in [
        (TextureFamily::Bc, channels.bc()),
        (TextureFamily::Astc, TextureFormat::Astc4x4),
    ] {
        let mut texture = fallback.clone();
        if full.width.is_multiple_of(4) && full.height.is_multiple_of(4) {
            let encoded = encode(full, format, channels)?;
            if !exact || exactly(full, &encoded, channels) {
                texture = encoded;
            }
        }
        texture
            .validate()
            .map_err(|e| format!("texture `{name}` {family:?}: {e}"))?;
        out.push((family.name(name), texture));
    }
    Ok(out)
}

fn exactly(full: &TextureData, encoded: &TextureData, channels: Channels) -> bool {
    (0..full.mips.len()).all(|level| {
        let decoded = decode(encoded, level);
        full.mips[level]
            .chunks_exact(4)
            .zip(decoded.chunks_exact(4))
            .all(|(a, b)| channels.compared().iter().all(|&c| a[c] == b[c]))
    })
}

/// Encode every level of an RGBA8 chain into one block format.
pub fn encode(
    rgba: &TextureData,
    format: TextureFormat,
    channels: Channels,
) -> Result<TextureData, String> {
    if format == TextureFormat::Rgba8 || rgba.format != TextureFormat::Rgba8 {
        return Err(format!("cannot encode {:?} as {format:?}", rgba.format));
    }
    let mut mips = Vec::with_capacity(rgba.mips.len());
    for (level, texels) in rgba.mips.iter().enumerate() {
        let (w, h) = level_size(rgba, level);
        let (pw, ph, mut padded) = pad(texels, w, h);
        if let Channels::Mask(cut) = channels {
            // Coverage-preserving mips leave alpha beside the cut; move each
            // texel a margin away on its own side, so block error cannot flip it.
            // @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
            for texel in padded.chunks_exact_mut(4) {
                texel[3] = if texel[3] >= cut {
                    texel[3].max(cut.saturating_add(MASK_MARGIN))
                } else {
                    texel[3].min(cut.saturating_sub(1 + MASK_MARGIN))
                };
            }
        }
        let mut blocks = vec![0; format.level_bytes(pw, ph) as usize];
        let out = Blocks {
            bytes: &mut blocks,
            row: format.level_bytes(pw, 4) as usize,
        };
        let plane = |texel: usize| Plane {
            texels: if texel == 4 {
                padded.clone()
            } else {
                padded
                    .chunks_exact(4)
                    .flat_map(|p| p[..texel].to_vec())
                    .collect()
            },
            width: pw,
            height: ph,
            texel: texel as u32,
        };
        let stateless = || Ok(());
        use ctt_intel_texture_compressor as ispc;
        match format {
            TextureFormat::Bc7 => {
                let settings = if matches!(channels, Channels::ColorAlpha | Channels::Mask(_)) {
                    ispc::bc7::alpha_basic_settings()
                } else {
                    ispc::bc7::opaque_basic_settings()
                };
                strips(&plane(4), out, stateless, |_, data, rows, out| {
                    let surface = ispc::RgbaSurface::new(data, pw, rows, pw * 4);
                    ispc::bc7::compress_blocks_into(&settings, &surface, out);
                    Ok(())
                })?;
            }
            TextureFormat::Bc5 => strips(&plane(2), out, stateless, |_, data, rows, out| {
                let surface = ispc::RgSurface::new(data, pw, rows, pw * 2);
                ispc::bc5::compress_blocks_into(&surface, out);
                Ok(())
            })?,
            TextureFormat::Bc4 => strips(&plane(1), out, stateless, |_, data, rows, out| {
                ispc::bc4::compress_blocks_into(&ispc::RSurface::new(data, pw, rows, pw), out);
                Ok(())
            })?,
            TextureFormat::Astc4x4 => {
                let config = astc_config(rgba.srgb, channels, false)?;
                let swizzle = astc_swizzle(channels);
                let context = || ctt_astcenc::Context::new(&config).map_err(|e| e.to_string());
                strips(&plane(4), out, context, |context, data, rows, out| {
                    let mut texels = data[..(pw * rows * 4) as usize].to_vec();
                    let mut planes = [texels.as_mut_ptr().cast::<std::ffi::c_void>()];
                    let mut image = astc_image(pw, rows, &mut planes);
                    let result = context
                        .compress(&mut image, swizzle, out)
                        .map_err(|e| e.to_string());
                    context.compress_reset().map_err(|e| e.to_string())?;
                    result
                })?;
            }
            TextureFormat::Rgba8 => unreachable!("RGBA8 is the unencoded chain"),
        }
        mips.push(blocks);
    }
    Ok(TextureData {
        mips,
        format,
        ..rgba.clone()
    })
}

fn level_size(texture: &TextureData, level: usize) -> (u32, u32) {
    (
        (texture.width >> level).max(1),
        (texture.height >> level).max(1),
    )
}

/// Replicate the last row and column to whole 4×4 blocks (mip tails, 2×2 and 1×1).
fn pad(texels: &[u8], w: u32, h: u32) -> (u32, u32, Vec<u8>) {
    let (pw, ph) = (w.next_multiple_of(4), h.next_multiple_of(4));
    if (pw, ph) == (w, h) {
        return (w, h, texels.to_vec());
    }
    let mut out = Vec::with_capacity((pw * ph * 4) as usize);
    for y in 0..ph {
        let row = y.min(h - 1) * w;
        for x in 0..pw {
            let i = ((row + x.min(w - 1)) * 4) as usize;
            out.extend_from_slice(&texels[i..i + 4]);
        }
    }
    (pw, ph, out)
}

/// Padded texels of one level with `texel` bytes each (RGBA, RG or R).
struct Plane {
    texels: Vec<u8>,
    width: u32,
    height: u32,
    texel: u32,
}
/// One level's encoded bytes and the bytes of one row of blocks.
struct Blocks<'a> {
    bytes: &'a mut [u8],
    row: usize,
}

/// Encode block rows on every core; strips are independent whole block rows.
/// Each worker owns the state `init` makes (an ASTC context), never shared.
fn strips<S>(
    plane: &Plane,
    out: Blocks,
    init: impl Fn() -> Result<S, String> + Sync,
    encode: impl Fn(&mut S, &[u8], u32, &mut [u8]) -> Result<(), String> + Sync,
) -> Result<(), String> {
    const ROWS: u32 = 16;
    let input_row = (plane.width * plane.texel * 4) as usize;
    let jobs = std::sync::Mutex::new(
        out.bytes
            .chunks_mut(out.row * ROWS as usize)
            .enumerate()
            .collect::<Vec<_>>(),
    );
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let block_rows = plane.height / 4;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.min(block_rows.div_ceil(ROWS) as usize))
            .map(|_| {
                scope.spawn(|| -> Result<(), String> {
                    let mut state = init()?;
                    loop {
                        let Some((index, out)) = jobs.lock().unwrap().pop() else {
                            return Ok(());
                        };
                        let first = index as u32 * ROWS;
                        let rows = ROWS.min(block_rows - first) * 4;
                        let input = &plane.texels[first as usize * input_row..];
                        encode(&mut state, input, rows, out)?;
                    }
                })
            })
            .collect();
        workers.into_iter().try_for_each(|w| {
            w.join()
                .map_err(|_| "texture encoder panicked".to_owned())?
        })
    })
}

fn astc_config(
    srgb: bool,
    channels: Channels,
    decompress: bool,
) -> Result<ctt_astcenc::bindings::astcenc_config, String> {
    use ctt_astcenc::{config_init, Flags, Preset, Profile};
    let profile = if srgb { Profile::LdrSrgb } else { Profile::Ldr };
    let flags = if decompress {
        Flags::DECOMPRESS_ONLY
    } else {
        Flags::empty()
    };
    // A cut-out's edge is decided per texel: search harder for it.
    let preset = if matches!(channels, Channels::Mask(_)) {
        Preset::Thorough
    } else {
        Preset::Medium
    };
    let mut config = config_init(profile, 4, 4, 1, preset, flags).map_err(|e| e.to_string())?;
    // Error weights follow what the shader reads (see `Channels`).
    let weights = match channels {
        Channels::ColorAlpha => [1., 1., 1., 1.],
        // A cut-out's shape is its alpha: weigh it as the three colours together.
        Channels::Mask(_) => [1., 1., 1., 3.],
        Channels::Rgb => [1., 1., 1., 0.],
        Channels::Normal => [1., 1., 0., 0.],
        Channels::R => [1., 0., 0., 0.],
    };
    [
        config.cw_r_weight,
        config.cw_g_weight,
        config.cw_b_weight,
        config.cw_a_weight,
    ] = weights;
    Ok(config)
}

/// Unused channels become constants the encoder spends no bits on; the
/// sampled channels keep their positions, so the shader reads every format alike.
fn astc_swizzle(channels: Channels) -> ctt_astcenc::Swizzle {
    use ctt_astcenc::{Swizzle, SwizzleChannel as C};
    match channels {
        Channels::ColorAlpha | Channels::Mask(_) => Swizzle::IDENTITY,
        Channels::Rgb => Swizzle::RGB1,
        Channels::Normal => Swizzle {
            r: C::R,
            g: C::G,
            b: C::Zero,
            a: C::One,
        },
        Channels::R => Swizzle::RRR1,
    }
}

fn astc_image(
    width: u32,
    height: u32,
    planes: &mut [*mut std::ffi::c_void; 1],
) -> ctt_astcenc::bindings::astcenc_image {
    ctt_astcenc::bindings::astcenc_image {
        dim_x: width,
        dim_y: height,
        dim_z: 1,
        data_type: ctt_astcenc::bindings::astcenc_type_ASTCENC_TYPE_U8,
        data: planes.as_mut_ptr(),
    }
}

/// Decode one level to RGBA8 as a GPU samples it: BC4 is (r,0,0,1), BC5 (r,g,0,1).
pub fn decode(texture: &TextureData, level: usize) -> Vec<u8> {
    let (w, h) = level_size(texture, level);
    let (pw, ph) = (w.next_multiple_of(4), h.next_multiple_of(4));
    let data = &texture.mips[level];
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    match texture.format {
        TextureFormat::Rgba8 => return data.clone(),
        TextureFormat::Astc4x4 => {
            let config = astc_config(texture.srgb, Channels::ColorAlpha, true).expect("astc");
            let mut context = ctt_astcenc::Context::new(&config).expect("astc context");
            let mut planes = [out.as_mut_ptr().cast::<std::ffi::c_void>()];
            let mut image = astc_image(pw, ph, &mut planes);
            context
                .decompress(data, &mut image, ctt_astcenc::Swizzle::IDENTITY)
                .expect("astc decode");
        }
        format => {
            let (_, bytes) = format.block();
            for (index, block) in data.chunks_exact(bytes).enumerate() {
                let (bx, by) = (index as u32 % (pw / 4), index as u32 / (pw / 4));
                let mut texels = [0u8; 64];
                match format {
                    TextureFormat::Bc7 => bcdec_rs::bc7(block, &mut texels, 16),
                    TextureFormat::Bc5 => {
                        let mut rg = [0u8; 32];
                        bcdec_rs::bc5(block, &mut rg, 8, false);
                        for i in 0..16 {
                            texels[i * 4..i * 4 + 4].copy_from_slice(&[
                                rg[i * 2],
                                rg[i * 2 + 1],
                                0,
                                255,
                            ]);
                        }
                    }
                    _ => {
                        let mut r = [0u8; 16];
                        bcdec_rs::bc4(block, &mut r, 4, false);
                        for i in 0..16 {
                            texels[i * 4..i * 4 + 4].copy_from_slice(&[r[i], 0, 0, 255]);
                        }
                    }
                }
                for y in 0..4 {
                    let at = (((by * 4 + y) * pw + bx * 4) * 4) as usize;
                    out[at..at + 16].copy_from_slice(&texels[(y * 16) as usize..][..16]);
                }
            }
        }
    }
    // Crop the padded blocks back to the level's texels.
    (0..h)
        .flat_map(|y| out[(y * pw * 4) as usize..][..(w * 4) as usize].to_vec())
        .collect()
}

/// Peak signal-to-noise ratio in dB over the listed channels; infinite when equal.
pub fn psnr(a: &[u8], b: &[u8], channels: &[usize]) -> f64 {
    let (mut error, mut count) = (0f64, 0f64);
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for &c in channels {
            let d = f64::from(x[c]) - f64::from(y[c]);
            error += d * d;
            count += 1.;
        }
    }
    if error == 0. {
        f64::INFINITY
    } else {
        10. * (255f64 * 255. * count / error).log10()
    }
}
