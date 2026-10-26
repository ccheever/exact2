// The variant gallery on the JS dev loop (LLP 1139 D5, §9 Q5; the design
// tool's G3): `/__dev/gallery/` is a second app, `contract gallery`'s source
// (each exported library component the app uses, once per variant value,
// with the app's design), built by the JS target's builder into a dist of
// its own. Nothing runs until a page asks for it: then the source is made in
// a build cache and built. Its reloads and errors go on its own stream
// (`/__dev/gallery/stream`), never the app's, and the app's never on its; it
// shares nothing of the app's build (its stage, its sequence, its design
// reader), and its maps are its own. It is made and built again after the
// app publishes a reload or a kept design (`designKept`), never before, so
// it follows each kept edit, not a scrub's frames. It stops with the loop,
// and a minute after it was last asked for with no page of it open. The
// page registers on the build channel with `page: 'gallery'` (tamber/1.15),
// a label only.
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { buildTreeFileAsync as buildTreeFile, replaceBuild, sendStaticBody, webContentType } from '../web/serve.mjs';
import { fresh } from './module.mjs';

/** The gallery's build cache: under the app's nearest `node_modules` (which
 * package resolution reaches, and git and describe's files don't), else the
 * repo's `target`, keyed by the app's id (one `node_modules` may serve many
 * apps of a workspace). */
export function galleryCache(app, root) {
  for (let dir = app.dir; ; dir = dirname(dir)) {
    if (existsSync(resolve(dir, 'node_modules'))) return resolve(dir, 'node_modules/.cache/exact-gallery', app.id);
    if (dirname(dir) === dir) return resolve(root, 'target/dev-gallery', app.id);
  }
}

const json = (frame) => `data: ${JSON.stringify(frame)}\n\n`;
const html = (text) => text.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
// Before its first build, a page that waits on the gallery's stream and loads again when one lands.
const WAITING = `<!doctype html><meta charset="utf-8"><title>Gallery</title><pre id="g" style="font:12px/1.4 ui-monospace,monospace;padding:12px">Building the gallery…</pre><script>new EventSource('/__dev/gallery/stream').onmessage=e=>{const m=JSON.parse(e.data);if(m.error)document.getElementById('g').textContent=m.error;else if(m.reload)location.reload()}</script>`;
const MAPS = 4;

/** The gallery for one loop. `client(n)` is the dev client script for a
 * page of build `n` (the loop's own, with the gallery's stream, design and
 * page); `channel()` the build channel; `appDist` the app's dist (the
 * gallery's assets are the app's, served from there); `tableTool()` the
 * command that makes a plan's inspect table, as the loop makes the app's
 * (`{cmd, pre}`); `compiler()` the loop's fresh JS-target compiler, if any;
 * `design()` the app's committed design digest. Answers its routes
 * (`handle`), hears that the app published (`appChanged`, `designKept`, each
 * called after the app's own publication), keeps its maps (`map`), and stops
 * (`stop`), every process it started with it. */
