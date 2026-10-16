// MIDI on the JS target (LLP 1125 D6–D8): the runner's (runner/src/midi.rs,
// runner/src/runner/midi.rs, runner/src/agent/midi.rs) with the same rules,
// ids, order, journal lines and `state.midi`; read it beside them. Imported
// by an app's module for a plan that keeps a MIDI queue (`uses::keeps_midi`:
// it asks for ports, hears an input, or declares a sequence, whose notes and
// clock go on the queue through transport.js). Outside the agent the ops go
// to the web host's midi-glue.js, fetched after the first frame for a plan
// that uses MIDI; under the agent nothing is sent and a request is answered
// from the drive's `prefer midi.*`.
// - `sendMIDI` and `requestMIDIAccess` are the runtime's own (rt.js
//   `Sounds.own`), applied once their commit stood, after the voice table's
//   (`Sounds.midiApply`).
// - `midimessage` (rt.js `onMIDI`): every connected element whose
//   `data-exact-midiinputs` lists the input hears each message, in document
//   order, a commit each.
import { journal, clock, Sounds, Mutations, Resources, R, data, commit } from './rt.js';
import { plain } from './format.js';

const HOLD = 50, QUEUED = 1024, RECORD = 1024, SHOWN = 64;
const say = line => journal.push(`t=${clock.now} ${line}`);
const num = n => !Number.isFinite(n) ? 'null' : n < 0 ? `-${plain(-n)}` : plain(n); // the runner's `num`
const hex = d => d.map(b => b.toString(16).padStart(2, '0')).join(' ');
const ACCESS = ['prompt', 'granted', 'denied', 'unavailable'];

// A message's length from its status byte, or why it has none (midi.rs `length`).
function length(s) {
  if (s >= 0x80 && s <= 0xBF || s >= 0xE0 && s <= 0xEF || s === 0xF2) return 3;
  if (s >= 0xC0 && s <= 0xDF || s === 0xF1 || s === 0xF3) return 2;
  if ([0xF6, 0xF8, 0xFA, 0xFB, 0xFC, 0xFE, 0xFF].includes(s)) return 1;
  if (s === 0xF0 || s === 0xF7) return 'SysEx is not sent';
  if ([0xF4, 0xF5, 0xF9, 0xFD].includes(s)) return `${hex([s])} is undefined`;
  return `${hex([s])} is a data byte where a status belongs`;
}
// Whole messages, or `{ why }` (midi.rs `split`).
function split(bytes) {
  if (!bytes.length) return { why: 'no message' };
  const out = [];
  for (let i = 0; i < bytes.length;) {
    const n = length(bytes[i]);
    if (typeof n === 'string') return { why: `byte ${i + 1}: ${n}` };
    if (i + n > bytes.length) return { why: `the message at byte ${i + 1} needs ${n} bytes` };
    for (let k = i + 1; k < i + n; k++) if (bytes[k] >= 0x80) return { why: `byte ${k + 1} is ${hex([bytes[k]])}, not a data byte` };
    out.push(bytes.slice(i, i + n)); i += n;
  }
  return out;
}
// `sendMIDI`'s data: numbers, each a byte (midi.rs `messages`).
function messages(data) {
  for (const v of data) {
    if (typeof v !== 'number') return { why: 'a byte is not a number' };
    if (!(v % 1 === 0 && v >= 0 && v <= 255)) return { why: `${num(v)} is not a byte` };
  }
  return split(data);
}
const noteOff = d => d.length === 3 && ((d[0] & 0xF0) === 0x80 || (d[0] & 0xF0) === 0x90 && d[2] === 0);
const noteKey = d => d.length === 3 && ((d[0] & 0xF0) === 0x80 || (d[0] & 0xF0) === 0x90) ? `${d[0] & 0x0F},${d[1]}` : null;
const waits = (s, t) => s.by == null && s.at - HOLD > t, handed = (s, t) => s.by == null && s.at - HOLD <= t;
const exempt = s => s.tick != null || noteOff(s.data) || s.data.length === 1 && s.data[0] === 0xFC;
const offOfHanded = (s, t) => s.onAt != null && s.onAt - HOLD <= t;
const stateOf = (s, t) => s.by != null ? 'retracted' : s.at - HOLD > t ? 'waiting' : 'handed';

