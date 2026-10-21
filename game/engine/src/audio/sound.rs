//! Sound definitions that are either synthesized or sampled, and their saved forms.
//! Synth records, and voices using none of the sample-era fields, encode exactly as
//! they did before samples existed, so synth-only saves and hashes are unchanged.
//! @ref LLP 1046.003 §AU4 (saved forms)
use super::{Synth, Voice};
use crate::asset::SOUND_RATES;
use crate::{Data, DataError, Reader, Writer};

/// A baked `.sound` asset, declared in `Game::ASSETS` so setup waits for it.
/// `World::sounds` resolves its frames, rate and channels from the delivered asset;
/// those saved values give voices their lengths without the PCM.
#[derive(Data, Clone, Debug, PartialEq)]
pub struct Sample {
    /// Declared asset name, ending in `.sound`.
    pub asset: String,
    /// Definition gain, nonnegative.
    pub gain: f32,
    /// Repeat the whole sample. Loops are not limited to 60 seconds.
    pub looping: bool,
    /// Frames per channel, resolved at registration.
    pub frames: u64,
    /// Source frames per second, resolved at registration.
    pub rate: u32,
    /// One or two, resolved at registration.
    pub channels: u32,
}
impl Default for Sample {
    fn default() -> Self {
        Self {
            asset: String::new(),
            gain: 1.0,
            looping: false,
            frames: 0,
            rate: 0,
            channels: 0,
        }
    }
}
impl Sample {
    /// A declared `.sound` asset at unit gain.
    pub fn new(asset: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            ..Self::default()
        }
    }
    /// Definition gain.
    pub fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }
    /// Repeat until stopped or faded.
    pub fn looped(mut self) -> Self {
        self.looping = true;
        self
    }
    /// Seconds of one pass.
    pub fn duration(&self) -> f32 {
        if self.rate == 0 {
            0.0
        } else {
            (self.frames as f64 / self.rate as f64) as f32
        }
    }
    pub(crate) fn validate_data(&self) -> Result<(), DataError> {
        let fail = |message: &str, field: &str| Err(DataError::new(message).at(field));
        if !crate::asset::asset_name(&self.asset) || !self.asset.ends_with(".sound") {
            return fail("sample asset must be a .sound name", "asset");
        }
        if !self.gain.is_finite() || self.gain < 0.0 {
            return fail("sample gain must be finite and nonnegative", "gain");
        }
        if self.rate == 0 && self.frames == 0 && self.channels == 0 {
            return fail(
                "sample is unresolved; register it with World::sounds after its asset arrives",
                "rate",
            );
        }
        if !SOUND_RATES.contains(&self.rate) {
            return fail("sample rate is outside 8000..=192000 Hz", "rate");
        }
        if !(1..=2).contains(&self.channels) {
            return fail("sample must be mono or stereo", "channels");
        }
        if self.frames == 0
            || self.frames * u64::from(self.channels) * 2 > crate::asset::SOUND_BYTE_BUDGET as u64
        {
            return fail("sample frames exceed the sound residency budget", "frames");
        }
        Ok(())
    }
}

/// What a definition plays.
#[derive(Clone, Debug)]
pub enum Sound {
    /// Rendered from parameters by the executor.
    Synth(Synth),
    /// Baked PCM, resident once in the world's asset store.
    Sample(Sample),
}
impl Default for Sound {
    fn default() -> Self {
        Self::Synth(Synth::default())
    }
}
impl From<Synth> for Sound {
    fn from(synth: Synth) -> Self {
        Self::Synth(synth)
    }
}
impl From<Sample> for Sound {
    fn from(sample: Sample) -> Self {
        Self::Sample(sample)
    }
}
impl Sound {
    /// Seconds of one pass (the longest synth layer, or the sample's frames).
    pub fn duration(&self) -> f32 {
        match self {
            Self::Synth(s) => s.duration(),
            Self::Sample(s) => s.duration(),
        }
    }
    /// Whether voices and sources repeat it.
    pub fn looping(&self) -> bool {
        match self {
            Self::Synth(s) => s.looping,
            Self::Sample(s) => s.looping,
        }
    }
    /// The synth, when synthesized.
    pub fn synth(&self) -> Option<&Synth> {
        match self {
            Self::Synth(s) => Some(s),
            Self::Sample(_) => None,
        }
    }
    /// The sample, when sampled.
    pub fn sample(&self) -> Option<&Sample> {
        match self {
            Self::Synth(_) => None,
            Self::Sample(s) => Some(s),
        }
    }
    pub(crate) fn validate_data(&self) -> Result<(), DataError> {
        match self {
            Self::Synth(s) => s.validate_data(),
            Self::Sample(s) => s.validate_data().map_err(|e| e.at("sample")),
        }
    }
}

