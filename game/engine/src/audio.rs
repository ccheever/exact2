//! Deterministic sound descriptions and journal events. This module opens no device.
use crate::{math, Component, Data, Entity, Resource, Vec3, World};
use std::collections::BTreeMap;

/// Oscillator shape. Noise uses a fixed local integer stream, never the world's RNG.
#[derive(Data, Default, Clone, Copy, Debug)]
pub enum Wave {
    /// Sinusoid.
    #[default]
    Sine,
    /// Positive for the first half of a cycle.
    Square,
    /// Rising ramp from -1 to 1.
    Saw,
    /// Triangle starting at -1.
    Triangle,
    /// Uniform white noise.
    Noise,
}

/// A mono subtractive voice. Seconds includes release; layers start together.
/// ADSR releases from the current envelope level at `seconds - release`.
#[derive(Data, Clone, Debug)]
pub struct Synth {
    /// Oscillator.
    pub wave: Wave,
    /// Starting frequency in Hz.
    pub hz: f32,
    /// Pitch glide in semitones per second.
    pub slide: f32,
    /// Vibrato frequency in Hz.
    pub vibrato_hz: f32,
    /// Vibrato amplitude in semitones.
    pub vibrato_depth: f32,
    /// Attack seconds.
    pub attack: f32,
    /// Decay seconds.
    pub decay: f32,
    /// Sustained level, 0 to 1.
    pub sustain: f32,
    /// Release seconds, included in duration.
    pub release: f32,
    /// Total duration of this oscillator (layers may extend it).
    pub seconds: f32,
    /// Low-pass cutoff in Hz; zero disables it.
    pub lowpass_hz: f32,
    /// High-pass cutoff in Hz; zero disables it.
    pub highpass_hz: f32,
    /// This oscillator's gain; layers have independent gains.
    pub gain: f32,
    /// Simultaneous voices, summed without clipping.
    pub layers: Vec<Synth>,
    /// Crossfade the loop seam after synthesis. One-shots default to false.
    pub looping: bool,
}
impl Default for Synth {
    fn default() -> Self {
        Self {
            wave: Wave::Sine,
            hz: 440.0,
            slide: 0.0,
            vibrato_hz: 0.0,
            vibrato_depth: 0.0,
            attack: 0.005,
            decay: 0.15,
            sustain: 0.0,
            release: 0.05,
            seconds: 0.5,
            lowpass_hz: 0.0,
            highpass_hz: 0.0,
            gain: 0.5,
            layers: vec![],
            looping: false,
        }
    }
}
macro_rules! setters {
    ($($name:ident),*) => { $(
        #[doc = concat!("Set `", stringify!($name), "`.")]
        pub fn $name(mut self, value: f32) -> Self { self.$name = value; self }
    )* };
}
impl Synth {
    /// A sine at the given pitch.
    pub fn sine(hz: f32) -> Self {
        Self {
            hz,
            ..Self::default()
        }
    }
    /// A square at the given pitch.
    pub fn square(hz: f32) -> Self {
        Self {
            wave: Wave::Square,
            ..Self::sine(hz)
        }
    }
    /// A sawtooth at the given pitch.
    pub fn saw(hz: f32) -> Self {
        Self {
            wave: Wave::Saw,
            ..Self::sine(hz)
        }
    }
    /// A triangle at the given pitch.
    pub fn triangle(hz: f32) -> Self {
        Self {
            wave: Wave::Triangle,
            ..Self::sine(hz)
        }
    }
    /// A reproducible noise burst.
    pub fn noise() -> Self {
        Self {
            wave: Wave::Noise,
            ..Self::default()
        }
    }
    setters!(
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
        gain
    );
    /// Sum another independently enveloped voice.
    pub fn layer(mut self, voice: Synth) -> Self {
        self.layers.push(voice);
        self
    }
    /// Render with a 10 ms equal-power overlap at the loop seam.
    pub fn looped(mut self) -> Self {
        self.looping = true;
        self
    }
    /// Longest layer, in seconds.
    pub fn duration(&self) -> f32 {
        self.layers
            .iter()
            .fold(self.seconds.max(0.0), |d, s| d.max(s.duration()))
    }
    fn validate(&self) {
        for n in [
            self.hz,
            self.slide,
            self.vibrato_hz,
            self.vibrato_depth,
            self.attack,
            self.decay,
            self.sustain,
            self.release,
            self.seconds,
            self.lowpass_hz,
            self.highpass_hz,
            self.gain,
        ] {
            assert!(n.is_finite(), "nonfinite synth parameter");
        }
        assert!(
            (0.0..=60.0).contains(&self.seconds),
            "synth duration must be 0..60 seconds"
        );
        assert!(self.hz >= 0.0 && self.attack >= 0.0 && self.decay >= 0.0 && self.release >= 0.0);
        for layer in &self.layers {
            layer.validate();
        }
    }
}
fn held(s: &Synth, t: f32) -> f32 {
    if t < s.attack {
        t / s.attack
    } else if t < s.attack + s.decay {
        math::lerp(1.0, s.sustain, (t - s.attack) / s.decay)
    } else {
        s.sustain
    }
}
fn envelope(s: &Synth, t: f32) -> f32 {
    let off = (s.seconds - s.release).max(0.0);
    if t >= s.seconds {
        0.0
    } else if t < off {
        held(s, t)
    } else if s.release > 0.0 {
        held(s, off) * (s.seconds - t) / (s.seconds - off)
    } else {
        held(s, t)
    }
}
/// Pure mono PCM, using only libm math, fixed operation order and a local noise seed.
/// No normalization: authored layers can exceed ±1; device outputs handle clipping.
pub fn render(s: &Synth, sample_rate: u32) -> Vec<f32> {
    assert!(sample_rate > 0, "sample rate must be positive");
    s.validate();
    let mut out = vec![0.0; math::ceil(s.duration() * sample_rate as f32) as usize];
    render_into(s, sample_rate, &mut out);
    for sample in &mut out {
        *sample = if sample.is_finite() {
            sample.clamp(-4.0, 4.0)
        } else {
            0.0
        };
    }
    if s.looping && out.len() > 3 {
        // Overlap the tail onto the head, then remove that tail. The end of the
        // overlap joins the untouched head; the wrap joins adjacent tail samples.
        let n = (sample_rate as usize / 100).max(2).min(out.len() / 2);
        let end = out.len() - n;
        for i in 0..n {
            let angle = i as f32 / (n - 1) as f32 * std::f32::consts::FRAC_PI_2;
            out[i] = (out[end + i] * math::cos(angle) + out[i] * math::sin(angle)).clamp(-4.0, 4.0);
        }
        out.truncate(end);
    }
    out
}
fn render_into(s: &Synth, rate: u32, out: &mut [f32]) {
    let tau = std::f32::consts::TAU;
    let dt = 1.0 / rate as f32;
    let lp = if s.lowpass_hz > 0.0 {
        1.0 - math::exp(-tau * s.lowpass_hz * dt)
    } else {
        1.0
    };
    let hp = if s.highpass_hz > 0.0 {
        math::exp(-tau * s.highpass_hz * dt)
    } else {
        0.0
    };
    let (mut phase, mut low, mut high, mut previous) = (0.0, 0.0, 0.0, 0.0);
    let mut noise = 0x12345678u32;
    for (i, sample) in out
        .iter_mut()
        .enumerate()
        .take(math::ceil(s.seconds * rate as f32) as usize)
    {
        let t = i as f32 * dt;
        let x = match s.wave {
            Wave::Sine => math::sin(tau * phase),
            Wave::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Saw => 2.0 * phase - 1.0,
            Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Wave::Noise => {
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                (noise >> 8) as f32 / 8388608.0 - 1.0
            }
        };
        low += lp * (x - low);
        let filtered = if hp != 0.0 {
            high = hp * (high + low - previous);
            previous = low;
            high
        } else {
            low
        };
        *sample += filtered * envelope(s, t) * s.gain;
        let semitones = s.slide * t + s.vibrato_depth * math::sin(tau * s.vibrato_hz * t);
        let frequency = (s.hz * math::powf(2.0, semitones / 12.0)).min(rate as f32 * 0.5);
        phase += frequency * dt;
        phase -= math::floor(phase);
    }
    for layer in &s.layers {
        render_into(layer, rate, out);
    }
}

