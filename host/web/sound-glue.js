// @ref LLP 1096 D6, D7. The web's output for the runner's voice table, one
// file for the wasm glue and the JS target, loaded after first paint by a
// plan that declares a sound and never under the agent (whose clock is
// virtual: nothing plays, D10). Web Audio's model in its smallest form: one
// decoded `AudioBuffer` per declared sound, a one-shot `AudioBufferSourceNode`
// and a `GainNode` per voice, started at `start(when)` on the context's clock.
//
// Runner time is `performance.now()` minus the host's origin (the wasm
// glue's `t0`, the JS target's `clock.start`). While the context's output
// timestamp is live (`performanceTime > 0`, whatever its `state`):
//   when = max(0, contextTime + (origin + at - performanceTime) / 1000)
// `max(0, …)` keeps a late voice from throwing (`start(-0.01)` is a
// RangeError); a `when` below `currentTime` starts at once, attack included.
// Before it is live (`{0, 0}`: Chrome and WebKit at the press), a due voice
// starts with `start(0)` and a future one is held with no node, started once
// by whichever comes first: the timestamp going live, or its own time (a
// timer). An `End` for a held voice only sets its `stopAt`: `stop()` on a
// node never started throws InvalidStateError in all three engines.
//
// A voice before the page's first user activation is dropped and journaled,
// never queued (a queue would burst on the first tap): a capture-phase
// `pointerdown`, `keydown` or `touchend` resumes the context, so the press
// that runs a commit plays that commit's sound (trivia F5).
const ACTIVATING = ['pointerdown', 'keydown', 'touchend'];

/** Where every browser measured has `getOutputTimestamp`, the pair it gives;
 * else `currentTime - outputLatency` against now, Firefox's measured gap
 * (Chrome's also holds `baseLatency`, WebKit's was `baseLatency`: named here,
 * not added). */
function outputTimestamp(ctx) {
  if (typeof ctx.getOutputTimestamp === 'function') return ctx.getOutputTimestamp();
  return { contextTime: Math.max(0, ctx.currentTime - (ctx.outputLatency || 0)), performanceTime: performance.now() };
}

/** Install the output. `files`: the plan's sounds, in declaration order (a
 * voice names one by index); `log`: the host's journal; `origin`: where
 * runner time 0 is on `performance.now()`'s timeline. A test passes
 * `context`, `timestamp`, `now` and `activated` (an OfflineAudioContext has
 * no output timestamp and no activation). */
