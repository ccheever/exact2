// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): the dev loop's
// running development builds, registered, reporting up and taking commands
// down, for the design server's clients.
//
// - **Registration.** Each dev stream a build opens (`/__dev`; a Linux build,
//   which holds no stream, `GET /__dev/hello` and then acknowledged polls of
//   `GET /__dev/commands`) is given a nonce in its hello, while fewer than
//   256 streams are open: past that a stream has none and a hello is 503. The build answers with one `POST /__dev/report`
//   `{nonce, kind: "register", id, host, capabilities, plan, design, page?}`; `id`
//   is the build's own, stable across its reconnects (a tab's, a process's),
//   so its pins are replayed when it registers again. `page: "gallery"`
//   (tamber/1.15) is the variant gallery's page (host/web-js/dev-gallery.mjs),
//   absent for the app's own: the page's own claim, a label that grants and
//   withholds nothing, listed on the build as it said it. A report whose nonce
//   is no open stream's is refused: the dev loop takes other POSTs with no
//   token.
// - **Reports up**, each with its nonce: `status` (on connect and on any
//   change: the effective `scheme`, `contrast`, `mode` and `rootFontSize`,
//   the `plan` and `design` digests, the pin `revision` applied),
//   `unsupported` (a command or batch it cannot apply, by name), and
//   `selected` (LLP 1122 D5: a node selected in inspect mode, or null when
//   the page left inspect mode itself).
// - **Commands down**, each a frame whose `kind` a host dispatches before any
//   generation handling: `pin` `{revision, pin}` here (`{scheme?, contrast?,
//   mode?: {axis: value}, rootFontSize?}`, or null to hand them back);
//   `inspect` `{on}` or `{refresh}` (T2b); `outline` `{sites, plan}` (T2b);
//   `hold` `{revision, hold}` (LLP 1122 D8, LLP 1121 stage 8 E, *1.11*):
//   `{site, instance, state}`, a state held on one node of the build's own
//   plan, or null to let it go. A host that cannot hold that state answers
//   `unsupported`; one that applies or drops a hold reports `held`.
// - **Ownership.** A pin belongs to the client that set it. When that
//   client's last subscription closes, its pins are cleared on every build.
//   A build that registers again gets its pins replayed. So with a hold, one
//   a build, which is replayed when the build registers again or runs
//   another plan only where exactly one site corresponds (`correspond`: its
//   place and call-site chain), else cleared with a diagnostic
//   (`hold-cleared`).

import { randomBytes } from 'node:crypto';

