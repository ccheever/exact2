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
    /// Output curve; `None` preserves linear radiance before the output transfer.
    pub tone_mapping: ToneMapping,
    /// Gentle horizon fog by default; None disables it.
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
            tone_mapping: ToneMapping::Aces,
            fog: Some(Fog::default()),
            bloom: Some(Bloom::default()),
        }
    }
}
/// An authored environment for image-based lighting: it replaces the procedural
/// sky (`Environment`'s zenith, horizon and ground) as the source of ambient
/// diffuse and specular light; the visible sky and background are unchanged.
/// `texture` names an equirectangular `.tex` declared in `Game::ASSETS`, +Y the
/// top row and −Z the centre column.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct EnvironmentMap {
    /// The equirectangular texture asset.
    pub texture: String,
    /// Linear radiance multiplier.
    pub intensity: f32,
    /// RGBM range: zero reads RGB as radiance; otherwise radiance is
    /// `rgb × alpha × rgbm`, which carries HDR through an 8-bit texture.
    pub rgbm: f32,
    /// Draw the map as the visible sky too, in place of the procedural
    /// gradient and `background` (the sun disc and fog still apply).
    pub visible: bool,
    /// Yaw of the map about +Y in radians, for both the light and the sky.
    pub rotation: f32,
}
impl EnvironmentMap {
    /// A plain (non-RGBM) map at unit intensity.
    pub fn new(texture: impl Into<String>) -> Self {
        Self {
            texture: texture.into(),
            intensity: 1.0,
            rgbm: 0.0,
            visible: false,
            rotation: 0.0,
        }
    }
}

/// Screen-space ambient occlusion, an optional effect: inserting this resource
/// turns it on. Creases and contacts darken where nearby geometry covers a
/// surface's hemisphere within `radius`.
#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct AmbientOcclusion {
    /// Sampling radius in metres.
    pub radius: f32,
    /// Darkening at full coverage: 1 turns a fully covered pixel black.
    pub intensity: f32,
    /// Cost against detail; `Medium` by default.
    pub quality: AoQuality,
}
impl Default for AmbientOcclusion {
    fn default() -> Self {
        Self {
            radius: 0.5,
            intensity: 1.0,
            quality: AoQuality::Medium,
        }
    }
}
/// How `AmbientOcclusion` trades GPU time for detail. Every level upsamples
/// with a depth-aware blur, so edges stay crisp at half resolution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum AoQuality {
    /// Half resolution, 8 samples a pixel.
    Low,
    /// Half resolution, 12 samples a pixel.
    #[default]
    Medium,
    /// Full resolution, 16 samples a pixel.
    High,
}
impl AoQuality {
    /// The occlusion texture's downscale and its samples a pixel.
    pub fn plan(self) -> (u32, u32) {
        match self {
            Self::Low => (2, 8),
            Self::Medium => (2, 12),
            Self::High => (1, 16),
        }
    }
}

/// The output curve applied after exposure and bloom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum ToneMapping {
    /// ACES fitted curve (the default).
    #[default]
    Aces,
    /// Linear output, encoded to sRGB by the target or the shader.
    None,
}
/// Scene colour/depth sampling; selected once by `Game::SAMPLES`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneSamples {
    /// One sample per pixel.
    One,
    /// Four-sample MSAA (the default).
    #[default]
    Four,
}
impl SceneSamples {
    /// GPU attachment and pipeline sample count.
    pub fn count(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Four => 4,
        }
    }
}
/// Height fog, or near/far fog along the camera's forward axis.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Fog {
    /// Linear colour; None uses the horizon.
    pub color: Option<[f32; 3]>,
    /// Extinction per metre at Y=0.
    pub density: f32,
    /// Exponential height falloff per metre.
    pub height_falloff: f32,
    /// Near/far view depths. When present, replaces height fog with smoothstep fog.
    pub linear: Option<[f32; 2]>,
}
impl Fog {
    /// Three.js Fog: smoothstep between near and far camera-space depths, in metres.
    pub fn linear(near: f32, far: f32) -> Self {
        assert!(near.is_finite() && far.is_finite() && near >= 0. && far > near);
        Self {
            linear: Some([near, far]),
            density: 0.,
            ..Self::default()
        }
    }
    /// Exponential extinction per metre and height falloff, not near/far planes.
    pub fn new(density: f32, height_falloff: f32) -> Self {
        assert!(density.is_finite() && density >= 0.0);
        assert!(height_falloff.is_finite() && height_falloff >= 0.0);
        Self {
            color: None,
            density,
            height_falloff,
            linear: None,
        }
    }
}
impl Default for Fog {
    fn default() -> Self {
        Self {
            color: None,
            density: 0.012,
            height_falloff: 0.1,
            linear: None,
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
            intensity: 0.16,
            radius: 1.5,
        }
    }
}