/// Sound definitions, authored during setup and included in saves and hashes.
#[derive(Resource, Default, Clone)]
pub struct Sounds(pub BTreeMap<String, Synth>);
impl Sounds {
    /// Define or replace a named sound.
    pub fn add(&mut self, name: impl Into<String>, synth: Synth) -> &mut Self {
        synth.validate();
        self.0.insert(name.into(), synth);
        self
    }
}
/// Where a voice is heard. UI sounds bypass listener attenuation.
#[derive(Data, Default, Clone, Debug, PartialEq)]
pub enum At {
    /// Non-spatial sound.
    #[default]
    Ui,
    /// Follow this entity's global position.
    Entity(Entity),
    /// Fixed world position.
    Point(Vec3),
}
impl At {
    /// Journal and agent spelling.
    pub fn label(&self, world: &World) -> String {
        match self {
            Self::Ui => "ui".into(),
            Self::Entity(e) => world
                .name(*e)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("#{}", e.index())),
            Self::Point(p) => format!("({:.3},{:.3},{:.3})", p.x, p.y, p.z),
        }
    }
}
/// One finite sound in simulation state.
#[derive(Data, Default, Clone, Debug)]
pub struct Voice {
    /// Stable identity, allowing multiple identical sounds in one tick.
    pub id: u64,
    /// Sound definition name.
    pub sound: String,
    /// Source position.
    pub at: At,
    /// Per-play gain.
    pub gain: f32,
    /// Playback rate, saved with the voice.
    pub pitch: f32,
    /// Last observed world position, retained after despawn.
    pub position: Option<Vec3>,
    /// Definition at play time; edits affect subsequent plays.
    pub synth: Synth,
    /// Inclusive starting tick.
    pub began: u64,
    /// Exclusive ending tick, rounded up to a whole tick.
    pub ends: u64,
}
/// Finite voices and their saved identity allocator.
#[derive(Default, Clone)]
pub struct Voices {
    /// Currently playing voices.
    pub voices: Vec<Voice>,
    /// Next identity.
    pub next_id: u64,
}
impl crate::Resource for Voices {
    const NAME: &'static str = "Voices";
    const AMBIENT: bool = true;
}
impl Data for Voices {
    fn write(&self, w: &mut dyn crate::Writer) {
        w.begin_struct();
        w.field("voices");
        self.voices.write(w);
        w.field("next_id");
        self.next_id.write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn crate::Reader) -> Result<(), crate::DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field.as_str() {
                "voices" => self.voices.read(r)?,
                "next_id" => self.next_id.read(r)?,
                _ => r.skip()?,
            }
        }
        Ok(())
    }
}
/// A looping sound attached to an entity.
#[derive(Component, Clone)]
pub struct AudioSource {
    /// Sound definition name.
    pub sound: String,
    /// Per-source gain.
    pub gain: f32,
    /// Whether the loop is audible.
    pub playing: bool,
}
impl Default for AudioSource {
    fn default() -> Self {
        Self {
            sound: String::new(),
            gain: 1.0,
            playing: true,
        }
    }
}
/// A stable handle returned by `Play::start`.
pub type VoiceId = u64;
/// Saved master gain and source journal baseline.
#[derive(Resource, Clone)]
pub struct Audio {
    /// Linear master gain, default 1; games can set this from a live argument.
    pub master: f32,
    reports: Vec<SourceReport>,
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            master: 1.0,
            reports: Vec::new(),
        }
    }
}
#[derive(Data, Default, Clone)]
struct SourceReport {
    entity: Entity,
    sound: String,
    gain: f32,
    playing: bool,
    refused: bool,
}
/// Finite linear gain, bounded to avoid overflow at the output boundary.
pub fn gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 4.0)
    } else {
        0.0
    }
}
/// Stop a saved finite voice.
pub fn stop(world: &mut World, id: VoiceId) {
    if world.has_audio() {
        world.resource_mut::<Voices>().voices.retain(|v| v.id != id);
    }
}
/// The ears. `step` refuses extra listeners, keeping the lowest entity index.
#[derive(Component, Default, Clone)]
pub struct AudioListener;

