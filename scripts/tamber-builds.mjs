// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): the dev loop's
// running development builds, registered, reporting up and taking commands
// down, for the design server's clients.
//
// - **Registration.** Each dev stream a build opens (`/__dev`; a Linux build,
//   which holds no stream, `GET /__dev/hello` and then a poll) is given a
//   nonce in its hello. The build answers with one `POST /__dev/report`
//   `{nonce, kind: "register", id, host, capabilities, plan, design}`; `id`
//   is the build's own, stable across its reconnects (a tab's, a process's),
//   so its pins are replayed when it registers again. A report whose nonce
//   is no open stream's is refused: the dev loop takes other POSTs with no
//   token.
// - **Reports up**, each with its nonce: `status` (on connect and on any
//   change: the effective `scheme`, `contrast`, `mode` and `rootFontSize`,
//   the `plan` and `design` digests, the pin `revision` applied),
//   `unsupported` (a command or batch it cannot apply, by name), and
//   `selected` (LLP 1122 D5; its sender is T2b's).
// - **Commands down**, each a frame whose `kind` a host dispatches before any
//   generation handling: `pin` `{revision, pin}` here (`{scheme?, contrast?,
//   mode?: {axis: value}, rootFontSize?}`, or null to hand them back);
//   `inspect`, `outline` and `hold` are reserved (T2b, stage 8), which a host
//   answers `unsupported`.
// - **Ownership.** A pin belongs to the client that set it. When that
//   client's last subscription closes, its pins are cleared on every build.
//   A build that registers again gets its pins replayed.

import { randomBytes } from 'node:crypto';

const ID = /^[\w.:-]{1,64}$/;
// A polled stream unpolled this long is gone; at most this many are open,
// and this many builds remembered (the oldest gone first): the routes take
// no token, so what they keep is bounded.
// A Linux build polls twice a second: unpolled 5 s, it is disconnected.
const POLLED_MS = 5_000, POLLED = 64, BUILDS = 256;
const KINDS = new Set(['register', 'status', 'unsupported', 'selected']);
const RESERVED = new Set(['inspect', 'outline', 'hold']);

/** A pin as a client gives it: an object of the facts it pins, or null.
 * The refusal's text, or null. */
function pinRefused(pin) {
  if (pin === null) return null;
  if (typeof pin !== 'object' || Array.isArray(pin)) return '`pin` is an object of facts, or null';
  for (const key of Object.keys(pin)) if (!['scheme', 'contrast', 'mode', 'rootFontSize'].includes(key)) return `\`pin\` names no fact \`${key}\``;
  if (pin.scheme !== undefined && !['light', 'dark'].includes(pin.scheme)) return '`scheme` is light or dark';
  if (pin.contrast !== undefined && !['more', 'less', 'no-preference'].includes(pin.contrast)) return '`contrast` is more, less or no-preference';
  if (pin.mode !== undefined && (typeof pin.mode !== 'object' || pin.mode === null || Array.isArray(pin.mode) || Object.entries(pin.mode).some(([a, v]) => typeof v !== 'string' || /[,=]/.test(a + v)))) return '`mode` is {axis: value}, with no `,` or `=`';
  if (pin.rootFontSize !== undefined && !(Number.isFinite(pin.rootFontSize) && pin.rootFontSize >= 4 && pin.rootFontSize <= 64)) return '`rootFontSize` is px, 4 to 64';
  return null;
}

/** One dev loop's builds. `send(event)` tells the design server's clients
 * (`builds`, `build-status`, `unsupported`, `selected`). */
