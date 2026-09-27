// The recorder on the web (LLP 1067.000): one object, the microphone, shared
// by `<waveform-view>` and `later`. The view draws the recorder's own levels
// every frame; the source only starts, stops and lists takes. One page is one
// session, so the module's top level is the session's instance.
//
// Under the agent (`connect`'s `agent`) the microphone is substituted before
// the browser is asked (LLP 1067.000 Q7): a synthetic level that follows the
// agent's clock, so a screenshot after `clock +N` is the same every run, and
// takes stay in memory. Otherwise takes are kept in the origin private file
// system, as `recorder/<name>.webm` beside `recorder/takes.json`.
export const abi = 1;
export const roster = { 'waveform-view': { snapshot: false } };

const RATE = 20; // levels a second
const KEEP = RATE * 60 * 10; // ten minutes of levels

const context = {
  agent: typeof location !== 'undefined' && new URL(location.href).searchParams.has('agent'),
  now: () => performance.now(),
  changed: () => {},
};

// A level for step `i` of the synthetic input: a voice-like swell, the same
// on every run.
const synthetic = (i) => Math.min(1, 0.12 + 0.75 * Math.abs(Math.sin(i * 0.37)) * (0.55 + 0.45 * Math.sin(i * 0.05)));

const recorder = {
  recording: false,
  started: 0,
  levels: [], // real input: one level every 1/RATE s while recording
  last: [], // the finished take's levels, shown when idle
  stream: null,
  audio: null,
  media: null,
  chunks: [],
  timer: null,
  count: 0,
  memory: [], // the agent's takes

  elapsed() { return this.recording ? Math.max(0, context.now() - this.started) : 0; },

  // The levels to draw: the live take's while recording, else the last one's.
  visible() {
    if (!this.recording) return this.last;
    if (!context.agent) return this.levels;
    const steps = Math.floor(this.elapsed() * RATE / 1000);
    const out = [];
    for (let i = Math.max(0, steps - KEEP); i < steps; i++) out.push(synthetic(i));
    return out;
  },

  status(message) {
    const text = message ?? (this.recording ? 'Recording…' : context.agent ? 'Ready (agent input)' : 'Ready');
    return { available: true, recording: this.recording, message: text };
  },

  async start() {
    if (this.recording) return this.status();
    if (!context.agent) {
      if (!navigator.mediaDevices?.getUserMedia) throw new Error('this browser has no microphone access');
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      this.audio = new AudioContext();
      const analyser = this.audio.createAnalyser();
      analyser.fftSize = 2048;
      this.audio.createMediaStreamSource(this.stream).connect(analyser);
      const samples = new Float32Array(analyser.fftSize);
      this.levels = [];
      this.timer = setInterval(() => {
        analyser.getFloatTimeDomainData(samples);
        let peak = 0;
        for (const s of samples) peak = Math.max(peak, Math.abs(s));
        this.levels.push(Math.min(1, peak * 1.4));
        if (this.levels.length > KEEP) this.levels.splice(0, this.levels.length - KEEP);
      }, 1000 / RATE);
      this.chunks = [];
      this.media = new MediaRecorder(this.stream);
      this.media.ondataavailable = (e) => { if (e.data.size) this.chunks.push(e.data); };
      this.media.start();
    }
    this.started = context.now();
    this.recording = true;
    context.changed('status');
    return this.status();
  },

  async stop() {
    if (!this.recording) throw new Error('not recording');
    const ms = this.elapsed();
    this.last = this.visible().slice();
    this.recording = false;
    let blob = null;
    if (!context.agent) {
      clearInterval(this.timer);
      blob = await new Promise((resolve) => {
        this.media.onstop = () => resolve(new Blob(this.chunks, { type: this.media.mimeType }));
        this.media.stop();
      });
      for (const track of this.stream.getTracks()) track.stop();
      await this.audio.close();
      this.stream = this.audio = this.media = null;
    }
    const takes = await this.list();
    const name = `take-${takes.length + 1}`;
    const take = { name, seconds: Math.round(ms / 100) / 10, level: average(this.last) };
    await this.save(take, blob, [...takes, take]);
    context.changed('status');
    context.changed('takes');
    return { name, message: `Saved ${label(take)}` };
  },

  async folder() {
    const root = await navigator.storage.getDirectory();
    return root.getDirectoryHandle('recorder', { create: true });
  },

  async list() {
    if (context.agent) return this.memory;
    try {
      const file = await (await (await this.folder()).getFileHandle('takes.json')).getFile();
      return JSON.parse(await file.text());
    } catch { return []; }
  },

  async save(take, blob, takes) {
    if (context.agent) { this.memory = takes; return; }
    const folder = await this.folder();
    const write = async (name, data) => {
      const w = await (await folder.getFileHandle(name, { create: true })).createWritable();
      await w.write(data);
      await w.close();
    };
    await write(`${take.name}.webm`, blob);
    await write('takes.json', JSON.stringify(takes));
  },
};