function installSound({ files, log, origin, root = document.getElementById('exact-root'), context, timestamp, now = () => performance.now(), activated, fetchFile = src => fetch(new URL(src, document.baseURI)).then(r => r.ok ? r.arrayBuffer() : Promise.reject(new Error(`HTTP ${r.status}`))) }) {
  const ctx = context ?? new AudioContext({ latencyHint: 'interactive' });
  const stamp = timestamp ?? (() => outputTimestamp(ctx));
  let sources = [], buffers = [], decoded = 0, gesture = false;
  const counts = { blocked: 0, late: 0, dropped: 0 };
  /** Every voice not yet over: `{id, sound, at, gain, stopAt, node, held, timer}`. */
  const voices = new Map();
  // `app.json`'s `audio_session`, where the browser has the Audio Session API (WebKit).
  const session = root?.dataset.audioSession;
  if (session && navigator.audioSession) navigator.audioSession.type = session;
  const userActivated = activated ?? (() => navigator.userActivation?.hasBeenActive ?? gesture);
  if (!context) {
    const resume = e => { if (!e.isTrusted) return; gesture = true; if (ctx.state !== 'running') ctx.resume().catch(() => {}); else for (const t of ACTIVATING) document.removeEventListener(t, resume, true); };
    for (const t of ACTIVATING) document.addEventListener(t, resume, true);
  }
  const live = () => stamp().performanceTime > 0;
  /** The context's time for runner time `at`, clamped, while the timestamp is live. */
  const when = at => { const { contextTime, performanceTime } = stamp(); return Math.max(0, contextTime + (origin() + at - performanceTime) / 1000); };
  function load(list) {
    const generation = sources = list;
    buffers = list.map(() => null); decoded = 0;
    list.forEach((src, i) => {
      buffers[i] = fetchFile(src).then(bytes => ctx.decodeAudioData(bytes)).then(b => {
        if (sources !== generation) return null;
        decoded++; buffers[i] = b; return b;
      }, e => { if (sources === generation) log(`sound: ${src} could not be decoded: ${e.message}`); return null; });
    });
  }
  /** Start voice `v` from its first frame, now or at its time, unless it is cut before it starts. */
  function begin(v, at, immediately) {
    v.held = false; clearTimeout(v.timer);
    const buffer = buffers[v.sound];
    if (v.stopAt != null && v.stopAt <= v.at) return voices.delete(v.id);
    const node = new AudioBufferSourceNode(ctx, { buffer }), gain = new GainNode(ctx, { gain: v.gain });
    node.connect(gain).connect(ctx.destination);
    const start = immediately ? 0 : at;
    if (!immediately && start <= ctx.currentTime) counts.late++;
    node.start(start);
    if (v.stopAt != null) node.stop(immediately ? Math.max(ctx.currentTime, ctx.currentTime + (origin() + v.stopAt - now()) / 1000) : when(v.stopAt));
    node.onended = () => { voices.delete(v.id); gain.disconnect(); };
    v.node = node;
  }
  /** Start a voice by the formula, or hold it until the timestamp is live. */
  function schedule(v) {
    if (live()) return begin(v, when(v.at), false);
    const wait = origin() + v.at - now();
    if (wait <= 0) return begin(v, 0, true);
    v.held = true;
    v.timer = setTimeout(() => { if (v.held) begin(v, 0, true); }, wait);
    watch();
  }
  // While a voice is held, each frame looks for the timestamp to go live.
  let watching = false;
  function watch() {
    if (watching) return;
    watching = true;
    const look = () => {
      const held = [...voices.values()].filter(v => v.held);
      if (!held.length) { watching = false; return; }
      if (live()) { watching = false; for (const v of held) begin(v, when(v.at), false); return; }
      requestAnimationFrame(look);
    };
    requestAnimationFrame(look);
  }
  function play({ id, sound, at, gain }) {
    const src = sources[sound];
    if (!userActivated()) { counts.blocked++; return log(`sound blocked: the page has had no user activation (${src})`); }
    const v = { id, sound, at, gain, stopAt: null, node: null, held: false, timer: 0 };
    voices.set(id, v);
    const buffer = buffers[sound];
    if (buffer && !(buffer instanceof Promise)) return schedule(v);
    // Still decoding: it starts when it is decoded, from its first frame, unless its end has passed.
    Promise.resolve(buffer).then(b => {
      if (!voices.has(id) || sources[sound] !== src) return;
      const length = b ? 1000 * b.duration : 0, ends = Math.min(at + length, v.stopAt ?? Infinity);
      if (!b || origin() + ends <= now()) { voices.delete(id); counts.dropped++; return log(`sound dropped: ${src} was still decoding at its end`); }
      schedule(v);
    });
  }
  function end({ id, at }) {
    const v = voices.get(id);
    if (!v) return;
    v.stopAt = at;
    if (v.node) v.node.stop(live() ? when(at) : Math.max(ctx.currentTime, ctx.currentTime + (origin() + at - now()) / 1000));
    else if (v.held && at <= v.at) { clearTimeout(v.timer); voices.delete(id); }
  }
  load(files);
  return {
    /** The runner's ops, in order: `play` and `end`. */
    ops(list) { for (const op of list) op.op === 'play' ? play(op) : end(op); },
    /** A new boot's files: every voice of the last one ends now. */
    reset(list) {
      for (const v of voices.values()) { clearTimeout(v.timer); try { v.node?.stop(); } catch {} }
      voices.clear();
      if (list.length !== sources.length || list.some((s, i) => s !== sources[i])) load(list);
    },
    /** The files this output decodes (a host compares a boot's against them). */
    get files() { return sources; },
    /** `state.sounds.output` outside the agent (D10). */
    state: () => ({ kind: 'web-audio', state: ctx.state, decoded: `${decoded}/${sources.length}`, ...counts }),
    context: ctx,
  };
}
globalThis.exact ??= {};
globalThis.exact.installSound = installSound;
