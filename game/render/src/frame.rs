use crate::{shadows::Cascades, FrameInput};
use glam::Vec3;

pub(crate) const FLOATS: usize = 184;

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
        || e.fog
            .is_some_and(|fog| fog.density > 0.0 && fog.color.is_some_and(|c| c != e.horizon))
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
        data[20..23].copy_from_slice(&sun.direction.to_array());
        data[23] = sun.illuminance;
        data[24..27].copy_from_slice(&sun.color.to_array());
    }
    data[27] = lights[0];
    data[28..31].copy_from_slice(&frame.environment.zenith);
    data[31] = frame.environment.ambient;
    data[32..35].copy_from_slice(&frame.environment.ground);
    data[35] = frame.environment.exposure;
    if let Some(fill) = frame.fill {
        data[36..39].copy_from_slice(&fill.direction.to_array());
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
        data[72..75].copy_from_slice(&fog.color.unwrap_or(frame.environment.horizon));
        data[75] = fog.density.max(0.0);
        data[76] = fog.height_falloff.max(0.0);
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
    }
    data[140] = size.0 as f32;
    data[141] = size.1 as f32;
    data[144..180].copy_from_slice(irradiance);
    if let Some(map) = frame.environment_map.filter(|_| map) {
        data[180] = map.intensity.max(1e-9);
        data[181] = map.rotation;
        data[182] = map.rgbm.max(0.);
    }
    data
}
