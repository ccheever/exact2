//! Sound executors. Construction of Player<NullOutput> never opens an audio device.
#![deny(unsafe_code)]
mod surface;
mod synth;
pub use surface::SurfacePlayer;
pub use synth::render;

use exact_game::{
    audio::{self, At, AudioListener, AudioSource, Sounds, Voices},
    math, Quat, Vec3, World,
};
use std::{collections::BTreeMap, sync::Arc};
#[cfg(any(target_os = "macos", target_os = "ios", test))]
#[allow(unsafe_code)]
mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
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
/// One device or a test recorder. PCM is mono; set applies stereo gains.
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
    fn retain_pcm(&mut self, _pcm: &[Arc<[f32]>]) {}
    fn start(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool) {
        self.start_at(id, pcm, rate, looping, 0, 1.0);
    }
    fn start_at(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    );
    /// Accept a start, or leave it inactive so the Player retries next sync.
    fn try_start_at(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> bool {
        self.start_at(id, pcm, rate, looping, offset, pitch);
        true
    }
    fn set(&mut self, id: u64, gain_l: f32, gain_r: f32);
    fn stop(&mut self, id: u64);
}
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Start {
        id: u64,
        samples: usize,
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
    fn start_at(&mut self, _: u64, _: &Arc<[f32]>, _: u32, _: bool, _: usize, _: f32) {}
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
    fn start_at(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) {
        self.calls.push(Call::Start {
            id,
            samples: pcm.len(),
            rate,
            looping,
            offset,
            pitch,
        });
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
/// Inverse distance beyond 1m, smoothstep fade from 1m to 40m, equal-power pan.
pub fn spatial_gains(listener: Listener, point: Vec3, gain: f32) -> (f32, f32) {
    let delta = point - listener.position;
    let distance = delta.length();
    if distance >= 40.0 {
        return (0.0, 0.0);
    }
    let local = listener.rotation.conjugate() * delta;
    let pan = if distance > 0.000001 {
        (local.x / distance).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let level = gain / distance.max(1.0) * (1.0 - math::smoothstep(1.0, 40.0, distance));
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
}
struct Wanted {
    key: Key,
    synth: audio::Definition,
    gains: (f32, f32),
    began: u64,
    looping: bool,
    pitch: f32,
    offset: usize,
}
/// PCM cache and identities belong to the executor, never to the world.
pub struct Player<O: Output> {
    pub output: O,
    rate: u32,
    cache: BTreeMap<u64, Arc<[f32]>>,
    wanted: Vec<Wanted>,
    keep: Vec<u64>,
    pcm: Vec<Arc<[f32]>>,
    active: BTreeMap<Key, Active>,
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
            keep: Vec::new(),
            pcm: Vec::new(),
            active: BTreeMap::new(),
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
        let gains = |at: &At, position: Option<Vec3>, gain: f32| {
            let gain = audio::gain(gain) * master;
            let point = match at {
                At::Ui => return (audio::gain(gain), audio::gain(gain)),
                At::Point(p) => Some(*p),
                At::Entity(e) => world.global(*e).map(|t| t.translation.into()).or(position),
            };
            let (l, r) = listener
                .zip(point)
                .map(|(l, p)| spatial_gains(l, p, gain))
                .unwrap_or((0.0, 0.0));
            (audio::gain(l), audio::gain(r))
        };
        let wanted = &mut self.wanted;
        wanted.clear();
        let keep = &mut self.keep;
        keep.clear();
        if world.has_audio() {
            for synth in world.resource::<Sounds>().0.values() {
                keep.push(synth.revision());
            }
            if effective.playing {
                for v in &world.resource::<Voices>().voices {
                    if v.began <= world.tick() && world.tick() < v.ends {
                        wanted.push(Wanted {
                            key: Key::Voice(v.id),
                            synth: v.synth.clone(),
                            gains: gains(&v.at, v.position, v.gain),
                            began: v.began,
                            looping: false,
                            pitch: if v.pitch.is_finite() {
                                v.pitch.clamp(0.01, 16.0)
                            } else {
                                1.0
                            },
                            offset: 0,
                        });
                    }
                }
                for (e, source) in world.query::<&AudioSource>().iter() {
                    if source.playing {
                        if let Some(synth) = world.resource::<Sounds>().0.get(&source.sound) {
                            wanted.push(Wanted {
                                key: Key::Source(e),
                                synth: synth.clone(),
                                gains: gains(&At::Entity(e), None, source.gain),
                                began: 0,
                                looping: true,
                                pitch: 1.0,
                                offset: 0,
                            });
                        }
                    }
                }
            }
        }
        wanted.retain_mut(|w| {
            if w.gains.0.max(w.gains.1) <= 0.0 {
                return false;
            }
            let len = synth::sample_count(&w.synth, self.rate);
            if len == 0 {
                return false;
            }
            let elapsed = ((world.tick() - w.began) as f64 * self.rate as f64 * w.pitch as f64
                / world.hz() as f64)
                .floor();
            w.offset = if w.looping {
                (elapsed % len as f64) as usize
            } else {
                elapsed as usize
            };
            w.offset < len
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
        wanted.truncate(self.output.capacity());
        wanted.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        let signature = |w: &Wanted| (w.synth.revision(), w.began, w.pitch.to_bits());
        self.active.retain(|key, a| {
            if wanted
                .binary_search_by(|w| w.key.cmp(key))
                .is_ok_and(|i| signature(&wanted[i]) == a.signature)
            {
                true
            } else {
                self.output.stop(a.output_id);
                false
            }
        });
        for w in wanted.iter() {
            let revision = w.synth.revision();
            let pcm = self
                .cache
                .entry(revision)
                .or_insert_with(|| render(&w.synth, self.rate).into());
            keep.push(revision);
            let active = match self.active.entry(w.key.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    if !self
                        .output
                        .try_start_at(id, pcm, self.rate, w.looping, w.offset, w.pitch)
                    {
                        continue;
                    }
                    entry.insert(Active {
                        output_id: id,
                        signature: signature(w),
                    })
                }
            };
            self.output.set(active.output_id, w.gains.0, w.gains.1);
        }
        keep.sort_unstable();
        keep.dedup();
        self.cache.retain(|key, _| keep.binary_search(key).is_ok());
        self.pcm.clear();
        self.pcm.extend(self.cache.values().cloned());
        self.output.retain_pcm(&self.pcm);
        self.output.flush();
    }
}
impl<O: Output> Drop for Player<O> {
    fn drop(&mut self) {
        self.stop_all();
        self.output.flush();
    }
}
