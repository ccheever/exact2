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
// - MIDI (LLP 1125 D7): a step's note and clock out go on midi.js's queue
//   (`Midi`); a track's `offset` shifts its outputs, placed as `fill` says.
import { journal, clock, Sounds, Mutations, Resources, R, data, drive, untracked, Refusal } from './rt.js';
import { plain } from './format.js';
import { Midi } from './midi.js';

const LOOKAHEAD = 1000, REFILL = 500, SEGMENTS = 1024, LOOPS_PER_SECOND = 1000, FROM_LIMIT = 2147483648, SHORTEST = 1 / 1048576, EXACT = 4503599627370496, OFFSET_LIMIT = 500, HOLD = 50, TICKS_PER_SECOND = 1000;
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
// Issued: where the last fill stopped, by beat (a tempo edit moves its time): the hits before it, and the first [1] at it,
// were issued.
let IssuedTo = 0, Skip = 0, Issued = [-Infinity, 0], RefillDue = Infinity, PhaseDue = Infinity, EndedDue = Infinity, Pending = null, UnsentTold = false;
const T = () => Sounds.seq;
const Answer = { name: 'transport', requests: 0, e: () => null, due: Infinity, next: Infinity, then: () => { for (const r of Resources) if (r.source === 'exactTransport') R(r); } };
let End = null; // the armed end's `clock.timers` entry: a new one per arming, so one being fired keeps the kind it was chosen as
const Wake = { due: Infinity, hatch: true, i: Infinity, name: '', action: () => (wake(clock.now), true) };

const whole = (n, low, high) => n % 1 === 0 && n >= low && n <= high;
// A step read from its record, or null (journaled when it is not played for a reason); `output` its track's.
function step(v, length, output, label) {
  const f = items(v), beat = number(f[0]), src = text(f[1]), haptic = text(f[8]);
  const gain = number(f[2]), rate = f.length > 9 ? number(f[9]) : 1, pan = f.length > 10 ? number(f[10]) : 0;
  const note = f.length > 4 ? number(f[4]) : -1, velocity = number(f[5]), channel = number(f[6]), duration = number(f[7]);
  const why = !(Number.isFinite(beat) && beat >= 0 && beat < length) ? `beat ${num(beat)} is outside [0, ${num(length)})`
    : src && !T().declared(src) ? `${JSON.stringify(src)} is not a declared sound`
    : haptic && !KINDS.includes(haptic) ? `${JSON.stringify(haptic)} is not a haptic kind`
    : src && !Number.isFinite(gain) ? 'gain is not a number' : src && !Number.isFinite(rate) ? 'playbackRate is not a number'
    : src && !Number.isFinite(pan) ? 'pan is not a number'
    : note !== -1 && !whole(note, 0, 127) ? `note ${num(note)} is not -1 or a MIDI note 0–127`
    : note !== -1 && !whole(velocity, 1, 127) ? `velocity ${num(velocity)} is not 1–127`
    : note !== -1 && !whole(channel, 0, 15) ? `channel ${num(channel)} is not 0–15`
    : note !== -1 && !(Number.isFinite(duration) && duration > 0) ? `duration ${num(duration)} is not a positive number of beats` : null;
  if (why) { say(`${label} dropped: ${why}`); return null; }
  if (!src && !haptic && (note === -1 || !output)) return null;
  return { beat, src, gain, group: text(f[3]), note: output ? note : -1, velocity, channel, duration, rate, pan, haptic };
}
// `play`'s arguments read (runner `Play::read`): the play, or `{ why }` for a tempo that is none.
function read(name, args) {
  const bpm = number(args[1]);
  if (!(Number.isFinite(bpm) && bpm > 0)) return { why: `bpm ${num(bpm)} is not a tempo` };
  const from = number(args[2]);
  if (!Number.isFinite(from)) return { why: 'from is not a number' };
  if (Math.abs(from) > FROM_LIMIT) return { why: `from ${num(from)} is beyond 2^31 beats` };
  const tracks = [];
  items(args[0]).forEach((track, i) => {
    const f = items(track), length = number(f[0]), loops = number(f[1]), offset = f.length > 2 ? number(f[2]) : 0, output = text(f[3]), label = `sequence ${name}: track ${i + 1}`;
    const why = !(Number.isFinite(length) && length > 0) ? `its length ${num(length)} is not a positive number of beats`
      : !(Number.isFinite(loops) && loops >= 0 && loops % 1 === 0) ? `loops ${num(loops)} is not a whole number`
      : length < SHORTEST ? `its length ${num(length)} is under 2^-20 beats`
      : bpm / 60 / length > LOOPS_PER_SECOND ? `it would loop more than ${num(LOOPS_PER_SECOND)} times a second`
      : !(Number.isFinite(offset) && Math.abs(offset) <= OFFSET_LIMIT) ? `its offset ${num(offset)} is not within ${num(OFFSET_LIMIT)} ms` : null;
    if (why) return void say(`${label} dropped: ${why}`);
    tracks.push({ length, loops, offset, output, steps: items(f[4]).map((s, j) => step(s, length, output, `${label} step ${j + 1}`)).filter(Boolean) });
  });
  const countIn = from < 0 ? items(args[3]).map((s, j) => step(s, -from, '', `sequence ${name}: count-in step ${j + 1}`)).filter(Boolean) : [];
  let clockOut = text(args[5]);
  if (clockOut && bpm * 24 / 60 > TICKS_PER_SECOND) { say(`sequence ${name}: clock out dropped: bpm ${num(bpm)} is more than ${num(TICKS_PER_SECOND)} ticks a second`); clockOut = ''; }
  return { tracks, bpm, from, countIn, clockOut };
}
const samePlay = (a, b) => !!a && !!b && Object.is(a.bpm, b.bpm) && Object.is(a.from, b.from) && a.clockOut === b.clockOut && sameSteps(a.countIn, b.countIn)
  && a.tracks.length === b.tracks.length && a.tracks.every((t, i) => ['length', 'loops', 'offset'].every(k => Object.is(t[k], b.tracks[i][k])) && t.output === b.tracks[i].output && sameSteps(t.steps, b.tracks[i].steps));
