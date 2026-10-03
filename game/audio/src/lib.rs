//! Sound executors. Construction of Player<NullOutput> never opens an audio device.
#![deny(unsafe_code)]
mod surface;
mod synth;
pub use surface::SurfacePlayer;
pub use synth::render;

use exact_game::{
    audio::{self, At, AudioListener, AudioSource, Sound, Sounds, Voices},
    math, Quat, Vec3, World,
};
use std::{collections::BTreeMap, sync::Arc};
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos", test))]
#[allow(unsafe_code)]
mod apple;
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
pub use apple::AppleOutput;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebOutput;

/// Playback state supplied by the frame owner. Increment generation on seek,
/// restore, rebuild, and successful output unlock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    pub generation: u64,
    pub playing: bool,
}
impl Default for Transport {
    fn default() -> Self {
        Self {
            generation: 0,
            playing: true,
        }
    }
}
/// Immutable PCM handed to an output: synthesized mono `f32`, or a sound asset's
/// interleaved 16-bit frames shared with the world. Outputs key caches by address.
/// @ref LLP 1046.003 §AU4 (outputs)
#[derive(Clone, Debug)]
pub enum Pcm {
    /// Mono samples rendered by the Player.
    F32(Arc<[f32]>),
    /// Interleaved frames of a delivered `.sound` asset.
    I16 { samples: Arc<[i16]>, channels: u32 },
}
impl From<Arc<[f32]>> for Pcm {
    fn from(samples: Arc<[f32]>) -> Self {
        Self::F32(samples)
    }
}
impl Pcm {
    /// One or two.
    pub fn channels(&self) -> usize {
        match self {
            Self::F32(_) => 1,
            Self::I16 { channels, .. } => *channels as usize,
        }
    }
    /// Frames per channel.
    pub fn frames(&self) -> usize {
        match self {
            Self::F32(s) => s.len(),
            Self::I16 { samples, channels } => samples.len() / (*channels as usize).max(1),
        }
    }
    /// Retained bytes.
    pub fn bytes(&self) -> usize {
        match self {
            Self::F32(s) => s.len() * 4,
            Self::I16 { samples, .. } => samples.len() * 2,
        }
    }
    /// Allocation identity, for output caches.
    pub fn address(&self) -> usize {
        match self {
            Self::F32(s) => s.as_ptr() as usize,
            Self::I16 { samples, .. } => samples.as_ptr() as usize,
        }
    }
    /// One channel at a frame as a float in -1..1 (16-bit samples divide by 32768).
    pub fn sample(&self, frame: usize, channel: usize) -> f32 {
        match self {
            Self::F32(s) => s[frame],
            Self::I16 { samples, channels } => {
                f32::from(samples[frame * *channels as usize + channel]) / 32768.0
            }
        }
    }
}
/// One device or a test recorder. Set applies left and right gains: mono PCM feeds
/// both, stereo PCM's left channel feeds the left gain and its right the right.
pub trait Output {
    /// Request device activation from an input gesture.
    fn unlock(&mut self) {}
    fn capacity(&self) -> usize {
        usize::MAX
    }
    fn ready(&self) -> bool {
        true
    }
    fn flush(&mut self) {}
    fn retain_pcm(&mut self, _pcm: &[Pcm]) {}
    /// Whether the device still owns this allocation (including unacknowledged stops).
    fn owns_pcm(&self, _pcm: &Pcm) -> bool {
        false
    }
    /// Accept a start at `offset` frames of PCM recorded at `rate`, or leave it
    /// inactive so the Player retries next sync. The output resamples to its device.
    fn start(
        &mut self,
        id: u64,
        pcm: &Pcm,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> bool;
    fn set(&mut self, id: u64, gain_l: f32, gain_r: f32);
    fn stop(&mut self, id: u64);
}
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Start {
        id: u64,
        /// Frames per channel.
        samples: usize,
        channels: usize,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    },
    Set {
        id: u64,
        left: f32,
        right: f32,
    },
    Stop {
        id: u64,
    },
}
/// Discarding output for agent sessions and headless Linux. No call history.
#[derive(Default)]
pub struct NullOutput;
impl Output for NullOutput {
    fn capacity(&self) -> usize {
        0
    }
    fn start(&mut self, _: u64, _: &Pcm, _: u32, _: bool, _: usize, _: f32) -> bool {
        false
    }
    fn set(&mut self, _: u64, _: f32, _: f32) {}
    fn stop(&mut self, _: u64) {}
}
/// Explicit test recorder; never used by agent sessions.
pub struct RecordingOutput {
    pub calls: Vec<Call>,
    pub capacity: usize,
    pub ready: bool,
}
impl Default for RecordingOutput {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            capacity: usize::MAX,
            ready: true,
        }
    }
}
impl Output for RecordingOutput {
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn start(
        &mut self,
        id: u64,
        pcm: &Pcm,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> bool {
        self.calls.push(Call::Start {
            id,
            samples: pcm.frames(),
            channels: pcm.channels(),
            rate,
            looping,
            offset,
            pitch,
        });
        true
    }
    fn set(&mut self, id: u64, left: f32, right: f32) {
        self.calls.push(Call::Set { id, left, right });
    }
    fn stop(&mut self, id: u64) {
        self.calls.push(Call::Stop { id });
    }
}
/// World-space ears: local +X is right, local -Z is forward.
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub position: Vec3,
    pub rotation: Quat,
}
impl Listener {
    /// Lowest entity-order listener. Missing ears silence spatial sources.
    pub fn from_world(world: &World) -> Option<Self> {
        let e = world.query::<&AudioListener>().iter().next()?.0;
        let pose = world.global(e)?;
        let (_, rotation, position) = pose.to_scale_rotation_translation();
        Some(Self { position, rotation })
    }
}
/// Configurable Web Audio distance attenuation with equal-power pan.
pub fn spatial_gains(
    listener: Listener,
    point: Vec3,
    gain: f32,
    spatial: audio::Spatial,
) -> (f32, f32) {
    let delta = point - listener.position;
    let distance = delta.length();
    if !distance.is_finite() {
        return (0.0, 0.0);
    }
    let local = listener.rotation.conjugate() * delta;
    let pan = if distance > 0.000001 {
        (local.x / distance).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let level = gain * spatial.attenuation(distance);
    (
        level * math::sqrt((1.0 - pan) * 0.5),
        level * math::sqrt((1.0 + pan) * 0.5),
    )
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Voice(u64),
    Source(exact_game::Entity),
}
struct Active {
    output_id: u64,
    signature: (u64, u64, u32),
    pcm: Pcm,
}
struct Wanted {
    preferred: bool,
    key: Key,
    definition: audio::Definition,
    gains: (f32, f32),
    began: u64,
    looping: bool,
    pitch: f32,
    /// Seconds into the sound at `began`.
    start: f32,
    /// Frames into the sound now.
    offset: usize,
    /// The source's frames per second: the Player's for synths, the asset's for samples.
    rate: u32,
    frames: usize,
    /// A sample's resident PCM; synths render into the cache.
    sample: Option<Pcm>,
}
/// Synthesized PCM cache and identities belong to the executor, never to the world.
/// Sampled PCM stays the world's allocation; outputs share it.
pub struct Player<O: Output> {
    pub output: O,
    rate: u32,
    cache: BTreeMap<u64, Arc<[f32]>>,
    wanted: Vec<Wanted>,
    revisions: Vec<u64>,
    pcm: Vec<Pcm>,
    active: BTreeMap<Key, Active>,
    refused_sources: std::collections::BTreeSet<(exact_game::Entity, String)>,
    next_id: u64,
    transport: Option<Transport>,
}
impl<O: Output> Player<O> {
    pub fn new(output: O, rate: u32) -> Self {
        assert!(rate > 0);
        Self {
            output,
            rate,
            cache: BTreeMap::new(),
            wanted: Vec::new(),
            revisions: Vec::new(),
            pcm: Vec::new(),
            active: BTreeMap::new(),
            refused_sources: Default::default(),
            next_id: 0,
            transport: None,
        }
    }
    pub fn cached_sounds(&self) -> usize {
        self.cache.len()
    }
    fn stop_all(&mut self) {
        for active in self.active.values() {
            self.output.stop(active.output_id);
        }
        self.active.clear();
    }
    /// Stable priority: loops first, then louder (max stereo gain), then newer
    /// start boundary, then larger stable identity. Stops precede all starts.
    pub fn sync(&mut self, world: &World, listener: Option<Listener>, transport: Transport) {
        self.output.flush();
        let effective = Transport {
            playing: transport.playing && self.output.ready(),
            ..transport
        };
        if self.transport != Some(effective) {
            self.stop_all();
        }
        self.transport = Some(effective);
        if self.output.capacity() == 0 {
            self.stop_all();
            self.cache.clear();
            return;
        }
        let master = world
            .try_resource::<audio::Audio>()
            .map_or(1.0, |a| audio::gain(a.master));
        let gains = |at: &At,
                     position: Option<Vec3>,
                     gain: f32,
                     pan: f32,
                     spatial: Option<audio::Spatial>| {
            let spatial = spatial
                .or_else(|| match at {
                    At::Entity(e) => world.get::<audio::Spatial>(*e).map(|s| *s),
                    _ => None,
                })
                .unwrap_or_default();
            let gain = audio::gain(gain) * master;
            let point = match at {
                At::Ui => Some(None),
                At::Point(p) => Some(Some(*p)),
                At::Entity(e) => world
                    .global(*e)
                    .map(|t| t.translation.into())
                    .or(position)
                    .map(Some),
            };
            let (l, r) = match point {
                Some(None) => (gain, gain),
                Some(Some(p)) => {
                    listener.map_or((0.0, 0.0), |l| spatial_gains(l, p, gain, spatial))
                }
                None => (0.0, 0.0),
            };
            // Balance after spatialization: 0 leaves both channels unchanged.
            (
                audio::gain(l * (1.0 - pan).min(1.0)),
                audio::gain(r * (1.0 + pan).min(1.0)),
            )
        };
        self.refused_sources.retain(|(e, name)| {
            world
                .get::<AudioSource>(*e)
                .is_some_and(|s| s.sound == *name)
        });
        let wanted = &mut self.wanted;
        wanted.clear();
        // Synth gains are rendered into their PCM; a sample's shared PCM is scaled here.
        let authored = |d: &audio::Definition| d.sample().map_or(1.0, |s| s.gain);
        if world.has_audio() && effective.playing {
            let tick = world.tick();
            for v in &world.resource::<Voices>().voices {
                if v.began <= tick && tick < v.ends {
                    wanted.push(Wanted {
                        preferred: false,
                        key: Key::Voice(v.id),
                        definition: v.definition.clone(),
                        gains: gains(
                            &v.at,
                            v.position,
                            v.gain * authored(&v.definition) * v.fade_at(tick),
                            v.pan,
                            v.spatial,
                        ),
                        began: v.began,
                        looping: v.looping(),
                        pitch: if v.pitch.is_finite() {
                            v.pitch.clamp(0.01, 16.0)
                        } else {
                            1.0
                        },
                        start: v.offset,
                        offset: 0,
                        rate: 0,
                        frames: 0,
                        sample: None,
                    });
                }
            }
            for (e, source) in world.query::<&AudioSource>().iter() {
                if source.playing {
                    if let Some(definition) = world.resource::<Sounds>().0.get(&source.sound) {
                        if !definition.looping() {
                            if self.refused_sources.insert((e, source.sound.clone())) {
                                world.log(format!("refusal: AudioSource `{}` requires a looping definition; use World::play for finite sounds", source.sound));
                            }
                            continue;
                        }
                        wanted.push(Wanted {
                            preferred: false,
                            key: Key::Source(e),
                            definition: definition.clone(),
                            gains: gains(
                                &At::Entity(e),
                                None,
                                source.gain * authored(definition),
                                0.0,
                                None,
                            ),
                            began: 0,
                            looping: true,
                            pitch: 1.0,
                            start: 0.0,
                            offset: 0,
                            rate: 0,
                            frames: 0,
                            sample: None,
                        });
                    }
                }
            }
        }
        let rate = self.rate;
        wanted.retain_mut(|w| {
            if w.gains.0.max(w.gains.1) <= 0.0 {
                return false;
            }
            // Each source plays at its own rate; outputs resample to the device.
            (w.rate, w.frames, w.sample) = match &*w.definition {
                Sound::Synth(synth) => (rate, synth::sample_count(synth, rate), None),
                Sound::Sample(sample) => match world.sound_asset(&sample.asset) {
                    Some(asset) => (
                        asset.rate,
                        asset.frames as usize,
                        Some(Pcm::I16 {
                            samples: asset.samples.clone(),
                            channels: asset.channels,
                        }),
                    ),
                    None => return false,
                },
            };
            if w.frames == 0 {
                return false;
            }
            let elapsed = ((world.tick() - w.began) as f64 * w.rate as f64 * w.pitch as f64
                / world.hz() as f64)
                .floor()
                + (w.start as f64 * w.rate as f64).floor();
            w.offset = if w.looping {
                (elapsed % w.frames as f64) as usize
            } else {
                elapsed as usize
            };
            w.offset < w.frames
        });
        wanted.sort_unstable_by(|a, b| {
            b.looping
                .cmp(&a.looping)
                .then_with(|| {
                    b.gains
                        .0
                        .max(b.gains.1)
                        .total_cmp(&a.gains.0.max(a.gains.1))
                })
                .then_with(|| b.began.cmp(&a.began))
                .then_with(|| b.key.cmp(&a.key))
        });
        let capacity = self.output.capacity();
        let signature = |w: &Wanted| (w.definition.revision(), w.began, w.pitch.to_bits());
        // Plan priority winners with the byte bound too, so a small fallback
        // retains its identity across frames. Actual allocation below also counts
        // PCM still waiting for acknowledgement, which can temporarily refuse one.
        self.revisions.clear();
        let (mut bytes, mut count) = (0usize, 0usize);
        for w in wanted.iter_mut() {
            let revision = w.definition.revision();
            // Sampled PCM is already resident in the world; only synthesis allocates.
            let extra = if self.revisions.contains(&revision) || w.sample.is_some() {
                0
            } else {
                w.frames.saturating_mul(4)
            };
            w.preferred = count < capacity && extra <= audio::PCM_BYTE_BUDGET.saturating_sub(bytes);
            if w.preferred {
                bytes += extra;
                count += 1;
                self.revisions.push(revision);
            }
        }
        // Stop priority losers before starts, then walk past output refusals.
        self.active.retain(|key, a| {
            if wanted
                .iter()
                .any(|w| w.preferred && &w.key == key && signature(w) == a.signature)
            {
                true
            } else {
                self.output.stop(a.output_id);
                false
            }
        });
        self.pcm.clear();
        for active in self.active.values() {
            if !self.pcm.iter().any(|p| p.address() == active.pcm.address()) {
                self.pcm.push(active.pcm.clone());
            }
        }
        self.output.retain_pcm(&self.pcm);
        self.pcm.clear();
        self.cache.retain(|revision, pcm| {
            self.active.values().any(|a| a.signature.0 == *revision)
                || self.output.owns_pcm(&Pcm::F32(pcm.clone()))
        });
        let mut reserved: usize = self.cache.values().map(|pcm| pcm.len() * 4).sum();
        let mut accepted = 0;
        let mut preferred_waiting = false;
        for w in wanted.iter() {
            if preferred_waiting && !w.preferred {
                continue;
            }
            if accepted == capacity {
                break;
            }
            let revision = w.definition.revision();
            let pcm = match (&w.sample, w.definition.synth()) {
                (Some(sample), _) => sample.clone(),
                (None, Some(synth)) => {
                    if !self.cache.contains_key(&revision) {
                        let bytes = w.frames.saturating_mul(4);
                        if bytes > audio::PCM_BYTE_BUDGET.saturating_sub(reserved) {
                            // A stopped allocation is still owned by the callback. Do not
                            // restart losers from it while a winner waits for that release.
                            preferred_waiting |= w.preferred;
                            continue;
                        }
                        // Reserve before synthesis, once per shared definition allocation.
                        reserved += bytes;
                        self.cache.insert(revision, render(synth, self.rate).into());
                    }
                    Pcm::F32(self.cache[&revision].clone())
                }
                (None, None) => continue,
            };
            let active = match self.active.entry(w.key.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    if !self
                        .output
                        .start(id, &pcm, w.rate, w.looping, w.offset, w.pitch)
                    {
                        if w.sample.is_none()
                            && !self.output.owns_pcm(&pcm)
                            && !self.active.values().any(|a| a.signature.0 == revision)
                        {
                            reserved -= pcm.bytes();
                            self.cache.remove(&revision);
                        }
                        continue;
                    }
                    entry.insert(Active {
                        output_id: id,
                        signature: signature(w),
                        pcm,
                    })
                }
            };
            accepted += 1;
            self.output.set(active.output_id, w.gains.0, w.gains.1);
        }
        self.output.flush();
    }
}
impl<O: Output> Drop for Player<O> {
    fn drop(&mut self) {
        self.stop_all();
        self.output.flush();
    }
}

#[cfg(test)]
mod named_targets {
    use exact_game::{audio::Synth, Transform, World};
    #[test]
    fn named_audio_keeps_the_transform_lease_and_names_missing_targets() {
        let mut w = World::new(60, 0);
        w.sounds([("footstep", Synth::default())]);
        w.spawn_named("player", Transform::default());
        let mut pose = w.get_mut::<Transform>("player").unwrap();
        w.play("footstep").at("player").pitch(1.).start();
        pose.position.x = 1.;
        drop(pose);
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.play("footstep").at("absent").start();
        }))
        .unwrap_err();
        let message = error.downcast_ref::<String>().unwrap();
        assert!(message.contains("audio target `absent` does not exist"));
    }
}
