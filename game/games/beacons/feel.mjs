// Bench-only observer, injected into the built host by feel.mjs. No agent clock.
export function normalize(trace) {
  if (trace.stride !== 14 || trace.frames.length % 14 || trace.missing) throw new Error('Invalid exact trace');
  const frames = [];
  for (let i = 0; i < trace.frames.length; i += 14) frames.push(...trace.frames.slice(i, i + 8));
  return frames;
}

export async function install() {
  while (!globalThis.exact) await new Promise(r => setTimeout(r, 20));
  await exact.ready;
  if (exact.now || exact.agent) throw new Error('Feel requires the live host clock');
  const play = () => document.querySelector('[data-testid="play"]');
  const surface = () => document.querySelector('[data-gpu-input]');
  // Warm the real GPU module and pipelines, then fresh-boot the authored title.
  play().click();
  while (!surface() || !exact.gpu?.wantsInput(Number(surface().dataset.view))) await new Promise(r => setTimeout(r, 20));
  await new Promise(r => setTimeout(r, 1500));
  await exact.reload();
  play().focus();
  let events, stamps, eventCount = 0, active = false, overflow = false, armedTrial = -1;
  let view, capacity, tracing = false, hidden = 0, unfocused = 0;
  const codes = { Enter: 13, KeyW: 87, KeyD: 68, KeyS: 83, KeyA: 65, Space: 32 };
  function input(e) {
    if (!active || !(e.code in codes) || e.repeat) return;
    if ((eventCount + 1) * 4 > events.length) { overflow = true; return; }
    stamps[eventCount] = e.timeStamp;
    const i = eventCount++ * 4;
    events[i] = performance.now(); events[i + 1] = codes[e.code];
    events[i + 2] = +(e.type === 'keydown'); events[i + 3] = armedTrial;
    armedTrial = -1;
  }
  addEventListener('keydown', input, true);
  addEventListener('keyup', input, true);
  function observe() {
    if (!active) return;
    if (!tracing) {
      const el = surface();
      if (el) {
        view = Number(el.dataset.view);
        const reply = exact.gpu.agent(view, { op: 'state', trace: { entity: 'player', frames: capacity } });
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
      events = new Float64Array(128 * 4);
      stamps = new Float64Array(128);
      active = true;
      requestAnimationFrame(observe);
    },
    arm(trial) { armedTrial = trial; },
    end() {
      active = false;
      if (!tracing) throw new Error('Exact trace never armed');
      const { trace } = exact.gpu.agent(view, { op: 'state', trace: 'read' });
      const canvas = surface().querySelector('canvas');
      const state = exact.gpu.agent(view, { op: 'state' });
      return { schema: 1, stride: 8, frames: normalize(trace),
        events: Array.from(events.subarray(0, eventCount * 4)), overflow: overflow || trace.overflow,
        hidden_frames: hidden, unfocused_frames: unfocused,
        window_pixels: [canvas.width, canvas.height], viewport_css: [innerWidth, innerHeight],
        device_pixel_ratio: devicePixelRatio, engine_version: `exact 0.1.0 / ${state.world.hz} Hz`,
        timestamp_source: 'Frame::now_ms (rAF); window.performance.now in WorldSurface for latency',
        event_stamps_ms: Array.from(stamps.subarray(0, eventCount)),
        tick_hz: state.world.hz, interpolation: true, exact_trace: trace, exact_perf: state.world.perf };
    },
  };
}
if (typeof window !== 'undefined') install();