/// A camera-independent, square sun projection, in light-space metres.
#[derive(Debug, Clone, Copy, PartialEq, Data)]
pub struct ShadowBox {
    /// World-space centre of the box.
    pub center: crate::Vec3,
    /// Half of the width and height perpendicular to the sun direction.
    pub half_width: f32,
    /// Half of the extent along the sun direction.
    pub half_depth: f32,
}

impl Default for ShadowBox {
    fn default() -> Self {
        Self {
            center: crate::Vec3::ZERO,
            half_width: 30.,
            half_depth: 60.,
        }
    }
}

/// Sun shadow quality and reach. Up to three depth layers.
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct Shadows {
    /// One fixed sun box instead of camera-fitted cascades (default none).
    pub fixed_box: Option<ShadowBox>,
    /// Reuse unchanged opaque, unskinned model casters (default false).
    /// Moving, fading, custom-shaded and primitive casters still draw each frame.
    pub cache_static: bool,
    /// Width and height of each depth layer, clamped to 128–4096 (default 2048).
    pub resolution: u32,
    /// Camera view depth reached by shadows, in metres (default 60).
    pub distance: f32,
    /// Number of cascades, clamped to 1–3 (default 3).
    pub cascades: u32,
    /// Receiver-plane PCF radius in shadow texels (default 1.5).
    pub softness: f32,
    /// Receiver-plane filtering by default.
    pub filter: ShadowFilter,
    /// Shadow darkness, clamped to 0–1 (default 1).
    pub strength: f32,
}
impl Default for Shadows {
    fn default() -> Self {
        Self {
            fixed_box: None,
            cache_static: false,
            resolution: 2048,
            distance: 60.0,
            cascades: 3,
            softness: 1.5,
            filter: ShadowFilter::default(),
            strength: 1.,
        }
    }
}

/// Sun-shadow reconstruction. Receiver-plane filtering preserves steep contacts;
/// PCF-soft uses a fixed three-texel box and explicit bias.
#[derive(Debug, Clone, Copy, PartialEq, Default, Data)]
pub enum ShadowFilter {
    /// Project the filter onto the receiver and correct each tap's depth.
    #[default]
    ReceiverPlane,
    /// The PCF-soft weight pattern used by three.js, reconstructed with four
    /// hardware bilinear comparisons. `Shadows::softness` does not affect it.
    PcfSoft {
        /// Subtracted from normalized shadow depth; positive values reduce acne.
        depth_bias: f32,
        /// Offset along the receiver normal in world metres, as in three.js normalBias.
        /// Independent of cascade extent and shadow-map resolution.
        normal_bias: f32,
    },
    /// One hardware bilinear comparison (four depth texels), with a narrower
    /// footprint than PCF-soft. `Shadows::softness` does not affect it.
    Bilinear {
        /// Subtracted from normalized shadow depth; positive values reduce acne.
        depth_bias: f32,
        /// Offset along the receiver normal in world metres.
        normal_bias: f32,
    },
}
