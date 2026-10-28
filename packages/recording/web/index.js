// @exact/recording on the web (LLP 1133.009 D1, D6, §10): the browser's
// `MediaRecorder` over a track `@exact/media-capture` opened, reached through
// the session's track table, and a level computed from an `AnalyserNode`.
//
// - A recorder is a consumer of its stream's audio track, so it holds the
//   track's lease while it is not `inactive`; its start is a start (D6), which
//   reopens an idle track. A level reader is a consumer too, and never a start.
// - A recording is a file at the admitted `app:/` path, in the store
//   `storage.fs` reads (`context.files`): each slice the browser delivers,
//   asked every second, is appended as it arrives, and at `pagehide` the rest
//   is asked for (D6, "the web promises less").
// - A recorder's state is a value, sticky once `inactive` (D1); an
//   interruption pauses it (D8), and a track that ends stops it.
// - `audioLevel` (D1): `audioLevel` is the RMS of the last `interval`, linear
//   from 0 to 1; `totalAudioEnergy` and `totalSamplesDuration` integrate it;
//   `levels` holds the last `history` intervals, one entry per interval since
//   the last answer, however many polls were skipped.
// - `<audio-waveform>` (D13) draws a track's level each animation frame from
//   a meter of its own, so no level crosses into Contract.
// - Under the agent (§10) no browser API is used: the level is the synthetic
//   signal on the session clock, and a stopped recorder's file is WAV, 16-bit
//   PCM, of a tone that follows the signal, answered as `audio/wav`.

/** The synthetic signal at step `i` (one step a 50 ms of the session clock): the recorder's formula (§10). */
export const synthetic = i => Math.min(1, 0.12 + 0.75 * Math.abs(Math.sin(i * 0.37)) * (0.55 + 0.45 * Math.sin(i * 0.05)));
const STEP = 50;
// What a read of no handle (`""`) answers: an app reads before it holds one, and a poll does not ask it again.
const NO_RECORDER = { id: '', state: 'inactive', mimeType: '', audioBitsPerSecond: null, path: '', duration: 0, reason: null, error: null, result: null };
const SILENCE = { audioLevel: 0, totalAudioEnergy: 0, totalSamplesDuration: 0, levels: [] };
const EXTENSIONS = { 'audio/webm': '.webm', 'audio/ogg': '.ogg', 'audio/mp4': '.m4a', 'audio/wav': '.wav' };

/** A WAV file, 16-bit mono PCM at `rate`, of `samples` (-1 to 1). */
export function wav(samples, rate = 8000) {
  const bytes = new ArrayBuffer(44 + samples.length * 2), v = new DataView(bytes);
  const text = (at, s) => { for (let i = 0; i < s.length; i++) v.setUint8(at + i, s.charCodeAt(i)); };
  text(0, 'RIFF'); v.setUint32(4, 36 + samples.length * 2, true); text(8, 'WAVE');
  text(12, 'fmt '); v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, rate, true); v.setUint32(28, rate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  text(36, 'data'); v.setUint32(40, samples.length * 2, true);
  samples.forEach((s, i) => v.setInt16(44 + i * 2, Math.max(-1, Math.min(1, s)) * 0x7fff, true));
  return bytes;
}

/** The agent's take: a 440 Hz tone whose amplitude follows the signal over [from, to) ms of the session clock. */
function tone(spans, rate = 8000) {
  const out = [];
  for (const [from, to] of spans) {
    for (let t = from; t < to; t += 1000 / rate) out.push(0.8 * synthetic(Math.floor(t / STEP)) * Math.sin(2 * Math.PI * 440 * t / 1000));
  }
  return out;
}