// A take's average level, in percent: what it shows, and what the smoke holds
// equal across hosts, so this module and the Swift one cannot drift apart.
const average = (levels) => levels.length ? Math.round(100 * levels.reduce((a, b) => a + b, 0) / levels.length) : 0;
const label = (take) => `${take.name.replace('take-', 'Take ')} · ${take.seconds.toFixed(1)} s · level ${take.level ?? 0}%`;

export function connect({ changed, agent, now }) {
  context.changed = changed;
  if (typeof agent === 'boolean') context.agent = agent;
  if (typeof now === 'function') context.now = now;
}

export async function later(request) {
  switch (request?.op) {
    case 'status': return recorder.status();
    case 'start': return recorder.start();
    case 'stop': return recorder.stop();
    case 'takes': return { takes: (await recorder.list()).map((t) => ({ name: t.name, label: label(t) })) };
    default: throw new Error(`the recorder answers no ${JSON.stringify(request?.op)}`);
  }
}

// `<waveform-view>`: a canvas that draws the recorder's levels, newest at the
// right, and the elapsed time. It reads the object directly, every frame.
const STYLE = ':host { display: block; position: relative; background: #0b0f13; border-radius: 12px; overflow: hidden; } canvas { position: absolute; inset: 0; width: 100%; height: 100%; }';

class Waveform {
  constructor(element) {
    this.root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
    this.root.innerHTML = `<style>${STYLE}</style><canvas role="img" aria-label="Waveform"></canvas>`;
    this.canvas = this.root.querySelector('canvas');
    this.frame = requestAnimationFrame(() => this.draw());
  }

  draw() {
    this.frame = requestAnimationFrame(() => this.draw());
    const c = this.canvas, dpr = devicePixelRatio || 1;
    const w = Math.round(c.clientWidth * dpr), h = Math.round(c.clientHeight * dpr);
    if (!w || !h) return;
    if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
    const g = c.getContext('2d');
    g.clearRect(0, 0, w, h);
    const levels = recorder.visible();
    const bar = 3 * dpr, gap = 2 * dpr, mid = h / 2;
    const fit = Math.floor((w - 16 * dpr) / (bar + gap));
    const shown = levels.slice(-fit);
    g.fillStyle = recorder.recording ? '#e5484d' : '#2f81f7';
    shown.forEach((level, i) => {
      const x = w - 8 * dpr - (shown.length - i) * (bar + gap);
      const half = Math.max(1 * dpr, level * (mid - 12 * dpr));
      g.fillRect(x, mid - half, bar, half * 2);
    });
    if (!shown.length) { g.fillStyle = '#2a333d'; g.fillRect(8 * dpr, mid - dpr / 2, w - 16 * dpr, dpr); }
    const s = recorder.elapsed() / 1000;
    g.fillStyle = '#e8edf2';
    g.font = `${13 * dpr}px system-ui`;
    g.fillText(recorder.recording ? `● ${Math.floor(s / 60)}:${(s % 60).toFixed(1).padStart(4, '0')}` : '', 10 * dpr, 20 * dpr);
  }

  destroy() { cancelAnimationFrame(this.frame); }
}

export function create(tag, element) {
  if (tag !== 'waveform-view') throw new Error(`no view ${tag}`);
  return new Waveform(element);
}
export function setProps() {}
export function destroy(handle) { handle.destroy(); }