const ID = /^[\w.:-]{1,64}$/;
// A polled stream unpolled this long is gone; at most this many are open,
// this many streams of both kinds (so as many builds connected), and this
// many builds remembered (the oldest disconnected gone first): the routes
// take no token, so what they keep is bounded.
// A Linux build polls twice a second: unpolled 5 s, it is disconnected.
const POLLED_MS = 5_000, POLLED = 64, STREAMS = 256, BUILDS = 256, DESIGN_TOLD_MS = 150;
// A polled stream's queue: at most this many frames and bytes, past which
// its live-table frames collapse into one notice (only the latest pin is
// ever queued).
const QUEUED = 64, QUEUE_BYTES = 128 * 1024;
// One poll's answer, and so any frame in it, is at most this many bytes:
// Linux reads a commands response of at most 64 KiB, and the resident loop
// writes each frame it relays as one event, which Apple bounds at 64 KiB
// (PlanURL.swift). A frame over it goes as a notice.
const POLL_BYTES = 32 * 1024;
// A long poll (`wait`) is held at most this long: well under the 5 s a
// polled stream lives unpolled, and the 5 s a client's request lasts.
const WAIT_MS = 4_000;
// A report's body: at most this many bytes, read within this long.
const REPORT_BYTES = 64 * 1024, REPORT_MS = 10_000;
const KINDS = new Set(['register', 'status', 'unsupported', 'selected', 'held']);
// A location on the build's own origin (LLP 1122 T7, `navigate`; a status's
// `location`): a path with its query and fragment, never a scheme, a host or
// a path a browser reads as one (`//`, `/\`), at most 2 KiB, in printable
// ASCII (percent-encoded, as a page's own location is), no backslash.
const LOCATION = /^\/(?![/\\])[\x21-\x5b\x5d-\x7e]*$/;
// (Resolved, it stays a path, within the bound as a page then spells it: no
// dot segment makes it `//…`, which the history API reads as another host,
// and no character it escapes takes it past 2 KiB.)
const located = v => {
  if (typeof v !== 'string' || v.length > 2048 || !LOCATION.test(v)) return false;
  const u = new URL(v, 'http://h.invalid');
  return !u.pathname.startsWith('//') && (u.pathname + u.search + u.hash).length <= 2048;
};
// A location `navigate` takes is one more: spelled as URLs and the router
// write it already (RFC 3986's path, query and fragment characters but
// `'`, which a query escapes; each `%` a whole escape), so the copy arrives at exactly that spelling and
// nothing it routes to is escaped past the bound.
const CANONICAL = /^\/(?:[A-Za-z0-9\-._~!$&()*+,;=:@/?#]|%[0-9A-Fa-f]{2})*$/;
const routable = v => located(v) && CANONICAL.test(v);
// The states a hold names (LLP 1122 D8), and why a build lets one go.
const HELD = ['active', 'hover', 'focus-visible'], DROPS = ['input', 'disabled', 'gone', 'replaced'];

/** A hold as a client gives it: `{site, instance, state}`, or null. The
 * refusal's text, or null. */
function holdRefused(hold) {
  if (hold === null) return null;
  if (!object(hold)) return '`hold` is `{site, instance, state}`, or null';
  if (!Number.isSafeInteger(hold.site) || hold.site < 0) return '`site` is a site of the build\'s plan';
  if (!Array.isArray(hold.instance) || hold.instance.length > 64 || JSON.stringify(hold.instance).length > 4096) return '`instance` is the site\'s instance path (an array, at most 64 steps)';
  if (!HELD.includes(hold.state)) return `\`state\` is one of ${HELD.join(', ')}`;
  return null;
}

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
// for the snapshot; a scrub frame's keeps its mark (*1.11*), so a build
// still orders it.
const notice = f => ({ kind: 'tamber-batch', structure: f.structure, base: f.design, design: f.design, tokens: [], ...(f.scrub ? { scrub: f.scrub } : {}) });
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
    if (!(r.page === undefined || r.page === 'gallery')) return '`page` is "gallery" (the variant gallery\'s page), or absent for the app\'s own';
  }
  if (!maybe(r.plan, v => text(v, 1024)) || !maybe(r.design, v => text(v, 1024))) return '`plan` and `design` are strings, or null';
  if (!maybe(r.planDigest, v => typeof v === 'string' && /^[0-9a-f]{64}$/.test(v))) return '`planDigest` is a SHA-256 in hex, or null';
  if (r.kind === 'status') {
    if (!maybe(r.scheme, v => text(v, 32)) || !maybe(r.contrast, v => text(v, 32))) return '`scheme` and `contrast` are strings, or null';
    if (!maybe(r.mode, v => object(v) && Object.keys(v).length <= 64 && Object.entries(v).every(([a, x]) => a.length <= 64 && text(x, 64)))) return '`mode` is {axis: value}, or null';
    if (!maybe(r.rootFontSize, Number.isFinite) || !maybe(r.revision, v => Number.isSafeInteger(v) && v >= 0)) return '`rootFontSize` is a number and `revision` a whole number, or null';
    if (!maybe(r.app, v => object(v) && Buffer.byteLength(JSON.stringify(v)) <= 1024 && Object.keys(v).every(k => ['scheme', 'mode', 'rootFontSize'].includes(k)) && maybe(v.scheme, s => text(s, 32)) && maybe(v.rootFontSize, s => text(s, 64)) && maybe(v.mode, m => object(m) && Object.entries(m).every(([a, x]) => a.length <= 64 && text(x, 64))))) return '`app` is what the app asked instead of a copy\'s pin: `{scheme?, mode?, rootFontSize?}`, strings (mode `{axis: value}`), at most 1 KiB, or null';
    if (!maybe(r.location, located)) return '`location` is a path on the build\'s own origin (with its query and fragment, printable ASCII, at most 2 KiB), or null';
  }
  if (r.kind === 'selected' && !maybe(r.inspect, v => Number.isSafeInteger(v) && v >= 0)) return '`inspect` is a whole number';
  if (r.kind === 'selected' && !maybe(r.click, v => typeof v === 'string' && v.length > 0 && v.length <= 64)) return '`click` is a string of 1 to 64 characters';
  if (r.kind === 'held' && (!(r.state === null || HELD.includes(r.state)) || !maybe(r.reason, v => DROPS.includes(v)) || !(Number.isSafeInteger(r.revision) && r.revision >= 0))) return `a held report names its \`state\` (${HELD.join(', ')}, or null), a \`reason\` it let one go (${DROPS.join(', ')}) and its hold frame's \`revision\``;
  if (r.kind === 'unsupported' && (!maybe(r.what, v => text(v, 256)) || !maybe(r.detail, v => text(v, 4096)) || !maybe(r.revision, v => Number.isSafeInteger(v) && v >= 0))) return '`what` and `detail` are strings, and `revision` a whole number';
  return null;
}

/** One dev loop's builds. `send(event)` tells the design server's clients
 * (`builds`, `build-status`, `unsupported`, `selected`, `held`,
 * `hold-cleared`). `correspond(from, to, site)` is the one site of plan `to`
 * that site `site` of plan `from` is, by their source maps, or null.
 * `timers` (`{set, clear}`, a test's clock) wait out a design-only status. */
