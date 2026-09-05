#!/usr/bin/env node
// The agent API's driver (LLP 1012): the eight operations —
//   tree · screenshot · tap · type · state · layout · logs · clock
// — against a running app on either host, from one script, with the clock in
// the driver's hands: nothing moves between two calls unless a call moved it.
//
// Usage:  node scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--plan <file>] [--url <page>] [--session <label>] [--json] <op> [<op> …]
//   tree | layout | state | logs | screenshot <png> [window]
//   tap <target> [wheel <dx> <dy> | hover] | type <target> <text…> | type <target> key <Name>
//   clock <ms|+ms|settle>
// A target is a testId or a view id; each op is one argument (quote it).
// `tap … hover` moves the pointer onto the target (a hover, LLP 1005 §3);
// `type … key Enter` presses a key at it, by the web's key names — forms of
// tap and type, not operations of their own (rules/NOT-DOING.md).
// As a library:  import { open } from '../scripts/agent.mjs'
//   const s = await open({ host: 'web' }); await s.tap('change-station'); const t = await s.tree(); await s.close();
//
// Carriers. The web app runs in headless Chrome driven over the DevTools
// protocol on a pipe (no port, no dependency): `tap` and `type` are CDP
// input events — Chrome's own hit-testing and dispatch, the path a click
// takes — `screenshot` is Page.captureScreenshot, and the rest is
// `exact.agent(…)` in the page (`host/web/glue.js`). The macOS app and the
// Linux host run with EXACT_AGENT=1 and answer JSON lines on stdio
// (`Agent.swift`; `host/linux/src/agent.rs` — the Linux binary runs headless
// on any machine, macOS included, so `linux` works wherever it was built);
// the driver resolves a target to a view id through `tree` first, so every
// host sees the same request. Console and stderr lines ride along with `logs`.
// The Linux host is launched with the pinned font (scripts/fixtures/fonts,
// LLP 1015 §5) so a pixel taken through this driver is the same pixel on
// every machine; an `env` option, or the environment, overrides it.
// The iOS app runs on a simulator (`host/apple/build.mjs --ios` builds and
// installs it; a booted iPhone is used, else the newest, EXACT_SIM names
// one) and answers the same JSON lines over a Unix socket — a simulator app
// has no stdin (`AgentIOS.swift`); its stdout and stderr come through the
// `simctl launch --console` that stays attached.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { appBundle, bundleId, hostBundle, install, macBinary, macHostBinary, simulator } from '../host/apple/build.mjs';
import { builtAppMatches, serveStatic } from '../host/web/serve.mjs';
import { resolveApp } from './app.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Browser-process diagnostics that do not describe the page or Exact. Page
 * exceptions and console errors arrive over CDP separately and remain logs. */
export function browserDiagnosticNoise(line) {
  return /crashpad|updater|gcm|VERBOSE|DevTools listening/i.test(line)
    || /CVDisplayLinkCreateWithCGDisplay failed|CVReturn:\s*-6670/i.test(line);
}

// ---------------------------------------------------------------- web

/** The DevTools protocol over Chrome's --remote-debugging-pipe (fd 3 in, fd 4 out; NUL-delimited JSON). A closed pipe or a dead Chrome fails every pending call; every call has a deadline. */
export class Cdp {
  constructor(input, output) {
    this.input = input;
    this.next = 1;
    this.pending = new Map();
    this.listeners = [];
    this.closed = null;
    let buf = '';
    output.setEncoding('utf8');
    output.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\0')) >= 0) {
        const msg = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        if (msg.id) {
          const p = this.pending.get(msg.id);
          this.pending.delete(msg.id);
          if (msg.error) p?.reject(new Error(`${msg.error.message} (${p.method})`));
          else p?.resolve(msg.result);
        } else for (const l of this.listeners) l(msg);
      }
    });
    output.on('end', () => this.fail('the DevTools pipe closed'));
    output.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
    input.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
  }
  fail(why) {
    this.closed ??= why;
    for (const [id, p] of this.pending) { this.pending.delete(id); p.reject(new Error(`${why} (${p.method})`)); }
  }
  send(method, params = {}, sessionId, timeoutMs = 15000) {
    if (this.closed) return Promise.reject(new Error(`${this.closed} (${method})`));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`${method} did not answer within ${timeoutMs} ms`)); }, timeoutMs);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); }, method });
      this.input.write(JSON.stringify({ id, method, params, sessionId }) + '\0');
    });
  }
}

