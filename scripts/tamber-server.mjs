// The design server's wire (LLP 1121 D13, stage 3): HTTP to clients, JSON
// lines to `contract tamber serve --stdio`, which owns the files (digests,
// the lock, the journal, every write). This writes no design file.
//
// - `GET /__tamber/token`: the token, minted in memory at start, only to a
//   request the development gate calls local (loopback peer and host, no
//   forwarding header); any other gets the install endpoint's not-found.
// - `POST /__tamber`: one request (`hello`, `describe`, `resolve`, `explain`,
//   `propose`, `transaction`, `undo`, `uses`), its reply as the server gave
//   it. A
//   write names its `client`: the clients share one server, so a hello's
//   name is no write's. The wire adds to `hello` the `app` it serves and
//   its run's `epoch`, the events' (`tamber/1.1`), and `page`, the dev loop's
//   own origin or null (`tamber/1.3`).
// - The change stream, an event per change whoever made it: a client's
//   write (its client, the files it wrote), a hand edit or a `git checkout`
//   (seen by the watcher, `client` null). Each event has a sequence number;
//   the server's own writes are known by digest and announced once.
//   - `POST /__tamber/subscribe`: a streamed response, one `data:` line an
//     event.
//   - `POST /__tamber/events` `{"after": n, "epoch": e, "wait"?: false}`:
//     the events after `n` (the last 256 kept), answered once there is one
//     or after 25 s with none (at once with `wait: false`): for a client whose `fetch` reads a body whole, as a JS-target
//     data module's does. A number this run did not give, or one older than
//     the events kept, is `missed`: no events, and the files to start from.
//     Each answer has the held proposals' revision (`held`, tamber/1.12),
//     and the proposals unless the request named that revision (`"held": r`
//     with this run's epoch).
// - The wire's own requests (LLP 1122 D9, D10), beside the server's:
//   - `commit` `{client, message?}`: the files this session wrote, committed
//     to git as scripts/tamber-commit.mjs guards it; never pushed.
//   - `propose` with `hold: true` and a `client`: kept 15 min for review
//     (64 at most) and announced (`proposal`, its operations by index).
//     `accept` `{proposal, client, items?}` applies it against the digests
//     it was planned with; a file moved since, or only some items,
//     recomputes it as a new proposal for a fresh accept. `drop` `{proposal,
//     client}` forgets it. The ones held now come with `subscribed` and
//     each long poll's answer. A `review` object rides along for the tool
//     (an import's offers by item, its unmapped report and count).
//   - A conflict names who last changed each file that moved (`by`: a
//     client, or "changed on disk").
//   - `history` `{limit?, skip?}` (`tamber/1.1`, LLP 1122 D9): the git log of
//     the design's files, newest first (20, at most 100; at most 10,000
//     skipped), each commit's
//     files with their diff and their token-level edits (`what`, `name`,
//     `when`, `before`, `after`), scripts/tamber-history.mjs reading git.
//   - `revert` `{commit, client}`: that commit's inverse, planned on the
//     files as they are now and held as a proposal (its `review.revert`
//     says which files moved since). Refused, with each edit that cannot
//     apply (`refused`), when the files have moved there. Accepted after
//     the files move, it is planned again from the commit.
//   - `import` `{kind, source, name?, prefix?, client}` (LLP 1122 T1a): a
//     shadcn or Tailwind stylesheet or a DTCG file's text (several DTCG
//     files as `sources: [{name, text}]`), imported as `bun exact.mjs tamber
//     import` does and held with its `review`. Astryx keeps
//     its CLI: resolving its package needs a Bun install.
// - The build channel (LLP 1122 D4, scripts/tamber-builds.mjs): `hello`
//   lists the dev loop's registered builds; `pin` `{client, build?, pin}`
//   and `preview` `{client, build, axis, value}` send one a command, the
//   client's while a subscription names it (`subscribe` `{client}`); the
//   builds' reports are events (`builds`, `build-status`, `unsupported`,
//   `selected`).
// - Scrub sessions (LLP 1121 D13, LLP 1122 D6; scripts/tamber-scrub.mjs):
//   `scrub` `{client, ops, files, scrub?}` previews a live edit on every
//   build through the dev loop (`scrub`: `{preview, restore}`), written
//   nowhere; `scrub-release` writes it once, `scrub-cancel` drops it. One
//   at a time, `write` scope, a present client; a competing write, its
//   client gone, the server lost or 30 s idle end it, and the builds get
//   the committed design back. Each opening and end is a `scrub` event.
// - `GET /__tamber/tool/…`: the design tool's built files (`TOOL_DIST`), to
//   a local request only, its pages' base set to that path.
// - Every POST carries the token as `x-exact-tamber-token`, never in a URL,
//   and no cookie is read. Each request's scope is checked in one place
//   (scripts/tamber-pairing.mjs, tamber/1.13).
// - `POST /__tamber/pair {code}` (stage 10 A2): a code a local client minted
//   (`pair-code`) for a paired client's own token, at the code's scope; the
//   local token lists (`clients`) and revokes (`revoke`) them.
//
// Mounted by both dev loops (host/web/dev.mjs, host/web-js/dev.mjs) when the
// app has a design; `bun exact.mjs tamber serve <app>` runs it alone and
// `bun exact.mjs tamber op <url> <request>` is its scripted client. Never in a
// release build or a bake. D13 names WebSocket; the dev loops serve
// server-sent events and no WebSocket, so stage 3 streams a POST's response.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';
import { existsSync, readFileSync, realpathSync, statSync, watch } from 'node:fs';
import { messageScrubber, namedPaths, pathName, scrubbedMessages } from './tamber-paths.mjs';
import { createServer } from 'node:http';
import { basename, dirname, relative, resolve, sep } from 'node:path';
import { developmentGate, installBrowserOrigins } from './install-page.mjs';
import { stageTool, TOOL_DIST, TOOL_ORIGIN, TOOL_PORT } from './tamber-tool-port.mjs';
export { stageTool, TOOL_DIST, TOOL_ORIGIN, TOOL_PORT } from './tamber-tool-port.mjs';
import { buildChannel, navigateRequest, selectRequest } from './tamber-builds.mjs';
import { scrubSessions } from './tamber-scrub.mjs';
import { correspondingSite, current, join, levelsUp, paramsOf, published, sitesAt, staleMap } from './tamber-inspect.mjs';
import { designSession } from './tamber-commit.mjs';
import { designCommit, designLog } from './tamber-history.mjs';
import { openRemote, pluginOnLoopback, remoteArgs } from './tamber-remote.mjs';
import { planAlone, stopImports } from './tamber-isolate.mjs';
import { designPath, manifestTamber } from './tamber-design-path.mjs'; export { designPath };
import { importReview, marked } from './tamber-import-review.mjs'; export { importTheme } from './tamber-import-review.mjs';
import { canvasGateway } from './tamber-canvas.mjs'; import { answerCanvasCheck, canvasCheck, canvasLine } from './tamber-frame.mjs';
import { packageDesigns } from './tamber-package-designs.mjs';
import { serveTool } from './tamber-tool-files.mjs';
export { serveTool, serveToolFiles } from './tamber-tool-files.mjs';
import { allows, needs, pairing, PAIR_TTL_MS, parseDuration, refusal } from './tamber-pairing.mjs';
import { earlyAnswer, readBody, parseRequest as parse, tooDeep, viaDoor } from './tamber-request.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
// The server's own line limit, less room for the id this adds and an import's `recognised` digests (64, stage 10b).
const LINE_LIMIT = 1 << 20;
const BODY_LIMIT = LINE_LIMIT - 64 - 64 * 72;
const body = (req, res) => readBody(req, BODY_LIMIT, res); // over it, answered before the connection closes
const ANSWER_MS = 120_000, POLL_MS = 25_000, KEPT = 256, HOLD_MS = 15 * 60_000, HELD = 64;
// A paired client's name (scripts/tamber-pairing.mjs): no local client takes one.
const REMOTE = 'remote:';
const GONE = 'this paired client was revoked or its token expired';
export const METHODS = new Set(['hello', 'describe', 'resolve', 'explain', 'uses', 'propose', 'transaction', 'undo', 'commit', 'accept', 'drop', 'history', 'revert', 'import', 'export', 'pin', 'preview', 'inspect', 'outline', 'hold', 'navigate', 'scrub', 'scrub-release', 'scrub-cancel', 'clients', 'revoke', 'pair-code', 'canvas']);
const WRITES = new Set(['transaction', 'undo']);

const json = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(body));
};
const notFound = res => json(res, 404, { message: 'Not found.' });

/** `contract tamber serve --stdio <app>`, through cargo as the dev loops run
 * the compiler, in a process group of its own so stopping it stops the
 * compiler cargo starts. */