let Ports = { access: 'prompt', inputs: [], outputs: [] }, Granted = false, Uses = false;
let Queue = [], Next = 0, Received = [], Heard = 0, Ops = [], Open = [], Clock = null, Ticks = 0, Played = null, ClockFloor = -Infinity;
let Preferred = { access: 'granted', inputs: [], outputs: [] }, Readers = {}, Glue = null, Output = null, Boot = true;
const Listening = () => Sounds.midi ?? []; // [element, handler] pairs from rt.js `onMIDI`, registered as the view is built
const Answer = { name: 'midi', requests: 0, due: Infinity, next: Infinity, then: () => { Answer.due = Infinity; for (const r of Resources) if (r.source === 'exactMIDI') R(r); } };

const portsEqual = (a, b) => a.access === b.access && ['inputs', 'outputs'].every(k => a[k].length === b[k].length
  && a[k].every((p, i) => ['id', 'name', 'manufacturer', 'connected'].every(f => p[f] === b[k][i][f])));
function merged(reported) {
  const out = { access: reported.access, inputs: [...reported.inputs], outputs: [...reported.outputs] };
  for (const k of ['inputs', 'outputs']) for (const p of Ports[k]) if (!out[k].some(q => q.id === p.id)) out[k].push({ ...p, connected: false });
  return out;
}
const connectedOutput = id => Ports.access === 'granted' && Ports.outputs.some(p => p.id === id && p.connected);
const connectedInput = id => Ports.access === 'granted' && Ports.inputs.some(p => p.id === id && p.connected);
function setPorts(reported) {
  if (!Granted) return false;
  const ports = merged(reported);
  if (portsEqual(ports, Ports)) return false;
  const count = l => l.filter(p => p.connected).length;
  say(`midi ports: ${ports.access}, inputs ${count(ports.inputs)} of ${ports.inputs.length}, outputs ${count(ports.outputs)} of ${ports.outputs.length} connected`);
  Ports = ports;
  return true;
}
const answered = () => Preferred.access === 'granted' ? { ...Preferred } : { access: Preferred.access, inputs: [], outputs: [] };
// The readers asked again in one commit (the runner's `recommit`), when there are any.
const again = why => Resources.some(r => r.source === 'exactMIDI') ? commit(() => { for (const r of Resources) if (r.source === 'exactMIDI') R(r); }, why) : true;
const waiting = t => Queue.filter(s => waits(s, t) && !exempt(s)).length;

