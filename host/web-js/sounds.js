// The voice table on the JS target (LLP 1096 D5), the runner's
// (runner/src/sound.rs) with the same rules and journal lines: bundled only
// for a plan that declares a sound, runs `playSound`, `playSounds` or
// `stopSounds`, or declares a sequence. rt.js records the three commands in
// `Sounds.pending` (never a host's, so never `refused:`), and applies them
// here once the commit has stood, so a voice gets one id. A sequence's
// voices and timed haptics come from transport.js through `Sounds.seq`
// (LLP 1125 D3). Outside the agent the ops go to the web host's
// sound-glue.js, fetched after the first frame (D7); under the agent nothing
// plays and `state.sounds.output` is `"agent"` (D10).
import { journal, clock, Sounds } from './rt.js';
import { plain } from './format.js';

const LIVE = 32, WAITING = 256, RECORD = 1024, HAPTICS = 1024, SHOWN = 64;
const say = line => journal.push(`t=${clock.now} ${line}`);
let Table = [], Voices = [], Next = 0, Evicted = 0, EvictedThrough = 0, Ops = [], Glue = null, Output = null, Haptics = [], NextHaptic = 0;
const sounds = by => by !== 'cut' && by !== 'cancelled' && by !== 'edit';
const live = (v, t) => sounds(v.by) && v.ends > t;
const sounding = (v, t) => sounds(v.by) && v.at <= t && v.ends > t;
const waits = (v, t) => sounds(v.by) && v.at > t;
const waiting = t => Voices.filter(v => waits(v, t)).length;
const given = v => v ?? null;
const finite = (v, d) => v == null ? d : typeof v === 'number' && Number.isFinite(v) ? v : null;
// A number as the runner's journal writes it (`agent::num`): shortest digits, never exponential; `null` when not finite.
const num = n => !Number.isFinite(n) ? 'null' : n < 0 ? `-${plain(-n)}` : plain(n);
// Whether `voice` would make more than LIVE voices sound at some voice's start (LLP 1125 D3): its own, or a later one's while it sounds.
function overfull(voice) {
  if (!sounds(voice.by) || voice.ends <= voice.at) return false;
  const starts = [voice.at, ...Voices.filter(w => sounds(w.by) && w.at > voice.at && w.at < voice.ends).map(w => w.at)];
  return starts.some(s => Voices.filter(w => sounding(w, s)).length >= LIVE);
}

