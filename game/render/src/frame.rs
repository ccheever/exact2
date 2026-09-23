use crate::{shadows::Cascades, FrameInput};
use glam::Vec3;

pub(crate) const FLOATS: usize = 296;

pub(crate) fn has_sky(frame: &FrameInput<'_>) -> bool {
    let e = frame.environment;
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
    cascades: Option<&Cascades>,
    size: (u32, u32),
    irradiance: &[f32; 36],
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
    data[27] = frame.points.len().min(16) as f32;
    data[28..31].copy_from_slice(&frame.environment.zenith);
    data[31] = frame.environment.ambient;
    data[32..35].copy_from_slice(&frame.environment.ground);
    data[35] = frame.environment.exposure;
    for (point, out) in frame
        .points
        .iter()
        .take(16)
        .zip(data[36..164].chunks_exact_mut(8))
    {
        out[..3].copy_from_slice(&point.position.to_array());
        out[3] = point.range;
        out[4..7].copy_from_slice(&point.color.to_array());
        out[7] = point.intensity;
    }
    if has_sky(frame) {
        data[164..180].copy_from_slice(&(frame.proj * frame.view).inverse().to_cols_array());
    }
    data[180..184].copy_from_slice(&(-frame.view.row(2)).to_array());
    data[184..187].copy_from_slice(&frame.environment.horizon);
    data[187] = frame.environment.sun_disc;
    if let Some(fog) = frame.environment.fog {
        data[188..191].copy_from_slice(&fog.color.unwrap_or(frame.environment.horizon));
        data[191] = fog.density.max(0.0);
        data[192] = fog.height_falloff.max(0.0);
    }
    if let Some(bloom) = frame.environment.bloom {
        data[193] = bloom.threshold.max(0.0);
        data[194] = bloom.intensity.max(0.0);
        data[195] = bloom.radius.max(0.0);
    }
    if let Some(cascades) = cascades {
        for i in 0..3 {
            data[196 + i * 16..212 + i * 16].copy_from_slice(&cascades.matrices[i].to_cols_array());
        }
        data[244..247].copy_from_slice(&cascades.splits);
        data[247] = cascades.count as f32;
        data[248..251].copy_from_slice(&cascades.texels);
        data[251] = frame.sun.unwrap().shadows.unwrap().softness.max(0.0);
        data[252] = -frame.proj.inverse().project_point3(Vec3::ZERO).z;
    }
    data[256] = size.0 as f32;
    data[257] = size.1 as f32;
    data[260..296].copy_from_slice(irradiance);
    data
}
