//! Deterministic sound descriptions and journal events. This module opens no device.
use crate::data::text::{Fixed, Float};
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

/// A mono subtractive voice. All numeric parameters must be finite.
/// Seconds includes release; layers start together.
/// ADSR releases from the current envelope level at `seconds - release`.
#[derive(Data, Clone, Debug)]
pub struct Synth {
    /// Oscillator.
    pub wave: Wave,
    /// Starting frequency in Hz, nonnegative.
    pub hz: f32,
    /// Pitch glide in semitones per second, either sign.
    pub slide: f32,
    /// Vibrato frequency in Hz, nonnegative.
    pub vibrato_hz: f32,
    /// Vibrato amplitude in semitones, nonnegative.
    pub vibrato_depth: f32,
    /// Attack seconds, nonnegative.
    pub attack: f32,
    /// Decay seconds, nonnegative.
    pub decay: f32,
    /// Sustained level, 0 to 1.
    pub sustain: f32,
    /// Release seconds, nonnegative, included in duration.
    pub release: f32,
    /// Total duration, 0..=60 seconds (layers may extend it).
    pub seconds: f32,
    /// Low-pass cutoff in Hz, nonnegative; zero disables it.
    pub lowpass_hz: f32,
    /// High-pass cutoff in Hz, nonnegative; zero disables it.
    pub highpass_hz: f32,
    /// This oscillator's gain, nonnegative; layers have independent gains.
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
    /// Validate authored parameters before synthesis or registration.
    pub fn validate(&self) {
        let result = self.validate_data();
        assert!(result.is_ok(), "invalid synth: {result:?}");
    }
    fn validate_data(&self) -> Result<(), crate::DataError> {
        for (name, n) in [
            ("hz", self.hz),
            ("slide", self.slide),
            ("vibrato_hz", self.vibrato_hz),
            ("vibrato_depth", self.vibrato_depth),
            ("attack", self.attack),
            ("decay", self.decay),
            ("sustain", self.sustain),
            ("release", self.release),
            ("seconds", self.seconds),
            ("lowpass_hz", self.lowpass_hz),
            ("highpass_hz", self.highpass_hz),
            ("gain", self.gain),
        ] {
            if !n.is_finite() {
                return Err(crate::DataError::new("nonfinite synth parameter").at(name));
            }
        }
        if !(0.0..=60.0).contains(&self.seconds) {
            return Err(
                crate::DataError::new("synth duration must be 0..60 seconds").at("seconds"),
            );
        }
        if !(0.0..=1.0).contains(&self.sustain) {
            return Err(crate::DataError::new("synth sustain must be 0..1").at("sustain"));
        }
        for (name, n) in [
            ("hz", self.hz),
            ("vibrato_hz", self.vibrato_hz),
            ("vibrato_depth", self.vibrato_depth),
            ("lowpass_hz", self.lowpass_hz),
            ("highpass_hz", self.highpass_hz),
            ("gain", self.gain),
            ("attack", self.attack),
            ("decay", self.decay),
            ("release", self.release),
        ] {
            if n < 0.0 {
                return Err(crate::DataError::new("synth parameter must be nonnegative").at(name));
            }
        }
        for (i, layer) in self.layers.iter().enumerate() {
            layer.validate_data().map_err(|e| e.at(i).at("layers"))?;
        }
        Ok(())
    }
}
/// Immutable definition shared by the registry and voices that started with it.
/// The content identity is computed once at registration/restore, never per frame.
#[derive(Clone, Debug)]
pub struct Definition {
    synth: std::sync::Arc<Synth>,
    revision: u64,
}
impl Definition {
    /// Freeze an authored definition.
    pub fn new(synth: Synth) -> Self {
        synth.validate();
        let revision = crate::hash::of(&synth);
        Self {
            synth: std::sync::Arc::new(synth),
            revision,
        }
    }
    /// Stable content revision, also valid across save/restore.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
impl Default for Definition {
    fn default() -> Self {
        Self::new(Synth::default())
    }
}
impl std::ops::Deref for Definition {
    type Target = Synth;
    fn deref(&self) -> &Synth {
        &self.synth
    }
}
impl Data for Definition {
    fn write(&self, w: &mut dyn crate::Writer) {
        self.synth.write(w);
    }
    fn read(&mut self, r: &mut dyn crate::Reader) -> Result<(), crate::DataError> {
        let mut synth = Synth::default();
        synth.read(r)?;
        synth.validate_data()?;
        let revision = crate::hash::of(&synth);
        *self = Self {
            synth: std::sync::Arc::new(synth),
            revision,
        };
        Ok(())
    }
}
/// Retained mono PCM budget for presentation executors.
pub const PCM_BYTE_BUDGET: usize = 32 * 1024 * 1024;
/// Surface synthesis rate; executors with other rates also reserve before rendering.
pub const SAMPLE_RATE: u32 = 48000;

/// Sound definitions, authored during setup and included in saves and hashes.
#[derive(Resource, Default, Clone)]
pub struct Sounds(pub BTreeMap<String, Definition>);
impl Sounds {
    /// Define or replace a named sound.
    pub fn add(&mut self, name: impl Into<String>, synth: Synth) -> &mut Self {
        let name = name.into();
        synth.validate();
        self.0.insert(name, Definition::new(synth));
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
            Self::Point(p) => format!("({},{},{})", Fixed(p.x, 3), Fixed(p.y, 3), Fixed(p.z, 3)),
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
    pub synth: Definition,
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
/// A sound attached to an entity; looping belongs to its definition.
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
impl AudioSource {
    /// Play this named sound by default. Register the definition during setup.
    pub fn new(sound: impl Into<String>) -> Self {
        Self {
            sound: sound.into(),
            ..Self::default()
        }
    }
    /// Per-source gain, sanitized at playback and reporting.
    pub fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
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
    /// Build a play event; call `start()` to commit it.
    pub fn play(&self, sound: &str) -> Play<'_> {
        let synth = self
            .try_resource::<Sounds>()
            .and_then(|sounds| sounds.0.get(sound).cloned())
            .unwrap_or_else(|| panic!("unknown sound `{sound}`"));
        let duration = synth.duration();
        let began = self.audio_boundary();
        let ends = began.saturating_add(math::ceil(duration * self.hz() as f32) as u64);
        Play {
            world: self,
            voice: Voice {
                sound: sound.into(),
                at: At::Ui,
                gain: 1.0,
                pitch: 1.0,
                position: None,
                synth,
                began,
                ends,
                id: 0,
            },
        }
    }
}
/// A pending play. Only `start()` adds a voice and its journal line.
#[must_use = "call .start() to play the sound"]
pub struct Play<'a> {
    world: &'a World,
    voice: Voice,
}
impl Play<'_> {
    /// Follow an entity.
    pub fn at(mut self, entity: Entity) -> Self {
        let voice = &mut self.voice;
        voice.at = At::Entity(entity);
        self
    }
    /// Play at a fixed position.
    pub fn at_point(mut self, point: Vec3) -> Self {
        self.voice.at = At::Point(point);
        self
    }
    /// Play without spatial attenuation.
    pub fn ui(mut self) -> Self {
        self.voice.at = At::Ui;
        self
    }
    /// Set play gain.
    pub fn gain(mut self, gain: f32) -> Self {
        let sanitized = crate::audio::gain(gain);
        if sanitized != gain {
            self.world.log("refusal: invalid play gain");
        }
        self.voice.gain = sanitized;
        self
    }
    /// Playback rate (1 is authored pitch), bounded to 0.01..16.
    pub fn pitch(mut self, rate: f32) -> Self {
        let rate = if rate.is_finite() {
            rate.clamp(0.01, 16.0)
        } else {
            1.0
        };
        let voice = &mut self.voice;
        voice.pitch = rate;
        voice.ends = voice.began.saturating_add(math::ceil(
            voice.synth.duration() / rate * self.world.hz() as f32,
        ) as u64);
        self
    }
    /// Commit now and return the voice handle.
    pub fn start(self) -> VoiceId {
        let mut voice = self.voice;
        let mut voices = self.world.resource_mut::<Voices>();
        voice.id = voices.next_id;
        voices.next_id = voices.next_id.checked_add(1).expect("voice ids exhausted");
        self.world.log(format_args!(
            "sfx {} at {} gain {}",
            voice.sound,
            voice.at.label(self.world),
            Fixed(voice.gain, 2)
        ));
        let id = voice.id;
        voices.voices.push(voice);
        id
    }
}
// Snapshot voices when removing their entity, after all author borrows end.
pub(crate) fn detach(world: &World, entity: Entity) {
    if world.has_audio() {
        let mut voices = world.resource_mut::<Voices>();
        for voice in &mut voices.voices {
            if matches!(voice.at, At::Entity(e) if e == entity) {
                voice.position = world.current_global(entity).map(|t| t.translation.into());
            }
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
        let mut reports = Vec::with_capacity(audio.reports.len());
        let mut previous = audio.reports.iter().peekable();
        for (entity, source) in world.query::<&mut AudioSource>().iter() {
            while previous
                .peek()
                .is_some_and(|r| r.entity.index() < entity.index())
            {
                let old = previous.next().unwrap();
                if old.playing {
                    world.log(format_args!("loop {} off", old.sound));
                }
            }
            let old = if previous
                .peek()
                .is_some_and(|r| r.entity.index() == entity.index())
            {
                let old = previous.next().unwrap();
                if old.entity == entity {
                    Some(old)
                } else {
                    if old.playing {
                        world.log(format_args!("loop {} off", old.sound));
                    }
                    None
                }
            } else {
                None
            };
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
                    "loop {} on gain {}",
                    source.sound,
                    Fixed(source.gain, 2)
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
        for old in previous {
            if old.playing {
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
                    Float(v.gain),
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
                Float(gain(s.gain)),
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

#[cfg(test)]
mod decode_regression {
    use super::*;
    #[test]
    fn documented_synth_boundaries_round_trip() {
        for synth in [
            Synth::sine(0.).seconds(0.).sustain(0.).gain(0.),
            Synth::sine(440.)
                .seconds(60.)
                .sustain(1.)
                .slide(-48.)
                .gain(f32::MAX),
        ] {
            let definition = Definition::new(synth);
            let copy: Definition =
                crate::bin::from_slice(&crate::bin::to_vec(&definition)).unwrap();
            assert_eq!(copy.revision(), definition.revision());
        }
    }
    #[test]
    fn malformed_saved_synth_is_refused_without_changing_world() {
        let mut world = World::new(60, 42);
        world.register_audio();
        world
            .resource_mut::<Sounds>()
            .add("tone", Synth::sine(440.));
        world.play("tone").start();
        let before = world.save();
        let hash = world.hash();
        let journal = world.journal();
        let mut invalid = vec![
            Synth::sine(-1.),
            Synth::sine(440.).seconds(-1.),
            Synth::sine(440.).attack(-1.),
            Synth::sine(440.).decay(-1.),
            Synth::sine(440.).release(-1.),
            Synth::sine(440.).seconds(61.),
            Synth::sine(440.).sustain(2.),
            Synth::sine(440.).sustain(-1.),
            Synth::sine(440.).vibrato_hz(-1.),
            Synth::sine(440.).vibrato_depth(-1.),
            Synth::sine(440.).lowpass_hz(-1.),
            Synth::sine(440.).highpass_hz(-1.),
            Synth::sine(440.).gain(-1.),
            Synth::sine(440.).slide(f32::INFINITY),
            Synth::sine(440.).layer(Synth::sine(1.).sustain(2.)),
        ];
        let setters: [fn(Synth, f32) -> Synth; 12] = [
            Synth::hz,
            Synth::slide,
            Synth::vibrato_hz,
            Synth::vibrato_depth,
            Synth::attack,
            Synth::decay,
            Synth::sustain,
            Synth::release,
            Synth::seconds,
            Synth::lowpass_hz,
            Synth::highpass_hz,
            Synth::gain,
        ];
        for set in setters {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                invalid.push(set(Synth::default(), value));
            }
        }
        for synth in invalid {
            for voice in [false, true] {
                let mut malformed = World::new(60, 0);
                malformed.register_audio();
                malformed.load(&before).unwrap();
                let bad = Definition {
                    synth: std::sync::Arc::new(synth.clone()),
                    revision: 0,
                };
                if voice {
                    malformed.resource_mut::<Voices>().voices[0].synth = bad;
                } else {
                    malformed
                        .resource_mut::<Sounds>()
                        .0
                        .insert("tone".into(), bad);
                }
                let error = world
                    .load(&malformed.save())
                    .expect_err("invalid synth must be refused");
                assert!(error.to_string().contains("synth"), "{error}");
                assert_eq!(world.save(), before);
                assert_eq!(world.hash(), hash);
                assert_eq!(world.journal().len(), journal.len());
            }
        }
    }
}

#[cfg(test)]
mod carry_regression {
    use super::*;
    use crate::{Game, Sim};
    struct Tone;
    impl Game for Tone {
        type Args = ();
        const NAME: &'static str = "tone";
        const ID: &'static str = "test.tone";
        fn setup(world: &mut World, _: &()) {
            world.register_audio();
            world
                .resource_mut::<Sounds>()
                .add("tone", Synth::sine(880.));
        }
        fn tick(_: &mut World, _: &crate::Input, _: &()) {}
    }
    #[test]
    fn restore_keeps_saved_registry_and_runtime_registered_names() {
        let mut old = Sim::<Tone>::new(()).unwrap();
        old.world_mut()
            .resource_mut::<Sounds>()
            .add("tone", Synth::sine(220.));
        old.world_mut().play("tone").start();
        let mut fresh = Sim::<Tone>::new(()).unwrap();
        old.world_mut()
            .resource_mut::<Sounds>()
            .add("runtime", Synth::sine(330.));
        fresh.restore_bound(&old.save().unwrap()).unwrap();
        assert!(fresh.world().resource::<Sounds>().0.contains_key("runtime"));
        fresh.world_mut().play("tone").start();
        let voices = &fresh.world().resource::<Voices>().voices;
        assert_eq!(voices[0].synth.hz, 220.);
        assert_eq!(voices[1].synth.hz, 220.);
    }
}

#[cfg(test)]
mod authoring_regression {
    use super::*;
    #[test]
    fn registered_sound_plays_through_a_shared_world_during_component_borrow() {
        let mut world = World::new(60, 0);
        world.register_audio();
        world
            .resource_mut::<Sounds>()
            .add("chime", Synth::sine(440.));
        let entity = world.spawn((
            crate::Transform::default(),
            AudioSource::new("chime").gain(0.3),
        ));
        let shared: &World = &world;
        let _pose = shared.get_mut::<crate::Transform>(entity).unwrap();
        let source = shared.get_mut::<AudioSource>(entity).unwrap();
        assert!(source.playing);
        assert_eq!(source.gain, 0.3);
        shared.play("chime").at(entity).start();
        assert_eq!(shared.resource::<Voices>().voices.len(), 1);
    }
}