const sameSteps = (a, b) => a.length === b.length && a.every((s, i) => ['beat', 'gain', 'note', 'velocity', 'channel', 'duration', 'rate', 'pan'].every(k => Object.is(s[k], b[i][k])) && ['src', 'group', 'haptic'].every(k => s[k] === b[i][k]));
const endBeat = p => p.tracks.some(t => t.loops === 0) ? null : p.tracks.reduce((m, t) => Math.max(m, t.loops * t.length), 0);
// The hits whose time is in [from, to) under `seg`, in order: time, then track (0 the count-in), then step.
function hits(p, seg, from, to) {
  const out = [];
  p.countIn.forEach((s, j) => { const beat = p.from + s.beat, at = time(seg, beat); if (from <= at && at < to) out.push({ at, beat, track: 0, step: j }); });
  const b0 = beatAt(seg, from), b1 = beatAt(seg, to);
  p.tracks.forEach((t, i) => {
    let k = Math.max(Math.floor(b0 / t.length) - 1, 0), last = Math.ceil(b1 / t.length) + 1;
    if (t.loops > 0) last = Math.min(last, t.loops - 1);
    for (; k <= last; k++) t.steps.forEach((s, j) => { const beat = k * t.length + s.beat, at = time(seg, beat); if (from <= at && at < to) out.push({ at, beat, track: i + 1, step: j }); });
  });
  return out.sort((a, b) => a.at - b.at || a.track - b.track || a.step - b.step);
}
const stepOf = (p, h) => h.track === 0 ? p.countIn[h.step] : p.tracks[h.track - 1].steps[h.step];
const trackOf = (p, h) => h.track === 0 ? [0, ''] : [p.tracks[h.track - 1].offset, p.tracks[h.track - 1].output];
const seg = () => Segments.at(-1) ?? { at: 0, beat: 0, bpm: 0 };
const answerKey = () => { const g = seg(); return [State, Reason, g.bpm, g.at, g.beat].join('\0'); };
function segment(g) { Segments.push(g); Recorded++; if (Segments.length > SEGMENTS) Segments.shift(); }
// `t`: the arming's or the edit's time, before which nothing ends (an end already past is now, never back in time).
function timing(t) {
  if (!Play || !Segments.length) return;
  PhaseDue = State === 'countin' ? time(seg(), 0) : Infinity;
  const b = endBeat(Play); EndedDue = b == null ? Infinity : Math.max(time(seg(), b), t);
}
// Issue the hits in [IssuedTo, at + LOOKAHEAD), and clock out's ticks before its end; off the agent an output already
// past is skipped and said once. `how` places an output before `at`: played now at the arming, skipped at an edit
// (it was played already), late at a wake (the runner's `Fill`).
function fill(at, how) {
  if (!Play || Owner == null) return;
  // A track that loops forever (or as many times) whose loop index would pass 2^52 (a beat kept across an edit can reach
  // it) is dropped: past 2^53 an index no longer steps by 1. What is left may now end.
  const end = beatAt(seg(), at + LOOKAHEAD), n = Play.tracks.length;
  Play.tracks = Play.tracks.filter(t => {
    const far = (t.loops === 0 || t.loops - 1 >= EXACT) && !(Math.abs(end / t.length) < EXACT);
    if (far) say(`sequence ${Seqs[Owner].name}: a track dropped: beat ${num(end)} is past exact loop counting`);
    return !far;
  });
  if (Play.tracks.length !== n) timing(at);
  const from = IssuedTo, to = at + LOOKAHEAD, live = !clock.agent, g = seg();
  Midi.ticks(b => time(g, b), to, endBeat(Play), live ? at : -Infinity, at);
  const list = hitsAcross(Play, from, to);
  let skipped = 0; while (skipped < Skip && skipped < list.length && list[skipped].at === from) skipped++;
  // The hits an earlier fill issued (an edit fills from its time again): a run of the first, as hits are in time order.
  let issued = 0, tied = 0;
  for (const h of list) { if (h.beat === Issued[0] && tied < Issued[1]) tied++; else if (h.beat >= Issued[0]) break; issued++; }
  list.splice(0, skipped); Skip = 0;
  let late = 0, unsent = null;
  const full = T().batch(() => {
    for (let k = 0; k < list.length; k++) {
      const h = list[k], s = stepOf(Play, h), [offset, output] = trackOf(Play, h), when = h.at + offset;
      let counted = late;
      // Played now at the arming, skipped at an edit, late only at a stalled live wake (more than REFILL past).
      const place = w => w >= at ? w : how === 'arm' ? at : how === 'edit' ? null : live && w < at - REFILL ? (counted++, null) : w;
      // The note's place first: one the full queue blocks stops the fill before any of the hit's outputs is issued; it
      // takes a place once within the hold. A blocked sample's frees when it would start. (The runner's `fill`.)
      const skip = how === 'edit' && k + skipped < issued && when - HOLD <= at, on = s.note >= 0 ? place(when) : null, note = skip ? null : on;
      if (note != null && Midi.canSend(output) && Midi.blocks(note, at)) return [k, note - HOLD];
      if (s.src) { const w = place(when); if (w != null && T().issue(at, [s.src, w, s.gain, s.group, s.rate, s.pan], h.at) === 'full') return [k, w]; }
      if (note != null && Midi.canSend(output)) Midi.note(output, [0x90 | s.channel, s.note, s.velocity], note, Math.max(timeOf(h.at, h.beat + s.duration) + offset, note), h.at, at);
      else if (note != null) unsent ??= output;
      if (s.haptic) { const w = place(when); if (w != null) T().haptic(s.haptic, w, h.at, at); }
      if (counted > late) late++;
    }
    return null;
  });
  if (late) say(`sequence ${Seqs[Owner].name}: ${late} steps late, skipped`);
  if (unsent != null && !UnsentTold) { UnsentTold = true; say(`sequence ${Seqs[Owner].name}: notes not sent: ${Midi.access() !== 'granted' ? `access is ${Midi.access()}` : `${unsent} is not a connected output`}`); }
  if (full != null) {
    const [k, admit] = full, t = list[k].at;
    let before = 0; while (before < k && list[k - 1 - before].at === t) before++;
    IssuedTo = t; Skip = before + (t === from ? skipped : 0); Issued = [list[k].beat, Skip]; RefillDue = Math.min(T().nextStart(at), Midi.nextHanded(at), admit);
  } else { IssuedTo = to; Issued = [beatAt(g, to), 0]; RefillDue = to - REFILL; }
}
// `p`'s hits in [from, to), each timed by the segment it falls in: a fill an edit restarts before its time reaches back
// into the tempo before it (the runner's `Transport::hits`).
// When beat `b` of a hit at `at` falls: in the hit's own segment, or a later one the beat reaches (a note's end past a
// tempo edit), never across a restart, whose beat goes back (the runner's `time_of` in `fill`).
function timeOf(at, b) {
  let j = Math.max(Segments.findLastIndex(g => g.at <= at), 0);
  while (j + 1 < Segments.length && Segments[j + 1].beat <= b && Segments[j + 1].beat >= Segments[j].beat) j++;
  return Segments.length ? time(Segments[j], b) : time(seg(), b);
}
function hitsAcross(p, from, to) {
  const out = [], first = Math.max(Segments.findLastIndex(g => g.at <= from), 0);
  for (let j = first; j < Segments.length; j++) {
    const lo = j === first ? from : Math.max(Segments[j].at, from), hi = j + 1 < Segments.length ? Math.min(Segments[j + 1].at, to) : to;
    if (lo < hi) out.push(...hits(p, Segments[j], lo, hi));
  }
  return out;
}
function arm(i, t, args) {
  const name = Seqs[i].name;
  Last = i;
  const p = read(name, args);
  if (p.why) { say(`sequence ${name} did not play: ${p.why}`); stop(t, 'bpm'); State = 'stopped'; Reason = 'bpm'; return; }
  say(`sequence ${name} starts at beat ${num(p.from)}, bpm ${num(p.bpm)}`);
  Owner = i; Raw = args;
  segment({ at: t, beat: p.from, bpm: p.bpm });
  State = p.from < 0 ? 'countin' : 'playing'; Reason = '';
  Midi.clockTo(p.clockOut, Math.ceil(p.from * 24), t);
  Play = p; IssuedTo = t; Skip = 0; UnsentTold = false;
  timing(t); fill(t, 'arm');
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
  Midi.retractSeq(t);
  Midi.clockTo(p.clockOut, Math.ceil(beatAt(g, t) * 24), t);
  // From `t`, or from where a fill a full queue stopped before it: what it never issued is issued now, as the edit has it.
  Play = p; UnsentTold = false;
  if (IssuedTo > t) { IssuedTo = t; Skip = 0; }
  timing(t); fill(t, 'edit');
}
function stop(t, reason) {
  if (Owner == null) return;
  const name = Seqs[Owner].name;
  Owner = null;
  say(`sequence ${name} stopped (${reason})`);
  T().batch(() => T().stop(t, name));
  Midi.stopSeq(t);
  State = 'stopped'; Reason = reason; Play = null; Raw = []; RefillDue = PhaseDue = EndedDue = Infinity;
}
// A change's aftermath: the output hears it, the clock's entries follow it, and `exactTransport` is answered again.
function after(before) {
  T().flush(); Midi.flush();
  for (const e of [End, Wake]) { const k = e ? clock.timers.indexOf(e) : -1; if (k >= 0) clock.timers.splice(k, 1); }
  Wake.due = Math.min(RefillDue, PhaseDue);
  End = EndedDue < Infinity ? { due: EndedDue, once: true, hatch: !(Owner != null && Seqs[Owner].ended), i: Infinity, name: '', action: () => end() } : null;
  if (End) clock.timers.push(End);
  if (Wake.due < Infinity) clock.timers.push(Wake);
  if (Object.keys(Readers).length && answerKey() !== before) {
    Answer.due = Math.min(Answer.due, clock.now);
    // After every mutation's `then`, whatever the order the module declared them in.
    const k = Mutations.indexOf(Answer);
    if (k < 0 || k !== Mutations.length - 1) { if (k >= 0) Mutations.splice(k, 1); Mutations.push(Answer); }
    // `exactMIDI`'s answer due at the same time comes after this one, as the runner's (midi.js).
    const m = Mutations.indexOf(Sounds.midiAnswer); if (m >= 0 && m < Mutations.length - 1) Mutations.push(...Mutations.splice(m, 1));
  }
  if (!clock.agent) drive();
}
// The last finite track ended: stopped, then its `ended=` action as a timer's.
function end() {
  const before = answerKey(), q = Owner == null ? null : Seqs[Owner];
  if (!q) return true;
  Owner = null;
  say(`sequence ${q.name} ended`);
  Midi.endSeq(clock.now);
  State = 'stopped'; Reason = 'ended'; Play = null; Raw = []; RefillDue = PhaseDue = EndedDue = Infinity;
  after(before);
  return q.ended ? q.ended() : true;
}
function wake(at) {
  const before = answerKey();
  if (PhaseDue <= at) { PhaseDue = Infinity; State = 'playing'; }
  if (RefillDue <= at) fill(at, 'wake');
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
      else if (s.key !== wasKey) { T().batch(() => T().retract(t)); Midi.retractSeq(t); Midi.endSeq(t); arm(i, t, s.args); } // a restart: Stop, then Start at its new beat 0
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
Sounds.transportAnswer = Answer;
Sounds.gate = () => { Pending = null; if (Seqs.length) Pending = untracked(() => Seqs.map(look)); };
Sounds.stood = () => { const seen = Pending; Pending = null; if (seen) stood(seen); };
// The beat at `t` while a sequence plays, by the segment that holds it; −1 while stopped (a MIDI input's `beat`).
Sounds.beatAt = t => Owner == null || !Segments.length ? -1 : beatAt(Segments.findLast(g => g.at <= t) ?? Segments[0], t);
/** Every `sequence name when gate key=key` (LLP 1125 D2), `[gate, key, args, ended, name]` each, `args` making `play`'s six
 * arguments: all read first, then armed at mount where a gate holds, as the runner's boot does, so a refusal arms none. */
export function sequence(list) {
  for (const [gate, key, args, ended, name] of list) Seqs.push({ gate, key: key || null, args, ended, name, on: false, kt: undefined });
  stood(untracked(() => Seqs.map(look)));
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
