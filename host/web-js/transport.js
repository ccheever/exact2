// The transport on the JS target (LLP 1125 D2–D5): the runner's
// (runner/src/transport.rs and runner/src/runner/transport.rs) with the same
// rules, ids, order and journal lines; read it beside them. Imported by an
// app's module only for a plan that declares a sequence or reads
// `exactTransport` (host/web-js/src/timers.rs). The voices and haptics go
// into sounds.js's table (`Sounds.seq`).
// - The gate step (`Sounds.gate`, inside the commit's rollback) reads each
//   sequence's gate, key and six `play` arguments; once the commit stands
//   (`Sounds.stood`) the transport arms, restarts, edits or stops.
// - Its own wake (a fill, the count-in's end) is a `hatch` in `clock.timers`:
//   no commit, after everything due at its time. Its end is a timer after
//   every task (`i: Infinity`), a hatch too when there is no `ended=`.
// - `exactTransport` is answered again by a commit of its own, a `then` after
//   every mutation's: a pseudo-mutation pushed last into `Mutations`.
import { journal, clock, Sounds, Mutations, Resources, R, data, drive, untracked, Refusal } from './rt.js';
import { plain } from './format.js';

const LOOKAHEAD = 1000, REFILL = 500, SEGMENTS = 1024, LOOPS_PER_SECOND = 1000;
const KINDS = ['selection', 'impact', 'impact-light', 'impact-medium', 'impact-heavy', 'impact-soft', 'impact-rigid', 'success', 'warning', 'error'];
const say = line => journal.push(`t=${clock.now} ${line}`);
const num = n => !Number.isFinite(n) ? 'null' : n < 0 ? `-${plain(-n)}` : plain(n); // the runner's `num`
const number = v => typeof v === 'number' ? v : NaN, text = v => typeof v === 'string' ? v : '', items = v => Array.isArray(v) ? v : [];
// Two values the same: numbers by `Object.is` (the runner's bits), lists and records item by item.
const same = (a, b) => typeof a === 'number' ? typeof b === 'number' && Object.is(a, b) : Array.isArray(a) ? Array.isArray(b) && a.length === b.length && a.every((x, i) => same(x, b[i])) : a === b;
// One key's text, as schedule.js's (the runner's `key_text`).
const keyText = v => typeof v === 'string' ? 's:' + v : typeof v === 'number' ? (Number.isFinite(v) ? 'n:' + (v === 0 ? 0 : v) : null) : typeof v === 'boolean' ? 'b:' + v : null;
const time = (seg, b) => seg.at + (b - seg.beat) * 60000 / seg.bpm;
const beatAt = (seg, t) => seg.beat + (t - seg.at) * seg.bpm / 60000;

const Seqs = []; // each declared sequence, in plan order: its code, `ended`, name, and its gate and key as last stood
let Readers = {}, Owner = null, Last = null, Raw = [], Play = null, Segments = [], Recorded = 0, State = 'stopped', Reason = '';
let IssuedTo = 0, Skip = 0, RefillDue = Infinity, PhaseDue = Infinity, EndedDue = Infinity, Pending = null;
const T = () => Sounds.seq;
const Answer = { name: 'transport', due: Infinity, next: Infinity, then: () => { for (const r of Resources) if (r.source === 'exactTransport') R(r); } };
const End = { due: Infinity, once: true, hatch: true, i: Infinity, name: '', action: () => end() };
const Wake = { due: Infinity, hatch: true, i: Infinity, name: '', action: () => (wake(clock.now), true) };