// A hit's `playbackRate` and `pan` (LLP 1125 D10): clamped as `gain` is; the rate divides the file's length. `seq`: a
// sequence issues it, and stops expanding at the waiting cap ('full') where a tap is dropped.
function play(t, [src, asked0, gain0, group0, rate0, pan0], seq = false) {
  if (typeof src !== 'string') return say('sound dropped: the sound is not a string');
  const sound = Table.findIndex(r => r[0] === src);
  if (sound < 0) return say(`sound dropped: ${JSON.stringify(src)} is not a declared sound`);
  const asked = finite(given(asked0), t);
  if (asked == null) return say(`sound dropped: at is not a number (${src})`);
  const gain = finite(given(gain0), 1);
  if (gain == null) return say(`sound dropped: gain is not a number (${src})`);
  const group = given(group0) ?? '';
  let playbackRate = finite(given(rate0), 1);
  if (playbackRate == null) return say(`sound dropped: playbackRate is not a number (${src})`);
  let pan = finite(given(pan0), 0);
  if (pan == null) return say(`sound dropped: pan is not a number (${src})`);
  playbackRate = Math.min(Math.max(playbackRate, 0.25), 4); pan = Math.min(Math.max(pan, -1), 1);
  const at = Math.max(asked, t), [, frames, rate] = Table[sound];
  if (at > t && waiting(t) >= WAITING) return seq ? 'full' : say(`sound dropped: ${WAITING} voices wait (${src} at ${num(at)})`);
  const length = 1000 * frames / Math.max(rate, 1) / playbackRate, saved = Voices.map(v => [v.ends, v.by]);
  const id = ++Next, voice = { id, sound, src, at, gain: Math.min(Math.max(gain, 0), 1), group, playbackRate, pan, ends: at + length, by: 'end', seq, natural: at + length };
  const cuts = [];
  if (group) {
    // The next voice in the group that sounds ends this one; this one ends
    // every earlier voice still sounding at its start (a tie: the later call).
    const next = Voices.filter(w => w.group === group && sounds(w.by) && w.at > at).reduce((m, w) => Math.min(m, w.at), Infinity);
    if (next < voice.ends) { voice.ends = next; voice.by = 'group'; cuts.push([id, next]); }
    for (const w of Voices) if (w.group === group && sounds(w.by) && w.at <= at && w.ends > at) { w.ends = at; w.by = w.at < at ? 'group' : 'cut'; cuts.push([w.id, at]); }
  }
  if (overfull(voice)) {
    Voices.forEach((w, i) => { [w.ends, w.by] = saved[i]; });
    Next--;
    return say(`sound dropped: ${LIVE} voices sound at its start (${src} at ${num(at)})`);
  }
  say(`sound ${id} ${src} at ${num(at)} gain ${num(voice.gain)}${group ? ` group ${group}` : ''}${playbackRate !== 1 ? ` playbackRate ${num(playbackRate)}` : ''}${pan !== 0 ? ` pan ${num(pan)}` : ''}${asked < t ? ` (asked ${num(asked)})` : ''}`);
  Voices.push(voice);
  for (const [cut, ends] of cuts.sort((a, b) => a[0] - b[0])) say(`sound ${cut} ends at ${num(ends)} (group ${group})`);
  // Keep the last RECORD voices, the oldest no longer live first.
  while (Voices.length > RECORD) {
    const i = Voices.findIndex(v => !live(v, t));
    if (i < 0) break;
    const [gone] = Voices.splice(i, 1); Evicted++; EvictedThrough = Math.max(EvictedThrough, gone.at);
  }
}
// Every voice live at `t` (in `group`) ends: sounding stops at `t`, waiting is cancelled at its start — a sequence's
// waiting one kept when `seqWaits` (`stopSounds`: its next steps play, LLP 1125 D4).
function endLive(t, group, seqWaits) {
  let n = 0;
  for (const v of Voices) if (live(v, t) && !(seqWaits && v.seq && v.at >= t) && (group == null || v.group === group)) { n++; if (v.at < t) { v.ends = t; v.by = 'stop'; } else { v.ends = v.at; v.by = 'cancelled'; } }
  return n;
}
function queue(before, firstNew) {
  for (const v of Voices) {
    if (v.id >= firstNew) { Ops.push({ op: 'play', id: v.id, sound: v.sound, at: v.at, gain: v.gain, playbackRate: v.playbackRate, pan: v.pan }); if (v.ends < v.natural) Ops.push({ op: 'end', id: v.id, at: v.ends }); }
    else if (before.has(v.id) && v.ends < before.get(v.id)) Ops.push({ op: 'end', id: v.id, at: v.ends });
  }
}
/** Changes made by `f`, their ops queued once at the end (a commit's, or a sequence's at one instant). */
function batch(f) {
  const before = new Map(Voices.map(v => [v.id, v.ends])), firstNew = Next + 1;
  const out = f();
  queue(before, firstNew);
  return out;
}
// Haptics waiting at `t` (`at >= t`) retracted with why; one line when any was.
function retractHaptics(t, by) {
  let n = 0;
  for (const h of Haptics) if (h.by == null && h.at >= t) { h.by = by; Ops.push({ op: 'unhaptic', id: h.id }); n++; }
  if (n) say(`haptics retracted: ${n} (${by})`);
}
/** One commit's commands, at its time, once it stood (rt.js); a `reload` ends every live voice. */
function apply(commands) {
  if (!commands.some(([name]) => Sounds.own.has(name) || name === 'reload' && Table.length)) return;
  const t = clock.now;
  batch(() => {
    for (const [name, args] of commands) {
      if (!Sounds.own.has(name)) continue;
      if (name === 'playSound') play(t, args);
      else if (name === 'playSounds') { if (Array.isArray(args[0])) for (const hit of args[0]) Array.isArray(hit) ? play(t, hit) : say('sound dropped: a hit is not a record'); else say('sound dropped: playSounds takes a list'); }
      else { const g = given(args[0]); const n = endLive(t, g || null, true); say(g ? `sounds stopped: ${n} (group ${g})` : `sounds stopped: ${n}`); }
    }
    if (Table.length && commands.some(([name]) => name === 'reload')) say(`sounds stopped: ${endLive(t, null, false)} (reload)`);
  });
  if (Table.length && commands.some(([name]) => name === 'reload')) retractHaptics(t, 'cancelled');
  flush();
}
// A sequence's side of the table (LLP 1125 D3), as the runner's `Sounds::issue`, `issue_haptic`, `retract_seq` and
// `stop_seq`; transport.js calls each inside `batch`, then `flush`.
const Seq = {
  batch, flush: () => flush(), waiting, declared: src => Table.some(r => r[0] === src),
  nextStart: t => Voices.filter(v => waits(v, t)).reduce((m, v) => Math.min(m, v.at), Infinity),
  issue: (t, args) => play(t, args, true),
  haptic(kind, at, t) {
    const id = ++NextHaptic;
    say(`haptic ${id} ${kind} at ${num(at)}`);
    Ops.push({ op: 'haptic', id, kind, at });
    Haptics.push({ id, kind, at, by: null });
    // The last HAPTICS, the oldest played or retracted first: one still to come is kept for its retraction.
    while (Haptics.length > HAPTICS) { const i = Haptics.findIndex(h => h.by != null || h.at < t); if (i < 0) break; Haptics.splice(i, 1); }
  },
  retract(t) {
    let n = 0;
    for (const v of Voices) if (v.seq && sounds(v.by) && v.at >= t) { v.ends = v.at; v.by = 'edit'; n++; }
    say(`sounds retracted: ${n} (edit)`);
    retractHaptics(t, 'edit');
  },
  stop(t, name) {
    let n = 0;
    for (const v of Voices) if (v.seq && live(v, t)) { n++; if (v.at < t) { v.ends = t; v.by = 'stop'; } else { v.ends = v.at; v.by = 'cancelled'; } }
    say(`sounds stopped: ${n} (sequence ${name})`);
    retractHaptics(t, 'cancelled');
  },
  // `state.transport.haptics` (runner/src/agent/transport.rs): the last 64.
  haptics: () => Haptics.slice(-64).map(h => ({ id: h.id, kind: h.kind, at: h.at, state: h.by ?? (clock.now < h.at ? 'waiting' : 'played') })),
};
// The output: none under the agent; else the web host's glue, fetched after the first frame.
function load() {
  if (Glue || clock.agent || typeof requestAnimationFrame !== 'function') return;
  Glue = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import('./sound-glue.js'))
    .then(() => { Output = globalThis.exact.installSound({ files: Table.map(r => r[0]), log: say, origin: () => clock.start }); flush(); })
    .catch(err => say(`sound: unavailable: ${err.message}`));
}
function flush() {
  if (clock.agent) Ops = [];
  else if (Output) { if (Ops.length) Output.ops(Ops.splice(0)); }
  else load();
}
/** `table`: the plan's sounds, `[src, frames, rate]` each (host/web-js/src/main.rs). */
export function install(table) {
  Table = table;
  Sounds.apply = apply;
  Sounds.seq = Seq;
  (globalThis.exact ??= {}).sounds = {
    // `state.sounds`, as the runner's (runner/src/agent/sound.rs).
    state: all => ({
      voices: Voices.slice(all ? 0 : Math.max(0, Voices.length - SHOWN)).map(v => ({ id: v.id, src: v.src, at: v.at, gain: v.gain, group: v.group || null, ...(v.playbackRate !== 1 && { playbackRate: v.playbackRate }), ...(v.pan !== 0 && { pan: v.pan }), ends: v.ends, by: v.by, state: !sounds(v.by) || clock.now >= v.ends ? 'ended' : clock.now < v.at ? 'waiting' : 'sounding' })),
      live: Voices.filter(v => live(v, clock.now)).length, recorded: Voices.length, evicted: Evicted, evictedThrough: EvictedThrough,
      output: clock.agent ? 'agent' : Output?.state() ?? { kind: 'web-audio', state: 'loading' },
    }),
  };
  // Decoding starts after the first frame, before the first press needs it.
  if (Table.length && typeof requestAnimationFrame === 'function') requestAnimationFrame(() => setTimeout(load));
}