function spawnContract(appDir) {
  return spawn('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'serve', '--stdio', appDir], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
}

/** The wire for one app: `handle(req, res, url, access)` answers the
 * `/__tamber` paths (true when it did); `close()` stops the server and
 * resolves once it has let go of the design's lock. `scrub`, from a dev
 * loop, previews a scrub on its builds; `scopeOf(req)` is a request's scope
 * (LLP 1122 D6's ladder): the one token is the local tool's, which writes,
 * and paired clients' scoped tokens name theirs. `sourceMap(digest)` is the
 * source map of the plan with that digest (`app.plan.map.json` bytes, or
 * null), or with none the running generation's, which an inspected
 * selection is joined to and outlines are drawn from (LLP 1122 D5, T2c);
 * `designServed()` the
 * design it serves now (a live edit's) as `{digest, source}`, `source` the
 * SHA-256 of the design file it was made from, or null for the plan's own.
 * `pairTtlMs` is a paired token's lifetime (`--pair-ttl`), `clock` the
 * pairing's time (tests), `remote()` the run's open remote doors (stage 10
 * B's `list()`), shown with each pairing code. The returned `pairing` is the
 * run's paired clients, for stage 10 C's gateway (`lookup`, `onEnd`); `canvas(pairing)`
 * makes that gateway (scripts/tamber-canvas.mjs; null for none), which
 * `canvas` answers once its port is open (`openCanvas`). */
export function tamberServer({ appDir, spawnServe = spawnContract, log = console.log, settle = 50, toolDir = TOOL_DIST, live = false, answerMs = ANSWER_MS, scrub = null, scrubIdleMs, scopeOf = () => 'write', page = null, sourceMap = () => null, designServed = () => null, pairTtlMs = PAIR_TTL_MS, clock = Date.now, remote = () => [], canvas = pairing => canvasGateway({ pairing }), importLimits }) {
  const token = randomBytes(32).toString('hex');
  // This run's name on every event: a number from another run means nothing.
  const epoch = randomBytes(8).toString('hex');
  const waiting = new Map(), subscribers = new Set(), polls = new Set(), watchers = new Map(), events = [];
  let kept = 0;
  // A snapshot (the held list, the files) goes out only while all that is queued to open streams and polls stays under 128 MiB: slow readers can't hold the server's memory. (Queued: the response's count or its socket's, the more: behind a TLS door Bun counts only the socket's.)
  const queuedOn = r => Math.max(r.writableLength || 0, r.socket?.writableLength || 0); // (outbound: each response and its socket, the socket until it closes, so a finished answer's bytes still waiting count)
  const outbound = new Set(), fits = bytes => [...outbound].reduce((n, r) => n + (r.writableLength || 0), 0) + bytes <= 128 << 20;
  const track = res => { for (const w of [res, res.socket]) if (w?.on && !outbound.has(w)) { outbound.add(w); w.on('close', () => outbound.delete(w)); } };
  // The digests of the exports this run gave out, newest last (at most 64): a
  // client's `import` may name one as the baseline it pulled (stage 10b).
  const exported = [];
  // Each file's digest as last announced: what a later change is told from.
  const known = new Map();
  let next = 0, seq = 0, out = '', stderr = '', down = null, writing = 0, again = false, timer = null, first = true, proposalCount = 0;
  // What `commit` needs of the session, and who last changed each file.
  const session = designSession(appDir), lastBy = new Map();
  const design = basename(designPath(appDir) ?? 'app.tamber');
  // What a client reads of this machine's files (tamber/1.15):
  // every `file` named by one rule (`tamber-paths.mjs`, the design server's
  // too), for every client; and a client that is not local (paired, through
  // B's doors or a tunnel) gets machine-written messages with this machine's
  // directories taken out. Authored values and file contents go as they are.
  const scrubText = messageScrubber(appDir);
  const forRemote = body => scrubbedMessages(body, scrubText);
  // A source map's nodes with their files named by that rule, once a map read (a map can move under one plan digest).
  const named = new WeakMap();
  const namedMap = m => { if (!named.has(m)) named.set(m, { ...m, nodes: m.nodes.map(n => namedPaths(n, appDir)) }); return named.get(m); };

  const send = event => {
    let line;
    event = Object.assign(namedPaths(event, appDir), { seq: event.seq });
    try { line = `data: ${JSON.stringify(event)}\n\n`; } catch { log(`tamber: an event could not be sent (${event.event})`); return; }
    // The last 256 kept, at most 64 MiB of them (each event's size beside it).
    const bytes = Buffer.byteLength(line); events.push(Object.defineProperty(event, 'bytes', { value: bytes })); kept += bytes;
    while (events.length > KEPT || kept > 64 << 20) kept -= events.shift().bytes;
    // One `only` a client (a hold's refusal, its owner's: theme2's 07:48 (c)) is that client's alone.
    // A paired subscriber past its deadline gets nothing more (`live` ends it now).
    // A subscriber 8 MiB behind is cut (it reconnects and catches up from `events`), never buffered without end.
    const remoteLine = `data: ${JSON.stringify(forRemote(event))}\n\n`;
    for (const res of subscribers) if (!res.writableEnded && (!res.paired || pairs.live(res.paired)) && (!event.only || res.client === event.only)) { if (queuedOn(res) > 8 << 20) { subscribers.delete(res); res.destroy(); } else res.write(res.paired ? remoteLine : line); }
    for (const poll of polls) poll();
  };
  // The dev loop's running builds (LLP 1122 D4): registered on its streams,
  // their reports told to every client, their pins each client's own.
  // Each build's status a stale selection was last asked again under.
  const refreshed = new Map();
  // Each build's last published selection: what a client below `write` reads
  // (`inspect` `refresh`), for builds still registered, at most 256.
  const lastSelection = new Map();
  const pruneSelections = () => { const live = new Set(channel.list().map(b => b.id)); for (const id of lastSelection.keys()) if (!live.has(id)) lastSelection.delete(id); };
  const remember = (id, selection) => {
    lastSelection.delete(id); lastSelection.set(id, selection);
    pruneSelections();
    while (lastSelection.size > 256) lastSelection.delete(lastSelection.keys().next().value);
  };
  // A selection (LLP 1122 D5) joined to source: refused and asked again
  // when it is from before the build's latest status; else published as a
  // `selection`, and every copy of its node outlined.
  // Source maps by plan digest (LLP 1122 T2c): each build's own plan's, as
  // the dev loop gives it now (`sourceMap(digest)`; with none, the running
  // generation's). A map can change under one plan digest (an edit that
  // moves only source locations), so the dev loop is asked each time; its
  // bytes are parsed once.
  const parsed = new WeakMap();
  const mapOf = (digest) => {
    let raw = null, map = null;
    try { raw = sourceMap(digest); } catch { raw = null; }
    if (!raw || typeof raw !== 'object') return null;
    if (parsed.has(raw)) map = parsed.get(raw);
    else { try { map = JSON.parse(Buffer.from(raw).toString('utf8')); } catch { map = null; } parsed.set(raw, map); }
    return map?.nodes && typeof map.digest === 'string' && (!digest || map.digest === digest) ? map : null;
  };
  const channel = buildChannel({
    // A build that reports no selection (inspect mode left) has none to read.
    send: e => { if (e.event === 'selected' && e.selected == null) lastSelection.delete(e.build); send({ ...e, seq: ++seq, epoch }); }, log,
    // A held site in a build's new plan (LLP 1122 D8): the one site at the
    // old site's source location and call-site chain, by the two plans'
    // maps (a component called twice is two sites there); none otherwise.
    correspond: (from, to, site) => {
      const b = mapOf(to);
      // (A map it cannot read through is no correspondence: cleared, said so.)
      try { return b && !staleMap(b, readFileSync, appDir) ? correspondingSite(mapOf(from), b, site) : null; } catch (e) { log(`tamber: no correspondence for a hold: ${e.message}`); return null; }
    },
    selected: (build, report, ask) => {
      if (!current(build, report)) {
        // Asked again once per status: its status first (one lost on the way
        // would leave every selection stale), then the selection.
        const at = JSON.stringify([build.plan, build.planDigest ?? null, build.design, build.status?.revision]);
        if (refreshed.get(build.id) !== at) { refreshed.set(build.id, at); ask({ kind: 'inspect', refresh: true, status: true }); }
        return { status: 409, body: { message: 'a selection from before the build\'s latest status (a batch or a pin since): asked again' } };
      }
      refreshed.delete(build.id);
      // (A selection that does not name its plan joins with no map: read-only.)
      const map = report.selected.planDigest ? mapOf(report.selected.planDigest) : null;
      // A plan the params were not read for (a package's component changed,
      // which moves no file of the app's): read again, its names kept
      // meanwhile and no alias offered.
      if (report.selected.planDigest && report.selected.planDigest !== params.plan) learnParams(null, report.selected.planDigest);
      const selection = join(build, report.selected, map, { served: designServed(), design: designPath(appDir), params: params.map, app: appDir, own: known.size ? new Set(ownContracts()) : null });
      send({ event: 'selection', seq: ++seq, epoch, ...selection });
      remember(build.id, namedPaths(selection, appDir));
      // Outlined as the client that owns inspect mode on that build, naming
      // the click it answers (its page does not scroll for it).
      // Every copy on every build (D5): the selection's source span, from its
      // own plan's map, resolved through each build's own plan's map (the
      // running generation's for a build that says no plan); a build whose
      // map is missing or stale is sent none.
      if (selection.source) {
        const at = new Map();
        channel.outline(channel.inspector(build.id) ?? 'inspect', b => {
          const m = b.planDigest ? mapOf(b.planDigest) : mapOf();
          if (!m) return null;
          if (!at.has(m.digest)) at.set(m.digest, staleMap(m, readFileSync, appDir) ? null : sitesAt(m, selection.source));
          return at.get(m.digest) ? { sites: at.get(m.digest), plan: m.digest } : null;
        }, report.click ?? null);
      }
      return { publish: published(report.selected, params.map) };
    },
  });
  // Scrub sessions (tamber-scrub.mjs): previewed by the dev loop (`scrub`:
  // `{patch(values, frame, files)?, verify(texts)?, preview(texts, frame),
  // restore(frame)}`, each frame its batch's mark), one at a time, each end
  // told.
  const scrubs = scrubSessions({
    ask: r => ask(r),
    write: r => write(r),
    patch: scrub?.patch ? (values, frame, files) => scrub.patch(values, frame, files) : null,
    verify: scrub?.verify ? texts => scrub.verify(texts) : null,
    preview: (texts, frame) => scrub.preview(texts, frame),
    restore: frame => scrub?.restore(frame),
    released: frame => scrub?.released?.(frame),
    // A paired client's deadline is checked here too: its timer may not have run.
    allowed: client => !gone(client),
    opened: e => send({ event: 'scrub', seq: ++seq, epoch, ...e, reason: 'opened' }),
    ended: e => send({ event: 'scrub', seq: ++seq, epoch, ...e }),
    idleMs: scrubIdleMs,
  });
  // Which clients are present (LLP 1122 D4's ownership): a client holding a
  // subscription that names it, or a long poll open or answered within the
  // last 35 s (the poll's 25 s and some), as a client whose fetch reads a
  // body whole holds the wire. A client no longer present loses its pins on
  // every build. The same timer expires a polled build that stopped polling.
  const presence = new Map(); // client -> { open, answered }
  const PRESENT_MS = 35_000;
  // A paired client revoked or expired is never present again (its streams'
  // closes come after); one past its deadline is gone before its end runs.
  const ended = new Set();
  const gone = client => ended.has(client) || (!!client?.startsWith(REMOTE) && !pairs.live(client));
  const present = client => { const p = presence.get(client); return !gone(client) && !!p && (p.open > 0 || Date.now() - p.answered < PRESENT_MS); };
  const attend = (client, delta, answered = false) => {
    if (!client) return;
    const p = presence.get(client) ?? { open: 0, answered: 0 };
    p.open += delta;
    if (answered) p.answered = Date.now();
    presence.set(client, p);
  };
  const absent = () => { pairs.expire(); channel.sweep(); scrubs.absent(present); for (const client of channel.owners()) if (!present(client)) { channel.clear(client); presence.delete(client); } };
  const absence = setInterval(absent, 5_000);
  absence.unref?.();
  /** Announce `files` (path → digest): the paths whose digest moved, and
   * any `gone`. */
  const publish = (files, rest, gone = []) => {
    const edited = Object.entries(files).filter(([path, d]) => known.get(path) !== d).map(([path]) => path);
    for (const [path, d] of Object.entries(files)) known.set(path, d);
    for (const path of gone) known.delete(path);
    edited.push(...gone);
    for (const path of edited) lastBy.set(path, rest.client ?? 'changed on disk');
    if (!rest.client) session.seen(Object.fromEntries(edited.map(path => [path, files[path] ?? ''])));
    // A hand edit competes with a scrub as a write does.
    if (!rest.client && edited.length) scrubs.compete();
    if (edited.length || rest.client) send({ event: 'changed', seq: ++seq, epoch, ...rest, files: Object.fromEntries(known), edited });
    // A write's files are the design's now: its params are read again.
    if (edited.length) learnParams(Object.fromEntries(known));
  };

  // The server gone (never started, the lock another's, exited): every
  // request in flight, every subscriber and every later request told so.
  const fail = message => {
    if (down) return;
    down = message;
    log(`tamber: ${message}`);
    for (const reply of waiting.values()) reply({ error: { message } });
    waiting.clear();
    scrubs.lost();
    send({ event: 'down', seq: ++seq, epoch, message });
    for (const res of subscribers) res.end();
    subscribers.clear();
  };
  const child = spawnServe(appDir);
  child.on('error', e => fail(`the design server did not start: ${e.message}`));
  child.on('exit', code => fail(stderr.trim().split('\n').pop() || `the design server exited ${code}`));
  child.stdin.on('error', () => {});
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-4000); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    out += chunk;
    const lines = out.split('\n'); out = lines.pop();
    for (const text of lines) {
      if (!text.trim()) continue;
      let v; try { v = JSON.parse(text); } catch { continue; }
      // A write's event is published from its reply (below), in order.
      if (v.event) continue;
      // The server answers in order: a reply it could not tie to a request
      // (a line it could not read) is the oldest one's.
      const id = v.id ?? waiting.keys().next().value;
      const reply = waiting.get(id);
      waiting.delete(id);
      reply?.(v);
    }
  });

  /** One request's reply. `done`, when given, sees the server's own reply
   * even after the client was told it timed out (`wait` ms; none for the
   * first read, which may wait on a cold build of the compiler). */
  const ask = (request, { wait = answerMs, done = () => {} } = {}) => new Promise(reply => {
    if (down) { done(null); return reply({ error: { message: down } }); }
    let line;
    try { line = JSON.stringify({ ...request, id: ++next }); } catch { done(null); return reply({ status: 400, error: { message: 'the request is too deeply nested' } }); }
    if (Buffer.byteLength(line) >= LINE_LIMIT) { done(null); return reply({ status: 413, error: { message: 'a request is at most 1 MiB' } }); }
    const id = next;
    const late = wait && setTimeout(() => reply({ status: 504, error: { message: 'the design server did not answer' } }), wait);
    waiting.set(id, v => { clearTimeout(late); done(v); reply(v); });
    child.stdin.write(`${line}\n`);
  });

  // The design's files as the server has them now, announced where a
  // digest moved, and every directory holding one watched. A file that
  // appears (a new design, a checkout) is found by asking again.
  // Taken as the server answers, in order with its writes' replies, so an
  // older read never lands after a newer write.
  const read = reply => {
    // A failed read is said, and read again at the next change; until the
    // first succeeds, every 2 s too.
    if (!reply?.ok) {
      log(`tamber: ${reply?.error?.message ?? 'no answer'}`);
      if (first && !down) setTimeout(rescan, 2000).unref?.();
      return;
    }
    // The first read is the baseline only while nothing has been announced (its packages' too, *1.14*).
    const baseline = first && known.size === 0; if (baseline) { first = false; for (const [path, d] of Object.entries(reply.ok.files)) known.set(path, d); }
    else { first = false; publish(reply.ok.files, { changed: null, client: null }, [...known.keys()].filter(path => !(path in reply.ok.files))); }
    packaged(reply.ok.packages, baseline, reply.ok.watch); watchAll(Object.keys(reply.ok.files));
    learnParams(reply.ok.files);
  };
  // The app's component params (LLP 1122 T3), read again when its files
  // move: the join names a row's param by them, never by its IR token. A
  // file change empties the map until the read answers (a row reading a
  // param is read-only meanwhile, said so); a read that fails is tried
  // again at the next change. A new plan (`plan`) reads them again too.
  // While a new plan's read is out, the names stand but no alias is offered
  // (one may have moved with a package's component).
  const params = { key: null, plan: null, map: new Map(), reading: 0 };
  const learnParams = (files, plan = params.plan) => {
    const key = files ? JSON.stringify(Object.entries(files).sort()) : params.key;
    if (params.key === key && params.plan === plan) return;
    if (files && params.key !== key) params.map = new Map();
    else params.map = new Map([...params.map].map(([ir, p]) => [ir, { ...p, token: null }]));
    params.key = key; params.plan = plan;
    const at = ++params.reading;
    const failed = () => { if (at === params.reading) { params.key = null; params.plan = null; } };
    ask({ method: 'describe' }).then(r => {
      if (at !== params.reading) return;
      // Of the files this read was for, or not kept (they moved meanwhile).
      const of = r?.ok?.files ? JSON.stringify(Object.entries(r.ok.files).sort()) : null;
      if (r?.ok?.ir && (!files || of === key)) params.map = paramsOf(r.ok.ir);
      else failed();
    }, failed);
  };
  const refresh = () => ask({ method: 'hello', paths: true }, { wait: first ? 0 : answerMs, done: read });
  // The app's directory (where a new design beside it appears) and the
  // design's, whatever the first read gave, and any other holding a file.
  // A directory removed and made again is watched again (by its inode).
  const watchAll = paths => {
    // Each file's directory and every one between it and the app's, whose
    // names are holders too: a directory replaced (a checkout) is found by
    // its parent's event (a Contract source under `src/ui/`).
    const between = path => {
      for (let dir = dirname(resolve(appDir, path)); dir.startsWith(appDir + sep); dir = dirname(dir)) { holders.add(basename(dir)); holding.add(dir); }
    };
    // Every directory that ever held one stays watched (by its inode, once
    // it is there again): a read taken while a checkout has it empty or
    // gone does not drop it.
    paths.forEach(between);
    for (const dir of new Set([appDir, dirname(designPath(appDir) ?? resolve(appDir, design)), ...holding])) {
      let ino = null;
      try { ino = statSync(dir).ino; } catch { /* not there now */ }
      const held = watchers.get(dir);
      if (held && held.ino === ino) continue;
      held?.watcher.close();
      watchers.delete(dir);
      if (ino === null) continue;
      try {
        watchers.set(dir, { ino, watcher: watch(dir, (_, name) => {
          // A design file, or one that may be: `.tamber`, the design's own
          // name (app.json may name any), a directory a design is in, or a
          // name the platform did not give; and the app's own Contract
          // sources, whose state parts the server writes (LLP 1121 stage 8 D).
          const n = name && basename(String(name));
          if (n && !n.endsWith('.tamber') && !n.endsWith('.contract') && n !== design && !holders.has(n)) return;
          clearTimeout(timer); timer = setTimeout(rescan, settle);
        }) });
      } catch (e) { log(`tamber: cannot watch ${dir}: ${e.message}`); }
    }
  };
  // The directories that held a design file or a Contract source.
  const holding = new Set();
  // The directories between the app's and each design file's, by name.
  const holders = new Set([...relative(appDir, dirname(designPath(appDir) ?? resolve(appDir, design))).split(sep).filter(Boolean), 'node_modules']); const packaged = packageDesigns({ log, changed: () => { clearTimeout(timer); timer = setTimeout(rescan, settle); }, announce: (edited, packages) => { scrubs.compete(); params.key = null; send({ event: 'changed', seq: ++seq, epoch, changed: null, client: null, files: Object.fromEntries(known), edited, packages }); } });
  // While a write is in flight its own renames are not a hand edit.
  const rescan = () => {
    timer = null;
    if (writing) { again = true; return; }
    void refresh();
  };
  watchAll([]);
  const ready = refresh();
  // Paired clients (stage 10 A2, scripts/tamber-pairing.mjs). A burn or a
  // rate-limit hit is logged with its peer and told to the clients without
  // it, never with a code.
  const pairs = pairing({ ttlMs: pairTtlMs, clock, say: w => { log(w.relay ? `tamber: relayed pairing limited (10 a minute): try again in ${w.retry} s` : `tamber: pairing: ${w.what}${w.peer ? ` (from ${w.peer})` : ''}${w.why ? `: ${w.why}` : ''}`); send({ event: 'pairing', seq: ++seq, epoch, what: w.what }); } });
  const gateway = canvas ? canvas(pairs) : null;
  let relayGateway = null; // (stage 10c R1b: the relay's canvas, its own origin pair)
  // A build's report (a registration replays pins, inspect and holds) comes
  // after every paired client past its deadline has ended, as a request does.
  const reportFrom = channel.report;
  channel.report = body => { pairs.expire(); return reportFrom.call(channel, body); };
  // A revoked token's hash: its next request is told it was revoked (401),
  // any other unknown token gets not-found.
  const revokedTokens = new Set();

  const tokenOf = req => {
    const given = Buffer.from(String(req.headers['x-exact-tamber-token'] ?? ''));
    const mine = Buffer.from(token);
    return given.length === mine.length && timingSafeEqual(given, mine);
  };
  /** Who a request is: the local token (`local`, its scope `scopeOf`'s), a
   * paired client by its token, or null. The token is read from its header
   * only, never a cookie (the lead's rule: canvas page code can set cookies
   * this port receives). Through B's remote listener or a trusted tunnel
   * the local token is no one (ruling 3). */
  const principalOf = (req, access) => {
    // The local token is the local tool's only where the request is local
    // (the development gate's whole loopback test): a LAN peer holding it is
    // no one, as is any request through B's listener or a tunnel.
    if (access?.local === true && tokenOf(req)) return { scope: scopeOf(req), local: true, client: null };
    const paired = pairs.lookup(String(req.headers['x-exact-tamber-token'] ?? ''));
    return paired ? { scope: paired.scope, local: false, client: paired.client, token: String(req.headers['x-exact-tamber-token']) } : null;
  };
  const wasRevoked = req => revokedTokens.has(createHash('sha256').update(String(req.headers['x-exact-tamber-token'] ?? '')).digest('hex'));
  async function handle(req, res, url, access) {
    if (Math.max(req.socket?.writableLength || 0, res.writableLength || 0) > 8 << 20) { req.socket?.destroy(); return true; } // (a connection 8 MiB behind on what it was sent is cut: answers queue only while read)
    // Every paired client past its deadline ends before a request is read.
    pairs.expire();
    // Through B's remote listener or a trusted tunnel nothing is local,
    // whatever else `access` says: no token page, no tool, no local token.
    if (access?.remote || access?.tunnel) access = { ...access, local: false };
    if (url.pathname === '/__tamber/tool' || url.pathname.startsWith('/__tamber/tool/')) { serveTool(req, res, url, access, toolDir); return true; }
    if (url.pathname === '/__tamber/token') {
      if (req.method !== 'GET' || !access.local) notFound(res);
      else json(res, 200, { token });
      return true;
    }
    const wire = ['/__tamber', '/__tamber/subscribe', '/__tamber/events'].includes(url.pathname);
    // Plain HTTP from off this machine (a --lan bind's peer) reaches nothing
    // of the wire, pairing and paired tokens included: remote use is B's TLS
    // door or a tunnel the developer trusts, never plaintext (LLP 1122 D12).
    if ((wire || url.pathname === '/__tamber/pair') && !viaDoor(access)) { notFound(res); return true; }
    // The one path that needs no token (stage 10 A2): a pairing code for a
    // token, once.
    if (url.pathname === '/__tamber/pair') { await pairEndpoint(req, res, access); return true; }
    if (!wire) return false;
    const who = req.method === 'POST' ? principalOf(req, access) : null;
    if (!who) { if (req.method === 'POST' && wasRevoked(req)) json(res, 401, { error: { message: GONE } }); else notFound(res); return true; }
    const scope = who.scope;
    // The change stream's paths by the table too (scripts/tamber-pairing.mjs).
    const stream = url.pathname === '/__tamber' ? null : needs({ method: url.pathname.slice('/__tamber/'.length) });
    if (url.pathname !== '/__tamber' && !allows(scope, stream)) { const r = refusal(url.pathname.slice('/__tamber/'.length), scope, stream); json(res, r.status, { error: r.error }); return true; }
    // A remote door's request is read whole first: its connection deadline
    // (tamber-remote.mjs) clears only once the body has ended.
    const early = access.remote ? body(req, res) : null;
    // Over the limit, answered at once (not behind a server still starting, past refuseBody's 5 s); a client gone, nothing.
    const first = early && await earlyAnswer(early, ready);
    if (first === 'over' || first === 'gone') { if (first === 'over' && !res.destroyed) json(res, 413, { error: { message: 'a request is at most 1 MiB' } }); return true; }
    await ready;
    // The client left while the server started.
    if (res.destroyed || res.writableEnded) return true;
    const text = await (early ?? body(req, res));
    if (text == null) { if (text === null) json(res, 413, { error: { message: 'a request is at most 1 MiB' } }); return true; } // (undefined: the client left)
    const request = parse(text);
    // Nested past 64 levels: refused before anything of it is kept, since a
    // value kept (a held proposal's review) is sent on to every stream.
    if (request && tooDeep(request)) { json(res, 400, { error: { message: 'a request is nested at most 64 levels deep' } }); return true; }
    // Revoked or expired while its body came: refused before anything it asks runs.
    if (who.client && !pairs.live(who.client)) { json(res, 401, { error: { message: GONE } }); return true; }
    // A paired client is the client its token names, whatever its body says.
    // A local client may not name itself as a paired one (`remote:`).
    if (request && who.client) request.client = who.client;
    else if (request && !(url.pathname === '/__tamber' && request.method === 'revoke') && typeof request.client === 'string' && request.client.startsWith(REMOTE)) { json(res, 400, { error: { message: `a client named \`${REMOTE}…\` is a paired client's; name yours otherwise` } }); return true; }
    if (url.pathname === '/__tamber/subscribe') {
      if (down) { json(res, 503, { error: { message: who.local ? down : scrubText(down) } }); return true; }
      const snapshot = namedPaths({ event: 'subscribed', seq, epoch, files: Object.fromEntries(known), proposals: held(), builds: channel.list() }, appDir);
      const hello = `data: ${JSON.stringify(who.local ? snapshot : forRemote(snapshot))}\n\n`;
      if (!fits(Buffer.byteLength(hello))) { json(res, 503, { error: { message: 'too much is queued to slow readers: try again shortly' } }); return true; }
      track(res);
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      res.write(hello);
      // `{client}`: its commands live while it is present (LLP 1122 D4).
      const client = typeof request?.client === 'string' && request.client ? request.client : null;
      res.client = client;
      res.paired = who.client;
      subscribers.add(res);
      attend(client, 1);
      // Revoked or expired: its stream ends at once.
      const unend = who.client ? pairs.onEnd(who.client, () => { subscribers.delete(res); res.end(); }) : () => {};
      res.on('close', () => {
        unend();
        subscribers.delete(res);
        // A subscription counts while open; a long poll for 35 s after.
        attend(client, -1);
        if (client && !present(client)) { channel.clear(client); scrubs.absent(present); }
      });
      return true;
    }
    if (url.pathname === '/__tamber/events') {
      const after = request?.after ?? seq;
      if (!Number.isSafeInteger(after) || after < 0) { json(res, 400, { error: { message: '`after` is a sequence number' } }); return true; }
      if (request?.held !== undefined && (!Number.isSafeInteger(request.held) || request.held < 0)) { json(res, 400, { error: { message: '`held` is the held list\'s revision, a whole number' } }); return true; }
      // `{client}`: present while this poll is open, and for 35 s after.
      const client = typeof request?.client === 'string' && request.client ? request.client : null;
      attend(client, 1);
      try { return await longPoll(); } finally { attend(client, -1, true); }
    }
    async function longPoll() {
      const after = request?.after ?? seq;
      const answer = () => {
        const since = events.filter(e => e.seq > after && (!e.only || e.only === request?.client));
        // Fewer kept than asked for, or a number from before this server
        // started: the client missed some, and reads again.
        const missed = (request?.epoch !== undefined && request.epoch !== epoch) || after > seq || after < seq && (events[0]?.seq ?? seq + 1) > after + 1;
        // Missed: no events to apply, the files to start again from.
        // The held list, unless the client named its revision in this run (1.12).
        const has = request?.held === heldRevision && request?.epoch === epoch;
        const body = namedPaths({ events: missed ? [] : since, seq, epoch, missed, files: Object.fromEntries(known), held: heldRevision, ...(has ? {} : { proposals: held() }) }, appDir);
        if (!fits(Buffer.byteLength(JSON.stringify(body)))) return json(res, 503, { error: { message: 'too much is queued to slow readers: try again shortly' } }); else track(res);
        json(res, 200, who.local ? body : forRemote(body));
      };
      // The design server gone: what is kept after `after` (its `down`
      // among it), else an error, which a following client retries slowly,
      // never an empty answer it would ask again at once.
      if (down && !events.some(e => e.seq > after)) { json(res, 503, { error: { message: who.local ? down : scrubText(down) } }); return true; }
      if (down) { answer(); return true; }
      // `wait: false` (a client's first ask, for where to start) is answered at once.
      if (request?.wait === false || events.some(e => e.seq > after) || after > seq || (request?.epoch !== undefined && request.epoch !== epoch)) { answer(); return true; }
      await new Promise(ok => {
        // Revoked or expired: the poll is answered 401 at once.
        let unend = () => {}, done = false;
        const poll = () => { if (done) return; done = true; polls.delete(poll); clearTimeout(wait); unend(); ok(); };
        const wait = setTimeout(poll, POLL_MS);
        polls.add(poll);
        res.on('close', poll);
        // Revoked or expired: the poll is answered 401 at once (below).
        if (who.client) unend = pairs.onEnd(who.client, poll);
      });
      // Woken by its own client's end (revoke, expiry): told so, not answered.
      if (who.client && (ended.has(who.client) || !pairs.live(who.client))) { if (!res.writableEnded) json(res, 401, { error: { message: GONE } }); return true; }
      if (!res.writableEnded && !res.destroyed) answer();
      return true;
    }
    if (!request || !METHODS.has(request.method)) {
      json(res, 400, { error: { message: `a request is an object whose method is one of ${[...METHODS].join(', ')}` } });
      return true;
    }
    delete request.id;
    let reply;
    // An import stops with its request (the client gone) or its client (revoked, expired).
    const gone = new AbortController();
    let unend = () => {};
    if (request.method === 'import') {
      res.on('close', () => { if (!res.writableEnded) gone.abort(); });
      if (who.client) unend = pairs.onEnd(who.client, () => gone.abort());
    }
    // A request the server can't take (one too deeply nested to send on,
    // say) is refused, never an uncaught failure.
    try { reply = await dispatch(request, scope, who.local, who.token, gone.signal, access?.door); } catch (e) { reply = { status: 400, error: { message: `the request could not be handled: ${String(e?.message ?? e).slice(0, 200)}` } }; }
    unend();
    const status = reply.ok !== undefined ? 200 : reply.status ?? (down ? 503 : reply.error?.conflict ? 409 : 400);
    const answered = namedPaths(reply.ok !== undefined ? { ok: reply.ok } : { error: reply.error }, appDir);
    json(res, status, who.local ? answered : forRemote(answered));
    return true;
  }

  /** `POST /__tamber/pair {code, name?}` (stage 10 A2): the one request
   * that carries no token. The token is answered in the body, once; the
   * peer is B's (`access.peer`), else the socket's. A paired client's end
   * (revoke, expiry) takes its presence, pins, scrub and long polls with it,
   * keeps its held proposals marked as a revoked client's (theme2's ruling
   * (b)), and is told to the others. */
  async function pairEndpoint(req, res, access) {
    if (req.method !== 'POST') { notFound(res); return; }
    const peer = String(access?.peer ?? req.socket?.remoteAddress ?? 'unknown');
    // Limited before its body is read or parsed: a flood costs no parsing.
    const limited = pairs.admit(peer);
    if (limited) { res.setHeader('retry-after', String(limited.retry)); json(res, 429, { error: limited.error }); return; }
    const request = parse((await body(req, res)) ?? '');
    if (!request) { json(res, 400, { error: { message: 'pairing is `{"code": "…", "name"?: "…"}`' } }); return; }
    const reply = pairs.pair(request, peer, { admitted: true });
    if (!reply.ok) {
      if (reply.retry) res.setHeader('retry-after', String(reply.retry));
      json(res, reply.status, { error: reply.error });
      return;
    }
    const { client } = reply.ok, hash = createHash('sha256').update(reply.ok.token).digest('hex');
    pairs.onEnd(client, reason => {
      revokedTokens.add(hash);
      ended.add(client);
      presence.delete(client);
      channel.clear(client);
      scrubs.absent(present);
      let marked = false;
      for (const p of proposals.values()) if (p.client === client) { p.shown = { ...p.shown, revoked: true }; marked = true; }
      if (marked) heldRevision++;
      log(`tamber: ${client} ${reason}`);
      send({ event: 'revoked', seq: ++seq, epoch, client, reason });
    });
    log(`tamber: ${client} paired (${reply.ok.scope}) from ${peer}${access?.reported ? ` (${String(access.reported).slice(0, 64)}, as the relay reports it)` : ''}`);
    send({ event: 'paired', seq: ++seq, epoch, client, scope: reply.ok.scope });
    json(res, 200, { ok: reply.ok });
  }

  // What the wire itself reads of a review: the offers', dials' and pins' items, the tokens' count.
  const reviewShaped = r => typeof r === 'object' && r !== null && !Array.isArray(r)
    && ['offers', 'dials', 'pins'].every(k => r[k] === undefined || Array.isArray(r[k]) && r[k].every(o => typeof o === 'object' && o !== null && (o.items === undefined || Array.isArray(o.items) && o.items.every(Number.isInteger))))
    && (r.tokens === undefined || Number.isInteger(r.tokens) && r.tokens >= 0);
  const needsClient = request => typeof request.client === 'string' && request.client ? null : { status: 400, error: { message: `${request.method} names its \`client\`` } };

  /** One request: the server's, or one of the wire's own (`commit`, a held
   * proposal's `accept` and `drop`). */
  async function dispatch(request, scope, local = false, paired = null, signal = undefined, door = undefined) {
    // Every method's scope, in one place, before any handler: none is no
    // method this wire serves (fail closed; scripts/tamber-pairing.mjs).
    const need = needs(request, id => proposals.get(id)?.client ?? null);
    if (need === null) return { status: 400, error: { message: `\`${request.method}\` has no scope on this wire` } };
    if (!allows(scope, need, local)) return refusal(request.method, scope, need);
    // Presence is also checked as requests come, not only every 5 s.
    absent(); delete request.restore; // `restore` (a revert's or an import's override line, *1.14*) is the server's
    // A paired client's canvas grant (stage 10 C), for the token it came with.
    // (Plain HTTP from off this machine never reaches here: `viaDoor`, above.)
    // (minted by the gateway of the door the request came through: the relay's own pair, or the TLS door's; never across)
    const gw = door === 'relay' ? relayGateway : gateway;
    if (request.method === 'canvas') return !gw ? { status: 404, error: { message: 'this server has no canvas gateway' } } : paired ? gw.mint(paired) : { status: 400, error: { message: '`canvas` is a paired client\'s: the local tool frames the dev loop itself' } };
    if (request.method === 'pair-code') {
      // Beside the code, where a remote client reaches this server and the certificate it checks (1.13).
      const minted = pairs.mint({ scope: request.scope, name: request.name }), doors = minted.ok ? remote() : [];
      return doors.length ? { ...minted, ok: { ...minted.ok, remote: doors } } : minted;
    }
    if (request.method === 'clients') return { ok: { clients: [{ client: 'local', local: true, scope: 'write' }, ...pairs.list()] } };
    if (request.method === 'revoke') {
      if (typeof request.client !== 'string' || !pairs.has(request.client)) return { status: 404, error: { message: `no paired client \`${request.client}\`` } };
      pairs.revoke(request.client);
      return { ok: { revoked: request.client } };
    }
    if (WRITES.has(request.method)) return needsClient(request) ?? write(request);
    if (request.method === 'commit') {
      const refused = needsClient(request);
      if (refused) return refused;
      if (request.message !== undefined && typeof request.message !== 'string') return { status: 400, error: { message: '`message` is a string' } };
      if (request.dry !== undefined && typeof request.dry !== 'boolean') return { status: 400, error: { message: '`dry` is true or false' } };
      if (request.scope !== undefined && typeof request.scope !== 'string') return { status: 400, error: { message: '`scope` is a string' } };
      // A write in flight would land beside the commit's reading of the files.
      if (writing) return { status: 409, error: { message: 'a write is in flight; commit again once it lands' } };
      const done = session.commit({ client: request.client, message: request.message, dry: request.dry === true, scope: request.scope });
      // A scope the session has written past is a conflict, as a write in
      // flight is.
      if (done.error?.stale) return { status: 409, error: done.error };
      if (done.ok && !request.dry) send({ event: 'committed', seq: ++seq, epoch, client: request.client, ...done.ok });
      return done;
    }
    if (request.method === 'propose' && request.hold !== undefined) {
      if (request.hold !== true) return { status: 400, error: { message: '`hold` is true when given' } };
      if (request.review !== undefined && !reviewShaped(request.review)) return { status: 400, error: { message: '`review` is an object; its `offers`, `dials` and `pins` lists of objects with `items` (indices), its `tokens` a count' } };
      // A revert's review is the server's own: it is planned again from it.
      if (request.review?.revert !== undefined) return { status: 400, error: { message: '`review.revert` is the server\'s: ask for `revert`' } };
      return needsClient(request) ?? hold(request.client, request.ops, request.files, undefined, request.review, request.requires, scope, undefined, local && !!request.review?.importer);
    }
    if (request.method === 'accept') return needsClient(request) ?? accept(request);
    if (request.method === 'history') return history(request);
    // Which app this server is, and which run of it, so `tamber tool` can
    // tell its own from another's and a client can tell a restart.
    // `live`: a dev loop serves it, whose running builds take a value-only
    // edit in place, no reload (LLP 1121 stage 4's live table); `design`:
    // the file an operation with no `file` edits; `page`: the dev loop's own
    // origin, which the design tool frames, null with no dev loop (1.3).
    if (request.method === 'hello') return ask({ ...request, paths: false }).then(r => (r.ok ? { ok: { ...r.ok, scope, local, ...(local ? {} : { client: request.client }), app: local ? appDir : basename(appDir), epoch, live: typeof live === 'function' ? live() : live, builds: channel.list(), page: typeof page === 'function' ? page() : page, design: relative(appDir, designPath(appDir) ?? resolve(appDir, design)) } } : r));
    if (request.method === 'pin' || request.method === 'preview') return needsClient(request) ?? pin(request);
    if (request.method === 'inspect') return needsClient(request) ?? inspect(request, scope);
    if (request.method === 'outline') return needsClient(request) ?? outline(request);
    if (request.method === 'hold' || request.method === 'navigate') return needsClient(request) ?? (request.method === 'hold' ? holdState(request) : navigateRequest(channel, present, request));
    if (request.method.startsWith('scrub')) return needsClient(request) ?? scrubbing(request);
    if (request.method === 'revert') return needsClient(request) ?? revert(request, scope);
    if (request.method === 'import') return needsClient(request) ?? importHeld(request, scope, signal);
    if (request.method === 'drop') {
      const refused = needsClient(request);
      if (refused) return refused;
      if (!proposals.has(request.proposal)) return { status: 404, error: { message: `no held proposal \`${request.proposal}\`` } };
      forget(request.proposal, { client: request.client, reason: 'dropped' });
      return { ok: { dropped: request.proposal } };
    }
    // Below write an export keeps no baseline: keeping one prunes the oldest, another client's import's.
    if (request.method === 'export') return ask({ ...request, keep: allows(scope, 'write') && request.keep !== false }).then(r => {
      const d = r.ok?.digest;
      if (typeof d === 'string' && !exported.includes(d)) { exported.push(d); if (exported.length > 64) exported.shift(); }
      return r;
    });
    if (request.method === 'propose' && scrub?.inline) return proposeHere(request);
    return ask(request);
  }

  /** `propose` through a dev loop whose pages write some tokens inline (the
   * JS target, LLP 1121 stage 6): a `live` edit that moves one is `rebuild`
   * there, since `class` is the serving loop's (LLP 1121 D13), so the tool
   * says so before the gesture rather than failing a scrub. */
  async function proposeHere(request) {
    const reply = await ask({ ...request, texts: true });
    if (!reply.ok) return reply;
    const ok = { ...reply.ok };
    if (ok.class === 'live' && ok.texts) {
      const why = await scrub.inline(ok.texts).catch(() => null);
      if (why) { ok.class = 'rebuild'; log(`tamber: propose is rebuild here: ${why}`); }
    }
    if (request.texts !== true) delete ok.texts;
    return { ...reply, ok };
  }

  /** `inspect` `{client, build?, on?, refresh?}` (LLP 1122 D4, D5): inspect
   * mode on one build or every build that can, its client's while present.
   * Below `write` it is read-only (the lead's ruling 5; `on` is refused by
   * the table): `refresh` answers the selection each build last reported
   * (`selections`), asking no build again, since a build's answer outlines
   * on the developer's screens. */
  function inspect(request, scope) {
    // (tamber/1.16 `select`, tamber-builds.mjs: the sites n call sites up from the last selection, by its map; write, as `on` is.)
    if (request.select !== undefined) return !allows(scope, 'write') ? refusal('inspect', scope, 'write') : selectRequest(channel, request, (id, up) => { const s = lastSelection.get(id), plan = channel.list().find(b => b.id === id)?.planDigest, map = plan ? mapOf(plan) : null; return map && Number.isSafeInteger(s?.site) ? { levels: levelsUp(map, s.site, up), plan, from: s.site, instance: s.instance ?? [] } : null; });
    if (!allows(scope, 'write')) {
      const build = request.build ?? null;
      if (build !== null && typeof build !== 'string') return { status: 400, error: { message: '`build` is a registered build\'s id' } };
      pruneSelections();
      return { ok: { selections: [...lastSelection].filter(([id]) => build === null || id === build).map(([, s]) => s) } };
    }
    if (!present(request.client)) return { status: 409, error: { message: `inspect: \`${request.client}\` is not present: hold a long poll or a subscription that names it first (\`{client}\`)` } };
    const build = request.build ?? null;
    if (build !== null && typeof build !== 'string') return { status: 400, error: { message: '`build` is a registered build\'s id' } };
    if (request.refresh !== true && typeof request.on !== 'boolean') return { status: 400, error: { message: 'inspect names `on` (true or false), or `refresh: true`' } };
    const done = channel.inspect(request.client, build, { on: request.on === true, refresh: request.refresh === true });
    return done.error ? { status: 400, error: { message: done.error } } : { ok: done };
  }

  /** `outline` `{client, at: [{file, line, col}]}` (LLP 1122 D5): every
   * build that can inspect outlines its views at those source locations (a
   * token's `uses`, a changeset entry's), by the dev loop's current map. */
  function outline(request) {
    if (!present(request.client)) return { status: 409, error: { message: `outline: \`${request.client}\` is not present` } };
    if (!Array.isArray(request.at) || request.at.length > 4096 || !request.at.every(a => a && typeof a.file === 'string' && Number.isInteger(a.line) && Number.isInteger(a.col))) return { status: 400, error: { message: 'outline names `at`: up to 4096 source locations `{file, line, col}`' } };
    // Nothing to outline: the outlines are cleared, map or none.
    if (!request.at.length) return { ok: { sites: [], builds: channel.outline(request.client, null) } };
    // Each build its own plan's sites, from its plan's map (the running
    // one's for a build that does not say its plan); none from a plan with
    // no map here, or one whose sources have moved since.
    const map = mapOf(), at = new Map(), why = new Set();
    let mapless = 0;
    // A location's file and the map's, each named by the one rule (`pathName`), so a file outside the app round-trips and two packages' files stay apart.
    const places = request.at.map(a => ({ ...a, file: pathName(a.file, appDir) }));
    const sitesOf = (m) => { if (!at.has(m.digest)) { const stale = staleMap(m, readFileSync, appDir); if (stale) why.add(stale); at.set(m.digest, stale ? null : [...new Set(places.flatMap(a => sitesAt(namedMap(m), a)))]); } return at.get(m.digest); };
    const builds = channel.outline(request.client, b => { const m = b.planDigest ? mapOf(b.planDigest) : map; if (!m) mapless += 1; const s = m && sitesOf(m); return s ? { sites: s, plan: m.digest } : null; });
    if (!builds.length && (why.size || mapless || !map)) return why.size ? { status: 409, error: { message: [...why].join('; ') } } : { status: 503, error: { message: 'no source map here for the plans the builds run (yet)' } };
    const sites = (map && sitesOf(map)) ?? [];
    log(`tamber: ${request.client} outlined ${request.at.length} location(s) on ${builds.length} build(s)`);
    return { ok: { sites, builds } };
  }

  /** A state held on one node of one build (LLP 1122 D4, D8; *1.11*):
   * `{build, site, instance, state}`, `state` null to let it go; its
   * client's while present, one a build. */
  function holdState(request) {
    if (!present(request.client)) return { status: 409, error: { message: `hold: \`${request.client}\` is not present` } };
    if (typeof request.build !== 'string') return { status: 400, error: { message: 'hold names its `build`, a registered build\'s id' } };
    const held = request.state == null ? null : { site: request.site, instance: request.instance ?? [], state: request.state };
    const done = channel.hold(request.client, request.build, held, typeof request.plan === 'string' ? request.plan : null);
    if (done.error) return { status: 400, error: { message: done.error } };
    if (done.unsupported) return { status: 409, error: { message: done.unsupported, unsupported: 'hold' } };
    return { ok: { held: true, ...done } };
  }

  /** A pin on one build or every build (LLP 1122 D4), its client's while
   * the client holds a subscription naming it. `preview` `{build, axis,
   * value}` pins that build's mode for that axis. */
  function pin(request) {
    if (!present(request.client)) return { status: 409, error: { message: `${request.method}: \`${request.client}\` is not present: hold a long poll or a subscription that names it first (\`{client}\`); a command lives while its client is present` } };
    const build = request.build ?? null;
    if (build !== null && typeof build !== 'string') return { status: 400, error: { message: '`build` is a registered build\'s id' } };
    let facts = request.pin;
    if (request.method === 'preview') {
      if (typeof build !== 'string' || typeof request.axis !== 'string' || typeof request.value !== 'string') return { status: 400, error: { message: 'preview names its `build`, `axis` and `value`' } };
      facts = { mode: { [request.axis]: request.value } };
    }
    const done = channel.pin(request.client, build, facts === undefined ? null : facts);
    return done.error ? { status: 400, error: { message: done.error } } : { ok: done };
  }

  /** A scrub session's request (`write` scope, the dispatcher's): a present
   * client, and a dev loop whose builds preview it. */
  async function scrubbing(request) {
    if (!scrub) return { status: 501, error: { message: 'no builds preview here: a scrub needs the dev loop\'s design server' } };
    if (request.method === 'scrub-cancel') return scrubs.cancel(request);
    if (request.method === 'scrub-release') return scrubs.release(request);
    if (!present(request.client)) return { status: 409, error: { message: `scrub: \`${request.client}\` is not present: hold a long poll or a subscription that names it first; a scrub lives while its client is present` } };
    return scrubs.update(request);
  }

  /** A write, announced from the server's reply, even one that comes after
   * the client was told it timed out; the watcher waits for it. A conflict
   * names who last changed each file that moved. */
  let writes = 0;
  async function write(request, extra = {}, settled = () => {}) {
    // A write cancels a scrub first (its own release has closed it).
    scrubs.compete();
    // The git facts of each file it may touch, before it does (commit's
    // baseline); this write's hold on them ends with its reply.
    const designFile = relative(appDir, designPath(appDir) ?? resolve(appDir, design));
    let holds;
    try {
      // A state part's write without a `file` is the file that declares its
      // style, which the server finds: every Contract source is held.
      const contracts = [...known.keys()].filter(path => path.endsWith('.contract'));
      const opFiles = o => (typeof o?.file === 'string' ? [o.file] : contractOp(o) ? contracts : [designFile]);
      holds = session.prepare(request.method === 'undo' ? [...known.keys()] : (Array.isArray(request.ops) ? request.ops : []).flatMap(opFiles));
    } catch (e) {
      return { status: 500, error: { message: `reading git before the write: ${e.message}` } };
    }
    // Taken in the server's reply order, so a later reply has not yet moved
    // what this one is read against.
    const announce = v => {
      const written = new Set(v?.ok?.written ?? []);
      if (typeof v?.ok?.inverse === 'string') {
        // What moved by hand since the last announcement, as the write read
        // it (the files it did not write, the ones it did as they were
        // before it, and any gone), is a hand edit, said first and apart.
        const others = Object.fromEntries(Object.entries(v.ok.files).filter(([path]) => !written.has(path)));
        publish({ ...others, ...v.ok.before }, { changed: null, client: null }, [...known.keys()].filter(path => !(path in v.ok.files)));
        // A number for the write, on its reply, its event and the session's
        // edits, so a client can tell its own write's event from another's
        // that left the same bytes, and which write a commit's edit is.
        v.ok.write = ++writes;
        for (const path of written) {
          session.wrote(path, { before: v.ok.before[path], after: v.ok.files[path], client: v.ok.client, tokens: (v.ok.changed ?? []).filter(c => c.file === path).map(c => c.token), write: v.ok.write });
        }
        publish(v.ok.files, { changed: v.ok.changed, client: v.ok.client, write: v.ok.write, ...extra });
      }
      session.release(holds.filter(path => !written.has(path)));
      if (v?.error?.conflict) {
        // A file whose digest is not the last announced moved by hand,
        // though the watcher has not said so yet.
        v.error.by = Object.fromEntries((v.error.stale ?? []).map(path => [path, v.error.files?.[path] === known.get(path) ? lastBy.get(path) ?? 'changed on disk' : 'changed on disk']));
      }
      settled(v);
      if (--writing === 0 && again) { again = false; rescan(); }
    };
    writing++;
    // Held until the server answers, however long: a write that applied
    // after its client was told it timed out would leave the client wrong
    // (an undo's redo lost). A server that exits fails it at once.
    return ask(request, { wait: 0, done: announce });
  }
  // The held list's revision (tamber/1.12): moved by every change to it,
  // so a long poll that names the one it has is answered without the list.
  const proposals = new Map();
  let heldRevision = 0;

  const forget = (id, why) => { proposals.delete(id); heldRevision++; send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, ...why }); };
  const expire = () => { for (const [id, p] of proposals) if (p.expires < Date.now()) forget(id, { client: null, reason: 'expired' }); };
  const expiry = setInterval(expire, 30_000);
  expiry.unref?.();
  /** A proposal held again (an accept that did not apply it), announced,
   * within the cap. */
  const restore = (id, p) => {
    if (p.expires <= Date.now()) { send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: null, reason: 'expired' }); return; }
    // Its client ended while it was out being accepted: marked as held ones were.
    if (gone(p.client)) p.shown = { ...p.shown, revoked: true };
    proposals.set(id, p); heldRevision++;
    while (proposals.size > HELD) forget(proposals.keys().next().value, { client: null, reason: 'too many held' });
    send({ event: 'proposal', seq: ++seq, epoch, ...p.shown });
  };
  /** The proposals held now, as a reconnecting client reads them. */
  // A paired client past its deadline is shown as revoked before its end runs.
  const held = () => [...proposals].map(([id, p]) => (gone(p.client) && !p.shown.revoked ? { ...p.shown, revoked: true } : p.shown));
  async function hold(client, ops, files, replaces, review, requires, scope = 'write', signal = undefined, restoring = false) {
    expire();
    // A state part's write without a `file` is any Contract source's (as a write's `prepare`).
    const contracts = [...known.keys()].filter(path => path.endsWith('.contract'));
    const designFile = relative(appDir, designPath(appDir) ?? resolve(appDir, design));
    const touched = new Set((Array.isArray(ops) ? ops : []).flatMap(o => (typeof o?.file === 'string' ? [o.file] : contractOp(o) ? contracts : [designFile])));
    // The digests of files it was planned on but does not edit (a revert's
    // file already as before), and those the client requires, are its
    // preconditions too.
    const required = requires && typeof requires === 'object' ? requires : {};
    const untouched = Object.fromEntries(Object.entries(files ?? {}).filter(([path]) => !touched.has(path)));
    const clash = Object.keys(required).find(path => path in untouched && untouched[path] !== required[path]);
    if (clash) return { status: 400, error: { message: `\`requires\` and \`files\` give \`${clash}\` two digests` } };
    let guards = { ...untouched, ...required };
    // Guards it found itself (a source it left as it was, by the plan's own
    // digest): planned again on a fresh read, never carried as the caller's.
    let inferred = [];
    const propose = guards => ask({ method: 'propose', ops, ...(files === undefined ? {} : { files }), ...(Object.keys(guards).length ? { requires: guards } : {}), texts: true, restore: restoring });
    let planned = await propose(guards);
    if (!planned.ok) return planned;
    // A state part's write without a `file` touches the Contract source it
    // wrote, not every one: the others' digests guard it too, asked again.
    // Where it wrote none beyond the named files (already as asked), every
    // source stays its target, as `prepare` takes them.
    const named = new Set((Array.isArray(ops) ? ops : []).flatMap(o => (typeof o?.file === 'string' ? [o.file] : [])));
    const wrote = new Set(Object.keys(planned.ok.texts ?? {}).filter(path => path.endsWith('.contract') && !named.has(path)));
    if (wrote.size && (Array.isArray(ops) ? ops : []).some(o => typeof o?.file !== 'string' && contractOp(o))) {
      for (const path of contracts) if (!wrote.has(path) && !named.has(path)) touched.delete(path);
      // Each source it was planned on and left as it was guards it, the
      // caller's digest or the plan's own when the caller gave none.
      const more = Object.fromEntries(Object.entries({ ...(planned.ok.files ?? {}), ...(files ?? {}) }).filter(([path]) => path.endsWith('.contract') && !touched.has(path) && !(path in guards)));
      if (Object.keys(more).length) {
        inferred = Object.keys(more).filter(path => !(path in (files ?? {})));
        guards = { ...guards, ...more };
        planned = await propose(guards);
        if (!planned.ok) return planned;
      }
    }
    // Its import cancelled while it was planned (an import's `signal`): decided in the same turn as the insert, nothing evicted.
    if (signal?.aborted) return { status: 503, error: { message: 'the import was cancelled' } };
    delete planned.ok.texts;
    const id = `prop-${epoch}-${++proposalCount}`;
    const base = Object.fromEntries(Object.entries(planned.ok.files).filter(([path]) => touched.has(path)));
    const expires = Date.now() + HOLD_MS;
    // The operations by index, as `accept`'s items name them.
    const size = Buffer.byteLength(JSON.stringify({ diff: planned.ok.diff, changed: planned.ok.changed, ops, review }));
    if (size > 4 << 20 || size + [...proposals.values()].reduce((n, q) => n + (q.size ?? 0), 0) > 32 << 20) return /* a proposal at most 4 MiB, all held at most 32 MiB */ { status: 413, error: { message: 'the proposal is over 4 MiB, or the held ones together over 32 MiB: accept or drop some, or propose it in parts' } };
    const shown = { proposal: id, client, diff: planned.ok.diff, changed: planned.ok.changed, class: planned.ok.class, expires: new Date(expires).toISOString(), ops: ops.map((op, index) => ({ index, op })), ...(replaces ? { replaces } : {}), ...(review ? { review } : {}), ...(gone(client) ? { revoked: true } : {}) };
    // At most 64 held. A client below `write` may not drop another's
    // (theme2's ruling (a)): at the cap its own oldest goes, and with none
    // of its own its hold is refused, decided here, in the same turn as the
    // insert. A `write` client's makes room by the oldest, as before.
    if (proposals.size >= HELD && !allows(scope, 'write')) {
      const own = [...proposals].find(([, p]) => p.client === client);
      if (!own) return { status: 409, error: { message: `${HELD} proposals are held and none is this client's: one must be accepted or dropped first` } };
      forget(own[0], { client, reason: 'too many held' });
    }
    proposals.set(id, { client, ops, base, guards, inferred, expires, shown, size, restoring }); heldRevision++;
    while (proposals.size > HELD) forget(proposals.keys().next().value, { client: null, reason: 'too many held' });
    send({ event: 'proposal', seq: ++seq, epoch, ...shown });
    return { ok: { ...planned.ok, ...shown } };
  }
  /** Accept a held proposal: applied against the digests it was reviewed
   * with; a file moved since, or only some `items` accepted, recomputes it
   * on the current files as a new proposal, for a fresh accept. */
  async function accept(request) {
    expire();
    const id = request.proposal, p = proposals.get(id);
    if (!p) return { status: 404, error: { message: `no held proposal \`${id}\` (accepted, dropped, being accepted or expired)` } };
    const items = request.items;
    if (items !== undefined && (!Array.isArray(items) || items.some(i => !Number.isInteger(i) || i < 0 || i >= p.ops.length))) {
      return { status: 400, error: { message: `\`items\` lists indices of the proposal's ${p.ops.length} operations` } };
    }
    // Taken out before anything waits: a second accept or a drop meanwhile
    // finds nothing. Every way out is announced: accepted, replaced, or
    // held again.
    proposals.delete(id); heldRevision++;
    // The review rides along, its offers' items renumbered to the
    // operations kept.
    // A revert is planned from git on the files as they were: moved since,
    // it is planned again from the commit, never its old operations
    // replayed; refused where the files moved on its edits.
    const reverted = p.shown.review?.revert?.commit;
    const replan = async (only = p.shown.review.revert.only) => {
      const plan = await planRevert(reverted, only);
      if (!plan.ok) { send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: null, reason: 'stale' }); return plan; }
      return replaced(await hold(p.client, plan.ok.ops, plan.ok.files, id, plan.ok.review, undefined, 'write', undefined, true));
    };
    const replaced = fresh => {
      if (!fresh.ok) { restore(id, p); return fresh; } else if (p.reimport) proposals.get(fresh.ok.proposal).reimport = true;
      send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: request.client, reason: 'replaced', replacedBy: fresh.ok.proposal });
      return { ok: { recomputed: true, replaces: id, ...fresh.ok } };
    };
    const recompute = async (ops, kept = ops.map((_, i) => i)) => {
      const old = p.shown.review;
      const review = old && {
        ...old,
        offers: (old.offers ?? []).map(o => ({ ...o, items: (o.items ?? []).map(i => kept.indexOf(i)).filter(i => i >= 0) })),
        ...(old.tokens === undefined ? {} : { tokens: kept.filter(i => i < old.tokens).length }),
      };
      let fresh; // A re-import (the server's own mark, never its review's fields) is marked again on the design as it is, and held on that same text, else marked again (stage 10b).
      const remark = async () => { const m = await ask({ method: 'import-plan', marks: ops }); if (!m.ok) return m; delete review.dials; delete review.pins; Object.assign(review, marked(ops, m.ok.dials.map(d => ({ ...d, from: old.dials?.find(o => o.dial === d.dial)?.from ?? [] })), m.ok.pins)); return hold(p.client, ops, m.ok.digest ? { [relative(appDir, designPath(appDir) ?? resolve(appDir, design))]: m.ok.digest } : undefined, id, review, own, 'write', undefined, p.restoring); };
      // Some of a revert's items: on the files it was planned on, or that
      // selection planned again.
      if (reverted) review.revert = { ...review.revert, only: ops.map(revertKey) };
      // Its guards stay its guards: a part, or one planned again on files
      // moved since, checks what the whole was planned on.
      const own = Object.fromEntries(Object.entries(p.guards).filter(([path]) => !(p.inferred ?? []).includes(path)));
      if (!p.reimport && review) { delete review.dials; delete review.pins; } // a client's own marks are not carried past its operations
      try { if (p.reimport && !reverted) for (let i = 0; i < 3 && (!i || fresh.error?.conflict); i++) fresh = await remark(); else fresh = await hold(p.client, ops, reverted ? { ...p.base, ...p.guards } : undefined, id, review, reverted ? undefined : own, 'write', undefined, p.restoring); } catch (e) { restore(id, p); throw e; }
      if (reverted && fresh.error?.conflict) return replan(review.revert.only);
      return replaced(fresh);
    };
    const some = items && new Set(items).size < p.ops.length;
    if (some) {
      const kept = [...new Set(items)].sort((a, b) => a - b);
      return recompute(kept.map(i => p.ops[i]), kept);
    }
    // The write's own end, whenever the server answers (a client told it
    // timed out included): accepted, or held again unless it went stale.
    let waiting = true;
    const settled = v => {
      if (v?.ok) send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: request.client, reason: 'accepted' });
      else if (!v?.error?.conflict) restore(id, p);
      else if (!waiting) send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: null, reason: 'stale' });
    };
    const done = await write({ method: 'transaction', client: p.client, files: p.base, ops: p.ops, ...(Object.keys(p.guards).length ? { requires: p.guards } : {}), restore: p.restoring }, { acceptedBy: request.client, proposal: id }, settled);
    waiting = false;
    if (done.error?.conflict) return reverted ? replan() : recompute(p.ops);
    return done;
  }

  // The app's own Contract sources the server has now (stage 8 D), whose
  // state parts history reads and revert writes.
  const ownContracts = () => [...known.keys()].filter(path => path.endsWith('.contract'));
  // The design as app.json names it, whether or not it is there now.
  const designRel = (named = manifestTamber(appDir)) => relative(appDir, resolve(appDir, typeof named === 'string' ? named : 'app.tamber'));
  /** The git log of the design's files, each commit's files as edits. */
  async function history(request) {
    const { limit = 20, skip = 0 } = request;
    if (!Number.isInteger(limit) || limit < 1 || limit > 100) return { status: 400, error: { message: '`limit` is a count from 1 to 100' } };
    if (!Number.isInteger(skip) || skip < 0 || skip > 10_000) return { status: 400, error: { message: '`skip` is a count up to 10,000' } };
    let log;
    try { log = designLog(appDir, designRel(), { limit, skip, contracts: ownContracts() }); } catch (e) { return { status: 500, error: { message: `reading git: ${e.message}` } }; }
    if (!log.ok) return log;
    const commits = [];
    for (const c of log.ok.commits) {
      const files = [];
      // One file a request: each within the server's line, whatever the commit's size.
      for (const { before, after, ...f } of c.files) {
        const compared = await ask({ method: 'compare', pairs: [{ file: f.file, before, after }] });
        if (!compared.ok && compared.status !== 413) return compared;
        const read = compared.ok?.files[0];
        files.push({ ...f, edits: read?.edits ?? null, ...(read?.code ? { code: true } : {}), ...(compared.ok ? (read?.refused ? { refused: read.refused } : {}) : { refused: 'its versions are too large to compare (over 1 MiB)' }) });
      }
      // A Contract source's change outside its state parts is the app's
      // code, not its design: a commit with nothing else is no design commit
      // (so a page may hold fewer than `limit`).
      const design = files.filter(f => !f.file.endsWith('.contract') || f.edits?.length || f.refused);
      if (design.length) commits.push({ ...c, files: design });
    }
    return { ok: { commits, more: log.ok.more } };
  }
  /** A past commit's inverse on today's files, held for review. */
  async function revert(request, scope = 'write') {
    const plan = await planRevert(request.commit);
    return plan.ok ? hold(request.client, plan.ok.ops, plan.ok.files, undefined, plan.ok.review, undefined, scope, undefined, true) : plan;
  }
  /** A past commit's inverse planned on today's files: its operations, the
   * digests they were planned on, and the review; or refused, naming each
   * edit that cannot apply. */
  async function planRevert(rev, only) {
    let c;
    try { c = designCommit(appDir, designRel(), rev, 'all'); } catch (e) { return { status: 500, error: { message: `reading git: ${e.message}` } }; }
    if (!c.ok) return c;
    if (!c.ok.files.length) return { status: 400, error: { message: `commit ${c.ok.commit.slice(0, 12)} changed no design file` } };
    const ops = [], already = [], conflicts = [], moved = [], code = [], files = {};
    // One file a request, each within the server's line.
    for (const { file, before, after } of c.ok.files) {
      const planned = await ask({ method: 'revert-plan', pairs: [{ file, before, after }] });
      if (planned.status === 413) { conflicts.push({ file, edit: null, now: null, why: 'its versions are too large to compare (over 1 MiB)' }); continue; }
      if (!planned.ok) return planned;
      // Only the edits a person kept, when they kept some.
      const kept = key => !only || only.includes(key);
      ops.push(...planned.ok.ops.filter(o => kept(revertKey(o)))); already.push(...planned.ok.already);
      // An edit's conflict counts for a kept item on it (any of an axis's values).
      const on = (x, prefix) => only.some(k => k.startsWith(prefix));
      code.push(...(planned.ok.code ?? [])); conflicts.push(...planned.ok.conflicts.filter(x => !only || (x.edit ? (x.edit.what === 'axis' ? on(x, `${x.file}\0axis\0${x.edit.name}\0`) : kept(editKey(x.file, x.edit))) : on(x, `${x.file}\0`)))); moved.push(...planned.ok.moved);
      if (planned.ok.files[file] !== undefined) files[file] = planned.ok.files[file];
    }
    const short = c.ok.commit.slice(0, 12);
    if (conflicts.length) {
      const named = conflicts.map(x => `${x.file}${x.edit ? ` ${x.edit.name}${x.edit.what === 'state' ? ` :${x.edit.state} ${x.edit.row}` : x.edit.what === 'row' ? ` ${x.edit.row}` : ''}${x.edit.when.length ? ` when ${x.edit.when.map(w => `${w.axis}=${w.value}`).join(' ')}` : ''}` : ''}: ${x.why}`);
      return { status: 409, error: { message: `revert of ${short} refused: ${named.join('; ')}`, refused: conflicts } };
    }
    // Only Contract code (no state row) is no design change.
    if (!ops.length && !already.length) return { status: 400, error: { message: `commit ${short} changed no design file (only the app's code in ${code.join(', ')})` } };
    if (!ops.length) return { status: 409, error: { message: `nothing to revert: the files have ${short}'s edits undone already`, already } };
    const subject = c.ok.message.split('\n')[0];
    return { ok: { ops, files, review: { revert: { commit: c.ok.commit, subject, moved, already, ...(code.length ? { code } : {}), ...(only ? { only } : {}) } } } };
  }
  /** An import the tool started: planned by the server from the source's
   * text, held with its review. */
  async function importHeld(request, scope = 'write', signal = undefined) {
    if (typeof request.kind !== 'string') return { status: 400, error: { message: '`kind` is a string' } };
    for (const k of ['source', 'name', 'prefix', 'baseline']) if (request[k] !== undefined && typeof request[k] !== 'string') return { status: 400, error: { message: `\`${k}\` is a string` } };
    // One file (`source`, `name`), or several (`sources`). A `baseline` (the export the client pulled) is taken when this run gave it
    // out, is kept, or is the design's own (stage 10b). Planned in a process of its own, watched (scripts/tamber-isolate.mjs); held on the design text it was planned and marked on, else planned again.
    for (let tries = 0, target = designPath(appDir) ?? resolve(appDir, design); ; tries++) {
      const planned = await planAlone({ kind: request.kind, source: request.source, name: request.name, sources: request.sources, prefix: request.prefix, ...(request.baseline !== undefined ? { baseline: request.baseline, recognised: exported } : {}) }, target, { ...importLimits, signal }, appDir, request.client);
      if (!planned.ok) return planned.error?.stale ? { status: 409, error: planned.error } : planned;
      if (!planned.ok.digest) return { status: 409, error: { message: 'the design file is not there: create it, then import' } }; // (no digest, no guard)
      const { ops, review } = importReview(planned.ok);
      if (!planned.ok.ops.length) return { ok: { proposal: null, review, message: planned.ok.counted?.mapped ? 'nothing to propose: the design has every mapped value already' : 'nothing to propose: nothing in it maps to a token' } };
      // Cancelled while planned or held (its request gone, its client ended): never held.
      // (answered at once: the hold, when its plan comes back, sees the signal and holds nothing)
      const gone = { status: 503, error: { message: 'the import was cancelled' } };
      if (signal?.aborted) return gone;
      const cut = signal && new Promise(ok => signal.addEventListener('abort', () => ok(gone), { once: true }));
      const held = await Promise.race([hold(request.client, ops, { [relative(appDir, target)]: planned.ok.digest }, undefined, review, undefined, scope, signal, true), ...(cut ? [cut] : [])]);
      if (held === gone) return gone;
      if (held.ok) proposals.get(held.ok.proposal).reimport = true; else if (held.error?.conflict && tries < 2) continue; return held;
    }
  }

  function close() {
    stopImports();
    pairs.close();
    clearTimeout(timer);
    clearInterval(absence);
    clearInterval(expiry);
    for (const w of watchers.values()) w.watcher.close(); packaged.close();
    for (const res of subscribers) res.end();
    subscribers.clear();
    for (const poll of polls) poll();
    if (child.exitCode !== null || child.signalCode !== null || down) return Promise.resolve();
    const exited = new Promise(ok => child.once('exit', () => ok(true)));
    // Its group by pid only when that names a process of its own: a pid of 0
    // or 1 (a stand-in's, or none yet) would signal this process's own group
    // or every process this user may.
    const group = Number.isInteger(child.pid) && child.pid > 1;
    const signal = name => { try { if (!group) throw null; process.kill(-child.pid, name); } catch { child.kill?.(name); } };
    const within = ms => Promise.race([exited, new Promise(ok => setTimeout(() => ok(false), ms).unref?.())]);
    child.stdin.end();
    signal('SIGTERM');
    // Not gone in 5 s: killed, and said if even that does not end it.
    return within(5000).then(gone => gone || (signal('SIGKILL'), within(2000))).then(gone => {
      if (!gone) log('tamber: the design server did not exit; its lock may still be held');
    });
  }

  // `compete()`: the dev loop's build of an edit the watcher does not see
  // (a Contract source's), which ends a scrub as a hand edit does.
  // `pairing`: the run's paired clients, for stage 10 C's gateway (`lookup`, `onEnd`; scripts/tamber-pairing.mjs).
  return { handle, close, ready, channel, pairing: pairs, canvas: gateway, relayCanvas: g => { relayGateway = g; }, compete: () => scrubs.compete(), get token() { return token; }, get down() { return down; } };
}

