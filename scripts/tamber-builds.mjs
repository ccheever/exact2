// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): the dev loop's
// running development builds, registered, reporting up and taking commands
// down, for the design server's clients.
//
// - **Registration.** Each dev stream a build opens (`/__dev`; a Linux build,
//   which holds no stream, `GET /__dev/hello` and then acknowledged polls of
//   `GET /__dev/commands`) is given a nonce in its hello, while fewer than
//   256 streams are open: past that a stream has none and a hello is 503. The build answers with one `POST /__dev/report`
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
// this many streams of both kinds (so as many builds connected), and this
// many builds remembered (the oldest disconnected gone first): the routes
// take no token, so what they keep is bounded.
// A Linux build polls twice a second: unpolled 5 s, it is disconnected.
const POLLED_MS = 5_000, POLLED = 64, STREAMS = 256, BUILDS = 256;
// A polled stream's queue: at most this many frames and bytes, past which
// its live-table frames collapse into one notice (only the latest pin is
// ever queued).
const QUEUED = 64, QUEUE_BYTES = 128 * 1024;
// One poll's answer, and so any frame in it, is at most this many bytes:
// Linux reads a commands response of at most 64 KiB, and the resident loop
// writes each frame it relays as one event, which Apple bounds at 64 KiB
// (PlanURL.swift). A frame over it goes as a notice.
const POLL_BYTES = 32 * 1024;
// A report's body: at most this many bytes, read within this long.
const REPORT_BYTES = 64 * 1024, REPORT_MS = 10_000;
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
  if (JSON.stringify(pin).length > 4096) return 'a pin is at most 4 KiB';
  return null;
}

const live = f => String(f.kind).startsWith('tamber-');
// A live-table frame's notice: no tokens, so a build at another design asks
// for the snapshot.
const notice = f => ({ kind: 'tamber-batch', structure: f.structure, base: f.design, design: f.design, tokens: [] });
/** A frame as one poll may carry it: a live-table frame over the bound as
 * its notice; any other, which is never that large, as it is. */
function bounded(f) {
  return Buffer.byteLength(JSON.stringify(f)) <= POLL_BYTES || !live(f) ? f : notice(f);
}

const text = (v, max) => typeof v === 'string' && v.length <= max;
const maybe = (v, ok) => v == null || ok(v);
const object = v => typeof v === 'object' && v !== null && !Array.isArray(v);
/** A report's fields, each of its type and bounded: a peer on the LAN may
 * send any JSON, which is never coerced. The refusal's text, or null. */
function reportRefused(r) {
  if (r.kind === 'register') {
    if (!text(r.id, 64) || !ID.test(r.id)) return '`id` is the build\'s own, 1–64 of [A-Za-z0-9_.:-]';
    if (!text(r.host, 32) || !r.host || !Array.isArray(r.capabilities) || r.capabilities.length > 32 || !r.capabilities.every(c => text(c, 64))) return 'a registration names its `host` and its `capabilities` (strings)';
  }
  if (!maybe(r.plan, v => text(v, 1024)) || !maybe(r.design, v => text(v, 1024))) return '`plan` and `design` are strings, or null';
  if (r.kind === 'status') {
    if (!maybe(r.scheme, v => text(v, 32)) || !maybe(r.contrast, v => text(v, 32))) return '`scheme` and `contrast` are strings, or null';
    if (!maybe(r.mode, v => object(v) && Object.keys(v).length <= 64 && Object.entries(v).every(([a, x]) => a.length <= 64 && text(x, 64)))) return '`mode` is {axis: value}, or null';
    if (!maybe(r.rootFontSize, Number.isFinite) || !maybe(r.revision, v => Number.isSafeInteger(v) && v >= 0)) return '`rootFontSize` is a number and `revision` a whole number, or null';
  }
  if (r.kind === 'unsupported' && (!maybe(r.what, v => text(v, 256)) || !maybe(r.detail, v => text(v, 4096)))) return '`what` and `detail` are strings';
  return null;
}

/** One dev loop's builds. `send(event)` tells the design server's clients
 * (`builds`, `build-status`, `unsupported`, `selected`). */
