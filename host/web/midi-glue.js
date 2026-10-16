// @ref LLP 1125 D6–D8. The web's MIDI for the runner's queue, one file for
// the wasm glue and the JS target, loaded after first paint by a plan that
// uses MIDI and never under the agent (whose runner answers a request
// itself). Web MIDI's model: `navigator.requestMIDIAccess({sysex: false})`,
// each `MIDIOutput.send(data, timestamp)` and each `MIDIInput`'s
// `midimessage`. Where there is none (WebKit, so every iOS browser) the
// state is `unavailable`.
//
// - **Access.** At install, a permission already granted is taken silently
//   (rulings #4); otherwise nothing is said until the runner asks
//   (`request`), and a refusal is `denied`. `onstatechange` reports the
//   ports again; the runner keeps a port it no longer hears of as
//   disconnected.
// - **Sending.** A message is held on the page's timer until 50 ms before
//   it is due, then handed with `send(data, origin + at)`: Chromium has no
//   `MIDIOutput.clear()`, so holding is how a retraction takes one back. A
//   stall of more than 50 ms makes a message late (D7, a declared limit);
//   the late ones are counted.
// - **Receiving.** Only the inputs the runner opens listen. A message's
//   runner time is its `timeStamp` less the origin; Clock and Active
//   Sensing are left out (clock in is stage 4); what arrives in one frame
//   is one delivery, so the runner coalesces it as one (rulings #2).
const HOLD = 50;

/** Install MIDI. `log`: the host's journal; `origin`: where runner time 0 is
 * on `performance.now()`'s timeline; `deliver(message)`: hands
 * `{ports: …}` or `{messages: […]}` to the runner. A test passes `nav`,
 * `now`, `frame` and `hidden`. */