export function buildChannel({ send = () => {}, log = () => {}, selected = null, correspond = () => null, timers = { set: setTimeout, clear: clearTimeout } } = {}) {
  const picks = new Map(); // build id -> its last selection's {plan, site, instance}, for `select` (tamber/1.16)
  const streams = new Map(); // nonce -> { write(frame), build: id | null, seen?, poll? }
  const builds = new Map(); // id -> { id, host, capabilities, plan, design, status, nonce }
  const pins = new Map(); // id -> { client, pin, revision }
  // id -> the revision of the last pin frame it was sent (a pin, a hand
  // back, its pin again on registering): its status has caught up when it
  // reports that revision.
  const sent = new Map();
  // id -> the client that turned inspect mode on there; id -> the number of
  // the last on/off command it was sent, which a page's own exit names.
  const inspectors = new Map(), inspectSeq = new Map();
  // id -> the client whose outline a build shows.
  const outliners = new Map();
  // id -> { client, hold, revision, plan }: the state held on that build, of
  // plan `plan`'s site (its planDigest when held).
  const holds = new Map();
  // id -> the revision of the last hold frame sent it (a hold or null): a
  // build's `held` or hold `unsupported` naming any other is old news.
  const holdSent = new Map();
  // id -> { client, seq }: the last `navigate` sent a build, whose refusal
  // is that client's alone. A navigation holds nothing on the build.
  const navigated = new Map();
  let inspects = 0, holding = 0, navigations = 0;
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
  const close = (k) => { const s = streams.get(k); streams.delete(k); s?.unwait?.(); if (s?.build) announce(); };
  const shown = b => ({ id: b.id, host: b.host, ...(b.page ? { page: b.page } : {}), capabilities: b.capabilities, inspector: inspectors.get(b.id) ?? null, plan: b.plan, planDigest: b.planDigest ?? null, design: b.design, status: b.status ?? null, pin: pins.get(b.id)?.pin ?? null, pinRevision: sent.get(b.id) ?? 0, hold: holds.get(b.id)?.hold ?? null, held: b.held ?? null, unsupported: b.unsupported ? { what: b.unsupported.what, detail: b.unsupported.detail } : null });
  // A build is connected while the stream it registered on still names it.
  const connected = b => streams.get(b.nonce)?.build === b.id;
  const listed = () => [...builds.values()].filter(connected).map(shown);
  const announce = () => send({ event: 'builds', builds: listed() });
  // A status that moved only the build's design (a native host reports each
  // scrub frame's) is told once per DESIGN_TOLD_MS, the latest when it goes,
  // as a page reports its own ~150 ms after a scrub rests: not every
  // client's poll woken each frame. Anything else is told at once, with the
  // design as it is then. id -> the last told, less its design; its design;
  // the one waiting.
  const toldFacts = new Map(), toldDesign = new Map(), toldLater = new Map();
  // (Keys in order, so a mode's axes in another order are the same facts.)
  const canonical = v => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([p], [q]) => (p < q ? -1 : p > q ? 1 : 0))) : x));
  /** Its comparison and anything waiting forgotten: a build registered again, or gone. */
  const untell = id => { timers.clear(toldLater.get(id)); toldLater.delete(id); toldFacts.delete(id); toldDesign.delete(id); };
  const tellStatus = (b, due = false) => {
    const { design, ...facts } = shown(b), key = canonical(facts);
    if (!due && toldDesign.has(b.id) && toldFacts.get(b.id) === key && toldDesign.get(b.id) !== design) {
      // The first waiting design starts the wait; later ones replace it, not restart it.
      if (!toldLater.has(b.id)) {
        const later = timers.set(() => {
          toldLater.delete(b.id);
          // Only if that registration is still connected (one gone was announced gone).
          const now = builds.get(b.id);
          if (now === b && connected(now)) tellStatus(now, true);
        }, DESIGN_TOLD_MS);
        later.unref?.();
        toldLater.set(b.id, later);
      }
      return;
    }
    timers.clear(toldLater.get(b.id)); toldLater.delete(b.id);
    toldFacts.set(b.id, key); toldDesign.set(b.id, design);
    send({ event: 'build-status', build: { ...facts, design } });
  };
  // Inspect mode's owners, as they move (tamber/1.6): from `before`, each
  // client that lost builds (off) and each that took some (on), so a tool
  // follows another client's command and its own lapse with no gap.
  // An off says who took the builds (`to`), or null when inspect mode
  // ended there.
  const told = (before) => {
    const moved = new Map();
    const add = (client, on, id, to) => { const key = `${on} ${client} ${to}`; if (!moved.has(key)) moved.set(key, { event: 'inspect', client, on, builds: [], ...(on ? {} : { to }) }); moved.get(key).builds.push(id); };
    for (const [id, owner] of before) if (inspectors.get(id) !== owner) add(owner, false, id, inspectors.get(id) ?? null);
    for (const [id, owner] of inspectors) if (before.get(id) !== owner) add(owner, true, id);
    for (const e of moved.values()) send(e);
  };
  const frame = (id, f) => { const b = builds.get(id); const s = b && streams.get(b.nonce); if (s) s.write(f); };
  // A build with no pin is told so at revision 0: nothing for it to catch.
  const pinFrame = id => (pins.has(id) ? { kind: 'pin', revision: pins.get(id).revision, pin: pins.get(id).pin } : { kind: 'pin', revision: 0, pin: null });
  const sendPin = (id, f) => { sent.set(id, f.revision); frame(id, f); };
  /** Build `id` sent `hold` (or null) at a new revision, the last it was sent,
   * with the plan digest its site addresses when known (`plan`): a build
   * running another plan takes it as nothing, unsaid, and the replay after
   * its status names the new plan corresponds it. */
  const sendHold = (id, hold, plan = null) => {
    const revision = ++holding;
    holdSent.set(id, revision);
    if (hold === null) holds.delete(id);
    // The build's report was of the frame before: a listing's `held` is its hold's.
    const b = builds.get(id);
    if (b) b.held = null;
    frame(id, { kind: 'hold', revision, hold, ...(hold && plan ? { plan } : {}) });
    return revision;
  };
  /** Build `id`'s hold sent again (a new revision), as its plan `plan` (a
   * planDigest) is now: as it is under the same plan, at the one site that
   * corresponds under another, else let go with a diagnostic (LLP 1122 D8).
   * With none, `always` (a registration) says so: a page that kept one
   * across a reconnect, its owner gone meanwhile, lets it go. */
  const replayHold = (id, plan, always = false) => {
    const h = holds.get(id);
    if (!h) { if (always) sendHold(id, null); return; }
    // Held before the build named its plan: the first plan it names is the one held under.
    if (!h.plan && plan && !always) { h.plan = plan; return; }
    if (h.plan && plan && h.plan !== plan) {
      const site = correspond(h.plan, plan, h.hold.site);
      if (site == null) {
        sendHold(id, null);
        log(`tamber: hold on ${id} cleared: no single site corresponds in its new plan`);
        send({ event: 'hold-cleared', build: id, client: h.client, reason: 'no single site corresponds in the build\'s new plan' });
        announce();
        return;
      }
      h.hold = { ...h.hold, site };
    }
    h.plan = plan ?? h.plan;
    h.revision = sendHold(id, h.hold, h.plan);
  };

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
      // The long poll held for this stream, woken by the next frame.
      let waiting = null;
      const opened = this.open((f) => {
        f = bounded(f);
        // Only the latest pin matters: it replaces the whole set.
        if (f.kind === 'pin') queue = queue.filter(q => q.f.kind !== 'pin');
        // Likewise a hold: the latest replaces the one before.
        if (f.kind === 'hold') queue = queue.filter(q => q.f.kind !== 'hold');
        // And a navigation: only where the latest goes.
        if (f.kind === 'navigate') queue = queue.filter(q => q.f.kind !== 'navigate');
        // Likewise inspect: the latest on/off, and one refresh.
        if (f.kind === 'inspect') queue = queue.filter(q => q.f.kind !== 'inspect' || !!q.f.refresh !== !!f.refresh);
        queue.push({ f, n: Buffer.byteLength(JSON.stringify(f)) });
        queueMicrotask(() => waiting?.());
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
      // A long poll: answered at once when there is an answer to give (one
      // unacknowledged, or frames queued), else at the next frame or after
      // `ms`. One is held per stream: a newer one answers the older now.
      s.wait = (ack, ms) => new Promise((done) => {
        s.seen = Date.now();
        waiting?.();
        if ((last && ack < last.seq) || queue.length || ms <= 0) { done(s.poll(ack)); return; }
        let timer = null;
        const answer = () => {
          if (waiting !== answer) return;
          waiting = null;
          clearTimeout(timer);
          done(streams.has(opened.nonce) ? s.poll(ack) : null);
        };
        waiting = answer;
        timer = setTimeout(answer, ms);
      });
      s.unwait = () => waiting?.();
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
    /** The same as a long poll (*1.10*): held until a frame is queued for
     * it or `ms` (at most 4 s) passes. Resolves null for a nonce no stream
     * holds. */
    async waitPoll(n, ack = 0, ms = 0) {
      sweep();
      const k = Number(ack), s = streams.get(n);
      if (!s?.wait) return null;
      return s.wait(Number.isSafeInteger(k) && k >= 0 ? k : 0, Math.min(Math.max(0, Number(ms) || 0), WAIT_MS));
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
        const owners = new Map(inspectors);
        // The same build on a new stream: the old one's is no longer it.
        for (const other of streams.values()) if (other !== s && other.build === r.id) other.build = null;
        // A stream is one build: another id registered on it replaces the
        // build it was, and that build's pin goes with it.
        if (s.build && s.build !== r.id && builds.get(s.build)?.nonce === r.nonce) { builds.delete(s.build); untell(s.build); pins.delete(s.build); sent.delete(s.build); inspectors.delete(s.build); inspectSeq.delete(s.build); outliners.delete(s.build); holds.delete(s.build); holdSent.delete(s.build); navigated.delete(s.build); }
        s.build = r.id;
        builds.delete(r.id); untell(r.id);
        builds.set(r.id, { id: r.id, host: r.host, ...(r.page ? { page: r.page } : {}), capabilities: r.capabilities, plan: r.plan ?? null, planDigest: r.planDigest ?? null, design: r.design ?? null, status: null, nonce: r.nonce });
        // The oldest builds with no open stream forgotten, over the cap.
        for (const [id, b] of builds) { if (builds.size <= BUILDS) break; if (!connected(b)) { builds.delete(id); untell(id); pins.delete(id); sent.delete(id); inspectors.delete(id); inspectSeq.delete(id); outliners.delete(id); holds.delete(id); holdSent.delete(id); navigated.delete(id); } }
        log(`build registered: ${r.id} (${r.host})`);
        // Its pin, or none, as the server holds it now (a reload is a new
        // document; a pin cleared while it was away is cleared there too).
        const f = pinFrame(r.id);
        sent.set(r.id, f.revision);
        s.write(f);
        // And inspect mode as the server holds it: on when a present client
        // turned it on, else off (a page that kept it across a reconnect
        // whose owner went meanwhile leaves it).
        if (r.capabilities?.includes('inspect')) s.write({ kind: 'inspect', on: inspectors.has(r.id), seq: inspectSeq.get(r.id) ?? 0 });
        // And its hold, where its plan still has the site (a reload).
        if (r.capabilities?.includes('hold')) replayHold(r.id, r.planDigest ?? null, true);
        announce();
        told(owners);
        return { status: 200, body: { id: r.id } };
      }
      const b = s.build && builds.get(s.build);
      if (!b) return { status: 409, body: { message: 'register first' } };
      // Any other report (a selection, a held state, a refusal) is about the
      // design the build runs now: a design still waiting is told first.
      if (r.kind !== 'status' && toldLater.has(b.id)) tellStatus(b, true);
      if (r.kind === 'status') {
        // A late report (older than the status kept) cannot rewind it; a
        // revision this registration was never sent (an earlier run's, a
        // report queued before it registered) acknowledges nothing.
        const stored = b.status?.revision;
        if (Number.isSafeInteger(r.revision) && Number.isSafeInteger(stored) && r.revision < stored) return { status: 204, body: null };
        const at = Number.isSafeInteger(r.revision) && r.revision <= (sent.get(b.id) ?? 0) ? r.revision : null;
        // A command it could not apply is its frame's: the revision it
        // names, or (a host that names none) the one it reports next, as a
        // host answers a frame before its status for it. Kept while its
        // status stays at that revision, gone once it moves on.
        if (b.unsupported && b.unsupported.revision === undefined) b.unsupported.revision = at;
        else if (b.unsupported && b.unsupported.revision !== at) b.unsupported = null;
        b.status = { scheme: r.scheme ?? null, contrast: r.contrast ?? null, mode: r.mode ?? null, rootFontSize: r.rootFontSize ?? null, revision: at, location: r.location ?? null, app: r.app ?? null };
        if (typeof r.plan === 'string') b.plan = r.plan;
        // (Said unknown, null, it is unknown: such a build is sent every plan's outline.)
        const was = b.planDigest;
        if ('planDigest' in r) b.planDigest = r.planDigest ?? null;
        // A rebuild that kept its stream: its hold, under its new plan.
        if (b.planDigest && b.planDigest !== was) replayHold(b.id, b.planDigest);
        if (typeof r.design === 'string') b.design = r.design;
        tellStatus(b);
      } else if (r.kind === 'held') {
        // The build's held state is every client's to know (theme2's 07:48
        // (c)); one it let go of itself (real input, disabled, its node
        // gone) is not held any more, nor replayed.
        // Only a report of the last hold frame sent counts: one of an earlier
        // frame (a late echo, the `replaced` of the frame that replaced it)
        // is old news. A let-go at that revision ends the hold: the host's
        // own (input, disabled, gone), or an agent's hold in its place.
        // A let-go closes the revision: a late report of it applies nothing.
        const h = holds.get(b.id);
        if (r.revision !== holdSent.get(b.id) || (r.state !== null && !h)) return { status: 204, body: null };
        b.held = { state: r.state ?? null, reason: r.reason ?? null };
        if (r.state === null) { holdSent.set(b.id, null); if (h) { holds.delete(b.id); announce(); } }
        log(`build ${b.id}: ${r.state ? `holds ${r.state}` : `let go${r.reason ? ` (${r.reason})` : ''}`}`);
        send({ event: 'held', build: b.id, state: r.state ?? null, reason: r.reason ?? null });
      } else if (r.kind === 'unsupported') {
        // A navigation refused is its sender's alone, and only the last one
        // sent (its `revision`, the frame's seq): it holds nothing to clear.
        if (r.what === 'navigate') {
          const n = navigated.get(b.id);
          if (!n || r.revision !== n.seq) return { status: 204, body: null };
          navigated.delete(b.id);
          log(`build ${b.id}: unsupported navigate`);
          send({ event: 'unsupported', build: b.id, what: 'navigate', detail: r.detail ?? null, only: n.client });
          return { status: 204, body: null };
        }
        log(`build ${b.id}: unsupported ${r.what ?? '?'}`);
        // A hold refused is its hold frame's, an answer to its owner alone
        // (theme2's 07:48 (c)); a pin's is kept with the status.
        if (r.what === 'hold') {
          const h = holds.get(b.id);
          if (r.revision !== holdSent.get(b.id) || !h) return { status: 204, body: null };
          holds.delete(b.id);
          holdSent.set(b.id, null);
          b.held = null;
          send({ event: 'unsupported', build: b.id, what: 'hold', detail: r.detail ?? null, only: h.client });
          announce();
          return { status: 204, body: null };
        }
        // One for a frame its status has already moved past is old news.
        const stale = Number.isSafeInteger(r.revision) && Number.isSafeInteger(b.status?.revision) && r.revision < b.status.revision;
        if (!stale) b.unsupported = { what: r.what ?? '', detail: r.detail ?? null, ...(Number.isSafeInteger(r.revision) ? { revision: r.revision } : {}) };
        send({ event: 'unsupported', build: b.id, what: r.what ?? '', detail: r.detail ?? null });
      } else {
        // A page that left inspect mode itself (Escape, its close control):
        // no one owns it there any more.
        // The exit names the command it left; a newer owner's stays.
        // An exit from a command since replaced (another owner's now) is not
        // this build's news: nothing is published.
        if (r.selected == null) {
          if (r.inspect !== (inspectSeq.get(b.id) ?? 0)) return { status: 204, body: null };
          const owners = new Map(inspectors); inspectors.delete(b.id); told(owners);
        }
        // A selection made under inspect mode that has ended since (Stop, a
        // lapse, while it was on its way) is not published.
        // So is one made under a command since replaced (Stop then Inspect,
        // another client taking over), which names that command.
        if (r.selected != null && (!inspectors.has(b.id) || (r.inspect != null && r.inspect !== (inspectSeq.get(b.id) ?? 0)))) return { status: 409, body: { message: 'inspect mode is off on this build, or it is another command\'s now' } };
        // A selection is the server's to join (`selected`): it may refuse a
        // stale one, and ask the build again.
        // It answers a refusal, or the report as it is published (`publish`:
        // its names as the server's clients may see them).
        const joined = r.selected != null && selected ? selected(shown(b), r, f => frame(b.id, f)) : null;
        if (joined?.status) return joined;
        // (What a move up the chain is checked against: the selection as the page made it, its plan, site and instance.)
        picks.set(b.id, r.selected ? { plan: r.selected.planDigest ?? null, site: r.selected.site ?? null, instance: JSON.stringify(r.selected.instance ?? []) } : null);
        send({ event: 'selected', build: b.id, selected: joined?.publish ?? r.selected ?? null });
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
    /** The client that owns inspect mode on build `id`, or undefined. */
    inspector: id => inspectors.get(id),
    /** The clients that hold a pin, inspect mode or an outline on a build. */
    owners: () => [...new Set([...[...pins.values()].map(p => p.client), ...inspectors.values(), ...outliners.values(), ...[...holds.values()].map(h => h.client)])],
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
      // To no build: nothing sent, the revision as it is.
      if (!targets.length) return { builds: [], revision };
      revision += 1;
      for (const t of targets) {
        if (pin === null) pins.delete(t); else pins.set(t, { client, pin, revision });
        sendPin(t, { kind: 'pin', revision, pin });
      }
      // Commands are journaled with their client (LLP 1122 D4).
      log(`tamber: ${client} pinned ${JSON.stringify(pin)} on ${targets.join(', ') || 'no build'} (revision ${revision})`);
      announce();
      return { builds: targets, revision };
    },
    /** `client` turns inspect mode on or off (`refresh`: the selection
     * reported again, under the digests now) on build `id`, or on every
     * build that can (`id` null). The builds it went to, and those that
     * cannot inspect; inspect mode is its client's, turned off when it goes
     * (`clear`). */
    /** `client`'s selection on build `id` moved `up` call sites (tamber/1.16 `inspect` `select`): sent to the page,
     * which selects that ancestor and reports it as a click would; refused unless `client` inspects that build. */
    selectUp(client, id, up, { levels, plan, from, instance }) {
      sweep();
      if (!listed().some(b => b.id === id)) return { error: `no registered build \`${id}\`` };
      if (inspectors.get(id) !== client) return { error: `inspect is not on for \`${client}\` on \`${id}\`: turn it on first` };
      // The selection moved since its chain was read (another node, or a rebuild): stale, as the protocol says it
      // everywhere (theme2 15:02), so the client reads its chain again; never `unsupported`, which is a page that can't.
      const pick = picks.get(id);
      if (!pick || pick.plan !== plan || pick.site !== from || pick.instance !== JSON.stringify(instance ?? [])) return { stale: true, error: 'the selection moved since its chain was read: select again' };
      frame(id, { kind: 'inspect', select: { up, levels, plan, from, instance } });
      return { builds: [id] };
    },
    inspect(client, id, { on = false, refresh = false } = {}) {
      sweep();
      if (id != null && !listed().some(b => b.id === id)) return { error: `no registered build \`${id}\`` };
      const targets = id == null ? listed() : listed().filter(b => b.id === id);
      const can = targets.filter(b => b.capabilities?.includes('inspect')).map(b => b.id);
      const owners = new Map(inspectors);
      for (const t of can) {
        if (refresh) frame(t, { kind: 'inspect', refresh: true });
        else { const seq = ++inspects; inspectSeq.set(t, seq); frame(t, { kind: 'inspect', on: !!on, seq }); if (on) inspectors.set(t, client); else inspectors.delete(t); }
      }
      // Off everywhere is off on a build of this client's that is away too:
      // its registration is told off, not replayed on.
      if (id == null && !on && !refresh) for (const [t, owner] of inspectors) if (owner === client && !can.includes(t)) { inspectors.delete(t); inspectSeq.set(t, ++inspects); }
      told(owners);
      log(`tamber: ${client} ${refresh ? 'refreshed inspect' : `inspect ${on ? 'on' : 'off'}`} on ${can.join(', ') || 'no build'}`);
      return { builds: can, unsupported: targets.map(b => b.id).filter(t => !can.includes(t)) };
    },
    /** `client` outlines the sites of plan `plan` on every build that can
     * inspect (`outline {sites, plan}`); a build running another plan
     * leaves it. The outlines are that client's: cleared on a build when it
     * goes (`clear`), unless another client's came since. */
    /** `client` outlines on every build that can inspect: `sitesOf(build)`
     * gives that build its own plan's `{sites, plan}`, or null for none
     * (its plan has no map here); `sitesOf` null clears. The builds told. */
    outline(client, sitesOf, click = null) {
      sweep();
      const to = [];
      for (const b of listed().filter(b => b.capabilities?.includes('inspect'))) {
        const mine = sitesOf ? sitesOf(b) : null;
        // A build this outline cannot reach (its plan has no map, or a stale
        // one) loses the client's old outline rather than keeping it.
        if (sitesOf && !mine) { if (outliners.get(b.id) === client) { frame(b.id, { kind: 'outline', clear: true }); outliners.delete(b.id); } continue; }
        // (`click`: the click's token the outline answers, tamber/1.7.)
        frame(b.id, mine ? { kind: 'outline', sites: mine.sites, plan: mine.plan, ...(click ? { click } : {}) } : { kind: 'outline', clear: true });
        outliners.set(b.id, client);
        to.push(b.id);
      }
      return to;
    },
    /** `client` holds `hold` (`{site, instance, state}` of build `id`'s
     * plan `plan`, a planDigest) on build `id`, replacing any other; null
     * lets it go. One hold a build; its client's while present (`clear`).
     * The refusal, or the revision sent. */
    hold(client, id, hold, plan = null) {
      const why = holdRefused(hold);
      if (why) return { error: why };
      sweep();
      const b = listed().find(b => b.id === id);
      if (!b) return { error: `no registered build \`${id}\`` };
      if (!b.capabilities?.includes('hold')) return { unsupported: `build \`${id}\` (${b.host}) holds no state` };
      const held = plan ?? b.planDigest ?? null;
      const revision = sendHold(id, hold, held);
      if (hold !== null) holds.set(id, { client, hold, revision, plan: held });
      log(`tamber: ${client} ${hold ? `held ${hold.state} on site ${hold.site}` : 'let go'} on ${id} (hold ${revision})`);
      announce();
      return { build: id, revision };
    },
    /** `client` routes build `id` to `location`, a path with its query and
     * fragment on the build's own origin, in place as a deep link (LLP 1122
     * T7). One-shot: nothing is held, replayed or cleared. The refusal, the
     * build's lack of it, or the seq sent. */
    navigate(client, id, location) {
      if (!routable(location)) return { error: '`location` is a path on the build\'s own origin, with its query and fragment, at most 2 KiB and spelled as a URL writes it (RFC 3986\'s characters, each `%` a whole escape): no scheme or host' };
      sweep();
      const b = listed().find(b => b.id === id);
      if (!b) return { error: `no registered build \`${id}\`` };
      if (!b.capabilities?.includes('navigate')) return { unsupported: `build \`${id}\` (${b.host}) cannot navigate` };
      const seq = ++navigations;
      navigated.set(id, { client, seq });
      frame(id, { kind: 'navigate', seq, location });
      log(`tamber: ${client} navigated ${id} to ${JSON.stringify(location)} (navigate ${seq})`);
      return { build: id, seq };
    },
    /** Client `client` is gone: its pins are cleared on every build, and
     * its holds let go. */
    clear(client) {
      let letGo = false;
      for (const [id, h] of holds) if (h.client === client) { sendHold(id, null); letGo = true; }
      for (const [id, owner] of outliners) if (owner === client) { outliners.delete(id); frame(id, { kind: 'outline', clear: true }); }
      const owners = new Map(inspectors);
      for (const [id, owner] of inspectors) if (owner === client) { inspectors.delete(id); const seq = ++inspects; inspectSeq.set(id, seq); frame(id, { kind: 'inspect', on: false, seq }); }
      told(owners);
      const mine = [...pins].filter(([, p]) => p.client === client).map(([id]) => id);
      if (!mine.length) { if (letGo) announce(); return; }
      revision += 1;
      for (const id of mine) { pins.delete(id); sendPin(id, { kind: 'pin', revision, pin: null }); }
      announce();
    },
  };
}