function push(m, t, note = '') {
  const id = ++Next;
  if (m.tick == null) say(`midi ${id} to ${m.output} ${hex(m.data)} at ${num(m.at)}${note}`); else Ticks++;
  Ops.push({ op: 'send', id, output: m.output, data: m.data, at: m.at });
  Queue.push({ id, output: m.output, data: m.data, at: m.at, seq: m.seq, by: null, stepAt: m.stepAt, tick: m.tick ?? null, onAt: m.onAt ?? null });
  trim(t);
  return id;
}
// Clock out's Start taken back is sent again before its next tick.
function retract(s, by) { s.by = by; Ops.push({ op: 'retract', id: s.id }); if (Clock && s.data.length === 1 && s.data[0] === 0xFA && s.output === Clock.output) Clock.started = false; }
function trim(t) {
  for (let i = 0; i < Queue.length;) {
    const s = Queue[i];
    if (s.tick != null && handed(s, t)) { if (Played == null || Played[1] <= s.at) Played = [s.tick, s.at]; Queue.splice(i, 1); }
    else if (s.tick != null && s.by != null) Queue.splice(i, 1);
    else i++;
  }
  while (Queue.length > RECORD) { const i = Queue.findIndex(s => !waits(s, t)); if (i < 0) return; Queue.splice(i, 1); }
}
function send(args, t) {
  const [output, bytes, at0] = args;
  if (typeof output !== 'string') return say('midi dropped: the output is not a string (sendMIDI)');
  const data = Array.isArray(bytes) ? messages(bytes) : { why: 'the data is not a list' };
  if (data.why) return say(`midi dropped: ${data.why} (sendMIDI to ${output})`);
  const asked = at0 == null ? t : typeof at0 === 'number' ? at0 : NaN;
  if (!Number.isFinite(asked)) return say(`midi dropped: at is not a number (sendMIDI to ${output})`);
  if (!connectedOutput(output)) return say(`midi dropped: ${Ports.access !== 'granted' ? `access is ${Ports.access}` : `${output} is not a connected output`} (sendMIDI)`);
  const at = Math.max(asked, t), note = asked < t ? ` (asked ${num(asked)})` : '';
  for (const d of data) {
    if (!(noteOff(d) || d.length === 1 && d[0] === 0xFC) && waiting(t) >= QUEUED) { say(`midi dropped: ${QUEUED} messages wait (sendMIDI to ${output} ${hex(d)})`); continue; }
    push({ output, data: d, at, stepAt: at, seq: false }, t, note);
  }
}
function request(t) {
  if (!Granted) return say('midi access: denied, the grants name no device.midi (requestMIDIAccess)');
  if (clock.agent) {
    say(`midi access: ${Preferred.access} (agent)`);
    if (setPorts(answered())) { Answer.due = Math.min(Answer.due, t); placeAnswer(); }
  } else Ops.push({ op: 'request' });
}
// The `exactMIDI` answer after every mutation's `then` and after the transport's own at the same time (the runner's order).
function placeAnswer() {
  const k = Mutations.indexOf(Answer); if (k >= 0) Mutations.splice(k, 1);
  const tr = Mutations.indexOf(Sounds.transportAnswer);
  if (tr >= 0) Mutations.splice(tr + 1, 0, Answer); else Mutations.push(Answer);
}
function stopClock(t) {
  if (!Clock) return;
  const out = Clock.output;
  for (const s of Queue) if ((s.tick != null || s.data.length === 1 && s.data[0] === 0xFA && s.output === out) && waits(s, t)) retract(s, 'cancelled');
  const c = Clock; Clock = null;
  if (!c.started) return;
  let last = t;
  for (const s of Queue) if (s.tick != null && handed(s, t) && s.output === c.output) last = Math.max(last, s.at);
  if (Played) last = Math.max(last, Played[1]);
  ClockFloor = last; // the next clock starts after this Stop
  push({ output: c.output, data: [0xFC], at: last, stepAt: last, seq: true }, t);
}
// A `reload` (the runner's `Midi::reload`): the sequence's waiting messages go as at a stop, every other waiting note-off is
// handed now, after every note-on handed (`t + HOLD`), and the rest is taken back.
function reload(t) {
  Midi.stopSeq(t);
  let n = 0; const moved = [];
  for (const s of [...Queue]) if (waits(s, t)) { if (noteOff(s.data)) moved.push(s); retract(s, 'cancelled'); n++; }
  if (n) say(`midi retracted: ${n} (reload)`);
  for (const s of moved) push({ output: s.output, data: s.data, at: t + HOLD, stepAt: t, seq: s.seq }, t);
}

