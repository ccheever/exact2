// Clock in's follower on the JS target (LLP 1125 D9): runner/src/transport/clock.rs's `Follow`, line for line, its
// arithmetic in the same order so the tempo is the same double; read it beside it. transport.js holds one while the
// playing sequence follows an input's MIDI clock, and does what it hears.
// - Tempo: a window of the last WINDOW tick stamps, `(n − 1)` ticks over its span, taken only on a packet-leading tick
//   once MIN_TICKS are in it; a stamp earlier than the last or GAP after it empties the window. Before the first
//   estimate, the prior (`bpm=`).
// - Position counts ticks: the kth tick after Start is beat `from + k / 24`. Start puts `from` at the tick it predicts
//   next (never before Start), else at Start's own time; Continue resumes at Song Position since the last Stop, else
//   where that Stop held it, else beat 0.
// - Each run of ticks ends in an anchor (the last tick's time, its beat, the tempo), at each transport message and at
//   the delivery's end.
export const WINDOW = 48, MIN_TICKS = 13, GAP = 500, BATCH = 50, LOOKAHEAD = BATCH + 50, DRIFT = 2;

/** A follower of `input`, `prior` its tempo before an estimate, Start putting beat `from` at the next tick. */
export const follower = (input, prior, from) => ({ input, stamps: [], estimate: null, prior, from, running: false, base: 0, next: 0, spp: null, held: null, lastTick: null, ticks: 0, seen: null });
export const tempo = f => f.estimate ?? f.prior;

// One tick's stamp into the window (euxy's `engine.ts`).
function stamp(f, at) {
  const prev = f.stamps.at(-1);
  if (prev != null && (at < prev || at - prev > GAP)) f.stamps.length = 0;
  f.stamps.push(at);
  if (f.stamps.length > WINDOW) {
    let removed = f.stamps.shift();
    // The window's first tick stays packet-leading.
    while (f.stamps.length > 1 && f.stamps[0] === removed) removed = f.stamps.shift();
  }
  const n = f.stamps.length, elapsed = at - f.stamps[0], leading = prev == null || at > prev;
  if (leading && n >= MIN_TICKS && elapsed > 0) f.estimate = 60000 * (n - 1) / (elapsed * 24);
}
const anchor = f => ({ at: f.lastTick ?? 0, beat: f.base + (f.next - 1) / 24, bpm: tempo(f) });

/** One delivery's clock messages, `[stamp, bytes]` each in arrival order: what they said, `{ start: {at, beat, cont} }`,
 * `{ stop: true }` or `{ anchor: segment }` each, in order; `say` journals Song Position ignored while playing. */
export function feed(f, msgs, name, say) {
  const out = [];
  let run = false;
  for (const [at, data] of msgs) {
    const status = data[0];
    if (status === 0xF8) { stamp(f, at); f.ticks++; if (f.running) { f.next += 1; run = true; } f.lastTick = at; continue; }
    if (run && status >= 0xFA && status <= 0xFC) { run = false; out.push({ anchor: anchor(f) }); f.seen = anchor(f); }
    if (status === 0xFA || status === 0xFB) {
      const cont = status === 0xFB, interval = 60000 / (24 * tempo(f));
      const start = f.lastTick != null && at >= f.lastTick && at - f.lastTick <= GAP ? Math.max(f.lastTick + interval, at) : at;
      f.base = cont ? f.spp ?? f.held ?? 0 : f.from;
      f.next = 0; f.running = true; f.spp = null; f.held = null;
      out.push({ start: { at: start, beat: f.base, cont } });
      f.seen = { at: start, beat: f.base, bpm: tempo(f) };
    } else if (status === 0xFC && f.running) {
      f.held = f.base + f.next / 24; f.running = false; f.spp = null;
      out.push({ stop: true });
    } else if (status === 0xF2 && data.length === 3) {
      if (f.running) say(`sequence ${name}: song position ignored while playing`);
      else f.spp = ((data[2] << 7) | data[1]) / 4;
    }
  }
  if (run) { out.push({ anchor: anchor(f) }); f.seen = anchor(f); }
  return out;
}
