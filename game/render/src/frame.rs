use crate::{shadows::Cascades, FrameInput};
use exact_gpu::wgpu;
use glam::Vec3;

pub(crate) const FLOATS: usize = 192;

/// Whether the sky pass draws; `map` is a visible environment map resident.
pub(crate) fn has_sky(frame: &FrameInput<'_>, map: bool) -> bool {
    let e = frame.environment;
    if map {
        return true;
    }
    if e.background.is_some() {
        return false;
    }
    e.zenith != e.horizon
        || e.ground != e.horizon
        || (e.sun_disc > 0.0 && frame.sun.is_some())
        || e.fog.is_some_and(|fog| {
            fog.linear.is_none() && fog.density > 0.0 && fog.color.is_some_and(|c| c.0 != e.horizon)
        })
}

/// The scene's clear: opaque, as the canvas promises (`Surface::opaque`).
pub(crate) fn clear_color(frame: &FrameInput<'_>) -> wgpu::Color {
    let sky = frame
        .environment
        .background
        .map_or(frame.environment.horizon, |c| c.0);
    wgpu::Color {
        r: sky[0] as f64,
        g: sky[1] as f64,
        b: sky[2] as f64,
        a: 1.0,
    }
}

pub(crate) fn uniform(
    frame: &FrameInput<'_>,
    map: bool,
    cascades: Option<&Cascades>,
    size: (u32, u32),
    irradiance: &[f32; 36],
    lights: [f32; 4],
) -> [f32; FLOATS] {
    let mut data = [0.0; FLOATS];
    data[..16].copy_from_slice(&(frame.proj * frame.view).to_cols_array());
    data[16..19].copy_from_slice(&frame.camera_position.to_array());
    data[19] = frame.alpha.clamp(0.0, 1.0);
    if let Some(sun) = frame.sun {
        // Unit length: the forward shader takes it as it is (lights.wgsl `brdf`'s `l`).
        data[20..23].copy_from_slice(&sun.direction.normalize_or(Vec3::NEG_Y).to_array());
        data[23] = sun.illuminance;
        data[24..27].copy_from_slice(&sun.color.to_array());
    }
    data[27] = lights[0];
    data[28..31].copy_from_slice(&frame.environment.zenith);
    data[31] = frame.environment.ambient;
    data[32..35].copy_from_slice(&frame.environment.ground);
    data[35] = frame.environment.exposure;
    if let Some(fill) = frame.fill {
        data[36..39].copy_from_slice(&fill.direction.normalize_or(Vec3::NEG_Y).to_array());
        data[39] = fill.illuminance;
        data[40..43].copy_from_slice(&fill.color.to_array());
    }
    data[44..48].copy_from_slice(&lights);
    if has_sky(frame, map) {
        data[48..64].copy_from_slice(&(frame.proj * frame.view).inverse().to_cols_array());
    }
    data[64..68].copy_from_slice(&(-frame.view.row(2)).to_array());
    data[68..71].copy_from_slice(&frame.environment.horizon);
    data[71] = frame.environment.sun_disc;
    if let Some(fog) = frame.environment.fog {
        data[72..75].copy_from_slice(&fog.color.map_or(frame.environment.horizon, |c| c.0));
        data[75] = fog.density.max(0.0);
        data[76] = fog.height_falloff.max(0.0);
        if let Some([near, far]) = fog.linear {
            data[184] = near;
            data[185] = far;
        }
    }
    if let Some(bloom) = frame.environment.bloom {
        data[77] = bloom.threshold.max(0.0);
        data[78] = bloom.intensity.max(0.0);
        data[79] = bloom.radius.max(0.0);
    }
    if let Some(cascades) = cascades {
        for i in 0..3 {
            data[80 + i * 16..96 + i * 16].copy_from_slice(&cascades.matrices[i].to_cols_array());
        }
        data[128..131].copy_from_slice(&cascades.splits);
        data[131] = cascades.count as f32;
        data[132..135].copy_from_slice(&cascades.texels);
        data[135] = frame.sun.unwrap().shadows.unwrap().softness.max(0.0);
        data[136] = -frame.proj.inverse().project_point3(Vec3::ZERO).z;
        let shadows = frame.sun.unwrap().shadows.unwrap();
        if let crate::ShadowFilter::PcfSoft {
            depth_bias,
            normal_bias,
        }
        | crate::ShadowFilter::Bilinear {
            depth_bias,
            normal_bias,
        } = shadows.filter
        {
            data[188] = if matches!(shadows.filter, crate::ShadowFilter::Bilinear { .. }) {
                2.
            } else {
                1.
            };
            data[189] = depth_bias;
            data[190] = normal_bias;
        }
        data[191] = shadows.strength.clamp(0., 1.);
    }
    data[140] = size.0 as f32;
    data[141] = size.1 as f32;
    // `logical_size.z`: the tone curve's white (LLP 1100 D12b).
    data[142] = if frame.headroom.is_finite() {
        frame.headroom.max(1.0)
    } else {
        1.0
    };
    data[143] = f32::from(frame.environment.tone_mapping == exact_game::ToneMapping::None);
    data[186] =
        2. / (frame.proj.y_axis.y * frame.viewport_css_height.unwrap_or(size.1 as f32).max(1.));
    data[187] = f32::from(frame.proj.w_axis.w == 1.);
    data[144..180].copy_from_slice(irradiance);
    if let Some(map) = frame.environment_map.filter(|_| map) {
        data[180] = map.intensity.max(1e-9);
        data[181] = map.rotation;
        data[182] = map.rgbm.max(0.);
    }
    data
}

/// tone.wgsl's curve on the CPU: a direct frame's clear colour, which no shader
/// tonemaps.
pub(crate) fn tone(color: [f32; 3], frame: &FrameInput<'_>, encode_srgb: bool) -> wgpu::Color {
    let h = if frame.headroom.is_finite() {
        frame.headroom.max(1.0)
    } else {
        1.0
    };
    let finite = |x: f32| if x.is_nan() { 0. } else { x.clamp(0., 65472.) };
    let channel = |c: f32| {
        let y = finite(finite(c) * frame.environment.exposure) / h;
        let mut v = h * ((y * (2.51 * y + 0.03)) / (y * (2.43 * y + 0.59) + 0.14)).clamp(0., 1.);
        if frame.environment.tone_mapping == exact_game::ToneMapping::None {
            v = finite(finite(c) * frame.environment.exposure);
        }
        if encode_srgb {
            v = if v <= 0.003_130_8 {
                12.92 * v
            } else {
                1.055 * v.powf(1. / 2.4) - 0.055
            };
        }
        f64::from(v)
    };
    wgpu::Color {
        r: channel(color[0]),
        g: channel(color[1]),
        b: channel(color[2]),
        a: 1.,
    }
}
