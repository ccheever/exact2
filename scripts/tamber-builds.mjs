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

const ID = /^[\w.:-]{1,64}$/;
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
  let next = 0, revision = 0;
  const nonce = () => `${Date.now().toString(36)}${(++next).toString(36)}${Math.random().toString(36).slice(2, 10)}`;
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
      return { nonce: n, close: () => { const s = streams.get(n); streams.delete(n); if (s?.build) announce(); } };
    },
    /** A stream for a build that polls (Linux): frames queue until it
     * polls. */
    openPolled() {
      const queue = [];
      const opened = this.open(f => queue.push(f));
      let seen = Date.now();
      const s = streams.get(opened.nonce);
      s.poll = () => { seen = Date.now(); return queue.splice(0); };
      s.stale = () => Date.now() - seen > 30_000;
      return opened;
    },
    /** A polled build's frames since its last poll; null for a nonce no
     * stream holds. */
    poll(n) {
      for (const [k, s] of streams) if (s.stale?.()) { streams.delete(k); if (s.build) announce(); }
      return streams.get(n)?.poll?.() ?? null;
    },
    /** `POST /__dev/report`'s body: `{status, body}`. */
    report(text) {
      let r;
      try { r = JSON.parse(text); } catch { return { status: 400, body: { message: 'a report is a JSON object' } }; }
      const s = typeof r?.nonce === 'string' && streams.get(r.nonce);
      if (!s) return { status: 403, body: { message: 'no open dev stream holds this nonce' } };
      if (!KINDS.has(r.kind)) return { status: 400, body: { message: `a report's kind is one of ${[...KINDS].join(', ')}` } };
      if (r.kind === 'register') {
        if (typeof r.id !== 'string' || !ID.test(r.id)) return { status: 400, body: { message: '`id` is the build\'s own, 1–64 of [A-Za-z0-9_.:-]' } };
        if (typeof r.host !== 'string' || !Array.isArray(r.capabilities)) return { status: 400, body: { message: 'a registration names its `host` and its `capabilities`' } };
        // The same build on a new stream: the old one's is no longer it.
        for (const [k, other] of streams) if (other !== s && other.build === r.id) other.build = null;
        s.build = r.id;
        builds.set(r.id, { id: r.id, host: r.host, capabilities: r.capabilities.filter(c => typeof c === 'string'), plan: r.plan ?? null, design: r.design ?? null, status: null, nonce: r.nonce });
        log(`build registered: ${r.id} (${r.host})`);
        // Its pins, replayed (a reload is a new document).
        if (pins.has(r.id)) s.write(pinFrame(r.id));
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
 * channel. */
export async function relayChannel(main, write, every = 250) {
  try {
    const opened = await (await fetch(`${main}/__dev/hello`, { signal: AbortSignal.timeout(5000) })).json();
    if (typeof opened.nonce !== 'string') return null;
    let live = true;
    (async () => {
      while (live) {
        try {
          const frames = await (await fetch(`${main}/__dev/commands?nonce=${encodeURIComponent(opened.nonce)}`, { signal: AbortSignal.timeout(5000) })).json();
          if (!Array.isArray(frames)) return;
          for (const f of frames) write(f);
        } catch { /* that loop restarting: asked again */ }
        await new Promise(ok => setTimeout(ok, every));
      }
    })();
    return { nonce: opened.nonce, close: () => { live = false; } };
  } catch { return null; }
}