/** The design server's `navigate` `{client, build, location}` (LLP 1122 T7,
 * *1.14*; write scope, the dispatcher's) on `channel`, from a client
 * `present(client)` says is: its answer, `{ok}` or `{status, error}`. */
export function navigateRequest(channel, present, { client, build, location }) {
  if (!present(client)) return { status: 409, error: { message: `navigate: \`${client}\` is not present` } };
  if (typeof build !== 'string') return { status: 400, error: { message: 'navigate names its `build`, a registered build\'s id' } };
  const done = channel.navigate(client, build, location);
  if (done.error) return { status: 400, error: { message: done.error } };
  if (done.unsupported) return { status: 409, error: { message: done.unsupported, unsupported: 'navigate' } };
  return { ok: { navigated: true, ...done } };
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
export async function serveCommands(channel, url, res, hello = {}) {
  let status = 200, answer;
  if (url.pathname === '/__dev/hello') {
    const opened = channel.openPolled();
    answer = opened ? { hello: true, ...hello, nonce: opened.nonce } : { message: 'the dev loop holds as many streams as it keeps' };
    if (!opened) status = 503;
  } else if (url.searchParams.has('wait')) {
    // A long poll (*1.10*): held until a frame comes or `wait` ms pass.
    answer = await channel.waitPoll(url.searchParams.get('nonce') ?? '', url.searchParams.get('ack') ?? 0, url.searchParams.get('wait'));
    if (!answer) status = 404;
    if (res.destroyed) return;
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
 * That loop makes every batch; this one makes none. Each poll is a long
 * poll, held there until a frame comes (`wait`), so a frame reaches the
 * device at once; `every` ms pass after a poll that failed. Its nonce and
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
          const response = await fetch(`${main}/__dev/commands?nonce=${encodeURIComponent(opened.nonce)}&ack=${ack}&wait=${WAIT_MS}`, { signal: AbortSignal.timeout(WAIT_MS + 5000) });
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
          continue;
        } catch { /* that loop restarting: asked again */ }
        await new Promise(ok => setTimeout(ok, every));
      }
    })();
    return { nonce: opened.nonce, close: () => { going = false; } };
  } catch { return null; }
}