// One list, so the record's order and names cannot drift between write and read.
macro_rules! synth_fields {
    ($each:ident) => {
        $each!(
            wave,
            hz,
            slide,
            vibrato_hz,
            vibrato_depth,
            attack,
            decay,
            sustain,
            release,
            seconds,
            lowpass_hz,
            highpass_hz,
            gain,
            layers,
            looping
        )
    };
}
impl Synth {
    fn write_fields(&self, w: &mut dyn Writer) {
        macro_rules! write_each {
            ($($f:ident),*) => { $( w.field(stringify!($f)); self.$f.write(w); )* };
        }
        synth_fields!(write_each);
    }
    fn read_field(&mut self, field: &str, r: &mut dyn Reader) -> Result<bool, DataError> {
        macro_rules! read_each {
            ($($f:ident),*) => {
                match field {
                    $( stringify!($f) => self.$f.read(r).map_err(|e| e.at(stringify!($f)))?, )*
                    _ => return Ok(false),
                }
            };
        }
        synth_fields!(read_each);
        Ok(true)
    }
}
impl Data for Synth {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        self.write_fields(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            if !self.read_field(&field, r)? {
                r.skip().map_err(|e| e.at(&field))?;
            }
        }
        Ok(())
    }
}
/// A synth keeps its original record; a sample is a record whose one field is `sample`.
impl Data for Sound {
    fn write(&self, w: &mut dyn Writer) {
        match self {
            Self::Synth(synth) => synth.write(w),
            Self::Sample(sample) => {
                w.begin_struct();
                w.field("sample");
                sample.write(w);
                w.end_struct();
            }
        }
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut synth = Synth::default();
        let mut sample = None;
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            if field == "sample" {
                let mut value = Sample::default();
                value.read(r).map_err(|e| e.at("sample"))?;
                sample = Some(value);
            } else if !synth.read_field(&field, r)? {
                r.skip().map_err(|e| e.at(&field))?;
            }
        }
        *self = sample.map_or(Self::Synth(synth), Self::Sample);
        Ok(())
    }
}

/// A linear gain ramp from `start` at tick `from` to `end` at tick `to`, held after.
#[derive(Data, Default, Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    /// First tick of the ramp.
    pub from: u64,
    /// Tick at which the ramp reaches `end`.
    pub to: u64,
    /// Gain multiplier at `from`.
    pub start: f32,
    /// Gain multiplier from `to` on.
    pub end: f32,
}
impl Fade {
    /// Gain multiplier at a world tick.
    pub fn at(&self, tick: u64) -> f32 {
        if tick <= self.from {
            self.start
        } else if tick >= self.to {
            self.end
        } else {
            let t = (tick - self.from) as f32 / (self.to - self.from) as f32;
            self.start + (self.end - self.start) * t
        }
    }
}

/// Voice fields added with samples are written only when set.
impl Data for Voice {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("id");
        self.id.write(w);
        w.field("sound");
        self.sound.write(w);
        w.field("at");
        self.at.write(w);
        w.field("gain");
        self.gain.write(w);
        w.field("pitch");
        self.pitch.write(w);
        w.field("position");
        self.position.write(w);
        w.field("synth");
        self.definition.write(w);
        w.field("began");
        self.began.write(w);
        w.field("ends");
        self.ends.write(w);
        if self.offset != 0.0 {
            w.field("offset");
            self.offset.write(w);
        }
        if self.pan != 0.0 {
            w.field("pan");
            self.pan.write(w);
        }
        if self.spatial.is_some() {
            w.field("spatial");
            self.spatial.write(w);
        }
        if self.fade.is_some() {
            w.field("fade");
            self.fade.write(w);
        }
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            let at = field.clone();
            match field.as_str() {
                "id" => self.id.read(r),
                "sound" => self.sound.read(r),
                "at" => self.at.read(r),
                "gain" => self.gain.read(r),
                "pitch" => self.pitch.read(r),
                "position" => self.position.read(r),
                "synth" => self.definition.read(r),
                "began" => self.began.read(r),
                "ends" => self.ends.read(r),
                "offset" => self.offset.read(r),
                "pan" => self.pan.read(r),
                "fade" => self.fade.read(r),
                "spatial" => self.spatial.read(r),
                _ => r.skip(),
            }
            .map_err(|e| e.at(at))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod sample_tests;
