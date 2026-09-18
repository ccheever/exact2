use crate::{Data, Resource};

/// Linear sky colours and optional atmospheric/post-processing effects.
/// Absence of this resource has the same presentation as its default.
#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct Environment {
    /// Optional flat background, independent of ambient illumination.
    pub background: Option<[f32; 3]>,
    /// Overhead sky radiance.
    pub zenith: [f32; 3],
    /// Horizon sky radiance and default fog colour.
    pub horizon: [f32; 3],
    /// Ground hemisphere radiance.
    pub ground: [f32; 3],
    /// Hemisphere lighting multiplier.
    pub ambient: f32,
    /// Sun angular radius in radians; zero disables the disc.
    pub sun_disc: f32,
    /// Linear exposure before tonemapping.
    pub exposure: f32,
    /// None disables fog (the default).
    pub fog: Option<Fog>,
    /// Gentle HDR bloom by default; None disables its passes.
    pub bloom: Option<Bloom>,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            background: None,
            zenith: [0.12, 0.22, 0.4],
            horizon: [0.45, 0.6, 0.65],
            ground: [0.04, 0.035, 0.025],
            ambient: 0.5,
            sun_disc: 0.00465,
            exposure: 1.0,
            fog: None,
            bloom: Some(Bloom::default()),
        }
    }
}
/// Exponential fog, thinning with world height.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Fog {
    /// Linear colour; None uses the horizon.
    pub color: Option<[f32; 3]>,
    /// Extinction per metre at Y=0.
    pub density: f32,
    /// Exponential height falloff per metre.
    pub height_falloff: f32,
}
impl Default for Fog {
    fn default() -> Self {
        Self {
            color: None,
            density: 0.02,
            height_falloff: 0.1,
        }
    }
}
/// HDR bright-pass settings, independent of exposure.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Bloom {
    /// Bright-pass onset in peak linear channel.
    pub threshold: f32,
    /// Contribution before tonemapping.
    pub intensity: f32,
    /// Upsampling tent radius in source texels.
    pub radius: f32,
}
impl Default for Bloom {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            intensity: 0.08,
            radius: 1.0,
        }
    }
}