// A step read from its record, or null (journaled when it is not played for a reason).
function step(v, length, label) {
  const f = items(v), beat = number(f[0]), src = text(f[1]), haptic = text(f[8]);
  const gain = number(f[2]), rate = f.length > 9 ? number(f[9]) : 1, pan = f.length > 10 ? number(f[10]) : 0;
  const why = !(Number.isFinite(beat) && beat >= 0 && beat < length) ? `beat ${num(beat)} is outside [0, ${num(length)})`
    : src && !T().declared(src) ? `${JSON.stringify(src)} is not a declared sound`
    : haptic && !KINDS.includes(haptic) ? `${JSON.stringify(haptic)} is not a haptic kind`
    : src && !Number.isFinite(gain) ? 'gain is not a number' : src && !Number.isFinite(rate) ? 'playbackRate is not a number'
    : src && !Number.isFinite(pan) ? 'pan is not a number' : null;
  if (why) { say(`${label} dropped: ${why}`); return null; }
  if (!src && !haptic) return null;
  return { beat, src, gain, group: text(f[3]), rate, pan, haptic };
}
// `play`'s arguments read (runner `Play::read`): the play, or `{ why }` for a tempo that is none.
function read(name, args) {
  const bpm = number(args[1]);
  if (!(Number.isFinite(bpm) && bpm > 0)) return { why: `bpm ${num(bpm)} is not a tempo` };
  const from = number(args[2]);
  if (!Number.isFinite(from)) return { why: 'from is not a number' };
  const tracks = [];
  items(args[0]).forEach((track, i) => {
    const f = items(track), length = number(f[0]), loops = number(f[1]), label = `sequence ${name}: track ${i + 1}`;
    const why = !(Number.isFinite(length) && length > 0) ? `its length ${num(length)} is not a positive number of beats`
      : !(Number.isFinite(loops) && loops >= 0 && loops % 1 === 0) ? `loops ${num(loops)} is not a whole number`
      : bpm / 60 / length > LOOPS_PER_SECOND ? `it would loop more than ${num(LOOPS_PER_SECOND)} times a second` : null;
    if (why) return void say(`${label} dropped: ${why}`);
    tracks.push({ length, loops, steps: items(f[4]).map((s, j) => step(s, length, `${label} step ${j + 1}`)).filter(Boolean) });
  });
  const countIn = from < 0 ? items(args[3]).map((s, j) => step(s, -from, `sequence ${name}: count-in step ${j + 1}`)).filter(Boolean) : [];
  return { tracks, bpm, from, countIn };
}
const samePlay = (a, b) => !!a && !!b && Object.is(a.bpm, b.bpm) && Object.is(a.from, b.from) && sameSteps(a.countIn, b.countIn)
  && a.tracks.length === b.tracks.length && a.tracks.every((t, i) => Object.is(t.length, b.tracks[i].length) && Object.is(t.loops, b.tracks[i].loops) && sameSteps(t.steps, b.tracks[i].steps));