/** The sequence's side of the queue (transport.js), as midi.rs's `Midi` methods. */
export const Midi = {
  canSend: connectedOutput, access: () => Ports.access,
  nextHanded: t => Queue.filter(s => waits(s, t) && !exempt(s)).reduce((m, s) => Math.min(m, s.at - HOLD), Infinity),
  note(output, on, onAt, offAt, stepAt, t) {
    if (onAt - HOLD > t && waiting(t) >= QUEUED) return 'full'; // a note due within the hold takes no waiting place
    const key = noteKey(on);
    const moved = Queue.filter(s => s.seq && waits(s, t) && s.output === output && noteOff(s.data) && noteKey(s.data) === key && s.at > onAt && s.onAt != null && s.onAt <= onAt);
    if (moved.length) say(`midi retracted: ${moved.length} (overlap)`);
    for (const s of moved) retract(s, 'overlap');
    for (const s of moved) push({ output, data: s.data, at: onAt, stepAt: s.stepAt, seq: true, onAt: s.onAt }, t);
    push({ output, data: on, at: onAt, stepAt, seq: true }, t);
    push({ output, data: [0x80 | (on[0] & 0x0F), on[1], 0], at: offAt, stepAt, seq: true, onAt }, t);
    return 'done';
  },
  retractSeq(t) {
    let n = 0;
    for (const s of [...Queue]) if (s.seq && waits(s, t) && s.stepAt >= t && !offOfHanded(s, t)) { if (s.tick != null && Clock) Clock.next = Math.min(Clock.next, s.tick); else n++; retract(s, 'edit'); }
    if (n) say(`midi retracted: ${n} (edit)`);
  },
  stopSeq(t) {
    let n = 0; const moved = [];
    for (const s of [...Queue]) { if (!(s.seq && waits(s, t))) continue; if (offOfHanded(s, t)) moved.push(s); else if (s.tick == null) n++; retract(s, 'cancelled'); }
    if (n + moved.length) say(`midi retracted: ${n + moved.length} (cancelled)`);
    for (const s of moved) push({ output: s.output, data: s.data, at: Math.max(t, s.onAt ?? t), stepAt: s.stepAt, seq: true, onAt: s.onAt }, t);
    stopClock(t);
  },
  endSeq: stopClock,
  clockTo(output, first, t) {
    if (Clock && Clock.output === output) return;
    const next = Clock ? Clock.next : first;
    stopClock(t);
    if (output) Clock = { output, next, started: false };
  },
  ticks(time, to, end, late, t) {
    const c = Clock; if (!c) return;
    const can = connectedOutput(c.output);
    let n = 0, last = NaN;
    for (;;) {
      const k = c.next, at = time(k / 24);
      if (!(at < to) || end != null && k / 24 >= end) break;
      c.next += 1;
      if (!can || at < late || at < ClockFloor) continue;
      if (!c.started && k >= 0) { c.started = true; push({ output: c.output, data: [0xFA], at, stepAt: at, seq: true }, t); }
      push({ output: c.output, data: [0xF8], at, stepAt: at, seq: true, tick: k }, t);
      n++; last = at;
    }
    if (n) say(`midi clock: ${n} ticks to ${c.output} through ${num(last)}`);
  },
  flush: () => flush(),
};