export function implementation({ changed, agent, now, log, tracks, files }) {
  const recorders = new Map(), meters = new Map();
  let next = 0, views = 0;
  const say = line => log?.(`recording: ${line}`);
  const announce = () => changed('recording.mediaRecorderState');

  // ---------------------------------------------------------------- the recorder
  const info = r => ({
    id: r.id, state: r.state, mimeType: r.mimeType, audioBitsPerSecond: r.audioBitsPerSecond ?? null,
    path: r.path, duration: elapsed(r), reason: r.reason, error: r.error, result: r.result,
  });
  const elapsed = r => r.recorded + (r.state === 'recording' && r.since != null ? Math.max(0, now() - r.since) : 0);
  const pause = (r, reason) => {
    if (r.state !== 'recording') return;
    r.recorded = elapsed(r); r.since = null;
    if (agent) r.spans.at(-1)[1] = now();
    r.state = 'paused'; r.reason = reason;
    if (r.media?.state === 'recording') r.media.pause();
    say(`${r.id}: paused (${reason})`);
    announce();
  };
  const resume = r => {
    r.state = 'recording'; r.reason = null; r.since = now();
    if (agent) r.spans.push([now(), null]);
    if (r.media?.state === 'paused') r.media.resume();
    announce();
  };
  /** Write what the browser delivers (bytes, or a Blob), in order; the first slice replaces any file there. */
  const write = (r, data) => {
    r.writing = r.writing.then(async () => {
      const bytes = data instanceof ArrayBuffer ? data : await data.arrayBuffer();
      await (r.size ? files.append(r.path, bytes) : files.write(r.path, bytes));
      r.size += bytes.byteLength;
    }).catch(e => { if (!r.failed) { r.failed = true; say(`${r.path}: not written: ${e.message}`); } });
    return r.writing;
  };
  /** Stop `r` for `reason` and finish its file; its result is sticky (D1). */
  function finish(r, reason, error = null) {
    if (r.done) return r.done;
    r.recorded = elapsed(r); r.since = null;
    r.state = 'inactive'; r.reason = reason; r.error = error;
    r.done = (async () => {
      if (agent) {
        const spans = r.spans.map(([a, b]) => [a, b ?? now()]);
        await write(r, wav(tone(spans)));
      } else if (r.media && r.media.state !== 'inactive') {
        await new Promise(stopped => { r.media.addEventListener('stop', stopped, { once: true }); r.media.stop(); });
        await r.writing;
      }
      await r.writing;
      r.unsubscribe?.();
      r.result = { path: r.path, mimeType: r.mimeType, duration: r.recorded, size: r.size };
      say(`${r.id}: stopped (${reason}), ${r.path}, ${r.size} bytes`);
      announce();
      return r.result;
    })();
    announce();
    return r.done;
  }
  /** The browser's recorder over `source`, slices appended as they arrive. */
  function media(r, source) {
    if (agent || r.media || r.state === 'inactive') return;
    try {
      r.media = new MediaRecorder(new MediaStream([source]), { mimeType: r.mimeType, audioBitsPerSecond: r.audioBitsPerSecond ?? undefined, audioBitrateMode: r.audioBitrateMode ?? undefined });
    } catch (e) {
      return finish(r, 'error', e.message);
    }
    r.media.ondataavailable = e => { if (e.data.size) write(r, e.data); };
    r.media.onerror = e => finish(r, 'error', e.error?.message ?? 'the recorder failed');
    r.media.start(1000);
    if (r.state === 'paused') r.media.pause();
  }
  // The rest of every recording, asked for as the page goes (D6): what the store receives is kept.
  if (!agent && typeof addEventListener === 'function') addEventListener('pagehide', () => { for (const r of recorders.values()) if (r.media?.state === 'recording') r.media.requestData(); });

  /** The type to write: the written one if the browser writes it, else MP4, WebM or Ogg, the first it writes. */
  function typeFor(asked) {
    if (agent) return { type: 'audio/wav' };
    if (typeof MediaRecorder !== 'function') return { error: 'NotSupportedError' };
    if (asked) return MediaRecorder.isTypeSupported(asked) ? { type: asked } : { error: 'NotSupportedError' };
    // MP4 first where the browser writes it: its duration is in the file, where Chrome's WebM has none until played
    // through. Unwritten, the browser's own order would be WebM or Ogg (D2); a declared deviation.
    for (const t of ['audio/mp4', 'audio/webm', 'audio/ogg']) if (MediaRecorder.isTypeSupported(t)) return { type: t };
    return { error: 'NotSupportedError' };
  }
  /** The path with its container's extension: added when the app wrote none, refused when it disagrees. */
  function pathFor(path, type) {
    const ext = EXTENSIONS[type.split(';')[0]] ?? '';
    const name = path.slice(path.lastIndexOf('/') + 1), dot = name.lastIndexOf('.');
    if (dot <= 0) return path + ext;
    return name.slice(dot) === ext ? path : null;
  }

  // ---------------------------------------------------------------- the level
  /** A meter on one track with one argument group: the levels since it began. */
  function meter(args) {
    const key = JSON.stringify(args);
    let m = meters.get(key);
    if (m) return m;
    const interval = args.interval > 0 ? args.interval : STEP, history = args.history > 0 ? args.history : 100;
    m = { key, interval, history, levels: [], energy: 0, duration: 0, last: 0, running: false, at: now(), pending: [] };
    const sample = () => {
      m.analyser.getFloatTimeDomainData(m.buffer);
      let sum = 0;
      for (const s of m.buffer) sum += s * s;
      m.pending.push(Math.sqrt(sum / m.buffer.length));
    };
    const stopSampling = () => { clearInterval(m.timer); m.timer = null; m.audio?.close().catch(() => {}); m.audio = null; };
    try {
      m.unsubscribe = tracks.subscribe(args.track, {
        live(source) {
          m.running = true; m.at = now();
          if (agent || !source) return;
          stopSampling();
          // Firefox connects a stream only to a context at the stream's own rate.
          const connect = rate => {
            const audio = new AudioContext(rate ? { sampleRate: rate } : undefined);
            try { audio.createMediaStreamSource(new MediaStream([source])).connect(m.analyser = audio.createAnalyser()); return audio; }
            catch (e) { audio.close().catch(() => {}); throw e; }
          };
          try { m.audio = connect(); }
          catch { try { m.audio = connect(source.getSettings?.().sampleRate ?? 48000); } catch (e) { say(`a level on ${args.track} has no analyser: ${e.message}`); return; } }
          m.audio.resume?.().catch(() => {}); // a context made outside a gesture starts suspended
          m.analyser.fftSize = 2048;
          m.buffer = new Float32Array(m.analyser.fftSize);
          m.timer = setInterval(sample, interval);
        },
        muted() { m.muted = true; },
        unmuted() { m.muted = false; },
        ended() { m.running = false; stopSampling(); },
      }, { start: false });
    } catch (e) {
      return { error: e.code ?? 'NotFoundError' };
    }
    m.stop = () => { stopSampling(); m.unsubscribe?.(); };
    meters.set(key, m);
    return m;
  }
  /** Every interval since the last answer, as levels (D1). */
  function read(m) {
    const t = now(), steps = Math.floor((t - m.at) / m.interval);
    let fresh = [];
    if (steps > 0) {
      if (!m.running || m.muted) fresh = new Array(steps).fill(0);
      else if (agent) for (let k = 1; k <= steps; k++) fresh.push(synthetic(Math.floor((m.at + k * m.interval) / STEP)));
      else fresh = m.pending.splice(0).slice(-steps);
      m.at += steps * m.interval;
    }
    for (const level of fresh) {
      m.energy += level * level * m.interval / 1000;
      m.duration += m.interval / 1000;
      m.last = level;
    }
    m.levels.push(...fresh);
    if (m.levels.length > m.history) m.levels.splice(0, m.levels.length - m.history);
    return { audioLevel: m.last, totalAudioEnergy: m.energy, totalSamplesDuration: m.duration, levels: m.levels.slice() };
  }

  return {
    async startMediaRecorder({ stream, mimeType, audioBitsPerSecond, audioBitrateMode, path }) {
      const entry = tracks.lookup(stream);
      if (entry?.kind !== 'stream') return { error: 'NotFoundError' };
      const live = entry.tracks.filter(id => tracks.lookup(id)?.kind === 'audio');
      if (!live.length) return { error: 'NotSupportedError' };
      const type = typeFor(mimeType);
      if (type.error) return type;
      const full = pathFor(path, type.type);
      if (!full) return { error: 'NotSupportedError' };
      const r = {
        id: `recorder-${++next}`, state: 'recording', mimeType: type.type, audioBitsPerSecond: audioBitsPerSecond ?? null, audioBitrateMode,
        path: full, recorded: 0, since: now(), reason: null, error: null, result: null, size: 0,
        writing: Promise.resolve(), spans: [[now(), null]],
      };
      try {
        r.unsubscribe = tracks.subscribe(live[0], {
          live: source => media(r, source),
          muted: reason => { if (reason !== 'idle') pause(r, 'interrupted'); },
          unmuted() {},
          ended: () => finish(r, 'track-ended'),
        }, { start: true });
      } catch (e) {
        return { error: e.code ?? 'NotFoundError' };
      }
      recorders.set(r.id, r);
      say(`${r.id}: recording ${stream} to ${full}`);
      announce();
      return info(r);
    },
    async pauseMediaRecorder({ recorder }) {
      const r = recorders.get(recorder);
      if (!r) return { error: 'NotFoundError' };
      if (r.state === 'inactive') return { error: 'InvalidStateError' };
      pause(r, 'requested');
      return info(r);
    },
    async resumeMediaRecorder({ recorder }) {
      const r = recorders.get(recorder);
      if (!r) return { error: 'NotFoundError' };
      if (r.state === 'inactive') return { error: 'InvalidStateError' };
      if (r.state === 'paused') resume(r);
      return info(r);
    },
    async stopMediaRecorder({ recorder }) {
      const r = recorders.get(recorder);
      if (!r) return { error: 'NotFoundError' };
      return finish(r, 'requested');
    },
    async mediaRecorderState({ recorder }) {
      if (!recorder) return NO_RECORDER;
      const r = recorders.get(recorder);
      return r ? info(r) : { error: 'NotFoundError' };
    },
    async isTypeSupported({ type }) {
      return { type, supported: agent ? type === 'audio/wav' : typeof MediaRecorder === 'function' && MediaRecorder.isTypeSupported(type) };
    },
    async audioLevel(args) {
      if (!args.track) return SILENCE;
      const m = meter({ track: args.track, interval: args.interval ?? null, history: args.history ?? null });
      return m.error ? m : read(m);
    },
    /** The waveform view's own meter of `track` (D13): its levels, the last `fit`, at each frame. */
    waveform(track) {
      const m = meter({ track, interval: STEP, history: 1000, view: ++views });
      if (m.error) return { error: m.error, levels: () => [], stop() {} };
      return { levels: fit => read(m).levels.slice(-fit), stop: () => { m.stop(); meters.delete(m.key); } };
    },
    /** A level's argument group has no reader left: its meter goes (D10 item 12). */
    releaseArgs(name, args) {
      if (name !== 'audioLevel') return;
      const key = JSON.stringify({ track: args.track, interval: args.interval ?? null, history: args.history ?? null });
      meters.get(key)?.stop();
      meters.delete(key);
    },
    release(name) {
      if (name !== 'audioLevel') return;
      // A waveform view's meter is the view's, not a reader's.
      for (const [key, m] of meters) if (!key.includes('"view":')) { m.stop(); meters.delete(key); }
    },
    /** A recorder whose reply was dropped: stopped, its file finished, never the app's (D6). */
    dispose(ids) {
      for (const id of ids) { const r = recorders.get(id); if (r) finish(r, 'requested'); }
    },
  };
}