/** The app directory an argument names (a directory, or a file in it). */
const appOf = given => (/\.(contract|tamber)$/.test(given) ? dirname(given) : given);

/** A listener on `host`:`port` behind the development gate, each request
 * given to `handle(req, res, url, access)` (a false return is a not-found);
 * stopped, with `stop()`'s own end, on a signal or when its launcher goes. */
async function listen({ host, port, handle, stop, first = null }) {
  const server = createServer();
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(port, host, ok); });
  const origins = installBrowserOrigins({ host, port: server.address().port });
  const gate = developmentGate(origins, server.address().port);
  server.on('request', async (req, res) => {
    // A request `first` answers is no other's (a sandboxed page's wire request, stage 10b).
    if (first && await first(req, res)) return;
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    let url; try { url = new URL(req.url, 'http://localhost'); } catch { notFound(res); return; }
    if (!(await handle(req, res, url, access))) notFound(res);
  });
  const end = async () => { await stop(); server.close(); process.exit(0); };
  process.on('SIGINT', end); process.on('SIGTERM', end); process.on('SIGHUP', end);
  // Its launcher (bun exact.mjs) gone, even by SIGKILL: stop too.
  const launcher = Number(process.env.EXACT_LAUNCHER_PID);
  if (Number.isInteger(launcher) && launcher > 1) setInterval(() => { try { process.kill(launcher, 0); } catch (e) { if (e.code === 'ESRCH') end(); } }, 1000).unref();
  return origins;
}