export function buildChannel({ send = () => {}, log = () => {} } = {}) {
  const streams = new Map(); // nonce -> { write(frame), build: id | null, queue: [] }
  const builds = new Map(); // id -> { id, host, capabilities, plan, design, status, nonce }
  const pins = new Map(); // id -> { client, pin, revision }
  let revision = 0, greeting = () => null;
  // A bearer credential for the report route: unguessable.
  const nonce = () => randomBytes(16).toString('hex');
  /** Polled streams past their time, and the oldest over the cap, closed. */
  const sweep = () => {
    const polled = [...streams].filter(([, s]) => s.seen !== undefined);
    for (const [k, s] of polled) if (Date.now() - s.seen > POLLED_MS) close(k);
    const open = polled.filter(([k]) => streams.has(k)).sort((a, b) => a[1].seen - b[1].seen);
    for (const [k] of open.slice(0, Math.max(0, open.length - POLLED))) close(k);
  };
  const close = (k) => { const s = streams.get(k); streams.delete(k); if (s?.build) announce(); };
  const shown = b => ({ id: b.id, host: b.host, capabilities: b.capabilities, plan: b.plan, design: b.design, status: b.status ?? null, pin: pins.get(b.id)?.pin ?? null });
  const list = () => [...builds.values()].filter(b => streams.has(b.nonce)).map(shown);
  const announce = () => send({ event: 'builds', builds: list() });
  const frame = (id, f) => { const b = builds.get(id); const s = b && streams.get(b.nonce); if (s) s.write(f); };
  const pinFrame = id => ({ kind: 'pin', revision: pins.get(id)?.revision ?? revision, pin: pins.get(id)?.pin ?? null });

  return {
    /** A build's stream opened; `write(frame)` sends it one. Its nonce, for
     * its hello; `close()` when the stream ends. */
    open(write) {
      const n = nonce();
      streams.set(n, { write, build: null });
      return { nonce: n, close: () => close(n) };
    },
    /** A stream for a build that polls (Linux, the resident loop's relay):
     * frames queue until it polls; unpolled for 30 s it is gone. */
    openPolled() {
      sweep();
      const queue = [];
      const opened = this.open(f => queue.push(f));
      const s = streams.get(opened.nonce);
      s.seen = Date.now();
      s.poll = () => { s.seen = Date.now(); return queue.splice(0); };
      // The design to run, as a stream's hello says it (a notice).
      const hello = greeting();
      if (hello) queue.push(hello);
      return opened;
    },
    /** A polled build's frames since its last poll; null for a nonce no
     * stream holds. */
    poll(n) {
      sweep();
      return streams.get(n)?.poll?.() ?? null;
    },
    /** `POST /__dev/report`'s body: `{status, body}`. */
    report(text) {
      let r;
      try { r = JSON.parse(text); } catch { return { status: 400, body: { message: 'a report is a JSON object' } }; }
      sweep();
      const s = typeof r?.nonce === 'string' && streams.get(r.nonce);
      if (!s) return { status: 403, body: { message: 'no open dev stream holds this nonce' } };
      if (!KINDS.has(r.kind)) return { status: 400, body: { message: `a report's kind is one of ${[...KINDS].join(', ')}` } };
      if (r.kind === 'register') {
        if (typeof r.id !== 'string' || !ID.test(r.id)) return { status: 400, body: { message: '`id` is the build\'s own, 1–64 of [A-Za-z0-9_.:-]' } };
        if (typeof r.host !== 'string' || !Array.isArray(r.capabilities)) return { status: 400, body: { message: 'a registration names its `host` and its `capabilities`' } };
        // The same build on a new stream: the old one's is no longer it.
        for (const [k, other] of streams) if (other !== s && other.build === r.id) other.build = null;
        s.build = r.id;
        builds.delete(r.id);
        builds.set(r.id, { id: r.id, host: r.host, capabilities: r.capabilities.filter(c => typeof c === 'string'), plan: r.plan ?? null, design: r.design ?? null, status: null, nonce: r.nonce });
        // The oldest builds with no open stream forgotten, over the cap.
        for (const [id, b] of builds) { if (builds.size <= BUILDS) break; if (!streams.has(b.nonce)) { builds.delete(id); pins.delete(id); } }
        log(`build registered: ${r.id} (${r.host})`);
        // Its pin, or none, as the server holds it now (a reload is a new
        // document; a pin cleared while it was away is cleared there too).
        s.write(pinFrame(r.id));
        announce();
        return { status: 200, body: { id: r.id } };
      }
      const b = s.build && builds.get(s.build);
      if (!b) return { status: 409, body: { message: 'register first' } };
      if (r.kind === 'status') {
        b.status = { scheme: r.scheme ?? null, contrast: r.contrast ?? null, mode: r.mode ?? null, rootFontSize: r.rootFontSize ?? null, revision: r.revision ?? null };
        if (typeof r.plan === 'string') b.plan = r.plan;
        if (typeof r.design === 'string') b.design = r.design;
        send({ event: 'build-status', build: shown(b) });
      } else if (r.kind === 'unsupported') {
        log(`build ${b.id}: unsupported ${r.what ?? '?'}`);
        send({ event: 'unsupported', build: b.id, what: String(r.what ?? ''), detail: r.detail ?? null });
      } else {
        send({ event: 'selected', build: b.id, selected: r.selected ?? null });
      }
      return { status: 204, body: null };
    },
    /** The registered builds with an open stream. */
    list,
    /** A live-table frame (a batch, a snapshot, a notice) for every polled
     * stream: the loop's server is the one source of batches, and a stream
     * the loop serves itself is written to by the loop. */
    broadcast(frame) {
      sweep();
      for (const s of streams.values()) if (s.seen !== undefined) s.write(frame);
    },
    /** The clients that hold a pin. */
    owners: () => [...new Set([...pins.values()].map(p => p.client))],
    /** What a polled stream is told when it opens (`() => frame | null`). */
    greet(fn) { greeting = fn; },
    /** `client` pins `pin` on build `id`, or every build (`id` null); null
     * hands them back. The refusal, or the builds it went to. */
    pin(client, id, pin) {
      const why = pinRefused(pin);
      if (why) return { error: why };
      const targets = id == null ? list().map(b => b.id) : [id];
      if (id != null && !list().some(b => b.id === id)) return { error: `no registered build \`${id}\`` };
      revision += 1;
      for (const t of targets) {
        if (pin === null) pins.delete(t); else pins.set(t, { client, pin, revision });
        frame(t, { kind: 'pin', revision, pin });
      }
      // Commands are journaled with their client (LLP 1122 D4).
      log(`tamber: ${client} pinned ${JSON.stringify(pin)} on ${targets.join(', ') || 'no build'} (revision ${revision})`);
      announce();
      return { builds: targets, revision };
    },
    /** A command a host does not speak yet (`inspect`, `outline`, `hold`):
     * reserved, so a client is told, not ignored. */
    reserved: kind => RESERVED.has(kind),
    /** Client `client` is gone: its pins are cleared on every build. */
    clear(client) {
      const mine = [...pins].filter(([, p]) => p.client === client).map(([id]) => id);
      if (!mine.length) return;
      revision += 1;
      for (const id of mine) { pins.delete(id); frame(id, { kind: 'pin', revision, pin: null }); }
      announce();
    },
  };
}

