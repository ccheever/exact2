// The voice table on the JS target (LLP 1096 D5), the runner's
// (runner/src/sound.rs) with the same rules and journal lines: bundled only
// for a plan that declares a sound or runs `playSound`, `playSounds` or
// `stopSounds`. rt.js records the three commands in `Sounds.pending` (never
// a host's, so never `refused:`), and applies them here once the commit has
// stood, so a voice gets one id. Outside the agent the ops go to the web
// host's sound-glue.js, fetched after the first frame (D7); under the agent
// nothing plays and `state.sounds.output` is `"agent"` (D10).
import { journal, clock, Sounds } from './rt.js';

const LIVE = 32, RECORD = 1024, SHOWN = 64;
const say = line => journal.push(`t=${clock.now} ${line}`);
let Table = [], Voices = [], Next = 0, Evicted = 0, EvictedThrough = 0, Ops = [], Glue = null, Output = null;
const sounds = by => by !== 'cut' && by !== 'cancelled';
const live = (v, t) => sounds(v.by) && v.ends > t;
const given = v => v ?? null;
const finite = (v, d) => v == null ? d : typeof v === 'number' && Number.isFinite(v) ? v : null;

function play(t, [src, asked0, gain0, group0]) {
  if (typeof src !== 'string') return say('sound dropped: the sound is not a string');
  const sound = Table.findIndex(r => r[0] === src);
  if (sound < 0) return say(`sound dropped: ${JSON.stringify(src)} is not a declared sound`);
  const asked = finite(given(asked0), t);
  if (asked == null) return say(`sound dropped: at is not a number (${src})`);
  const gain = finite(given(gain0), 1);
  if (gain == null) return say(`sound dropped: gain is not a number (${src})`);
  const group = given(group0) ?? '', at = Math.max(asked, t), [, frames, rate] = Table[sound];
  const length = 1000 * frames / Math.max(rate, 1), saved = Voices.map(v => [v.ends, v.by]);
  const id = ++Next, voice = { id, sound, src, at, gain: Math.min(Math.max(gain, 0), 1), group, ends: at + length, by: 'end', natural: at + length };
  const cuts = [];
  if (group) {
    // The next voice in the group that sounds ends this one; this one ends
    // every earlier voice still sounding at its start (a tie: the later call).
    const next = Voices.filter(w => w.group === group && sounds(w.by) && w.at > at).reduce((m, w) => Math.min(m, w.at), Infinity);
    if (next < voice.ends) { voice.ends = next; voice.by = 'group'; cuts.push([id, next]); }
    for (const w of Voices) if (w.group === group && sounds(w.by) && w.at <= at && w.ends > at) { w.ends = at; w.by = w.at < at ? 'group' : 'cut'; cuts.push([w.id, at]); }
  }
  if (Voices.filter(v => live(v, t)).length + (live(voice, t) ? 1 : 0) > LIVE) {
    Voices.forEach((w, i) => { [w.ends, w.by] = saved[i]; });
    Next--;
    return say(`sound dropped: ${LIVE} voices sound or wait (${src} at ${at})`);
  }
  say(`sound ${id} ${src} at ${at} gain ${voice.gain}${group ? ` group ${group}` : ''}${asked < t ? ` (asked ${asked})` : ''}`);
  Voices.push(voice);
  for (const [cut, ends] of cuts.sort((a, b) => a[0] - b[0])) say(`sound ${cut} ends at ${ends} (group ${group})`);
  // Keep the last RECORD voices, the oldest no longer live first.
  while (Voices.length > RECORD) {
    const i = Voices.findIndex(v => !live(v, t));
    if (i < 0) break;
    const [gone] = Voices.splice(i, 1); Evicted++; EvictedThrough = Math.max(EvictedThrough, gone.at);
  }
}
function endLive(t, group) {
  let n = 0;
  for (const v of Voices) if (live(v, t) && (group == null || v.group === group)) { n++; if (v.at < t) { v.ends = t; v.by = 'stop'; } else { v.ends = v.at; v.by = 'cancelled'; } }
  return n;
}
function queue(before, firstNew) {
  for (const v of Voices) {
    if (v.id >= firstNew) { Ops.push({ op: 'play', id: v.id, sound: v.sound, at: v.at, gain: v.gain }); if (v.ends < v.natural) Ops.push({ op: 'end', id: v.id, at: v.ends }); }
    else if (before.has(v.id) && v.ends < before.get(v.id)) Ops.push({ op: 'end', id: v.id, at: v.ends });
  }
}
/** One commit's commands, at its time, once it stood (rt.js); a `reload` ends every live voice. */
function apply(commands) {
  if (!commands.some(([name]) => Sounds.own.has(name) || name === 'reload' && Table.length)) return;
  const t = clock.now, before = new Map(Voices.map(v => [v.id, v.ends])), firstNew = Next + 1;
  for (const [name, args] of commands) {
    if (!Sounds.own.has(name)) continue;
    if (name === 'playSound') play(t, args);
    else if (name === 'playSounds') { if (Array.isArray(args[0])) for (const hit of args[0]) Array.isArray(hit) ? play(t, hit) : say('sound dropped: a hit is not a record'); else say('sound dropped: playSounds takes a list'); }
    else { const g = given(args[0]); const n = endLive(t, g || null); say(g ? `sounds stopped: ${n} (group ${g})` : `sounds stopped: ${n}`); }
  }
  if (Table.length && commands.some(([name]) => name === 'reload')) say(`sounds stopped: ${endLive(t, null)} (reload)`);
  queue(before, firstNew);
  flush();
}
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
  (globalThis.exact ??= {}).sounds = {
    // `state.sounds`, as the runner's (runner/src/agent/sound.rs).
    state: all => ({
      voices: Voices.slice(all ? 0 : Math.max(0, Voices.length - SHOWN)).map(v => ({ id: v.id, src: v.src, at: v.at, gain: v.gain, group: v.group || null, ends: v.ends, by: v.by, state: !sounds(v.by) || clock.now >= v.ends ? 'ended' : clock.now < v.at ? 'waiting' : 'sounding' })),
      live: Voices.filter(v => live(v, clock.now)).length, recorded: Voices.length, evicted: Evicted, evictedThrough: EvictedThrough,
      output: clock.agent ? 'agent' : Output?.state() ?? { kind: 'web-audio', state: 'loading' },
    }),
  };
  // Decoding starts after the first frame, before the first press needs it.
  if (Table.length && typeof requestAnimationFrame === 'function') requestAnimationFrame(() => setTimeout(load));
}
