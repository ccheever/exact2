use crate::Output;
use std::{collections::BTreeMap, sync::Arc};
use wasm_bindgen::JsValue;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode, StereoPannerNode};
type CachedBuffer = (Arc<[f32]>, AudioBuffer);

struct Voice {
    source: AudioBufferSourceNode,
    gain: GainNode,
    pan: StereoPannerNode,
}
/// One suspended context. Construct only for a human-owned surface, never an agent.
pub struct WebOutput {
    context: AudioContext,
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
            buffers: BTreeMap::new(),
            voices: BTreeMap::new(),
        })
    }
    /// Invoke directly from the first trusted input event.
    pub fn unlock(&self) -> Result<(), JsValue> {
        let _ = self.context.resume()?;
        Ok(())
    }
    fn begin(
        &mut self,
        id: u64,
        pcm: &Arc<[f32]>,
        rate: u32,
        looping: bool,
        offset: usize,
    ) -> Result<(), JsValue> {
        self.stop(id);
        let key = (pcm.as_ptr() as usize, rate);
        if !self.buffers.contains_key(&key) {
            let buffer = self
                .context
                .create_buffer(1, pcm.len() as u32, rate as f32)?;
            buffer.copy_to_channel(pcm, 0)?;
            self.buffers.insert(key, (pcm.clone(), buffer));
        }
        let source = self.context.create_buffer_source()?;
        source.set_buffer(Some(&self.buffers[&key].1));
        source.set_loop(looping);
        let gain = self.context.create_gain()?;
        gain.gain().set_value(0.0);
        let pan = self.context.create_stereo_panner()?;
        source.connect_with_audio_node(&gain)?;
        gain.connect_with_audio_node(&pan)?;
        pan.connect_with_audio_node(&self.context.destination())?;
        source.start_with_when_and_grain_offset(0.0, offset as f64 / rate as f64)?;
        self.voices.insert(id, Voice { source, gain, pan });
        Ok(())
    }
}
impl Output for WebOutput {
    fn start(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool) {
        self.start_at(id, pcm, rate, looping, 0);
    }
    fn start_at(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool, offset: usize) {
        self.begin(id, pcm, rate, looping, offset)
            .expect("WebAudio start failed");
    }
    fn set(&mut self, id: u64, left: f32, right: f32) {
        if let Some(v) = self.voices.get(&id) {
            let amplitude = exact_game::math::sqrt(left * left + right * right);
            let pan = if amplitude > 0.0 {
                4.0 * exact_game::math::atan2(right, left) / std::f32::consts::PI - 1.0
            } else {
                0.0
            };
            v.gain.gain().set_value(amplitude);
            v.pan.pan().set_value(pan.clamp(-1.0, 1.0));
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
