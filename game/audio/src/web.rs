use crate::{Output, Pcm};
use std::{cell::Cell, collections::BTreeMap, rc::Rc};
use wasm_bindgen::JsValue;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioNode, ChannelMergerNode, GainNode,
};
type CachedBuffer = (Pcm, AudioBuffer);

/// source → (splitter, for stereo) → left and right gains → merger → destination.
/// Mono feeds both gains; stereo feeds each gain its own channel, as on Apple.
/// @ref LLP 1046.003 §AU4 (outputs)
struct Voice {
    source: AudioBufferSourceNode,
    nodes: Vec<AudioNode>,
    left: GainNode,
    right: GainNode,
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
    fn buffer(&self, pcm: &Pcm, rate: u32) -> Result<AudioBuffer, JsValue> {
        let (channels, frames) = (pcm.channels(), pcm.frames());
        let buffer = self
            .context
            .create_buffer(channels as u32, frames as u32, rate as f32)?;
        match pcm {
            Pcm::F32(samples) => buffer.copy_to_channel(samples, 0)?,
            Pcm::I16 { .. } => {
                let mut plane = vec![0.0f32; frames];
                for channel in 0..channels {
                    for (frame, value) in plane.iter_mut().enumerate() {
                        *value = pcm.sample(frame, channel);
                    }
                    buffer.copy_to_channel(&plane, channel as i32)?;
                }
            }
        }
        Ok(buffer)
    }
    fn begin(
        &mut self,
        id: u64,
        pcm: &Pcm,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> Result<(), JsValue> {
        if !self.ready() {
            return Err(JsValue::from_str("WebAudio is not ready"));
        }
        self.stop(id);
        let key = (pcm.address(), rate);
        let buffer = match self.buffers.get(&key) {
            Some((_, buffer)) => buffer.clone(),
            None => self.buffer(pcm, rate)?,
        };
        let source = self.context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        source.set_loop(looping);
        source.playback_rate().set_value(pitch);
        let (left, right) = (self.context.create_gain()?, self.context.create_gain()?);
        left.gain().set_value(0.0);
        right.gain().set_value(0.0);
        let merger: ChannelMergerNode = self
            .context
            .create_channel_merger_with_number_of_inputs(2)?;
        let mut nodes: Vec<AudioNode> = vec![
            left.clone().into(),
            right.clone().into(),
            merger.clone().into(),
        ];
        let start = (|| {
            if pcm.channels() == 2 {
                let splitter = self
                    .context
                    .create_channel_splitter_with_number_of_outputs(2)?;
                source.connect_with_audio_node(&splitter)?;
                splitter.connect_with_audio_node_and_output(&left, 0)?;
                splitter.connect_with_audio_node_and_output(&right, 1)?;
                nodes.push(splitter.into());
            } else {
                source.connect_with_audio_node(&left)?;
                source.connect_with_audio_node(&right)?;
            }
            left.connect_with_audio_node_and_output_and_input(&merger, 0, 0)?;
            right.connect_with_audio_node_and_output_and_input(&merger, 0, 1)?;
            merger.connect_with_audio_node(&self.context.destination())?;
            source.start_with_when_and_grain_offset(0.0, offset as f64 / rate as f64)
        })();
        if let Err(error) = start {
            let _ = source.disconnect();
            for node in &nodes {
                let _ = node.disconnect();
            }
            return Err(error);
        }
        self.buffers
            .entry(key)
            .or_insert_with(|| (pcm.clone(), buffer));
        self.voices.insert(
            id,
            Voice {
                source,
                nodes,
                left,
                right,
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
    fn owns_pcm(&self, pcm: &Pcm) -> bool {
        self.voices.values().any(|v| v.buffer.0 == pcm.address())
    }
    fn retain_pcm(&mut self, pcm: &[Pcm]) {
        self.buffers.retain(|key, _| {
            pcm.iter().any(|p| p.address() == key.0)
                || self.voices.values().any(|v| v.buffer == *key)
        });
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
        self.begin(id, pcm, rate, looping, offset, pitch).is_ok()
    }
    fn set(&mut self, id: u64, left: f32, right: f32) {
        if let Some(v) = self.voices.get(&id) {
            let now = self.context.current_time();
            let _ = v.left.gain().set_target_at_time(left, now, 0.01);
            let _ = v.right.gain().set_target_at_time(right, now, 0.01);
        }
    }
    fn stop(&mut self, id: u64) {
        if let Some(v) = self.voices.remove(&id) {
            let scheduled: &web_sys::AudioScheduledSourceNode = v.source.as_ref();
            let _ = scheduled.stop();
            let _ = v.source.disconnect();
            for node in &v.nodes {
                let _ = node.disconnect();
            }
        }
    }
}
impl Drop for WebOutput {
    fn drop(&mut self) {
        let _ = self.context.close();
    }
}
