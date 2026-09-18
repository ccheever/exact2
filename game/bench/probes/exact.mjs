// Bench-only observer, injected into the built host by feel.mjs. No agent clock.
import { inputRecorder } from './input.mjs';
export function normalize(trace) {
  if (![14, 32].includes(trace.stride) || trace.frames.length % trace.stride || trace.missing) throw new Error('Invalid exact trace');
  const frames = [];
  for (let i = 0; i < trace.frames.length; i += trace.stride) frames.push(...trace.frames.slice(i, i + 8));
  return frames;
}

// Mean alpha when ticks run; near 1 means just before the pose is needed.
export function tickPhase(trace) {
  let sum = 0, count = 0;
  for (let i = 0; i < trace.frames.length; i += trace.stride) {
    if (trace.frames[i + 9] > 0) { sum += trace.frames[i + 8]; count++; }
  }
  return count ? sum / count : null;
}

// Capture the actual rAF argument without changing the clock passed to any callback.
// Join after capture by draw-boundary wall time. Resize draws inherit the latest
// callback stamp and are dropped as duplicates by the analyzer.
export function callbackRecorder(target = globalThis, now = () => performance.now()) {
  const request = target.requestAnimationFrame.bind(target);
  let stamps, walls, count = 0, active = false, overflow = false;
  const wrappers = new WeakMap();
  target.requestAnimationFrame = callback => {
    let wrapped = wrappers.get(callback);
    if (!wrapped) {
      wrapped = stamp => {
        if (active) {
          if (count === stamps.length) overflow = true;
          else { stamps[count] = stamp; walls[count++] = now(); }
        }
        callback(stamp);
      };
      wrappers.set(callback, wrapped);
    }
    return request(wrapped);
  };
  return {
    begin(capacity) { stamps = new Float64Array(capacity * 8); walls = new Float64Array(capacity * 8); count = 0; overflow = false; active = true; },
    end(frames) {
      active = false;
      let at = 0;
      const raw = [];
      for (let i = 0; i < frames.length; i += 8) {
        while (at + 1 < count && walls[at + 1] <= frames[i + 1]) at++;
        if (!count || walls[at] > frames[i + 1]) throw new Error('Exact draw has no recorded raw callback');
        raw.push(stamps[at]);
      }
      return { raw_callback_ms: raw, overflow };
    },
  };
}

let installing;
export function install(options = {}) {
  return installing ??= installOnce(options);
}
async function installOnce({ entity = 'player', play: selector = '[data-testid="play"]' } = {}) {
  while (!globalThis.exact) await new Promise(r => setTimeout(r, 20));
  await exact.ready;
  if (exact.now || exact.agent) throw new Error('Feel requires the live host clock');
  const callbacks = callbackRecorder();
  const play = () => document.querySelector(selector);
  const surface = () => document.querySelector('[data-gpu-input]');
  // Warm the real GPU module and pipelines, then fresh-boot the authored title.
  play().click();
  while (!surface() || !exact.gpu?.wantsInput(Number(surface().dataset.view))) await new Promise(r => setTimeout(r, 20));
  await new Promise(r => setTimeout(r, 1500));
  await exact.reload();
  play().focus();
  const inputs = inputRecorder();
  let active = false, view, capacity, tracing = false, hidden = 0, unfocused = 0;
  function observe() {
    if (!active) return;
    if (!tracing) {
      const el = surface();
      if (el) {
        view = Number(el.dataset.view);
        exact.gpu.agent(view, { op: 'state', perf: true });
        const reply = exact.gpu.agent(view, { op: 'state', trace: { entity, frames: capacity } });
        if (reply?.trace !== 'armed') throw new Error(JSON.stringify(reply));
        tracing = true;
      }
    }
    hidden += document.visibilityState !== 'visible'; unfocused += !document.hasFocus();
    requestAnimationFrame(observe);
  }
  window.feel = {
    begin(durationMs) {
      capacity = Math.ceil((durationMs / 1000 + 10) * 1000);
      inputs.begin(); callbacks.begin(capacity);
      tracing = false; hidden = 0; unfocused = 0;
      active = true;
      requestAnimationFrame(observe);
    },
    arm(trial) { inputs.arm(trial); },
    end() {
      active = false;
      if (!tracing) throw new Error('Exact trace never armed');
      const delivered = inputs.end();
      const { trace } = exact.gpu.agent(view, { op: 'state', trace: 'read' });
      const canvas = surface().querySelector('canvas');
      const state = exact.gpu.agent(view, { op: 'state' });
      const frames = normalize(trace), callback = callbacks.end(frames);
      const camera_projection = trace.stride === 32 ? [] : null;
      if (camera_projection) for (let i = 0; i < trace.frames.length; i += trace.stride) camera_projection.push(...trace.frames.slice(i + 14, i + 32));
      return { schema: 1, stride: 8, frames, raw_callback_ms: callback.raw_callback_ms,
        drawn_clock_ms: frames.filter((_, i) => i % 8 === 0), camera_projection,
        ...delivered, overflow: delivered.overflow || trace.overflow || callback.overflow,
        hidden_frames: hidden, unfocused_frames: unfocused,
        window_pixels: [canvas.width, canvas.height], viewport_css: [innerWidth, innerHeight],
        device_pixel_ratio: devicePixelRatio, engine_version: `exact 0.1.0 / ${state.world.hz} Hz`,
        timestamp_source: 'raw_callback_ms: rAF argument; drawn_clock_ms: paced Frame::now_ms; frame slot 1: WorldSurface performance.now latency endpoint',
        tick_phase: tickPhase(trace), tick_hz: state.world.hz, interpolation: true, exact_trace: trace, exact_perf: state.world.perf };
    },
  };
}