// The elements that hear `input` now, in document order (the runner's `midi_listeners`).
function listeners(input) {
  const out = [];
  for (const pair of Listening()) {
    const [e] = pair;
    if (!e.isConnected) continue;
    const v = e.getAttribute('data-exact-midiinputs');
    if (v == null) continue;
    const inputs = v.split('\n').filter(Boolean);
    if (input == null || inputs.includes(input)) out.push([pair, inputs]);
  }
  return out.sort(([[a]], [[b]]) => a === b ? 0 : a.compareDocumentPosition(b) & 4 ? -1 : 1);
}
function inputsHeard() {
  const open = [];
  for (const [, inputs] of listeners(null)) for (const i of inputs) if (!open.includes(i)) open.push(i);
  return open;
}
// One host delivery, coalesced (midi.rs `coalesce`).
function coalesce(batch) {
  const out = [], latest = [], coalesced = [];
  for (const m of batch) {
    const parts = split(m.data);
    if (parts.why) { say(`midi in dropped: ${parts.why} (${m.input})`); continue; }
    if (parts.length !== 1) { say(`midi in dropped: one message at a time (${m.input})`); continue; }
    const status = m.data[0];
    if (status === 0xF8 || status === 0xFE) continue;
    const kind = status & 0xF0, ch = status & 0x0F;
    const key = kind === 0xB0 || kind === 0xA0 ? [m.input, kind, ch, m.data[1]] : kind === 0xE0 || kind === 0xD0 ? [m.input, kind, ch, 0] : null;
    if (key) {
      const slot = latest.findIndex(([k]) => k.every((x, i) => x === key[i]));
      if (slot >= 0) { out[latest[slot][1]] = null; latest[slot][1] = out.length; const c = coalesced.find(([i]) => i === m.input); if (c) c[1]++; else coalesced.push([m.input, 1]); }
      else latest.push([key, out.length]);
    } else if (status < 0xF0) { for (let i = latest.length - 1; i >= 0; i--) if (latest[i][0][0] === m.input && latest[i][0][2] === ch) latest.splice(i, 1); }
    else { for (let i = latest.length - 1; i >= 0; i--) if (latest[i][0][0] === m.input) latest.splice(i, 1); }
    out.push(m);
  }
  for (const [input, n] of coalesced) say(`midi coalesced: ${n} (${input})`);
  return out.filter(Boolean);
}
// One host delivery of input messages, each to every listener, a commit each (the runner's `midi_input`); a refusal does
// not stop the rest, and the first is returned.
function hear(batch) {
  let first = null;
  for (const m of coalesce(batch)) {
    if (!connectedInput(m.input)) { say(`midi in dropped: ${m.input} is not a connected input`); continue; }
    const beat = Sounds.beatAt ? Sounds.beatAt(m.timeStamp) : -1;
    say(`midi in ${m.input} ${hex(m.data)} at ${num(m.timeStamp)} beat ${num(beat)}`);
    Heard++; Received.push({ input: m.input, data: m.data, timeStamp: m.timeStamp, beat, at: clock.now }); if (Received.length > RECORD) Received.shift();
    const record = [m.input, m.data, m.timeStamp, beat];
    for (const [pair] of listeners(m.input)) if (pair[0].isConnected && pair[1](record) !== true) first ??= (journal.findLast(l => / refused /.test(l)) ?? 'refused').replace(/^t=\S+ /, '');
  }
  return first;
}
// A host's message (midi-glue.js), as the runner's `midi_host`.
function deliver(msg) {
  if (msg?.ports) {
    const p = msg.ports, list = k => (p[k] ?? []).map(q => ({ id: String(q.id), name: q.name ?? '', manufacturer: q.manufacturer ?? '', connected: q.state !== 'disconnected' }));
    if (!ACCESS.includes(p.state)) return;
    if (setPorts({ access: p.state, inputs: list('inputs'), outputs: list('outputs') })) again('midi');
  } else if (Array.isArray(msg?.messages)) hear(msg.messages.map(([input, timeStamp, data]) => ({ input, timeStamp, data })));
  flush();
}
// The agent's `prefer midi.*` (the runner's `midi_prefer`): the reply, or `{ error }`.
function prefer(m) {
  if (m.state != null && !ACCESS.includes(m.state)) return { error: 'prefer midi.state: prompt, granted, denied or unavailable' };
  if (m.state != null) Preferred.access = m.state;
  const named = k => m[k].map(n => ({ id: n, name: n, manufacturer: '', connected: true }));
  if (Array.isArray(m.inputs)) Preferred.inputs = named('inputs');
  if (Array.isArray(m.outputs)) Preferred.outputs = named('outputs');
  if ((m.state != null || Ports.access === 'granted') && setPorts(answered())) again('midi');
  return reply();
}
// `tap midi "<input>" 90 3c 64`: one delivery of the messages the bytes make (the runner's `midi_tap`).
function tap(req) {
  const input = typeof req.input === 'string' ? req.input : '', data = Array.isArray(req.data) && req.data.length && req.data.every(b => Number.isInteger(b) && b >= 0 && b < 256) ? req.data : null;
  if (!data) return { error: 'tap midi: the bytes are whole numbers 0–255' };
  const parts = split(data);
  if (parts.why) return { error: `tap midi: ${parts.why}` };
  if (!connectedInput(input)) return { error: `tap midi: ${Ports.access === 'granted' ? `${JSON.stringify(input)} is not a connected input (prefer midi.inputs)` : `MIDI access is ${Ports.access} (requestMIDIAccess, prefer midi.state)`}` };
  if (!listeners(input).length) return { error: `tap midi: no mounted element with a midimessage handler lists ${JSON.stringify(input)} in midiinputs` };
  const refused = hear(parts.map(d => ({ input, timeStamp: clock.now, data: d })));
  return refused ? { error: `tap midi: ${refused}` } : reply();
}
const reply = () => ({ midi: { state: Ports.access, inputs: Ports.inputs.map(p => p.id), outputs: Ports.outputs.map(p => p.id) } });