/** Refuse to drive anything but a complete, authenticated build of the
 * selected app. The build marker binds every public runtime artifact. */
export function assertWebDistApp(dist, app) {
  if (!builtAppMatches(dist, app)) throw new Error(`web dist is not a complete build for selected app ${app.id}; run node host/web/build.mjs ${app.crate('web')}`);
}

async function openWeb({ plan, size = [420, 900], url: pageURL, app, webDist }) {
  const selected = resolveApp(app);
  const dist = webDist ? resolve(webDist) : resolve(ROOT, 'host/web/dist');
  if (!pageURL) assertWebDistApp(dist, selected);
  let gpuMs = null;
  const server = createServer((req, res) => {
    if (req.url.startsWith('/__gpu')) { gpuMs = Number(new URL(req.url, 'http://x').searchParams.get('ms')); res.writeHead(204); res.end(); return; }
    if (req.url === '/__plan' && plan) { res.writeHead(200, { 'content-type': 'application/octet-stream' }); res.end(readFileSync(plan)); return; }
    if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    serveStatic(dist, req, res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-agent-'));
  const child = spawn(chrome, [
    '--headless=new', '--remote-debugging-pipe', `--window-size=${size[0]},${size[1]}`, '--hide-scrollbars',
    '--enable-unsafe-webgpu', '--disable-smooth-scrolling', `--user-data-dir=${profile}`, '--no-sandbox',
    '--disable-extensions', '--disable-background-networking', '--disable-component-update', '--no-first-run',
    '--no-default-browser-check', 'about:blank',
  ], { detached: true, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  const hostLines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l && !browserDiagnosticNoise(l)) hostLines.push('chrome: ' + l); });
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const exited = new Promise((r) => child.on('exit', (code, signal) => { cdp.fail(`Chrome exited (${code ?? signal})`); r(); }));
  const close = async () => {
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    await Promise.race([exited, sleep(2000)]);
    server.close();
    rmSync(profile, { recursive: true, force: true });
  };
  try {
    const { targetInfos } = await cdp.send('Target.getTargets');
    const target = targetInfos.find((t) => t.type === 'page') ?? (await cdp.send('Target.createTarget', { url: 'about:blank' }));
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    cdp.listeners.push((msg) => {
      if (msg.sessionId !== sessionId) return;
      if (msg.method === 'Runtime.consoleAPICalled') hostLines.push(`console.${msg.params.type}: ` + msg.params.args.map((a) => a.value ?? a.description ?? a.type).join(' '));
      else if (msg.method === 'Runtime.exceptionThrown') hostLines.push('exception: ' + (msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text));
      else if (msg.method === 'Log.entryAdded' && msg.params.entry.level !== 'verbose') hostLines.push(`${msg.params.entry.level}: ${msg.params.entry.text}`);
    });
    await call('Runtime.enable');
    await call('Log.enable');
    await call('Page.enable');
    // The viewport exactly: Chrome will not make a window narrower than 500.
    await call('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
    const evaluate = async (expression) => {
      const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
      return r.result.value;
    };
    // The page: this carrier's own server over dist/, or a URL the caller
    // named — the dev server, so a drive can watch an edit arrive.
    const page = pageURL ? new URL(pageURL) : new URL(`http://127.0.0.1:${port}/`);
    page.searchParams.set('agent', '1');
    page.searchParams.set('smoke', '1');
    await call('Page.navigate', { url: page.href });
    // The first frame: the glue stamps the root when it is in the DOM. A fresh profile's first launch can be slow.
    const t = Date.now();
    let boot = null;
    while (boot == null) {
      if (Date.now() - t > 30000) throw new Error('the page never booted; ' + hostLines.join('\n'));
      await sleep(15);
      boot = await evaluate("document.getElementById('exact-root')?.dataset.bootMs ?? null").catch(() => null);
    }
    if (plan) await evaluate("fetch('/__plan').then((r) => r.arrayBuffer()).then((b) => exact.reload(new Uint8Array(b)))");
    const frame = () => Promise.race([evaluate('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))'), sleep(250)]);
    const ask = async (req) => JSON.parse(await evaluate(`Promise.resolve(exact.agent(${JSON.stringify(req)})).then((r) => JSON.stringify(r))`));
    return {
      host: 'web', boot: Number(boot), hostLines, gpuMs: () => gpuMs,
      ask,
      async input(id, kind, opts) {
        const r = (await ask({ op: 'layout' })).nodes.find((n) => n.id === id);
        if (!r || (r.w === 0 && r.h === 0)) throw new Error(`view ${id} has no box on screen`);
        const x = r.x + r.w / 2, y = r.y + r.h / 2;
        if (kind === 'press' || kind === 'key' || kind === 'type') {
          const request = kind === 'press'
            ? { op: 'tap', id, selector: opts.selector, x: opts.x, y: opts.y }
            : { op: 'type', id, selector: opts.selector, ...(kind === 'key' ? { key: opts.key } : { text: opts.text }) };
          const guest = await ask(request);
          if (guest.guest === true) {
            if (guest.error) throw new Error(guest.error);
            await frame();
            return { ...guest, at: [x, y] };
          }
        }
        if (kind === 'wheel') await call('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: opts.wheel[0], deltaY: opts.wheel[1] });
        else if (kind === 'hover') await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
        else if (kind === 'key') {
          const f = await ask({ op: 'focus', id });
          if (f.error) throw new Error(f.error);
          const key = opts.key;
          const code = { Enter: 'Enter', Escape: 'Escape', Tab: 'Tab', Backspace: 'Backspace', ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown', ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight' }[key] ?? (key.length === 1 ? `Key${key.toUpperCase()}` : key);
          const vk = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39 }[key] ?? (key.length === 1 ? key.toUpperCase().charCodeAt(0) : 0);
          await call('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode: vk, ...(key.length === 1 ? { text: key } : {}) });
          await call('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: vk });
        }
        else if (kind === 'press') {
          await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
          await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
          await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
        } else if (kind === 'type') {
          const f = await ask({ op: 'focus', id });
          if (f.error) throw new Error(f.error);
          await call('Input.insertText', { text: opts.text });
        }
        await frame();
        return { at: [x, y] };
      },
      async screenshot(path) {
        await frame();
        const { data } = await call('Page.captureScreenshot', { format: 'png' });
        writeFileSync(path, Buffer.from(data, 'base64'));
        return { screenshot: path, w: size[0], h: size[1] };
      },
      close,
    };
  } catch (e) {
    await close();
    throw e;
  }
}

// ---------------------------------------------------------------- macOS and Linux, over stdio

/** JSON lines over a duplex: each request is answered by the next line the app writes; unmatched lines are host output (`hostLines`). `fail` rejects every pending request (the app is gone). */
function jsonLines(readable, writable, hostLines) {
  const waiting = [];
  let buf = '';
  readable.setEncoding('utf8');
  readable.on('data', (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      const w = waiting.shift();
      if (!w) { hostLines.push('app: ' + line); continue; }
      try { w.resolve(JSON.parse(line)); } catch { w.reject(new Error('unreadable reply: ' + line)); }
    }
  });
  const next = () => new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  return {
    next,
    ask: (req) => { const p = next(); writable.write(JSON.stringify(req) + '\n'); return p; },
    fail: (why) => { for (const w of waiting.splice(0)) w.reject(new Error(why)); },
  };
}

/** The stdio carrier: an app that answers JSON lines under EXACT_AGENT=1 — the macOS presenter (`Agent.swift`), the macOS sample host (`ExactHostMac`, LLP 1031 D10: several sessions of one plan, each request routed by its `session` label), and the Linux host (`host/linux/src/agent.rs`), one protocol. */
async function openStdio({ host, plan, size, app, env: extra = {}, session }) {
  const a = resolveApp(app);
  const linux = host === 'linux';
  const sample = host === 'host';
  const bin = linux ? (process.env.EXACT_LINUX_BIN ?? resolve(a.target, `release/${a.crate('linux')}`)) : sample ? macHostBinary : (process.env.EXACT_MAC_BIN ?? macBinary);
  if (!existsSync(bin)) throw new Error(linux ? `run cargo build --release -p ${a.crate('linux')} first` : sample ? 'run node host/apple/build.mjs --host first' : 'run node host/apple/build.mjs first');
  const env = { EXACT_ASSETS: a.dir, ...process.env, EXACT_AGENT: '1' };
  if (plan) env.EXACT_PLAN = plan;
  if (linux && size) env.EXACT_SIZE = `${size[0]}x${size[1]}`;
  if (linux) {
    // The pinned font: DejaVu Sans from scripts/fixtures/fonts shapes and
    // paints the host's text on every machine, so a pixel fixture recorded
    // here matches on a builder (LLP 1015 §5). The environment still wins.
    env.EXACT_FONTS ??= resolve(ROOT, 'scripts/fixtures/fonts/assets');
    env.EXACT_FONT ??= 'DejaVu Sans';
  }
  Object.assign(env, extra);
  const child = spawn(bin, [], { env, stdio: ['pipe', 'pipe', 'pipe'] });
  const hostLines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  const lines = jsonLines(child.stdout, child.stdin, hostLines);
  const exited = new Promise((r) => child.on('exit', (code, signal) => { r(code ?? signal); lines.fail(`the app exited (${code ?? signal}); ` + hostLines.join('\n')); }));
  const close = async () => { try { child.stdin.end(); } catch {} await Promise.race([exited, sleep(2000)]); try { process.kill(child.pid, 'SIGKILL'); } catch {} };
  try {
    const readyLine = lines.next();
    const ready = await Promise.race([readyLine, sleep(20000).then(() => { throw new Error('the app never became ready; ' + hostLines.join('\n')); })]);
    if (!ready.ready) throw new Error('unexpected first line: ' + JSON.stringify(ready));
    if (ready.error) throw new Error('the app booted with an error: ' + ready.error);
    // The sample host routes by label: the session the caller named, and
    // `s.session = "b"` moves every later request to another.
    const state = { session: session ?? null };
    const ask = (req) => lines.ask(state.session ? { ...req, session: state.session } : req);
    return {
      host, boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      ask,
      async input(id, kind, opts) {
        const guest = { selector: opts.selector, x: opts.x, y: opts.y };
        const r = kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest }) : kind === 'key' ? await ask({ op: 'type', id, key: opts.key, ...guest }) : await ask({ op: 'type', id, text: opts.text, ...guest });
        if (r.error) throw new Error(r.error);
        return r;
      },
      async screenshot(path, window = false) {
        const r = await ask({ op: 'screenshot', path, window });
        if (r.error) throw new Error(r.error);
        return r;
      },
      close,
    };
  } catch (e) {
    await close();
    throw e;
  }
}

// ---------------------------------------------------------------- iOS, over a Unix socket

/** The simulator carrier: the bundle `build.mjs --ios` assembled, installed and launched on a simulator with the agent socket's path in its environment (simctl passes SIMCTL_CHILD_*); then the same JSON lines over that socket (`AgentIOS.swift`). A `simctl launch --console` stays attached for the app's stdout and stderr (its `--stdout=`/`--stderr=` files stay empty on Xcode 26). One app per bundle id per device: a session replaces a running copy; closing hangs up the socket, which ends the app, and kills the pid the app reported if it lingers. */
async function openIOS({ plan, app, env: extra = {}, session, bundle = appBundle, id }) {
  const a = resolveApp(app);
  const hostFixture = bundle !== appBundle;
  if (!existsSync(bundle)) throw new Error(hostFixture ? 'run node host/apple/build.mjs --ios --host first' : 'run node host/apple/build.mjs --ios first');
  const dev = simulator();
  install(dev);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ios-'));
  const sock = resolve(dir, 'agent.sock');
  const env = { EXACT_ASSETS: a.dir, EXACT_AGENT: '1', EXACT_AGENT_SOCKET: sock, ...(plan ? { EXACT_PLAN: plan } : {}), ...extra };
  const childEnv = { ...process.env };
  for (const [k, v] of Object.entries(env)) childEnv[`SIMCTL_CHILD_${k}`] = v;
  const console_ = spawn('xcrun', ['simctl', 'launch', '--console', '--terminate-running-process', dev.udid, id ?? bundleId(a.crate('apple'))], { env: childEnv, stdio: ['ignore', 'pipe', 'pipe'] });
  const hostLines = [];
  for (const stream of [console_.stdout, console_.stderr]) stream.on('data', (d) => { for (const l of String(d).split('\n')) if (l && !/^com\.exact\.\w+: \d+$/.test(l)) hostLines.push('app: ' + l); });
  let consoleDone = false;
  const consoleExited = new Promise((r) => console_.on('exit', (code, signal) => { consoleDone = true; r({ code, signal }); }));
  let pid = null;
  // The app binds the socket once its first frame is applied: connect when it appears.
  let socket = null;
  const t = Date.now();
  while (!socket) {
    if (consoleDone) { rmSync(dir, { recursive: true, force: true }); throw new Error('the simulator launch exited before opening its agent socket; ' + hostLines.join('\n')); }
    if (Date.now() - t > 20000) {
      try { console_.kill('SIGKILL'); } catch {}
      await consoleExited;
      rmSync(dir, { recursive: true, force: true });
      throw new Error('the app never opened its agent socket; ' + hostLines.join('\n'));
    }
    socket = await new Promise((ok) => { const s = connect(sock); s.once('connect', () => ok(s)); s.once('error', () => { s.destroy(); ok(null); }); });
    if (!socket) await sleep(50);
  }
  socket.on('error', () => {});
  const lines = jsonLines(socket, socket, hostLines);
  const exited = new Promise((r) => socket.on('close', () => { lines.fail('the app hung up; ' + hostLines.join('\n')); r(); }));
  const close = async () => {
    try { socket.end(); } catch {}
    await Promise.race([exited, sleep(2000)]);
    if (pid) { try { process.kill(pid, 'SIGKILL'); } catch {} }
    await Promise.race([consoleExited, sleep(1000)]);
    if (!consoleDone) {
      try { console_.kill('SIGKILL'); } catch {}
      await consoleExited;
    }
    rmSync(dir, { recursive: true, force: true });
  };
  try {
    const ready = await Promise.race([lines.next(), sleep(20000).then(() => { throw new Error('the app never became ready; ' + hostLines.join('\n')); })]);
    if (!ready.ready) throw new Error('unexpected first line: ' + JSON.stringify(ready));
    if (ready.error) throw new Error('the app booted with an error: ' + ready.error);
    pid = ready.pid ?? null;
    // The sample host routes by label, as the macOS one does over stdio.
    const state = { session: session ?? null };
    const ask = (req) => lines.ask(state.session ? { ...req, session: state.session } : req);
    return {
      host: hostFixture ? 'host-ios' : 'ios', boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      ask,
      async input(id, kind, opts) {
        const guest = { selector: opts.selector, x: opts.x, y: opts.y };
        const r = kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest }) : kind === 'key' ? await ask({ op: 'type', id, key: opts.key, ...guest }) : await ask({ op: 'type', id, text: opts.text, ...guest });
        if (r.error) throw new Error(r.error);
        return r;
      },
      async screenshot(path, window = false) {
        const r = await ask({ op: 'screenshot', path, window });
        if (r.error) throw new Error(r.error);
        return r;
      },
      close,
    };
  } catch (e) {
    await close();
    throw e;
  }
}

