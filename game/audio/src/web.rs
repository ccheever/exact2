use crate::Output;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};
use wasm_bindgen::JsValue;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode, StereoPannerNode};
type CachedBuffer = (Arc<[f32]>, AudioBuffer);

struct Voice {
    source: AudioBufferSourceNode,
    gain: GainNode,
    pan: StereoPannerNode,
    buffer: (usize, u32),
}
/// One suspended context. Construct only for a human-owned surface, never an agent.
pub struct WebOutput {
    context: AudioContext,
    unlocked: Rc<Cell<bool>>,
    unlocking: Rc<Cell<bool>>,
    activation: Rc<Cell<u64>>,
    suspended: Rc<Cell<bool>>,
    // Retain PCM ownership: allocator address reuse cannot alias a cached buffer.
    buffers: BTreeMap<(usize, u32), CachedBuffer>,
    voices: BTreeMap<u64, Voice>,
}
impl WebOutput {
    pub fn new() -> Result<Self, JsValue> {
        let context = AudioContext::new()?;
        let _ = context.suspend()?;
        Ok(Self {
            context,
            unlocked: Rc::new(Cell::new(false)),
            unlocking: Rc::new(Cell::new(false)),
            activation: Rc::new(Cell::new(0)),
            suspended: Rc::new(Cell::new(false)),
            buffers: BTreeMap::new(),
            voices: BTreeMap::new(),
        })
    }
    /// Stop device time while hidden or interrupted.
    pub fn suspend(&mut self) -> Result<(), JsValue> {
        self.suspended.set(true);
        self.activation.set(self.activation.get().wrapping_add(1));
        self.unlocking.set(false);
        self.unlocked.set(false);
        self.context.suspend().map(|_| ())
    }
    /// Resume device time; SurfacePlayer observes readiness and bumps the epoch.
    pub fn resume(&mut self) -> Result<(), JsValue> {
        Output::unlock(self);
        Ok(())
    }
    fn begin(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> Result<(), JsValue> {
        if !self.ready() {
            return Err(JsValue::from_str("WebAudio is not ready"));
        }
        self.stop(id);
        let key = (pcm.as_ptr() as usize, rate);
        let buffer = if let Some((_, buffer)) = self.buffers.get(&key) {
            buffer.clone()
        } else {
            let buffer = self
                .context
                .create_buffer(1, pcm.len() as u32, rate as f32)?;
            buffer.copy_to_channel(pcm, 0)?;
            buffer
        };
        let source = self.context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        source.set_loop(looping);
        source.playback_rate().set_value(pitch);
        let gain = self.context.create_gain()?;
        gain.gain().set_value(0.0);
        let pan = self.context.create_stereo_panner()?;
        let start = (|| {
            source.connect_with_audio_node(&gain)?;
            gain.connect_with_audio_node(&pan)?;
            pan.connect_with_audio_node(&self.context.destination())?;
            source.start_with_when_and_grain_offset(0.0, offset as f64 / rate as f64)
        })();
        if let Err(error) = start {
            let _ = source.disconnect();
            let _ = gain.disconnect();
            let _ = pan.disconnect();
            return Err(error);
        }
        self.buffers
            .entry(key)
            .or_insert_with(|| (pcm.clone(), buffer));
        self.voices.insert(
            id,
            Voice {
                source,
                gain,
                pan,
                buffer: key,
            },
        );
        Ok(())
    }
}
impl Output for WebOutput {
    fn capacity(&self) -> usize {
        32
    }
    fn unlock(&mut self) {
        if self.ready() || self.unlocking.replace(true) {
            return;
        }
        self.suspended.set(false);
        let Ok(promise) = self.context.resume() else {
            self.unlocking.set(false);
            return;
        };
        let context = self.context.clone();
        let suspended = self.suspended.clone();
        let unlocked = self.unlocked.clone();
        let unlocking = self.unlocking.clone();
        let activation = self.activation.clone();
        let generation = activation.get().wrapping_add(1);
        activation.set(generation);
        wasm_bindgen_futures::spawn_local(async move {
            let ready = wasm_bindgen_futures::JsFuture::from(promise).await.is_ok();
            if suspended.get() {
                let _ = context.suspend(); // a late resume must not restart hidden device work
            }
            if activation.get() == generation {
                unlocked.set(ready);
                unlocking.set(false);
            }
        });
    }
    fn ready(&self) -> bool {
        self.unlocked.get() && self.context.state() == web_sys::AudioContextState::Running
    }
    fn owns_pcm(&self, pcm: &Arc<[f32]>) -> bool {
        self.voices
            .values()
            .any(|v| v.buffer.0 == pcm.as_ptr() as usize)
    }
    fn retain_pcm(&mut self, pcm: &[Arc<[f32]>]) {
        self.buffers.retain(|key, _| {
            pcm.iter().any(|p| p.as_ptr() as usize == key.0)
                || self.voices.values().any(|v| v.buffer == *key)
        });
    }
    fn start(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> bool {
        self.begin(id, pcm, rate, looping, offset, pitch).is_ok()
    }
    fn set(&mut self, id: u64, left: f32, right: f32) {
        if let Some(v) = self.voices.get(&id) {
            let amplitude = exact_game::math::sqrt(left * left + right * right);
            let pan = if amplitude > 0.0 {
                4.0 * exact_game::math::atan2(right, left) / std::f32::consts::PI - 1.0
            } else {
                0.0
            };
            let now = self.context.current_time();
            let _ = v.gain.gain().set_target_at_time(amplitude, now, 0.01);
            let _ = v
                .pan
                .pan()
                .set_target_at_time(pan.clamp(-1.0, 1.0), now, 0.01);
        }
    }
    fn stop(&mut self, id: u64) {
        if let Some(v) = self.voices.remove(&id) {
            let scheduled: &web_sys::AudioScheduledSourceNode = v.source.as_ref();
            let _ = scheduled.stop();
            let _ = v.source.disconnect();
            let _ = v.gain.disconnect();
            let _ = v.pan.disconnect();
        }
    }
}
impl Drop for WebOutput {
    fn drop(&mut self) {
        let _ = self.context.close();
    }
}