/** `bun exact.mjs tamber serve <app> [--port <n>] [--lan] [--remote <host:port>] [--trust-tunnel <port>]`:
 * the wire alone, on loopback unless `--lan` (whose clients still cannot
 * have the token); `--remote` and `--trust-tunnel` add its remote doors (scripts/tamber-remote.mjs). */
export async function serveAlone(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const dir = appOf(resolve(argv.find((a, i) => !a.startsWith('-') && !['--port', '--remote', '--trust-tunnel', '--pair-ttl', '--allow-host', '--relay', '--relay-account', '--relay-cert', '--relay-key', '--relay-name', '--relay-dns-hook', '--relay-acme'].includes(argv[i - 1]) && argv[i - 2] !== '--relay-name') ?? '.'));
  if (!designPath(dir)) { console.error(`tamber serve: ${dir} has no design (app.tamber, or the file app.json's "tamber" names)`); return 1; }
  let asked;
  try { asked = remoteArgs(argv); } catch (e) { console.error(`tamber serve: ${e.message}`); return 2; }
  const ttl = argv.includes('--pair-ttl') ? parseDuration(at('--pair-ttl')) : PAIR_TTL_MS;
  if (ttl == null) { console.error(`tamber serve: --pair-ttl ${at('--pair-ttl')}: a duration from 1m to 30d, as 90m, 12h or 2d`); return 2; }
  let doors = null;
  const tamber = tamberServer({ appDir: dir, pairTtlMs: ttl, remote: () => doors?.list() ?? [] });
  // The remote doors first: one that cannot bind leaves nothing listening.
  doors = await openRemote(tamber, asked).catch(async e => { console.error(`tamber serve: ${e.message}`); await tamber.close(); process.exit(1); });
  const origins = await listen({ host: argv.includes('--lan') ? '0.0.0.0' : '127.0.0.1', port: Number(at('--port') ?? 0), handle: tamber.handle, stop: async () => { await Promise.all([tamber.close(), doors?.close()]); }, first: asked.plugin ? (req, res) => pluginOnLoopback(tamber, req, res) : null })
    .catch(async e => { console.error(`tamber serve: cannot listen: ${e.code ?? e.message}`); await Promise.all([tamber.close(), doors?.close()]); process.exit(1); });
  console.log(origins.map(o => `${o.origin}/__tamber`).join('\n'));
  console.log(`  (the design server for ${dir}; the token at GET /__tamber/token from this machine only; ctrl-c to stop)`);
  await new Promise(() => {});
}