// The output: none under the agent; else the web host's glue, fetched after the first frame (a plan that uses MIDI).
function load() {
  if (Glue || clock.agent || !Uses || typeof requestAnimationFrame !== 'function') return;
  Glue = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import('./midi-glue.js'))
    .then(() => { Output = globalThis.exact.installMidi({ log: say, origin: () => clock.start, deliver }); flush(); })
    .catch(err => say(`midi: unavailable: ${err.message}`));
}
function flush() {
  const open = inputsHeard();
  if (open.join('\n') !== Open.join('\n') || open.length !== Open.length) { Ops.push({ op: 'open', inputs: open }); Open = open; }
  if (clock.agent) Ops = [];
  else if (Output) { if (Ops.length || Boot) { Output.ops(Ops.splice(0), Boot); Boot = false; } }
  else load();
}
// One commit's commands once it stood, after the voice table's (the runner's `apply_sounds`).
function apply(commands) {
  if (commands.some(([name]) => name === 'reload')) reload(clock.now);
  for (const [name, args] of commands) {
    if (name === 'sendMIDI') send(args, clock.now);
    else if (name === 'requestMIDIAccess') request(clock.now);
  }
  flush();
}
/** `readers`: resource name → [its fields, {a port list's field: its port fields}]; `uses`: the plan uses MIDI (`uses::uses_midi`). */
export function midi(readers, uses) {
  Readers = readers; Uses = uses;
  // The grants are the data module's, which may load after the first frame: then the readers are asked again.
  const granted = () => /^\s*device\.midi\s/m.test(data.grants ?? '');
  Granted = granted();
  Ports = { access: Granted ? 'prompt' : 'denied', inputs: [], outputs: [] };
  if (data.grants == null) data.ready(() => { if (!Granted && granted()) { Granted = true; Ports = { access: 'prompt', inputs: [], outputs: [] }; again('midi'); } });
  Sounds.midiApply = apply; Sounds.midiAnswer = Answer;
  (data.reserved ??= {}).exactMIDI = (_, a, name) => {
    const [fields, ports] = Readers[name];
    const port = (p, keys) => keys.map(k => k === 'state' ? (p.connected ? 'connected' : 'disconnected') : p[k]);
    return fields.map(f => f === 'state' ? Ports.access : Ports[f].map(p => port(p, ports[f])));
  };
  // `state.midi`, as the runner's (runner/src/agent/midi.rs).
  (globalThis.exact ??= {}).midi = {
    prefer, tap, deliver,
    state: all => {
      const t = clock.now, sent = Queue.filter(s => s.tick == null);
      const port = p => ({ id: p.id, name: p.name, manufacturer: p.manufacturer, state: p.connected ? 'connected' : 'disconnected' });
      return { state: Ports.access, inputs: Ports.inputs.map(port), outputs: Ports.outputs.map(port), open: inputsHeard(), queued: waiting(t),
        sent: sent.slice(all ? 0 : Math.max(0, sent.length - SHOWN)).map(s => ({ id: s.id, output: s.output, data: hex(s.data), at: s.at, state: stateOf(s, t), by: s.by, seq: s.seq })),
        recorded: Next - Ticks, received: Received.slice(-SHOWN).map(r => ({ input: r.input, data: hex(r.data), timeStamp: r.timeStamp, beat: r.beat, at: r.at })), heard: Heard,
        clock: { output: Clock ? Clock.output : null, ticks: Ticks } };
    },
  };
  if (Uses && typeof requestAnimationFrame === 'function') requestAnimationFrame(() => setTimeout(load));
}