export function galleryLoop({ app, root, appDist, channel = () => null, client, tableTool = () => null, compiler = () => null, design: designNow = () => null, env = process.env, log = console.log, idleMs = 60_000 }) {
  const cache = galleryCache(app, root);
  const source = resolve(cache, 'source'), live = resolve(cache, 'live'), stage = resolve(cache, 'stage'), dist = resolve(cache, 'dist');
  const clients = new Set(), maps = new Map(), tables = new Map(), procs = new Set();
  // `seq` only grows (a page that reconnects compares against it); `active` while a page has asked since the last
  // idle stop; `run` counts stops, so what a stopped run finishes says nothing.
  let seq = 0, built = false, active = false, failed = null, busy = false, again = false, idle = null, run = 0, design = null;

  const push = (frame) => { for (const res of clients) res.write(json(frame)); };
  const fail = (error) => { failed = error; log(`gallery: ${String(error).split('\n')[0]}`); push({ error: String(error) }); };

  // The contract compiler as built, while it is fresh by its dep-info (the design server's `cargo run` keeps it so),
  // never another `cargo run` on the app's target and its lock: else Cargo builds it in a target of the gallery's own.
  const target = env.CARGO_TARGET_DIR ?? resolve(root, 'target'), contractBin = resolve(target, 'debug/contract');

  /** `cmd args`, its output, in a process group of its own, which the gallery owns (`procs`) until the whole
   * group is gone (the builder's own children, its `git`, outlive it a moment), killed after 5 s more. */
  function step(cmd, args, options) {
    return new Promise((done) => {
      const c = spawn(cmd, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'], detached: true, ...options });
      procs.add(c);
      let output = '';
      c.stdout?.on('data', (d) => { output += d; });
      c.stderr?.on('data', (d) => { output += d; });
      const gone = (code, until = Date.now() + 5000) => {
        try { process.kill(-c.pid, 0); } catch (e) { if (e.code === 'ESRCH') { procs.delete(c); return done({ code, output }); } }
        if (Date.now() > until) { try { process.kill(-c.pid, 'SIGKILL'); } catch { /* gone */ } }
        setTimeout(() => gone(code, until), 20);
      };
      c.on('error', (e) => { procs.delete(c); done({ code: -1, output: String(e) }); });
      c.on('exit', (code) => gone(code));
    });
  }

  /** The source made in `source`, copied into `live` only where it changed; then built, and published. */
  async function refresh() {
    if (busy) { again = true; return; }
    busy = true;
    const mine = run;
    // The design it builds on: one kept since is news, this one isn't. A failure before is taken up by this run.
    design = designNow(); failed = null;
    try {
      const own = fresh(contractBin, `${contractBin}.d`);
      let made = own ? await step(contractBin, ['gallery', app.dir, '-o', source], { env }) : { code: 2 };
      if (mine !== run) return;
      if (made.code === 2) made = await step('cargo', ['run', '-q', '-p', 'contract', '--', 'gallery', app.dir, '-o', source], { env: { ...env, CARGO_TARGET_DIR: resolve(cache, 'cargo') } });
      if (mine !== run) return;
      if (made.code !== 0) return fail(made.output.trim() || `contract gallery exited ${made.code}`);
      mkdirSync(live, { recursive: true });
      for (const name of ['app.contract', 'app.json']) {
        const next = readFileSync(resolve(source, name));
        if (!existsSync(resolve(live, name)) || !next.equals(readFileSync(resolve(live, name)))) writeFileSync(resolve(live, name), next);
      }
      const link = (dir) => { try { return lstatSync(resolve(dir, 'assets')).isSymbolicLink() ? readlinkSync(resolve(dir, 'assets')) : null; } catch { return null; } };
      if (link(source) !== link(live)) { rmSync(resolve(live, 'assets'), { force: true }); if (link(source)) symlinkSync(link(source), resolve(live, 'assets')); }
      log(made.output.trim().split('\n').pop());
      // The JS target's build, rendering no pages, into its own stage: nothing of the app's build is shared.
      rmSync(stage, { recursive: true, force: true });
      const started = Date.now(), webCompiler = compiler();
      // (With no fresh compiler of the loop's, the builder's Cargo works in the gallery's own target, never the app's.)
      const out = await step(process.execPath, [resolve(root, 'host/web-js/build.mjs'), live, '--out', stage, '--render', 'none', '--dev'], { env: webCompiler ? { ...env, EXACT_WEB_COMPILER: webCompiler } : { ...env, CARGO_TARGET_DIR: resolve(cache, 'cargo') } });
      if (mine !== run) return;
      if (out.code !== 0) { rmSync(stage, { recursive: true, force: true }); return fail(out.output.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l)).slice(-20).join('\n') || `the gallery's build exited ${out.code}`); }
      replaceBuild(stage, dist);
      keepMap();
      seq += 1; built = true; failed = null;
      log(`gallery: built in ${Date.now() - started} ms · ${clients.size} page${clients.size === 1 ? '' : 's'}`);
      push({ reload: seq, revision: null });
    } catch (error) {
      if (mine === run) fail(`the gallery's build failed: ${error.message}`);
    } finally {
      busy = false;
      // Asked again meanwhile (an edit, or a page after a stop): once more.
      if (again) { again = false; refresh(); }
    }
  }

  /** Its plan's map, by its digest, its own last few (never the app's store). */
  function keepMap() {
    try {
      const digest = createHash('sha256').update(readFileSync(resolve(dist, 'app.plan'))).digest('hex'), bytes = readFileSync(resolve(dist, 'app.plan.map.json'));
      if (JSON.parse(bytes).digest !== digest) return;
      maps.delete(digest); maps.set(digest, bytes);
      for (const old of [...maps.keys()].slice(0, Math.max(0, maps.size - MAPS))) maps.delete(old);
    } catch { /* no map: a selection on it is read-only */ }
  }

  /** Its quiet-period stop, while no page of it is open. */
  function quiet() {
    clearTimeout(idle); idle = null;
    if (clients.size) return;
    idle = setTimeout(() => { if (!clients.size) { log('gallery: no page for a while; it stopped'); active = false; stop(); } }, idleMs);
  }

  /** Asked for by a page: made, if it has no build or was stopped since, or failed; its quiet period begun again.
   * A page joining the build under way (its first page's stream) adds none. */
  function ensure() {
    const wasActive = active;
    active = true;
    if (busy) { if (!wasActive || failed) again = true; } else if (!built || !wasActive || failed) refresh();
    quiet();
  }

  /** Its build stopped; started again by the next page. */
  function stop() {
    clearTimeout(idle); idle = null; run += 1; again = false; tables.clear();
    return Promise.all([...procs].map((c) => {
      const gone = new Promise((ok) => { if (c.exitCode !== null || c.signalCode !== null) ok(); c.once('exit', ok); setTimeout(ok, 5000); });
      try { process.kill(-c.pid, 'SIGKILL'); } catch { try { c.kill('SIGKILL'); } catch { /* gone */ } }
      return gone;
    }));
  }
  process.once('exit', () => { for (const c of procs) { try { process.kill(-c.pid, 'SIGKILL'); } catch { /* gone */ } } });

  /** Its plan's inspect table (the loop's own command, a process of the gallery's), the last two. */
  function inspectTable(digest) {
    if (!tables.has(digest)) {
      const tool = tableTool(), out = resolve(cache, `inspect-${digest}-${process.pid}.json`); // (this loop's own, as dev.mjs's)
      tables.set(digest, (async () => {
        if (!tool) return null;
        const made = await step(tool.cmd, [...tool.pre, 'inspect-table', resolve(dist, 'app.plan'), out], {});
        tool.done?.();
        let bytes = null;
        try { bytes = made.code === 0 ? readFileSync(out) : null; if (bytes && JSON.parse(bytes).plan !== digest) bytes = null; } catch { bytes = null; }
        rmSync(out, { force: true });
        if (!bytes) tables.delete(digest);
        return bytes;
      })());
      for (const old of [...tables.keys()].slice(0, Math.max(0, tables.size - 2))) tables.delete(old);
    }
    return tables.get(digest);
  }

  /** The stream: the build channel's (its commands, its nonce), the gallery's reloads and errors. */
  function stream(req, res) {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
    res.write(':\n\n');
    const opened = channel()?.open((frame) => res.write(json(frame))) ?? null;
    res.write(json({ ...(failed ? { error: failed } : {}), reload: seq, revision: null, ...(opened?.nonce ? { nonce: opened.nonce } : {}) }));
    clients.add(res);
    quiet();
    // Ended once, by whichever close comes first (under Bun a request on a handed-off socket never fires req 'close').
    let done = false;
    const end = () => { if (done) return; done = true; clients.delete(res); opened?.close(); quiet(); };
    req.on('close', end);
    res.on('close', end);
  }

  const send = (req, res, found, body) => sendStaticBody(req, res, body ?? readFileSync(found.path), { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });

  return {
    cache,
    /** Whether the request was the gallery's, answered. */
    async handle(req, res, url) {
      if (url.pathname !== '/__dev/gallery' && !url.pathname.startsWith('/__dev/gallery/')) return false;
      const rest = url.pathname.slice('/__dev/gallery'.length) || '/';
      if (rest === '/stream') { ensure(); stream(req, res); return true; }
      // No live design: the gallery is built again for a kept edit (a page asking for the design hears none).
      if (rest === '/design' || rest === '/reloaded') { res.writeHead(204, { 'cache-control': 'no-store' }); res.end(); return true; }
      // Nothing of a gallery page is carried across its reloads: it loads afresh.
      if (rest === '/checkpoint') { res.writeHead(403, { 'cache-control': 'no-store' }); res.end(); return true; }
      // Its inspect table, of its own plan.
      if (rest === '/dev-inspect.json' && req.method === 'GET') {
        quiet(); // (its table's process stopped with the rest when no page of it is open a while)
        let digest = null;
        try { digest = createHash('sha256').update(readFileSync(resolve(dist, 'app.plan'))).digest('hex'); } catch { /* no build */ }
        const bytes = digest && await inspectTable(digest);
        if (!bytes) { res.writeHead(503, { 'cache-control': 'no-store' }); res.end(); return true; }
        res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
        res.end(bytes);
        return true;
      }
      ensure();
      // Its assets are the app's (its own `assets` is a link the builder copies as one): from the app's dist.
      if (rest.startsWith('/assets/')) {
        const found = await buildTreeFile(appDist, rest, req.headers.accept);
        if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return true; }
        send(req, res, found);
        return true;
      }
      const found = built ? await buildTreeFile(dist, rest, req.headers.accept) : null;
      if (!found) {
        if (rest !== '/') { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return true; }
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
        res.end(failed ? WAITING.replace('Building the gallery…', html(failed)) : WAITING);
        return true;
      }
      let body = null;
      if (found.route.endsWith('.html')) {
        // Its files are under the gallery's path (a page with no `<base>` given one); its dev client the gallery's.
        body = readFileSync(found.path).toString();
        const base = '<base href="/__dev/gallery/">';
        body = body.includes('<base href="/">') ? body.replace('<base href="/">', base) : /<head[^>]*>/.test(body) ? body.replace(/<head[^>]*>/, (h) => `${h}${base}`) : body.replace(/^(<!doctype[^>]*>)?/i, (d) => `${d}${base}`);
        body += client(seq);
      }
      send(req, res, found, body);
      return true;
    },
    /** The app published a reload: the gallery made and built again, if a page asked for it since its last stop. */
    appChanged() { if (active) refresh(); },
    /** The app's committed design is `digest` (a kept edit or a scrub's end, never a scrub's frame): built again
     * when it moved. */
    designKept(digest) { if (digest === design) return; design = digest; if (active) refresh(); },
    /** Its plan's map by digest, if it is one of its own. */
    map: (digest) => maps.get(digest) ?? null,
    stop,
    /** For tests and the timing line. */
    state: () => ({ seq, built, active, busy, pages: clients.size, failed, cache }),
  };
}