impl World {
    fn audio_boundary(&self) -> u64 {
        self.tick().saturating_add(u64::from(self.in_tick))
    }
    /// Register audio types before loading a save; initialize resources only if absent.
    pub fn register_audio(&mut self) -> &mut Self {
        self.register::<AudioSource>()
            .register::<AudioListener>()
            .register_resource::<Sounds>()
            .register_resource::<Voices>()
            .register_resource::<Audio>();
        if self.try_resource::<Audio>().is_none() {
            self.insert_resource(Audio::default());
        }
        if self.try_resource::<Sounds>().is_none() {
            self.insert_resource(Sounds::default());
        }
        if self.try_resource::<Voices>().is_none() {
            self.insert_resource(Voices::default());
        }
        self
    }
    /// Whether audio resources are installed (worlds without audio stay unchanged).
    pub fn has_audio(&self) -> bool {
        self.try_resource::<Voices>().is_some()
    }
    /// Build a play event, committed once at the end of the statement.
    pub fn play(&mut self, sound: &str) -> Play<'_> {
        self.register_audio();
        let synth = self
            .resource::<Sounds>()
            .0
            .get(sound)
            .unwrap_or_else(|| panic!("unknown sound `{sound}`"))
            .clone();
        let duration = synth.duration();
        let began = self.audio_boundary();
        let ends = began.saturating_add(math::ceil(duration * self.hz() as f32) as u64);
        Play {
            world: self,
            voice: Some(Voice {
                sound: sound.into(),
                at: At::Ui,
                gain: 1.0,
                pitch: 1.0,
                position: None,
                synth,
                began,
                ends,
                id: 0,
            }),
        }
    }
}
/// A pending play. Dropping it commits exactly one journal line and one voice.
pub struct Play<'a> {
    world: &'a mut World,
    voice: Option<Voice>,
}
impl Play<'_> {
    /// Follow an entity.
    pub fn at(mut self, entity: Entity) -> Self {
        let voice = self.voice.as_mut().unwrap();
        voice.at = At::Entity(entity);
        voice.position = self.world.global(entity).map(|t| t.translation.into());
        self
    }
    /// Play at a fixed position.
    pub fn at_point(mut self, point: Vec3) -> Self {
        self.voice.as_mut().unwrap().at = At::Point(point);
        self
    }
    /// Play without spatial attenuation.
    pub fn ui(mut self) -> Self {
        self.voice.as_mut().unwrap().at = At::Ui;
        self
    }
    /// Set play gain.
    pub fn gain(mut self, gain: f32) -> Self {
        let sanitized = crate::audio::gain(gain);
        if sanitized != gain {
            self.world.log("refusal: invalid play gain");
        }
        self.voice.as_mut().unwrap().gain = sanitized;
        self
    }
    /// Playback rate (1 is authored pitch), bounded to 0.01..16.
    pub fn pitch(mut self, rate: f32) -> Self {
        let rate = if rate.is_finite() {
            rate.clamp(0.01, 16.0)
        } else {
            1.0
        };
        let voice = self.voice.as_mut().unwrap();
        voice.pitch = rate;
        voice.ends = voice.began.saturating_add(math::ceil(
            voice.synth.duration() / rate * self.world.hz() as f32,
        ) as u64);
        self
    }
    /// Commit now and return a handle. Drop still commits if this is omitted.
    pub fn start(mut self) -> VoiceId {
        self.commit().unwrap()
    }
    fn commit(&mut self) -> Option<VoiceId> {
        let mut voice = self.voice.take()?;
        let mut voices = self.world.resource_mut::<Voices>();
        voice.id = voices.next_id;
        voices.next_id = voices.next_id.checked_add(1).expect("voice ids exhausted");
        self.world.log(format_args!(
            "sfx {} at {} gain {:.2}",
            voice.sound,
            voice.at.label(self.world),
            voice.gain
        ));
        let id = voice.id;
        voices.voices.push(voice);
        Some(id)
    }
}
impl Drop for Play<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.commit();
        }
    }
}
/// Fixed-tick housekeeping, called after game logic like `physics::step`.
/// No audio resources are added to worlds that do not use sound.
pub fn step(world: &mut World) {
    if world.has_audio() {
        let tick = world.audio_boundary();
        world
            .resource_mut::<Voices>()
            .voices
            .retain(|v| v.ends > tick);
        for voice in &mut world.resource_mut::<Voices>().voices {
            if let At::Entity(e) = voice.at {
                if let Some(pose) = world.global(e) {
                    voice.position = Some(pose.translation.into());
                }
            }
        }
    }
    if world.has_audio() || world.query::<&AudioSource>().iter().next().is_some() {
        world.register_audio();
        let mut audio = world.resource_mut::<Audio>();
        audio.master = gain(audio.master);
        let mut reports = Vec::new();
        for (entity, source) in world.query::<&mut AudioSource>().iter() {
            let old = audio.reports.iter().find(|r| r.entity == entity);
            let sanitized = gain(source.gain);
            let invalid = sanitized != source.gain;
            let refused = old.is_some_and(|r| r.refused) || invalid;
            if invalid && !old.is_some_and(|r| r.refused) {
                world.log(format_args!(
                    "refusal: invalid AudioSource gain at {}",
                    At::Entity(entity).label(world)
                ));
            }
            source.gain = sanitized;
            if let Some(old) = old {
                if old.playing && (!source.playing || old.sound != source.sound) {
                    world.log(format_args!("loop {} off", old.sound));
                }
            }
            if source.playing
                && !old
                    .is_some_and(|r| r.playing && r.sound == source.sound && r.gain == source.gain)
            {
                world.log(format_args!(
                    "loop {} on gain {:.2}",
                    source.sound, source.gain
                ));
            }
            reports.push(SourceReport {
                entity,
                sound: source.sound.clone(),
                gain: source.gain,
                playing: source.playing,
                refused,
            });
        }
        for old in &audio.reports {
            if old.playing && !reports.iter().any(|r| r.entity == old.entity) {
                world.log(format_args!("loop {} off", old.sound));
            }
        }
        audio.reports = reports;
    }
    let listeners: Vec<_> = world
        .query::<&AudioListener>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for &e in listeners.iter().skip(1) {
        world.log(format_args!(
            "refusal: second AudioListener at {}",
            At::Entity(e).label(world)
        ));
        world.remove::<AudioListener>(e);
    }
}
/// The agent's audio section, derived from simulation state only.
pub fn state(world: &World) -> String {
    let voices = if world.has_audio() {
        world
            .resource::<Voices>()
            .voices
            .iter()
            .map(|v| {
                format!(
                    "{{\"sound\":{},\"at\":{},\"gain\":{},\"began\":{},\"ends\":{}}}",
                    crate::values::quote(&v.sound),
                    crate::values::quote(&v.at.label(world)),
                    v.gain,
                    v.began,
                    v.ends
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    } else {
        String::new()
    };
    let sources = world
        .query::<&AudioSource>()
        .iter()
        .map(|(e, s)| {
            format!(
                "{{\"sound\":{},\"entity\":{},\"gain\":{},\"playing\":{}}}",
                crate::values::quote(&s.sound),
                crate::values::quote(&At::Entity(e).label(world)),
                gain(s.gain),
                s.playing
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"voices\":[{voices}],\"sources\":[{sources}]}}")
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