/** Unread bytes past which a build reading an event stream is behind. */
export const BEHIND = 64 * 1024;
/** A live-table frame (its JSON `text`) written to an event stream `res`,
 * unless the build reading it is behind (a slow or stopped reader, with
 * more than BEHIND bytes unread): then nothing more is queued for it until
 * the stream drains, and it is sent `latest()` then, the latest design's
 * notice, which asks for its snapshot, never the frames between. */
export function streamFrame(res, text, latest) {
  if (res.behind) return;
  if (res.writableLength <= BEHIND) { res.write(`data: ${text}\n\n`); return; }
  res.behind = true;
  res.once('drain', () => { res.behind = false; const f = latest(); if (f) res.write(`data: ${JSON.stringify(f)}\n\n`); });
}

/** `inspect` `{select: {up: n}, build}` (tamber/1.16): the selection inspect made on that build moves n call sites up,
 * as a click on that ancestor would select it, past the root to the root; the inspecting client's alone (one at read
 * never inspects). The answer as `inspect`'s. */
export function selectRequest(channel, request, sitesOf) {
  const up = request.select?.up;
  if (!Number.isSafeInteger(up) || up < 1 || up > 64) return { status: 400, error: { message: '`select` is `{up: n}`, n a whole number from 1 to 64' } };
  if (typeof request.build !== 'string') return { status: 400, error: { message: '`select` names the `build` whose selection moves' } };
  // The plan sites it may land on, level by level, by the last selection's map, and the plan and site they were taken
  // from: the page moves only that selection, under that plan, choosing by its own tree.
  const at = sitesOf(request.build, up);
  if (!at) return { status: 409, error: { message: `\`${request.build}\` has no selection with a source map to move up from: select a node first` } };
  if (!at.levels.length) return { status: 409, error: { message: 'nothing above the selected node owns a node of its own to move to' } };
  const done = channel.selectUp(request.client, request.build, up, at);
  return done.error ? { status: 409, error: { message: done.error, ...(done.stale ? { stale: true } : {}) } } : { ok: done };
}
