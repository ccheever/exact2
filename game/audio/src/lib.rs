//! Sound executors. Construction of Player<NullOutput> never opens an audio device.
#![deny(unsafe_code)]
use exact_game::{
    audio::{self, At, AudioListener, AudioSource, Sounds, Voices},
    hash, math, Quat, Vec3, World,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[cfg(any(target_os = "macos", target_os = "ios", test))]
#[allow(unsafe_code)]
mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use apple::AppleOutput;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebOutput;

/// One device or a call recorder. PCM is mono; set applies stereo gains.
pub trait Output {
    fn start(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool);
    fn set(&mut self, id: u64, gain_l: f32, gain_r: f32);
    fn stop(&mut self, id: u64);
    /// Sample-accurate restore. Devices override this to retain their PCM cache.
    fn start_at(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool, offset: usize) {
        let samples: Arc<[f32]> = if looping && !pcm.is_empty() {
            let offset = offset % pcm.len();
            pcm[offset..]
                .iter()
                .chain(&pcm[..offset])
                .copied()
                .collect::<Vec<_>>()
                .into()
        } else {
            pcm[offset.min(pcm.len())..].into()
        };
        self.start(id, &samples, rate, looping);
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Start {
        id: u64,
        samples: usize,
        rate: u32,
        looping: bool,
        offset: usize,
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
/// The only output used by agent sessions and by default on Linux.
#[derive(Default)]
pub struct NullOutput {
    pub calls: Vec<Call>,
}
impl Output for NullOutput {
    fn start(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool) {
        self.start_at(id, pcm, rate, looping, 0);
    }
    fn start_at(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool, offset: usize) {
        self.calls.push(Call::Start {
            id,
            samples: pcm.len(),
            rate,
            looping,
            offset,
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
    signature: u64,
}
struct Wanted {
    key: Key,
    sound: String,
    at: At,
    gain: f32,
    began: u64,
    looping: bool,
}
/// PCM cache and identities belong to the executor, never to the world.
pub struct Player<O: Output> {
    pub output: O,
    rate: u32,
    cache: BTreeMap<(String, u64), Arc<[f32]>>,
    active: BTreeMap<Key, Active>,
    next_id: u64,
    last_tick: Option<u64>,
}
impl<O: Output> Player<O> {
    pub fn new(output: O, rate: u32) -> Self {
        assert!(rate > 0);
        Self {
            output,
            rate,
            cache: BTreeMap::new(),
            active: BTreeMap::new(),
            next_id: 0,
            last_tick: None,
        }
    }
    pub fn cached_sounds(&self) -> usize {
        self.cache.len()
    }
    /// Stop all outputs; call on explicit seeks/restores when reusing a Player.
    /// A fresh Player automatically starts each saved voice at its saved offset.
    pub fn reset(&mut self) {
        for active in self.active.values() {
            self.output.stop(active.output_id);
        }
        self.active.clear();
        self.last_tick = None;
    }
    /// Execute one frame's state without mutating it. None means no spatial ears.
    pub fn sync(&mut self, world: &World, listener: Option<Listener>) {
        if self.last_tick.is_some_and(|tick| tick > world.tick()) {
            self.reset();
        }
        self.last_tick = Some(world.tick());
        let mut wanted = Vec::new();
        if world.has_audio() {
            for voice in &world.resource::<Voices>().voices {
                if voice.began <= world.tick() && world.tick() < voice.ends {
                    wanted.push(Wanted {
                        key: Key::Voice(voice.id),
                        sound: voice.sound.clone(),
                        at: voice.at.clone(),
                        gain: voice.gain,
                        began: voice.began,
                        looping: false,
                    });
                }
            }
        }
        for (e, source) in world.query::<&AudioSource>().iter() {
            if source.playing {
                wanted.push(Wanted {
                    key: Key::Source(e),
                    sound: source.sound.clone(),
                    at: At::Entity(e),
                    gain: source.gain,
                    began: 0,
                    looping: true,
                });
            }
        }
        let mut retained = BTreeSet::new();
        for w in wanted {
            assert!(world.has_audio(), "unknown sound `{}`", w.sound);
            let sounds = world.resource::<Sounds>();
            let synth = sounds
                .0
                .get(&w.sound)
                .unwrap_or_else(|| panic!("unknown sound `{}`", w.sound));
            let digest = hash::of(synth);
            let cache_key = (w.sound.clone(), digest);
            let pcm = self
                .cache
                .entry(cache_key)
                .or_insert_with(|| audio::render(synth, self.rate).into());
            if pcm.is_empty() {
                continue;
            }
            let elapsed = (world.tick() - w.began) as u128 * self.rate as u128 / world.hz() as u128;
            let offset = if w.looping {
                (elapsed % pcm.len() as u128) as usize
            } else {
                elapsed.min(usize::MAX as u128) as usize
            };
            if offset >= pcm.len() {
                continue;
            }
            let signature = hash::of(&(w.sound.clone(), digest, w.began));
            if self
                .active
                .get(&w.key)
                .is_some_and(|a| a.signature != signature)
            {
                self.output
                    .stop(self.active.remove(&w.key).unwrap().output_id);
            }
            let active = self.active.entry(w.key.clone()).or_insert_with(|| {
                let id = self.next_id;
                self.next_id += 1;
                self.output.start_at(id, pcm, self.rate, w.looping, offset);
                Active {
                    output_id: id,
                    signature,
                }
            });
            let gains = match w.at {
                At::Ui => (w.gain, w.gain),
                At::Point(point) => listener
                    .map(|l| spatial_gains(l, point, w.gain))
                    .unwrap_or((0.0, 0.0)),
                At::Entity(e) => listener
                    .zip(world.global(e))
                    .map(|(l, t)| spatial_gains(l, t.translation.into(), w.gain))
                    .unwrap_or((0.0, 0.0)),
            };
            self.output.set(active.output_id, gains.0, gains.1);
            retained.insert(w.key);
        }
        self.active.retain(|key, active| {
            if retained.contains(key) {
                true
            } else {
                self.output.stop(active.output_id);
                false
            }
        });
    }
}
impl<O: Output> Drop for Player<O> {
    fn drop(&mut self) {
        self.reset();
    }
}