// ---------------------------------------------------------------- the waveform (D13)
/** `<audio-waveform track=…>`: the track's levels as bars, newest at the right, drawn each animation frame from a
 * meter of its own below the runner, so no level crosses into Contract. A reader of the track, never a start: an idle
 * track draws silence. Its colour is `tint=`, else the element's `color`. */
class Waveform {
  constructor({ implementation, element, props }) {
    this.implementation = implementation;
    this.canvas = element.ownerDocument.createElement('canvas');
    this.canvas.style.cssText = 'width:100%;height:100%;display:block';
    element.append(this.canvas);
    this.element = element;
    this.bars = [];
    this.setProps(props);
    const frame = () => { if (!this.gone) { this.draw(); this.raf = requestAnimationFrame(frame); } };
    this.raf = requestAnimationFrame(frame);
  }
  setProps(props) {
    this.color = props.tint || '';
    if (props.track === this.track) return;
    this.meter?.stop();
    this.track = props.track ?? '';
    this.meter = this.track ? this.implementation.waveform(this.track) : null;
    this.bars = [];
  }
  draw() {
    const c = this.canvas, scale = devicePixelRatio || 1, w = c.clientWidth, h = c.clientHeight;
    if (!w || !h) return;
    if (c.width !== Math.round(w * scale) || c.height !== Math.round(h * scale)) { c.width = Math.round(w * scale); c.height = Math.round(h * scale); }
    const fit = Math.max(0, Math.floor((w + GAP) / (BAR + GAP)));
    if (this.meter && !this.meter.error) this.bars = this.meter.levels(fit);
    const g = c.getContext('2d');
    g.setTransform(scale, 0, 0, scale, 0, 0);
    g.clearRect(0, 0, w, h);
    g.fillStyle = this.color || getComputedStyle(this.element).color;
    this.bars.slice(-fit).forEach((level, i, shown) => {
      const x = w - (shown.length - i) * (BAR + GAP) + GAP, half = Math.max(1, level * (h / 2 - 2));
      g.fillRect(x, h / 2 - half, BAR, half * 2);
    });
  }
  destroy() { this.gone = true; cancelAnimationFrame(this.raf); this.meter?.stop(); this.canvas.remove(); }
}
const BAR = 3, GAP = 2;
export const roster = { 'audio-waveform': { view: Waveform } };