export function buildChannel({ send = () => {}, log = () => {} } = {}) {
  const streams = new Map(); // nonce -> { write(frame), build: id | null, seen?, poll? }
  const builds = new Map(); // id -> { id, host, capabilities, plan, design, status, nonce }
  const pins = new Map(); // id -> { client, pin, revision }
  let revision = 0, greeting = () => null;
  // A bearer credential for the report route: unguessable.
  const nonce = () => randomBytes(16).toString('hex');
  /** Polled streams past their time, and the oldest over the cap, closed. */
  const sweep = (room = 0) => {
    const polled = [...streams].filter(([, s]) => s.seen !== undefined);
    for (const [k, s] of polled) if (Date.now() - s.seen > POLLED_MS) close(k);
    const open = polled.filter(([k]) => streams.has(k)).sort((a, b) => a[1].seen - b[1].seen);
    for (const [k] of open.slice(0, Math.max(0, open.length - POLLED + room))) close(k);
  };
  const close = (k) => { const s = streams.get(k); streams.delete(k); if (s?.build) announce(); };
  const shown = b => ({ id: b.id, host: b.host, capabilities: b.capabilities, plan: b.plan, design: b.design, status: b.status ?? null, pin: pins.get(b.id)?.pin ?? null });
  // A build is connected while the stream it registered on still names it.
  const connected = b => streams.get(b.nonce)?.build === b.id;
  const listed = () => [...builds.values()].filter(connected).map(shown);
  const announce = () => send({ event: 'builds', builds: listed() });
  const frame = (id, f) => { const b = builds.get(id); const s = b && streams.get(b.nonce); if (s) s.write(f); };
  const pinFrame = id => ({ kind: 'pin', revision: pins.get(id)?.revision ?? revision, pin: pins.get(id)?.pin ?? null });

  return {
    /** A build's stream opened; `write(frame)` sends it one. Its nonce, for
     * its hello, and `close()` when the stream ends; null with 256 open. */
    open(write) {
      if (streams.size >= STREAMS) return null;
      const n = nonce();
      streams.set(n, { write, build: null });
      return { nonce: n, close: () => close(n) };
    },
    /** A stream for a build that polls (Linux, the resident loop's relay):
     * frames queue until it polls; unpolled for 5 s it is gone. The oldest
     * polled stream gives way to it past 64; null with 256 open. */
    openPolled() {
      sweep(1);
      let queue = []; // [{f, n}], n its bytes
      const opened = this.open((f) => {
        f = bounded(f);
        // Only the latest pin matters: it replaces the whole set.
        if (f.kind === 'pin') queue = queue.filter(q => q.f.kind !== 'pin');
        queue.push({ f, n: Buffer.byteLength(JSON.stringify(f)) });
        if (queue.length <= QUEUED && queue.reduce((sum, q) => sum + q.n, 0) <= QUEUE_BYTES) return;
        // Over its bound: the live-table frames pending are one notice of
        // the latest's design, and the build fetches the snapshot.
        const latest = queue.findLast(q => live(q.f));
        queue = queue.filter(q => !live(q.f));
        if (latest) { const f = notice(latest.f); queue.push({ f, n: Buffer.byteLength(JSON.stringify(f)) }); }
      });
      if (!opened) return null;
      const s = streams.get(opened.nonce);
      s.seen = Date.now();
      // An answer is kept until a poll acknowledges its seq, and sent again
      // to one that does not (its response was lost); the next takes as many
      // queued frames as fit one answer, the rest waiting for the one after.
      let last = null, seq = 0;
      s.poll = (ack) => {
        s.seen = Date.now();
        if (last && ack < last.seq) return last;
        let size = 2, n = 0;
        for (; n < queue.length; n++) {
          size += queue[n].n + (n ? 1 : 0);
          // (A frame that fits no answer, which `bounded` leaves none, would
          // go alone rather than hold the queue.)
          if (size > POLL_BYTES) { n ||= 1; break; }
        }
        last = { seq: ++seq, frames: queue.splice(0, n).map(q => q.f) };
        return last;
      };
      // The design to run, as a stream's hello says it (a notice).
      const hello = greeting();
      if (hello) s.write(hello);
      return opened;
    },
    /** A polled build's answer, `{seq, frames}`, to a poll acknowledging
     * answer `ack` (0 for none yet): the last one again when `ack` is below
     * its seq, else the frames since. Null for a nonce no stream holds. */
    poll(n, ack = 0) {
      sweep();
      const k = Number(ack);
      return streams.get(n)?.poll?.(Number.isSafeInteger(k) && k >= 0 ? k : 0) ?? null;
    },
    /** `POST /__dev/report`'s body: `{status, body}`. */
    report(body) {
      let r;
      try { r = JSON.parse(body); } catch { r = null; }
      if (!object(r)) return { status: 400, body: { message: 'a report is a JSON object' } };
      sweep();
      const s = typeof r.nonce === 'string' && streams.get(r.nonce);
      if (!s) return { status: 403, body: { message: 'no open dev stream holds this nonce' } };
      if (typeof r.kind !== 'string' || !KINDS.has(r.kind)) return { status: 400, body: { message: `a report's kind is one of ${[...KINDS].join(', ')}` } };
      const why = reportRefused(r);
      if (why) return { status: 400, body: { message: why } };
      if (r.kind === 'register') {
        // The same build on a new stream: the old one's is no longer it.
        for (const other of streams.values()) if (other !== s && other.build === r.id) other.build = null;
        // A stream is one build: another id registered on it replaces the
        // build it was, and that build's pin goes with it.
        if (s.build && s.build !== r.id && builds.get(s.build)?.nonce === r.nonce) { builds.delete(s.build); pins.delete(s.build); }
        s.build = r.id;
        builds.delete(r.id);
        builds.set(r.id, { id: r.id, host: r.host, capabilities: r.capabilities, plan: r.plan ?? null, design: r.design ?? null, status: null, nonce: r.nonce });
        // The oldest builds with no open stream forgotten, over the cap.
        for (const [id, b] of builds) { if (builds.size <= BUILDS) break; if (!connected(b)) { builds.delete(id); pins.delete(id); } }
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
        send({ event: 'unsupported', build: b.id, what: r.what ?? '', detail: r.detail ?? null });
      } else {
        send({ event: 'selected', build: b.id, selected: r.selected ?? null });
      }
      return { status: 204, body: null };
    },
    /** The registered builds with an open stream. */
    list: () => { sweep(); return listed(); },
    /** Polled streams past their time closed (their builds announced gone):
     * on every use of the channel, and on the server's timer, so a build
     * that stops polling leaves when nothing else happens. */
    sweep,
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
      sweep();
      const targets = id == null ? listed().map(b => b.id) : [id];
      if (id != null && !listed().some(b => b.id === id)) return { error: `no registered build \`${id}\`` };
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

/** `POST /__dev/report` on `channel` (both dev loops). The body is read
 * within REPORT_MS; one over REPORT_BYTES, or not done in time, has its
 * socket destroyed, and one aborted is dropped: none rejects, as a rejection
 * in the server's handler would end the loop. */
export async function serveReport(channel, req, res) {
  const text = await new Promise((done) => {
    const chunks = [];
    let size = 0;
    const end = (value) => { clearTimeout(deadline); done(value); };
    const drop = () => { req.socket?.destroy(); req.destroy(); end(null); };
    const deadline = setTimeout(drop, REPORT_MS);
    req.on('data', (chunk) => { size += chunk.length; if (size > REPORT_BYTES) drop(); else chunks.push(chunk); });
    req.on('end', () => end(Buffer.concat(chunks).toString('utf8')));
    req.on('error', () => end(null));
    req.on('aborted', () => end(null));
    req.on('close', () => end(null));
  });
  if (text == null || res.destroyed) return;
  // Nothing a report does escapes: an exception in the server's handler
  // would end the loop.
  let answer;
  try { answer = channel.report(text); } catch { answer = { status: 500, body: { message: 'the report could not be handled' } }; }
  try {
    res.writeHead(answer.status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    res.end(answer.body == null ? undefined : JSON.stringify(answer.body));
  } catch { res.destroy(); }
}

/** `GET /__dev/hello` and `GET /__dev/commands` on `channel` (both dev
 * loops), for a build that holds no stream: the hello's nonce (with
 * `hello`'s fields), 503 with 256 streams open; a poll's answer `{seq,
 * frames}` to `?nonce=&ack=`, 404 with `null` for a nonce no stream holds
 * (its build says hello again). */
export function serveCommands(channel, url, res, hello = {}) {
  let status = 200, answer;
  if (url.pathname === '/__dev/hello') {
    const opened = channel.openPolled();
    answer = opened ? { hello: true, ...hello, nonce: opened.nonce } : { message: 'the dev loop holds as many streams as it keeps' };
    if (!opened) status = 503;
  } else {
    answer = channel.poll(url.searchParams.get('nonce') ?? '', url.searchParams.get('ack') ?? 0);
    if (!answer) status = 404;
  }
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(answer));
}

/** Serving as another loop's producers (`--serve-as`, host/web/dev.mjs): a
 * device's stream here takes its nonce from that loop's build channel
 * (`main`, its origin), and that loop's commands for it are polled, each
 * answer acknowledged by the next poll, and written to it (`write(frame)`).
 * Its reports go to that loop directly: the device's origin is that loop's.
 * Batches are this loop's own, as it watches the same files. Its nonce and
 * `close()`, or null when that loop keeps no channel; `ended()` when that
 * loop no longer holds the nonce. */
export async function relayChannel(main, write, every = 250, ended = () => {}) {
  try {
    const opened = await (await fetch(`${main}/__dev/hello`, { signal: AbortSignal.timeout(5000) })).json();
    if (typeof opened?.nonce !== 'string') return null;
    let going = true, ack = 0;
    (async () => {
      while (going) {
        try {
          const response = await fetch(`${main}/__dev/commands?nonce=${encodeURIComponent(opened.nonce)}&ack=${ack}`, { signal: AbortSignal.timeout(5000) });
          // The nonce gone (that loop restarted, or the poll lapsed): the
          // device's stream ends, so it connects again for a new one.
          if (response.status === 404) { going = false; ended(); return; }
          const answer = await response.json();
          // An answer read whole is acknowledged by the next poll; one lost
          // on the way is sent again, under the same seq.
          if (going && Number.isSafeInteger(answer?.seq) && answer.seq > ack && Array.isArray(answer.frames)) {
            ack = answer.seq;
            for (const f of answer.frames) write(f);
          }
        } catch { /* that loop restarting: asked again */ }
        await new Promise(ok => setTimeout(ok, every));
      }
    })();
    return { nonce: opened.nonce, close: () => { going = false; } };
  } catch { return null; }
}
