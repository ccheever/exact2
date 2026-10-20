use crate::{Color, Data, Resource};

/// Linear sky colours and optional atmospheric/post-processing effects.
/// Absence of this resource has the same presentation as its default.
#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct Environment {
    /// Optional flat background, independent of ambient illumination:
    /// `Some(Color::hex("#7daba3"))`.
    pub background: Option<Color<3>>,
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
impl Environment {
    /// Clear water seen from inside it: a tank or a shallow sea. The whole frame
    /// sits in one medium, so the sky is the water's colour and fog is uniform.
    /// Pair it with a sun of about 6,000 lux from above and no opaque surface
    /// between the sun and the floor; [the underwater recipe](crate::recipes::underwater)
    /// has the scene, its camera framing and the mean luminance it was measured at.
    pub fn underwater() -> Self {
        Self {
            // The fog hides the whole sky, so a flat background would never show.
            background: None,
            // Light through the surface overhead: brighter, green-blue.
            zenith: [0.3, 0.62, 0.7],
            // The water's own colour, which the fog fades everything towards.
            horizon: [0.03, 0.16, 0.2],
            // Little light comes back up from below.
            ground: [0.02, 0.05, 0.05],
            ambient: 0.6,
            // The sun is not seen as a disc through water.
            sun_disc: 0.,
            exposure: 0.9,
            tone_mapping: ToneMapping::Aces,
            // Uniform extinction (no height falloff): a surface 32.9 m away keeps
            // 10% of its colour, ln 10 / 0.07.
            fog: Some(Fog::new(0.07, 0.)),
            // The default threshold on HDR brightness (the brightest channel,
            // before exposure) at a weaker glow than the default 0.16.
            bloom: Some(Bloom {
                threshold: 1.,
                intensity: 0.1,
                radius: 1.5,
            }),
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
/// Water with a flat surface at `height`, an optional effect: inserting this
/// resource draws the surface and confines `Environment`'s exponential fog to
/// the part of each view ray below it. From below, the surface shows the sky
/// through Snell's window and reflects the depths outside it; from above, it
/// reflects the sky over what lies beneath. `caustics` and `shafts` add the
/// sun's light through the moving surface. Saved and hashed like any resource;
/// the wave phase and every GPU product derive from the displayed time.
#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct Water {
    /// The surface's world Y, in metres.
    pub height: f32,
    /// Linear colour of the water seen from above where the floor is not.
    pub color: [f32; 3],
    /// From above, the share of what lies beneath that shows through, 0–1.
    pub clarity: f32,
    /// The surface's two summed waves, which bend its normals (the plane stays flat).
    pub waves: Waves,
    /// The sun's light focused by the waves onto lit surfaces below the height.
    pub caustics: Option<Caustics>,
    /// Sunlight scattered in the water: shafts seen from below the surface.
    pub shafts: Option<Shafts>,
}
impl Water {
    /// A surface at `height` metres, clear blue-green, with gentle waves,
    /// caustics and shafts.
    pub fn at(height: f32) -> Self {
        assert!(height.is_finite(), "a water height in metres");
        Self {
            height,
            color: [0.02, 0.12, 0.15],
            clarity: 0.8,
            waves: Waves::default(),
            caustics: Some(Caustics::default()),
            shafts: Some(Shafts::default()),
        }
    }
}
/// The codec's default (`Data` needs one): a surface at Y = 0. A game names
/// its height with [`Water::at`].
impl Default for Water {
    fn default() -> Self {
        Self::at(0.)
    }
}
/// Two waves summed across the surface: the second at 0.61 of the wavelength,
/// crossing the first at about 70°.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Waves {
    /// Height of each wave, in metres.
    pub amplitude: f32,
    /// Crest to crest of the longer wave, in metres.
    pub wavelength: f32,
    /// Speed of the longer wave, in metres a second.
    pub speed: f32,
}
impl Default for Waves {
    fn default() -> Self {
        Self {
            amplitude: 0.02,
            wavelength: 2.,
            speed: 0.6,
        }
    }
}
/// A moving net of focused light on surfaces under the water, multiplied into
/// the sun's direct light, so a surface in the sun's shadow shows none.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Caustics {
    /// 0 leaves the sunlight even; 1 concentrates it wholly into the net.
    pub strength: f32,
    /// Width of one cell of the net, in metres.
    pub scale: f32,
}
impl Default for Caustics {
    fn default() -> Self {
        Self {
            strength: 0.6,
            scale: 1.5,
        }
    }
}
/// Sunlight scattered by the water between the camera and what it sees, in
/// streaks from the surface towards the sun. The scattering is the
/// `Environment`'s fog: without fog there are no shafts.
#[derive(Clone, Copy, Debug, PartialEq, Data)]
pub struct Shafts {
    /// Brightness of fully scattered light looking towards the sun, as a share
    /// of the sun's.
    pub intensity: f32,
    /// How far each streak reaches across the frame, as a share of its height.
    pub length: f32,
}
impl Default for Shafts {
    fn default() -> Self {
        Self {
            intensity: 1.,
            length: 0.6,
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
    /// Fog colour, `Some(Color::hex("#7daba3"))`; None uses the horizon.
    pub color: Option<Color<3>>,
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