// ---------------------------------------------------------------- the eight operations

/** Open a session on `host` ('web' | 'macos' | 'ios' | 'linux'); `plan` boots a compiled contract instead of the app's baked plan; `env` adds to a native host's environment. */
export async function open({ host, plan, size, env, app, session, url, webDist } = {}) {
  const carrier = host === 'macos' || host === 'mac' ? await openStdio({ host: 'macos', plan, env, app }) : host === 'host' ? await openStdio({ host: 'host', plan, env, app, session }) : host === 'host-ios' ? await openIOS({ plan, app, env, session, bundle: hostBundle, id: `${resolveApp(app).id}.host` }) : host === 'linux' ? await openStdio({ host: 'linux', plan, size, env, app }) : host === 'ios' ? await openIOS({ plan, env, app }) : await openWeb({ plan, size, url, app, webDist });
  const s = {
    host: carrier.host,
    /** The sample host's sessions by label, and which one the next request goes to (`s.session = "b"`). */
    sessions: carrier.sessions ?? null,
    get session() { return carrier.state?.session ?? null; },
    set session(label) { if (carrier.state) carrier.state.session = label; },
    /** Milliseconds from launch to the first frame. */
    boot: carrier.boot,
    /** The agent's clock, milliseconds: the last `clock` value (0 at boot). */
    now: 0,
    logCursor: 0,
    gpuMs: carrier.gpuMs,
    async op(req) {
      const r = await carrier.ask(req);
      if (r.error) throw new Error(`${req.op}: ${r.error}`);
      return r;
    },
    /** Every live node in preorder; an iframe also carries url, loading, and a reachable guest outline (@ref LLP 1020 D4). */
    tree: () => s.op({ op: 'tree' }),
    /** Every slot, derive, and resource by name, as typed JSON. */
    state: () => s.op({ op: 'state' }),
    /** What happened since the last read: the runner's journal (`lines`, from index `from` up to `next`) and the host's own output (`host`). `dropped` counts lines the journal ring let go before this read caught up. */
    async logs() {
      const r = await s.op({ op: 'logs', since: s.logCursor });
      const dropped = Math.max(0, r.from - s.logCursor);
      s.logCursor = r.next;
      return { lines: r.lines, host: carrier.hostLines.splice(0), from: r.from, next: r.next, dropped };
    },
    /** Every on-screen view's box in the viewport (scroll folded in), with its testId and type from the tree. */
    async layout() {
      const [l, t] = await Promise.all([s.op({ op: 'layout' }), s.tree()]);
      const by = new Map(t.nodes.map((n) => [n.id, n]));
      for (const n of l.nodes) { const k = by.get(n.id); if (k) { n.type = k.type; if (k.props.testId) n.testId = k.props.testId; } }
      return l;
    },
    /** The node for a target: a testId (first in preorder) or a view id. */
    async find(target) {
      const t = await s.tree();
      const node = typeof target === 'number' || /^\d+$/.test(String(target)) ? t.nodes.find((n) => n.id === Number(target)) : t.nodes.find((n) => n.props.testId === target);
      if (!node) throw new Error(`no view matches ${target}`);
      return node;
    },
    /** A press on the target through the host's input path (an iframe target accepts guest `selector` or `x`/`y`); with `{ wheel: [dx, dy] }`, a wheel over it (dy > 0 scrolls down); with `{ hover: true }`, the pointer moved onto it (a hover — and off whatever it was over). */
    async tap(target, opts = {}) {
      const node = await s.find(target);
      const r = await carrier.input(node.id, opts.wheel ? 'wheel' : opts.hover ? 'hover' : 'press', opts);
      return { ...r, tapped: node.id, target };
    },
    /** Set an input's text through the host's text input path; an iframe accepts `{text, selector}` or `{key, selector}` for its guest. */
    async type(target, text) {
      const node = await s.find(target);
      const options = typeof text === 'object' && text !== null ? text : { text };
      const key = options.key;
      const r = key != null ? await carrier.input(node.id, 'key', { ...options, key: String(key) }) : await carrier.input(node.id, 'type', { ...options, text: String(options.text ?? '') });
      return { ...r, typed: node.id, target };
    },
    /** Move the clock: to an absolute millisecond, by '+N', or to 'settle' — a fixed point at which nothing is in flight (`settled: false` if timers keep starting motion). Timers fire on the way, each at its own time; motion is seeked, never played. The clock lands where the runner says; a timer's refusal is the error. */
    async clock(spec = 'settle') {
      const req = { op: 'clock' };
      if (spec === 'settle') req.settle = true;
      else if (typeof spec === 'string' && spec.startsWith('+')) req.to = s.now + Number(spec.slice(1));
      else req.to = Number(spec);
      if (!req.settle && !Number.isFinite(req.to)) throw new Error(`clock: not a time: ${spec}`);
      const r = await s.op(req);
      s.now = r.clock;
      return r;
    },
    /** The pixels, as a PNG at `path`. On macOS `window: true` asks the window server (Metal layers included). */
    screenshot: (path, window = false) => carrier.screenshot(resolve(path), window),
    close: carrier.close,
  };
  return s;
}

