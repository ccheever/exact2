// The variant gallery on the JS dev loop (LLP 1139 D5, §9 Q5; the design
// tool's G3): `/__dev/gallery/` is a second app, `contract gallery`'s source
// (each exported library component the app uses, once per variant value,
// with the app's design), built by the JS target's builder into a dist of
// its own. Nothing runs until a page asks for it: then the source is made in
// a build cache and built. Its reloads and errors go on its own stream
// (`/__dev/gallery/stream`), never the app's, and the app's never on its; it
// shares nothing of the app's build (its stage, its sequence, its design
// reader). It is made and built again after the app publishes an edit or a
// kept design edit, never before, so it follows each kept edit, not a
// scrub's frames. It stops with the loop, and a minute after its last page
// goes. The page registers on the build channel with `page: 'gallery'`
// (tamber/1.15), a label only.
import { spawn } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readFileSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { buildTreeFile, replaceBuild, sendStaticBody, webContentType } from '../web/serve.mjs';

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

/** The gallery for one loop. `client(n)` is the dev client script for a
 * page of build `n` (the loop's own, with the gallery's stream, design and
 * page); `channel()` the build channel; `keepMap(dist)` keeps a build's map
 * for inspect, as the loop keeps its page's. Answers its routes (`handle`), hears
 * that the app published an edit (`appChanged`), and stops (`stop`). */
export function galleryLoop({ app, root, channel = () => null, keepMap = () => {}, client, env = process.env, log = console.log, idleMs = 60_000, contract = ['cargo', ['run', '-q', '-p', 'contract', '--']] }) {
  const cache = galleryCache(app, root);
  const source = resolve(cache, 'source'), live = resolve(cache, 'live'), stage = resolve(cache, 'stage'), dist = resolve(cache, 'dist');
  const clients = new Set();
  // `run` counts stops: what a stopped run finishes says nothing.
  let seq = 0, failed = null, child = null, busy = false, again = false, idle = null, run = 0;

  const push = (frame) => { for (const res of clients) res.write(json(frame)); };
  const fail = (error) => { failed = error; log(`gallery: ${error.split('\n')[0]}`); push({ error }); };

  /** `cmd args`, its output, in its own process group (stopped whole). */
  function step(cmd, args, options) {
    return new Promise((done) => {
      const c = child = spawn(cmd, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'], detached: true, ...options });
      let output = '';
      c.stdout.on('data', (d) => { output += d; });
      c.stderr.on('data', (d) => { output += d; });
      c.on('error', (e) => { if (child === c) child = null; done({ code: -1, output: String(e) }); });
      c.on('exit', (code) => { if (child === c) child = null; done({ code, output }); });
    });
  }

  /** The source made in `source`, copied into `live` only where it changed; then built, and published. */
  async function refresh() {
    if (busy) { again = true; return; }
    busy = true;
    const mine = run;
    try {
      const [cmd, args] = contract;
      const made = await step(cmd, [...args, 'gallery', app.dir, '-o', source], { env });
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
      const started = Date.now();
      const built = await step(process.execPath, [resolve(root, 'host/web-js/build.mjs'), live, '--out', stage, '--render', 'none', '--dev'], { env });
      if (mine !== run) return;
      if (built.code !== 0) { rmSync(stage, { recursive: true, force: true }); return fail(built.output.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l)).slice(-20).join('\n') || `the gallery's build exited ${built.code}`); }
      replaceBuild(stage, dist);
      keepMap(dist);
      seq += 1; failed = null;
      log(`gallery: built in ${Date.now() - started} ms · ${clients.size} page${clients.size === 1 ? '' : 's'}`);
      push({ reload: seq, revision: null });
    } finally {
      busy = false;
      if (again && mine === run) { again = false; refresh(); }
    }
  }

  /** Started once a page asks. */
  function ensure() {
    clearTimeout(idle); idle = null;
    if (!busy && seq === 0 && !failed) refresh();
  }

  /** Its build stopped; started again by the next page. */
  function stop() {
    clearTimeout(idle); idle = null; run += 1; again = false;
    const c = child;
    child = null;
    if (!c || c.exitCode !== null || c.signalCode !== null) return Promise.resolve();
    const gone = new Promise((ok) => c.once('exit', ok));
    try { process.kill(-c.pid, 'SIGKILL'); } catch { /* gone */ }
    return gone;
  }
  process.once('exit', () => { if (child) { try { process.kill(-child.pid, 'SIGKILL'); } catch { /* gone */ } } });

  /** The stream: the build channel's (its commands, its nonce), the gallery's reloads and errors. */
  function stream(req, res) {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
    res.write(':\n\n');
    const opened = channel()?.open((frame) => res.write(json(frame))) ?? null;
    res.write(json({ ...(failed ? { error: failed } : {}), reload: seq, revision: null, ...(opened?.nonce ? { nonce: opened.nonce } : {}) }));
    clients.add(res);
    req.on('close', () => {
      clients.delete(res); opened?.close();
      if (!clients.size) {
        clearTimeout(idle);
        idle = setTimeout(() => { if (!clients.size) { log('gallery: no page for a while; its build stopped'); stop(); seq = 0; failed = null; } }, idleMs);
      }
    });
  }

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
      ensure();
      const found = seq > 0 ? buildTreeFile(dist, rest === '/' ? '/' : rest, req.headers.accept) : null;
      if (!found) {
        if (rest !== '/') { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return true; }
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
        res.end(failed ? WAITING.replace('Building the gallery…', html(failed)) : WAITING);
        return true;
      }
      let body = readFileSync(found.path);
      if (found.route.endsWith('.html')) {
        // Its files are under the gallery's path; its dev client the gallery's.
        body = body.toString().replace('<base href="/">', '<base href="/__dev/gallery/">') + client(seq);
      }
      sendStaticBody(req, res, body, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
      return true;
    },
    /** The app published an edit (a build, or a kept design edit): the gallery made and built again, if it runs. */
    appChanged() { if (seq > 0 || busy) refresh(); },
    stop,
    /** For tests and the timing line. */
    state: () => ({ built: seq, busy, pages: clients.size, failed, cache }),
  };
}