// The header a dev loop's HTML carries (LLP 1122 T2): framed by the design
// tool's origin and the page's own ('self': a page the canvas shows may frame
// its own pages), never another's.
export const FRAME_ANCESTORS = `frame-ancestors 'self' ${TOOL_ORIGIN}`;

/** Every HTML response `res` sends carries FRAME_ANCESTORS, whichever route
 * answers (the page, an install page, `/__dev/open`, a forwarded one): a dev
 * loop calls it first thing per request. Node's implicit head goes through
 * writeHead too. A head's headers, in any form writeHead takes, are moved to
 * the response's own; a policy already there stays beside it (each one is
 * enforced, so it can only narrow who frames), and X-Frame-Options goes. */
export function frameOnlyByTool(res) {
  const writeHead = res.writeHead;
  res.writeHead = function (status, ...rest) {
    const at = rest.findIndex(h => h && typeof h === 'object');
    const given = at < 0 ? null : rest[at];
    // [[k, v], …], [k, v, k, v, …] or {k: v}, as pairs.
    const pairs = !given ? [] : !Array.isArray(given) ? Object.entries(given)
      : Array.isArray(given[0]) ? given : given.flatMap((v, i) => i % 2 ? [] : [[v, given[i + 1]]]);
    // HTML if any Content-Type it would send says so (a repeated one too).
    const types = [...pairs.filter(([k]) => String(k).toLowerCase() === 'content-type').map(([, v]) => v), this.getHeader('content-type')].flat();
    if (!types.some(t => /^\s*text\/html\b/i.test(String(t ?? '')))) return writeHead.call(this, status, ...rest);
    const values = new Map();
    for (const [k, v] of pairs) { const key = String(k).toLowerCase(); if (!values.has(key)) { values.set(key, []); this.removeHeader(key); } values.get(key).push(...[v].flat()); }
    for (const [k, v] of values) this.setHeader(k, v.length === 1 ? v[0] : v);
    const csp = [this.getHeader('content-security-policy') ?? []].flat().filter(p => p !== FRAME_ANCESTORS);
    this.setHeader('content-security-policy', [...csp, FRAME_ANCESTORS]);
    this.removeHeader('x-frame-options');
    return writeHead.call(this, status, ...rest.filter((_, i) => i !== at));
  };
  return res;
}