const sameSteps = (a, b) => a.length === b.length && a.every((s, i) => ['beat', 'gain', 'rate', 'pan'].every(k => Object.is(s[k], b[i][k])) && ['src', 'group', 'haptic'].every(k => s[k] === b[i][k]));
const endBeat = p => p.tracks.some(t => t.loops === 0) ? null : p.tracks.reduce((m, t) => Math.max(m, t.loops * t.length), 0);
// The hits whose time is in [from, to) under `seg`, in order: time, then track (0 the count-in), then step.
function hits(p, seg, from, to) {
  const out = [];
  p.countIn.forEach((s, j) => { const at = time(seg, p.from + s.beat); if (from <= at && at < to) out.push({ at, track: 0, step: j }); });
  const b0 = beatAt(seg, from), b1 = beatAt(seg, to);
  p.tracks.forEach((t, i) => {
    let k = Math.max(Math.floor(b0 / t.length) - 1, 0), last = Math.ceil(b1 / t.length) + 1;
    if (t.loops > 0) last = Math.min(last, t.loops - 1);
    for (; k <= last; k++) t.steps.forEach((s, j) => { const at = time(seg, k * t.length + s.beat); if (from <= at && at < to) out.push({ at, track: i + 1, step: j }); });
  });
  return out.sort((a, b) => a.at - b.at || a.track - b.track || a.step - b.step);
}
const stepOf = (p, h) => h.track === 0 ? p.countIn[h.step] : p.tracks[h.track - 1].steps[h.step];
const seg = () => Segments.at(-1) ?? { at: 0, beat: 0, bpm: 0 };
const answerKey = () => { const g = seg(); return [State, Reason, g.bpm, g.at, g.beat].join('\0'); };
function segment(g) { Segments.push(g); Recorded++; if (Segments.length > SEGMENTS) Segments.shift(); }
function timing() {
  if (!Play || !Segments.length) return;
  PhaseDue = State === 'countin' ? time(seg(), 0) : Infinity;
  const b = endBeat(Play); EndedDue = b == null ? Infinity : time(seg(), b);
}
// Issue the hits in [IssuedTo, at + LOOKAHEAD); off the agent a hit already past is skipped and said once.
function fill(at) {
  if (!Play || Owner == null) return;
  const from = IssuedTo, to = at + LOOKAHEAD, list = hits(Play, seg(), from, to), live = !clock.agent;
  let skipped = 0; while (skipped < Skip && skipped < list.length && list[skipped].at === from) skipped++;
  list.splice(0, skipped); Skip = 0;
  let late = 0;
  const full = T().batch(() => {
    for (let k = 0; k < list.length; k++) {
      const h = list[k];
      if (live && h.at < at) { late++; continue; }
      const s = stepOf(Play, h);
      if (s.src && T().issue(at, [s.src, h.at, s.gain, s.group, s.rate, s.pan]) === 'full') return k;
      if (s.haptic) T().haptic(s.haptic, h.at);
    }
    return null;
  });
  if (late) say(`sequence ${Seqs[Owner].name}: ${late} steps late, skipped`);
  if (full != null) {
    const t = list[full].at;
    let before = 0; while (before < full && list[full - 1 - before].at === t) before++;
    const next = T().nextStart(at);
    IssuedTo = t; Skip = before + (t === from ? skipped : 0); RefillDue = next < Infinity ? next : t;
  } else { IssuedTo = to; RefillDue = to - REFILL; }
}
function arm(i, t, args) {
  const name = Seqs[i].name;
  Last = i;
  const p = read(name, args);
  if (p.why) { say(`sequence ${name} did not play: ${p.why}`); stop(t, 'bpm'); State = 'stopped'; Reason = 'bpm'; return; }
  say(`sequence ${name} starts at beat ${num(p.from)}, bpm ${num(p.bpm)}`);
  Owner = i; Raw = args;
  segment({ at: t, beat: p.from, bpm: p.bpm });
  State = p.from < 0 ? 'countin' : 'playing'; Reason = ''; Play = p; IssuedTo = t; Skip = 0;
  timing(); fill(t);
}
function edit(t, args) {
  const name = Seqs[Owner].name, p = read(name, args);
  if (p.why) { say(`sequence ${name}: ${p.why}`); return stop(t, 'bpm'); }
  Raw = args;
  if (samePlay(Play, p)) return;
  const g = seg();
  if (!Object.is(p.bpm, g.bpm)) { say(`sequence ${name} edited: bpm ${num(p.bpm)}`); segment({ at: t, beat: beatAt(g, t), bpm: p.bpm }); }
  else say(`sequence ${name} edited`);
  T().batch(() => T().retract(t));
  Play = p; IssuedTo = t; Skip = 0;
  timing(); fill(t);
}
function stop(t, reason) {
  if (Owner == null) return;
  const name = Seqs[Owner].name;
  Owner = null;
  say(`sequence ${name} stopped (${reason})`);
  T().batch(() => T().stop(t, name));
  State = 'stopped'; Reason = reason; Play = null; Raw = []; RefillDue = PhaseDue = EndedDue = Infinity;
}
// A change's aftermath: the output hears it, the clock's entries follow it, and `exactTransport` is answered again.
function after(before) {
  T().flush();
  for (const e of [End, Wake]) { const k = clock.timers.indexOf(e); if (k >= 0) clock.timers.splice(k, 1); }
  End.due = EndedDue; End.hatch = !(Owner != null && Seqs[Owner].ended); Wake.due = Math.min(RefillDue, PhaseDue);
  if (End.due < Infinity) clock.timers.push(End);
  if (Wake.due < Infinity) clock.timers.push(Wake);
  if (Object.keys(Readers).length && answerKey() !== before) {
    Answer.due = Math.min(Answer.due, clock.now);
    // After every mutation's `then`, whatever the order the module declared them in.
    const k = Mutations.indexOf(Answer);
    if (k < 0 || k !== Mutations.length - 1) { if (k >= 0) Mutations.splice(k, 1); Mutations.push(Answer); }
  }
  if (!clock.agent) drive();
}
// The last finite track ended: stopped, then its `ended=` action as a timer's.
function end() {
  const before = answerKey(), q = Owner == null ? null : Seqs[Owner];
  if (!q) return true;
  Owner = null;
  say(`sequence ${q.name} ended`);
  State = 'stopped'; Reason = 'ended'; Play = null; Raw = []; RefillDue = PhaseDue = EndedDue = Infinity;
  after(before);
  return q.ended ? q.ended() : true;
}
function wake(at) {
  const before = answerKey();
  if (PhaseDue <= at) { PhaseDue = Infinity; State = 'playing'; }
  if (RefillDue <= at) fill(at);
  after(before);
}
function stood(seen, t = clock.now) {
  const before = answerKey();
  seen.forEach((s, i) => {
    const q = Seqs[i], wasOn = q.on, wasKey = q.kt;
    q.on = s.on; q.kt = s.key;
    const rising = s.on && (!wasOn || s.key !== wasKey);
    if (Owner === i) {
      if (!s.on) stop(t, 'gate');
      else if (s.key !== wasKey) { T().batch(() => T().retract(t)); arm(i, t, s.args); }
      else if (Raw.length !== s.args.length || !Raw.every((a, k) => same(a, s.args[k]))) edit(t, s.args);
    } else if (rising) {
      if (Owner != null) say(`sequence ${q.name} did not play: sequence ${Seqs[Owner].name} is playing`);
      else arm(i, t, s.args);
    }
  });
  after(before);
}
// Each sequence's gate, key text and arguments, over the settled state; a key that is no key refuses the commit.
const look = q => {
  const on = q.gate() === true, key = on && q.key ? keyText(q.key()) : undefined;
  if (key === null) throw new Refusal(`TaskKey { task: ${JSON.stringify(q.name)} }`);
  return { on, key, args: on ? q.args() : [] };
};
Sounds.gate = () => { Pending = null; if (Seqs.length) Pending = untracked(() => Seqs.map(look)); };
Sounds.stood = () => { const seen = Pending; Pending = null; if (seen) stood(seen); };
/** `sequence name when gate key=key` (LLP 1125 D2): `args` makes `play`'s six arguments; armed at mount when its gate holds. */
export function sequence(gate, key, args, ended, name) {
  Seqs.push({ gate, key: key || null, args, ended, name, on: false, kt: undefined });
  const seen = Seqs.map((q, k) => k === Seqs.length - 1 ? untracked(() => look(q)) : { on: q.on, key: q.kt, args: Owner === k ? Raw : [] });
  stood(seen);
}
/** The `exactTransport` readers, by resource name: each one's fields, in its shape's order. */
export function transport(readers) {
  Readers = readers;
  (data.reserved ??= {}).exactTransport = (_, a, name) => {
    const g = seg(), f = { state: State, reason: Reason, bpm: g.bpm, anchorAt: g.at, anchorBeat: g.beat, clock: 'internal' };
    return Readers[name].map(k => f[k]);
  };
  // `state.transport`, as the runner's (runner/src/agent/transport.rs), for a plan that declares a sequence.
  (globalThis.exact ??= {}).transport = {
    declared: () => Seqs.length > 0,
    state: () => {
      const g = seg();
      return { state: State, reason: Reason, sequence: Last == null ? null : Seqs[Last].name, bpm: g.bpm, anchorAt: g.at, anchorBeat: g.beat, clock: 'internal', issuedTo: IssuedTo,
        segments: Segments.slice(-16).map(s => ({ at: s.at, beat: s.beat, bpm: s.bpm })), recorded: Recorded, haptics: T()?.haptics() ?? [] };
    },
  };
}