/** Serving as another loop's producers (`--serve-as`, host/web/dev.mjs): a
 * device's stream here takes its nonce from that loop's build channel
 * (`main`, its origin), and that loop's commands for it are polled and
 * written to it (`write(frame)`). Its reports go to that loop directly: the
 * device's origin is that loop's. Batches are this loop's own, as it watches
 * the same files. Its nonce and `close()`, or null when that loop keeps no
 * channel; `ended()` when that loop no longer holds the nonce. */
export async function relayChannel(main, write, every = 250, ended = () => {}) {
  try {
    const opened = await (await fetch(`${main}/__dev/hello`, { signal: AbortSignal.timeout(5000) })).json();
    if (typeof opened.nonce !== 'string') return null;
    let live = true;
    (async () => {
      while (live) {
        try {
          const frames = await (await fetch(`${main}/__dev/commands?nonce=${encodeURIComponent(opened.nonce)}`, { signal: AbortSignal.timeout(5000) })).json();
          // The nonce gone (that loop restarted, or the poll lapsed): the
          // device's stream ends, so it connects again for a new one.
          if (!Array.isArray(frames)) { live = false; ended(); return; }
          for (const f of frames) write(f);
        } catch { /* that loop restarting: asked again */ }
        await new Promise(ok => setTimeout(ok, every));
      }
    })();
    return { nonce: opened.nonce, close: () => { live = false; } };
  } catch { return null; }
}