function installMidi({ log, origin, deliver, nav = globalThis.navigator, now = () => performance.now(), frame, hidden = () => globalThis.document?.hidden === true }) {
  const nextFrame = frame ?? (f => (hidden() || typeof requestAnimationFrame !== 'function' ? setTimeout(f, 16) : requestAnimationFrame(f)));
  let access = null, state = 'prompt', asking = null, timer = null, late = 0, inbox = [], flushing = false;
  /** Messages not yet handed, earliest first: `{id, output, data, at}`. */
  let held = [];
  /** The inputs the runner opened, by id. */
  let open = new Set();
  /** The latest note-on timestamp handed for each output, channel and note, and the latest timestamp handed to each output:
   * a note-off or Stop sent at a boot is stamped no earlier. */
  const sounding = new Map(), latest = new Map();
  /** The note-ons handed in about the last second, by id. The runner takes back only what its own clock has not seen
   * handed, so a retraction that finds one here lost a race with the hold: the note's off is sent instead, after it. */
  const handedOns = new Map();
  const list = map => [...(map?.values?.() ?? [])].map(p => ({ id: String(p.id), name: p.name ?? '', manufacturer: p.manufacturer ?? '', state: p.state === 'disconnected' ? 'disconnected' : 'connected' }));
  function report() {
    deliver({ ports: { state, inputs: list(access?.inputs), outputs: list(access?.outputs) } });
    listen();
  }
  function take(a) {
    access = a; state = 'granted';
    a.onstatechange = () => report();
    report();
  }
  function ask(silent) {
    asking ??= nav.requestMIDIAccess({ sysex: false }).then(take, e => {
      asking = null;
      if (silent) return;
      state = 'denied';
      log(`midi: access denied: ${e?.message ?? e}`);
      deliver({ ports: { state: 'denied' } });
    });
    return asking;
  }
  if (typeof nav?.requestMIDIAccess !== 'function') {
    state = 'unavailable';
    deliver({ ports: { state: 'unavailable' } });
  } else {
    Promise.resolve(nav.permissions?.query({ name: 'midi' })).then(p => { if (p?.state === 'granted') ask(true); }, () => {});
  }
  /** Each opened input listens; the others are let go. */
  function listen() {
    for (const input of access?.inputs?.values?.() ?? []) {
      if (open.has(String(input.id))) {
        if (!input.onmidimessage) input.onmidimessage = e => heard(String(input.id), e);
      } else if (input.onmidimessage) {
        input.onmidimessage = null;
        input.close?.();
      }
    }
  }
  function heard(input, e) {
    const data = [...e.data];
    if (data[0] === 0xf8 || data[0] === 0xfe) return;
    inbox.push([input, e.timeStamp - origin(), data]);
    if (!flushing) { flushing = true; nextFrame(flush); }
  }
  function flush() {
    flushing = false;
    if (inbox.length) deliver({ messages: inbox.splice(0) });
  }
  /** Hand every message within the hold, then wait for the next. */
  function pump() {
    clearTimeout(timer); timer = null;
    const t = now() - origin();
    while (held.length && held[0].at - HOLD <= t) {
      const m = held.shift(), output = access?.outputs?.get?.(m.output);
      if (!output || output.state === 'disconnected') { log(`midi: ${m.output} is not connected; message ${m.id} not sent`); continue; }
      if (t > m.at) late++;
      const stamp = origin() + m.at;
      latest.set(m.output, Math.max(latest.get(m.output) ?? stamp, stamp));
      if ((m.data[0] & 0xf0) === 0x90 && m.data[2] > 0) {
        const key = `${m.output} ${m.data[0] & 0x0f} ${m.data[1]}`;
        sounding.set(key, Math.max(sounding.get(key) ?? stamp, stamp));
        if (handedOns.size >= 64) for (const [id, on] of handedOns) if (on.stamp < now() - 1000) handedOns.delete(id);
        handedOns.set(m.id, { output: m.output, data: m.data, stamp });
      }
      try { output.send(m.data, stamp); } catch (e) { log(`midi: message ${m.id} not sent: ${e.message}`); }
    }
    if (held.length) timer = setTimeout(pump, Math.max(0, held[0].at - HOLD - t));
  }
  /** A new runner: what an earlier one left held is let go, but a note-off or a Stop goes now, so no note is left sounding
   * and no device follows a dead clock, stamped no earlier than its note-on or the output's last message handed (which
   * the browser may still hold). */
  function end() {
    clearTimeout(timer); timer = null;
    for (const m of held.splice(0)) {
      const [s, n, v] = m.data, note = (s & 0xf0) === 0x80 || ((s & 0xf0) === 0x90 && v === 0), stop = s === 0xfc;
      const output = note || stop ? access?.outputs?.get?.(m.output) : null;
      if (!output || output.state === 'disconnected') continue;
      const after = note ? sounding.get(`${m.output} ${s & 0x0f} ${n}`) : latest.get(m.output);
      try { output.send(m.data, Math.max(now(), after ?? -Infinity)); } catch (e) { log(`midi: message ${m.id} not sent: ${e.message}`); }
    }
  }
  const handlers = {
    send(op) {
      const i = held.findIndex(m => m.at > op.at || (m.at === op.at && m.id > op.id));
      held.splice(i < 0 ? held.length : i, 0, { id: op.id, output: op.output, data: op.data, at: op.at });
      pump();
    },
    retract({ id }) {
      const i = held.findIndex(m => m.id === id), on = handedOns.get(id);
      if (i >= 0) held.splice(i, 1);
      else if (on) {
        handedOns.delete(id);
        const output = access?.outputs?.get?.(on.output);
        if (output && output.state !== 'disconnected') try { output.send([0x80 | on.data[0] & 0x0f, on.data[1], 0], Math.max(now(), on.stamp)); } catch (e) { log(`midi: message ${id} not sent: ${e.message}`); }
      }
    },
    open({ inputs }) { open = new Set(inputs.map(String)); listen(); },
    request() { if (state !== 'unavailable') ask(false); },
  };
  return {
    /** The runner's ops, in order; `boot`: a new runner, so what an earlier one left held is forgotten. */
    ops(list, boot) {
      if (boot) end();
      for (const op of list ?? []) handlers[op.op]?.(op);
    },
    /** The output's own account, beside the runner's record. */
    state: () => ({ kind: 'web-midi', state, held: held.length, late }),
  };
}
globalThis.exact ??= {};
globalThis.exact.installMidi = installMidi;