/** The tool built into TOOL_DIST unless that build is current; the build's
 * exit code. */
async function buildTool() {
  const { resolveApp } = await import('./app.mjs');
  const { webChanges } = await import('./agent-launch.mjs');
  // This checkout's tool, whatever app the caller's EXACT_APP_DIR names.
  const caller = process.env.EXACT_APP_DIR;
  const staged = TOOL_PORT === 7740 ? null : stageTool();
  process.env.EXACT_APP_DIR = staged ? staged.dir : resolve(ROOT, 'apps/design-tool');
  let app;
  try { app = resolveApp(); } finally { if (caller === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = caller; }
  // This app's whole JS-target build (every file its marker lists), with
  // no input changed since, as verify accepts one.
  const { builtAppMatches, jsTargetBuild } = await import('../host/web/serve.mjs');
  if (await builtAppMatches(TOOL_DIST, app) && jsTargetBuild(TOOL_DIST) && !webChanges(TOOL_DIST, app).all.length) return 0;
  console.log(`tamber tool: building the design tool into ${relative(process.cwd(), TOOL_DIST)}`);
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), 'design-tool'], { cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_APP_DIR: app.dir, EXACT_WEB_DIST: TOOL_DIST } });
  return r.status ?? 1;
}

/** What answers at `origin` (TOOL_ORIGIN unless named) now: null for
 * nothing, else the app its design server serves (null when it gave none),
 * and that server run's epoch. */