// ---------------------------------------------------------------- the CLI

/**
 * The transcript form (LLP 1012 §7): the one text rendering of a reply, for
 * eyes — a pure function of the JSON, lossy on purpose (the JSON is
 * complete; only text, value, and label ride along), never parsed back.
 * `scripts/fixtures/transcript.txt` pins it. A part in [brackets] appears
 * only when its field is present (not null); strings are JSON-quoted.
 *
 *   tree    epoch E · incarnation I · clock C ms · N nodes
 *           {"  " × depth}{Type}#{id} [{testId}] "{text}" value="…" label="…" ({handlers, comma-separated})
 *           an iframe adds url="…" loading=true|false and `[guest]` outline lines
 *   layout  viewport W×H [· safe-area T R B L · keyboard K, when any is not 0] · clock C ms
 *           #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy}
 *   logs    "(N earlier lines dropped by the journal ring)" when dropped > 0; the journal lines as they are;
 *           the host's lines indented two spaces; "(nothing new)" when there is nothing
 *   state   the JSON, indented two spaces
 *   others  the JSON on one line
 */
export function render(op, r) {
  const q = JSON.stringify;
  switch (op) {
    case 'tree': {
      const lines = [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · ${r.nodes.length} nodes`];
      for (const n of r.nodes) {
        const p = n.props ?? {};
        lines.push(`${'  '.repeat(n.depth)}${n.type}#${n.id}${p.testId != null ? ` [${p.testId}]` : ''}${p.text != null ? ` ${q(p.text)}` : ''}${p.value != null ? ` value=${q(p.value)}` : ''}${p.accessibilityLabel != null ? ` label=${q(p.accessibilityLabel)}` : ''}${n.handlers?.length ? ` (${n.handlers.join(', ')})` : ''}${n.url != null ? ` url=${q(n.url)} loading=${n.loading}` : ''}`);
        for (const g of n.guest ?? []) lines.push(`${'  '.repeat(n.depth + g.depth + 1)}[guest] ${g.tag}${g.id != null ? `#${g.id}` : ''}${g.testId != null ? ` [${g.testId}]` : ''}${g.text != null ? ` ${q(g.text)}` : ''}`);
      }
      return lines.join('\n');
    }
    case 'layout': {
      const e = r.env;
      const env = e && Object.values(e).some((v) => v) ? ` · safe-area ${e['safe-area-inset-top']} ${e['safe-area-inset-right']} ${e['safe-area-inset-bottom']} ${e['safe-area-inset-left']} · keyboard ${e['keyboard-inset-height']}` : '';
      return [`viewport ${r.viewport.w}×${r.viewport.h}${env} · clock ${r.clock} ms`].concat(r.nodes.map((n) => `#${n.id}${n.testId != null ? ` [${n.testId}]` : ''}${n.type != null ? ` ${n.type}` : ''} ${n.x},${n.y} ${n.w}×${n.h}${n.sx != null ? ` scroll ${n.sx},${n.sy}` : ''}`)).join('\n');
    }
    case 'logs':
      return [...(r.dropped > 0 ? [`(${r.dropped} earlier lines dropped by the journal ring)`] : []), ...r.lines, ...(r.host ?? []).map((l) => '  ' + l)].join('\n') || '(nothing new)';
    case 'state':
      return q(r, null, 2);
    default:
      return q(r);
  }
}

/**
 * Run a `test "…"` file (LLP 1017 P7) against a host: `contract test <file>`
 * turns the blocks into steps — the eight operations, plus `expect` lines
 * over their replies — and this drives them through the same session the
 * operations use. One session per file; a failed expect names the test, the
 * line, and what was seen. Returns `{ passed, failed, results }`.
 */
export async function runTests({ host, file, plan, app, size, env, webDist } = {}) {
  const { spawnSync } = await import('node:child_process');
  const root = resolve(new URL('..', import.meta.url).pathname);
  let bin = resolve(root, 'target/debug/contract');
  if (!existsSync(bin)) {
    const b = spawnSync('cargo', ['build', '-q', '-p', 'contract'], { cwd: root, encoding: 'utf8' });
    if (b.status !== 0) throw new Error(`cargo build -p contract: ${b.stderr}`);
  }
  const c = spawnSync(bin, ['test', resolve(file)], { encoding: 'utf8' });
  if (c.status !== 0) throw new Error(c.stderr.trim());
  const tests = JSON.parse(c.stdout);
  const results = [];
  // Every test starts from the first frame: a session of its own.
  for (const t of tests) {
    const failures = [];
    const s = await open({ host, plan, size, env, app, webDist });
    try {
      for (const st of t.steps) {
        const at = `${t.name}: line ${st.line}`;
        try {
          switch (st.op) {
            case 'tap': await s.tap(st.target, st.hover ? { hover: true } : undefined); break;
            case 'type': await s.type(st.target, st.text); break;
            case 'key': await s.type(st.target, { key: st.key }); break;
            case 'clock': await s.clock(st.arg); break;
            case 'screenshot': await s.screenshot(st.path); break;
            case 'expect-tree': {
              const tree = await s.tree();
              const found = tree.nodes.some((n) => n.props.testId === st.target);
              if (found !== st.present) failures.push(`${at}: expected testId "${st.target}" ${st.present ? 'present' : 'absent'}, it was ${found ? 'present' : 'absent'}`);
              break;
            }
            case 'expect-text': {
              const tree = await s.tree();
              const n = tree.nodes.find((n) => n.props.testId === st.target);
              const got = n?.props.text;
              if (got !== st.value) failures.push(`${at}: text of "${st.target}" is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            case 'expect-state': {
              const state = await s.state();
              const bag = { ...(state.resources ?? {}), ...(state.derives ?? {}), ...(state.slots ?? {}) };
              if (!(st.name in bag)) { failures.push(`${at}: no state named "${st.name}"`); break; }
              const got = bag[st.name];
              const same = JSON.stringify(got) === JSON.stringify(st.value);
              if (!same) failures.push(`${at}: ${st.name} is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            default: failures.push(`${at}: unknown step ${st.op}`);
          }
        } catch (e) {
          failures.push(`${at}: ${e.message}`);
          break;
        }
      }
    } finally {
      await s.close();
    }
    results.push({ name: t.name, failures });
  }
  const failed = results.filter((r) => r.failures.length).length;
  return { passed: results.length - failed, failed, results };
}

async function main(argv) {
  const flags = { json: false };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') flags.json = true;
    else if (argv[i] === '--plan') flags.plan = resolve(argv[++i]);
    else if (argv[i] === '--app') flags.app = argv[++i];
    else if (argv[i] === '--size') flags.size = argv[++i].split('x').map(Number);
    else if (argv[i] === '--test') flags.test = argv[++i];
    else if (argv[i] === '--session') flags.session = argv[++i];
    else if (argv[i] === '--url') flags.url = argv[++i];
    else rest.push(argv[i]);
  }
  const [host, ...ops] = rest;
  if (host && flags.test) {
    const r = await runTests({ host, file: flags.test, plan: flags.plan, app: flags.app, size: flags.size });
    for (const t of r.results) {
      console.log(`test "${t.name}": ${t.failures.length ? 'FAIL' : 'ok'}`);
      for (const f of t.failures) console.error('  ' + f);
    }
    console.log(`${r.passed} passed, ${r.failed} failed`);
    return r.failed ? 1 : 0;
  }
  if (!host || !ops.length) {
    console.error('usage: node scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--app <name>] [--plan <file>] [--session <label>] [--json] <op> [<op> …]\n  tree | layout | state | logs | screenshot <png> [window] | tap <target> [wheel <dx> <dy> | hover] | type <target> <text…> | type <target> key <Name> | clock <ms|+ms|settle>\n       node scripts/agent.mjs <host> --test <file.test.contract>   (LLP 1017 P7: the file\'s `test` blocks, run here)');
    return 2;
  }
  const s = await open({ host, plan: flags.plan, size: flags.size, app: flags.app, session: flags.session, url: flags.url });
  try {
    for (const line of ops) {
      const [op, ...args] = line.trim().split(/\s+/);
      let r;
      switch (op) {
        case 'tree': case 'state': case 'logs': case 'layout': r = await s[op](); break;
        case 'screenshot': r = await s.screenshot(args[0] ?? 'screenshot.png', args[1] === 'window'); break;
        case 'tap': r = args[1] === 'wheel' ? await s.tap(args[0], { wheel: [Number(args[2]), Number(args[3])] }) : args[1] === 'hover' ? await s.tap(args[0], { hover: true }) : await s.tap(args[0]); break;
        case 'type': r = args[1] === 'key' && args[2] ? await s.type(args[0], { key: args[2] }) : await s.type(args[0], args.slice(1).join(' ')); break;
        case 'clock': r = await s.clock(args[0] ?? 'settle'); break;
        default: throw new Error(`unknown op: ${op} (tree, layout, state, logs, screenshot, tap, type, clock)`);
      }
      console.log(flags.json ? JSON.stringify(r) : render(op, r));
    }
    return 0;
  } finally {
    await s.close();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { console.error(e.message); process.exit(1); });
}