export async function toolAnswers(origin = TOOL_ORIGIN, tries = 1) { // `{app, epoch}`, or null; a token probe that times out is tried again, `tries` in all (a loaded dev loop answers late), one refused never
  try {
    let got; for (let i = 1; !got; i++) try { got = await fetch(`${origin}/__tamber/token`, { redirect: 'error', signal: AbortSignal.timeout(2000) }); } catch (e) { if (e?.name !== 'TimeoutError' || i >= tries) throw e; }
    if (!got.ok) return { app: null };
    const { token } = await got.json();
    const hello = await (await fetch(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}', signal: AbortSignal.timeout(tries > 1 ? 120_000 : 10_000) })).json(); // (a loop that gave its token is up: one loaded starts its design server late)
    return { app: hello.ok?.app ?? null, epoch: hello.ok?.epoch ?? null };
  } catch { return null; }
}

/** `bun exact.mjs tamber tool [<app>] [--via <dev loop url>] [--no-open]`
 * (LLP 1122 D2): the design tool, built if stale, at its own local URL,
 * TOOL_ORIGIN, served by the app's design server there: one started here,
 * or, when the app's dev loop already holds the design (one writer per
 * project), that loop's, reached through `--via`. A server already there
 * for this app is used as it is. */
export async function toolCommand(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const dir = appOf(resolve(argv.find((a, i) => !a.startsWith('-') && argv[i - 1] !== '--via') ?? '.'));
  if (!designPath(dir)) { console.error(`tamber tool: ${dir} has no design (app.tamber, or the file app.json's "tamber" names)`); return 1; }
  const via = at('--via');
  // `--via` with no URL (or as `--via=…`) is refused, never read as absent.
  if (argv.some(a => a.startsWith('--via')) && !via?.startsWith('http')) { console.error('tamber tool: --via takes the dev loop\'s URL as the next argument: --via http://127.0.0.1:<port>'); return 2; }
  // The loop's origin: the token goes only to this machine's loopback, by
  // address (a name could resolve anywhere), and never back to the tool's
  // own port.
  let target = null;
  if (via) {
    try { target = new URL(via); } catch { console.error(`tamber tool: --via takes the dev loop's URL, not ${via}`); return 2; }
    if (target.protocol !== 'http:' || !/^(127\.\d+\.\d+\.\d+|\[::1\])$/.test(target.hostname) || target.origin === TOOL_ORIGIN) {
      console.error(`tamber tool: --via takes the dev loop's loopback URL (http://127.0.0.1:<port>), not ${via}`);
      return 2;
    }
    target = target.origin;
  }
  const built = await buildTool();
  if (built) { console.error('tamber tool: the design tool did not build'); return built; }
  const url = `${TOOL_ORIGIN}/__tamber/tool/`;
  const open = () => { if (!argv.includes('--no-open') && process.platform === 'darwin') spawnSync('open', [url]); };
  const there = await toolAnswers();
  if (there) {
    let same = false;
    try { same = there.app !== null && realpathSync(there.app) === realpathSync(dir); } catch { /* gone */ }
    if (!same) { console.error(`tamber tool: ${TOOL_ORIGIN} already serves ${there.app ? `the design of ${there.app}` : 'something else'}; stop it first`); return 1; }
    // `--via` names the design server the tool goes through: the tool
    // there is reused only when it forwards to that server's run (the same
    // epoch), never its own server or another loop's.
    if (target && (await toolAnswers(target, 5))?.epoch !== there.epoch) { console.error(`tamber tool: ${TOOL_ORIGIN} already serves this design, not through ${target}; stop it first`); return 1; }
    // And this checkout's tool, as just built: another checkout's server
    // serves its own.
    // Its page, module and plan (whose names carry no digest) as built here.
    const served = async path => { try { return Buffer.from(await (await fetch(`${url}${path}`, { redirect: 'error', signal: AbortSignal.timeout(5000) })).arrayBuffer()); } catch { return Buffer.alloc(0); } };
    const page = Buffer.from(readFileSync(resolve(TOOL_DIST, 'index.html'), 'utf8').replace(/<base\s+href=["']\/["']\s*\/?\s*>/i, '<base href="/__tamber/tool/">'));
    const matches = async (path, bytes) => (await served(path)).equals(bytes);
    if (!(await matches('', page)) || !(await matches('app.js', readFileSync(resolve(TOOL_DIST, 'app.js')))) || !(await matches('app.plan', readFileSync(resolve(TOOL_DIST, 'app.plan'))))) { console.error(`tamber tool: the design server at ${TOOL_ORIGIN} serves another build of the design tool (another checkout's?); stop it first`); return 1; }
    console.log(`${url}\n  (the design tool, on the design server already running for ${dir})`);
    open();
    return 0;
  }
  const port = TOOL_PORT;
  if (target) {
    const upstream = await toolAnswers(target, 5);
    let same = false;
    try { same = upstream?.app != null && realpathSync(upstream.app) === realpathSync(dir); } catch { /* gone */ }
    if (!same) { console.error(`tamber tool: ${target} ${upstream ? `serves ${upstream.app ? `the design of ${upstream.app}` : 'no design server'}` : 'does not answer'}, not ${dir}'s`); return 1; }
    if (!(await listenOn(port, () => listen({ host: '127.0.0.1', port, handle: (req, res, url, access) => forward(req, res, url, access, target), stop: async () => {} })))) return 1;
    console.log(`${url}\n  (the design tool, through the dev loop at ${target}; ctrl-c to stop)`); tellCanvas(await canvasCheck(target, TOOL_ORIGIN));
  } else {
    // The design server up (or refused) before the port is taken, so a
    // refusal leaves nothing listening.
    const tamber = tamberServer({ appDir: dir });
    // Stopped while it starts (a cold compiler build), the server stops too.
    const codes = { SIGINT: 130, SIGTERM: 143, SIGHUP: 129 };
    const early = async signal => { await tamber.close(); process.exit(codes[signal]); };
    for (const signal of Object.keys(codes)) process.once(signal, early);
    await tamber.ready;
    for (const signal of Object.keys(codes)) process.off(signal, early);
    if (tamber.down) {
      const held = /another tamber server/.test(tamber.down);
      console.error(`tamber tool: ${tamber.down}${held ? `\n  the app's dev loop holds its design: run bun exact.mjs tamber tool ${dir} --via <its URL>` : ''}`);
      await tamber.close();
      return 1;
    }
    if (!(await listenOn(port, () => listen({ host: '127.0.0.1', port, handle: tamber.handle, stop: tamber.close })))) { await tamber.close(); return 1; }
    console.log(`${url}\n  (the design tool, on the design server for ${dir}; ctrl-c to stop)`);
  }
  open();
  await new Promise(() => {});
}

// Whether the loop's page lets this tool frame it, said on the terminal once each time it starts refusing (tamber-frame.mjs).
let canvasSaid = ''; const tellCanvas = check => { const { line, said } = canvasLine(check, TOOL_ORIGIN, canvasSaid); canvasSaid = said; if (line) console.error(line); };
/** `listening()`, false and said when the tool's port is another's. */
async function listenOn(port, listening) {
  try { await listening(); return true; } catch (e) {
    if (e.code !== 'EADDRINUSE') throw e;
    // Who holds it, where lsof can say: the process with a socket whose
    // own end is the port (a client's names it after `->`). Its state is
    // not asked for, as macOS's lsof does not always report LISTEN.
    let pid, command;
    for (const proc of (spawnSync('lsof', ['-nP', `-iTCP:${port}`, '-Fpcn'], { encoding: 'utf8' }).stdout ?? '').split(/^(?=p)/m)) {
      if (proc.split('\n').some(l => l.startsWith('n') && !l.includes('->') && l.endsWith(`:${port}`))) { pid = /^p(\d+)$/m.exec(proc)?.[1]; command = /^c(.+)$/m.exec(proc)?.[1]; break; }
    }
    console.error(`tamber tool: port ${port} is in use by ${pid ? `${command ?? 'a process'} (pid ${pid}), which is` : 'something that is'} no design server; the tool's grants name that port, so free it`);
    return false;
  }
}

/** `--via`: the tool's files from here, every other `/__tamber` request to
 * the dev loop at `target` as it came (the token only to a local request,
 * which the loop judges again by this request from loopback). */
export async function forward(req, res, url, access, target) {
  if (url.pathname === '/__tamber/tool/canvas-check') { await answerCanvasCheck(req, res, access, target, TOOL_ORIGIN, tellCanvas); return true; }
  if (url.pathname === '/__tamber/tool' || url.pathname.startsWith('/__tamber/tool/')) { serveTool(req, res, url, access); return true; }
  if (url.pathname !== '/__tamber' && !url.pathname.startsWith('/__tamber/')) return false;
  if (!access.local) { notFound(res); return true; }
  const headers = Object.fromEntries(['content-type', 'x-exact-tamber-token'].filter(h => req.headers[h]).map(h => [h, req.headers[h]]));
  // The client gone, the request upstream (a stream, a long poll) ends too.
  const gone = new AbortController();
  res.on('close', () => gone.abort());
  try {
    // A body within the server's own limit, read here (its bytes) before it is sent on; over it, answered 413 first.
    const bytes = await readBody(req, BODY_LIMIT, res, true); // a GET's or a HEAD's too (none is sent on), whatever its content-length
    if (bytes == null) { if (bytes === null && !res.destroyed) json(res, 413, { error: { message: 'a request is at most 1 MiB' } }); return true; } // (undefined: the client left)
    const answer = await fetch(`${target}${url.pathname}`, { method: req.method, headers, redirect: 'error', signal: gone.signal, ...(req.method === 'GET' || req.method === 'HEAD' ? {} : { body: bytes }) });
    res.writeHead(answer.status, { 'content-type': answer.headers.get('content-type') ?? 'application/json', 'cache-control': 'no-store' });
    for await (const chunk of answer.body ?? []) {
      if (!res.write(chunk)) await new Promise(ok => {
        const done = () => { res.off('drain', done); res.off('close', done); ok(); };
        res.on('drain', done); res.on('close', done);
      });
    }
    res.end();
  } catch (e) {
    // Cut part way: the answer begun cannot become another.
    if (res.headersSent) res.destroy();
    else if (!gone.signal.aborted) json(res, 502, { error: { message: `the dev loop at ${target} did not answer: ${e.message}` } });
  }
  return true;
}

/** `bun exact.mjs tamber op <url> <request JSON>|subscribe`: the scripted
 * client. It takes the token as any local client does, from that origin and
 * no other (no redirect is followed), then prints the reply, or each event
 * as it comes. */
export async function op(argv) {
  const [base, what] = argv;
  if (!base || !what) { console.error('usage: bun exact.mjs tamber op <server url> <request JSON>|subscribe'); return 2; }
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber op: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  if (what === 'subscribe') {
    const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers, redirect: 'error' });
    if (!res.ok) { console.error(`tamber op: ${await res.text()}`); return 1; }
    const decoder = new TextDecoder();
    let buf = '';
    for await (const chunk of res.body) {
      buf += decoder.decode(chunk, { stream: true });
      const parts = buf.split('\n\n'); buf = parts.pop();
      for (const part of parts) if (part.startsWith('data: ')) console.log(part.slice(6));
    }
    return 0;
  }
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: what, redirect: 'error' });
  console.log(await res.text());
  return res.ok ? 0 : 1;
}

/** Whether an operation writes the app's own Contract, with no `file`
 * when one file declares its style or component (stage 8 D's state ops,
 * stage 9 D's `set-row`, `add-param` and `remove-param`). */
const contractOp = o => /^((un)?set-state|set-row|add-param|remove-param)$/.test(o?.op ?? '');

/** What a revert's operation or edit is to: its file, the token, dial or
 * axis, and the `when` (conditions in any order), so a selection of a
 * revert's items is the same edits when it is planned again. */
const whenKey = when => JSON.stringify([...(when ?? [])].map(c => `${c.axis}=${c.value}`).sort());
// A state part's row is its style's, by state and row (stage 8 D); a
// style's own row by row (stage 9 D2).
const editKey = (file, e) => `${file}\0${e.what}\0${e.name}${e.what === 'state' ? `\0${e.state}\0${e.row}` : e.what === 'row' ? `\0${e.row}` : ''}\0${whenKey(e.when)}`;
const revertKey = o => /^(un)?set-state$/.test(o.op) ? editKey(o.file, { what: 'state', name: o.style, state: o.state, row: o.row, when: [] })
  : o.op === 'set-row' ? editKey(o.file, { what: 'row', name: o.style, row: o.row, when: [] }) : editKey(o.file, {
  what: o.op === 'set-dial' ? 'dial' : /axis/.test(o.op) ? 'axis' : 'token',
  name: o.op === 'add-axis' ? o.name : o.axis ?? o.name,
  // An axis's values each their own item, as a token's `when`s are.
  when: ['set-token', 'unset-token-when'].includes(o.op) ? o.when : /axis-value/.test(o.op) ? [{ axis: o.axis, value: o.value }] : [],
});
