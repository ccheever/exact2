#!/usr/bin/env bun
// The agent API's driver (LLP 1012): the eight operations —
//   tree · screenshot · tap · type · state · layout · logs · clock
// — against a running app on either host, from one script, with the clock in
// the driver's hands: nothing moves between two calls unless a call moved it.
//
// Usage:  bun scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--plan <file>] [--world <file>] [--url <page>] [--session <label>] [--json] <op> [<op> …]
//   tree | layout | state | logs | screenshot <png> [window] | screenshot <path> <canvas> save
//   tap <target> [wheel <dx> <dy> [gesture] | hover | history <n> | {"history":n} | contextmenu | dblclick] | type <target> <text…> | type <target> key <Name>
//   clock <ms|+ms|settle>
// A target is a testId or a view id; each op is one argument (quote it).
// `tap … wheel <dx> <dy> gesture` sends the wheel as a trackpad's gesture —
// began, changed, and the zero-delta lift that ends it — instead of a bare
// delta. Elastic overscroll lives entirely in those phases, so a plain wheel
// exercises a path a finger never takes (LLP 1033 D4a). macOS only: a host
// that cannot phase a wheel refuses rather than quietly sending a plain one.
// `tap … contextmenu` and `tap … dblclick` use browser secondary/double clicks;
// on iOS they inject the recognized event, not a UIKit finger gesture.
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
// ios --device connects back to a one-launch TCP listener on this Mac's LAN
// address (EXACT_AGENT_HOST overrides its selection); --phone selects a paired
// phone. This is a trusted-LAN developer carrier, not an encrypted remote agent.
// Build/install first with build.mjs --device. No Mac-local plan/assets paths.
import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
const WORLD_LIMIT = 256 * 1024 * 1024;
function worldFile(path) {
  if (statSync(path).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit');
  const bytes = readFileSync(path);
  if (bytes.length > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit');
  return bytes;
}
import { connect, createServer as createTCPServer } from 'node:net';
import { networkInterfaces, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { appleArtifacts, assertAppleIdentity, bundleId, developmentLaunchEnvironment, install, phone, simulator } from '../host/apple/build.mjs';
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
  if (!builtAppMatches(dist, app)) throw new Error(`web dist is not a complete build for selected app ${app.id}; run bun host/web/build.mjs ${app.crate('web')}`);
}

async function openWeb({ plan, world, size = [420, 900], url: pageURL, app, webDist }) {
  const selected = resolveApp(app);
  const dist = resolve(webDist ?? process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
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
    await exited;
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
    // Timestamp the actual browser input before a lazy surface module exists.
    // Its first-frame latency belongs in state.world.perf, outside the clock/hash.
    await call('Page.addScriptToEvaluateOnNewDocument', { source: `
      addEventListener('click', event => {
        if (event.isTrusted) { performance.clearMarks('exact-agent-input'); performance.mark('exact-agent-input', {startTime: event.timeStamp}); }
      }, true);
    ` });
    // The viewport exactly: Chrome will not make a window narrower than 500.
    await call('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
    const evaluate = async (expression) => {
      const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
      return r.result.value;
    };
    if (world && !plan) {
      const encoded = worldFile(world).toString('base64');
      await call('Page.addScriptToEvaluateOnNewDocument', { source: `globalThis.exactWorldCarry = Uint8Array.from(atob(${JSON.stringify(encoded)}), c => c.charCodeAt(0));` });
    }
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
    await evaluate('exact.ready'); // First pixel precedes deferred module readiness.
    if (plan) {
      const carry = world ? `exact.worldCarry = Uint8Array.from(atob(${JSON.stringify(worldFile(world).toString('base64'))}), c => c.charCodeAt(0));` : '';
      await evaluate(`fetch('/__plan').then((r) => r.arrayBuffer()).then((b) => { ${carry} return exact.reload(new Uint8Array(b)); })`);
    }
    const frame = () => Promise.race([evaluate('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))'), sleep(250)]);
    // The one contact this carrier may hold (LLP 1035.003 D1), and whether
    // Chrome's touch emulation is on — switched on by the first contact.
    let touch = false;
    let contact = null;
    const ask = async (req) => {
      const reply = JSON.parse(await evaluate(`Promise.resolve(exact.agent(${JSON.stringify(req)})).then((r) => { return JSON.stringify(r); })`));
      return reply;
    };
    return {
      host: 'web', boot: Number(boot), hostLines, gpuMs: () => gpuMs,
      ask,
      async input(id, kind, opts) {
        // @ref LLP 1038 D11 — history.go delivers popstate in the page.
        if (kind === 'history') {
          const reply = await ask({ op: 'tap', id, history: opts.history });
          if (reply.error) throw new Error(reply.error);
          await frame();
          return reply;
        }
        if (kind === 'key' && (opts.phase != null || await evaluate(`exact.gpu?.wantsInput(${id}) || exact.views.get(${id})?.matches('button, a[href], [role="button"], [role="link"]') || false`))) {
          return browserKey({ id, opts, evaluate, ask, call, frame });
        }
        const r = id == null ? null : (await ask({ op: 'layout' })).nodes.find((n) => n.id === id);
        if (id != null && (!r || (r.w === 0 && r.h === 0))) throw new Error(`view ${id} has no box on screen`);
        const x = r ? r.x + r.w / 2 : contact?.x, y = r ? r.y + r.h / 2 : contact?.y;
        if (kind === 'press' || kind === 'key' || kind === 'type') {
          const request = kind === 'press'
            ? { op: 'tap', id, selector: opts.selector, x: opts.x, y: opts.y }
            : { op: 'type', id, selector: opts.selector, ...(kind === 'key' ? { key: opts.key } : { text: opts.text }) };
          const guest = await ask(request);
          if (guest.guest === true || guest.handled === true) {
            if (guest.error) throw new Error(guest.error);
            await frame();
            return { ...guest, at: [x, y] };
          }
        }
        if (kind === 'down' || kind === 'move' || kind === 'hold' || kind === 'up' || kind === 'cancel') {
          // A held contact (LLP 1035.003 D1) is a finger here: CDP touch
          // events under touch emulation, enabled the first time a contact
          // is used. Chrome recognizes, scrolls and flings from them exactly
          // as it would from a hand; a timed move is delivered as steps on
          // real time so its velocity is real too.
          if (!touch) { await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 }); touch = true; }
          if (kind === 'down') {
            if (contact) throw new Error('a contact is already down; up it first');
            const px = opts.x ?? x, py = opts.y ?? y;
            await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: px, y: py }] });
            contact = { x: px, y: py };
            await frame();
            return { contact: id, phase: 'down', at: [px, py], delivery: 'platform' };
          }
          if (!contact) throw new Error('no contact is down');
          if (kind === 'move') {
            const to = { x: opts.x ?? contact.x + (opts.dx ?? 0), y: opts.y ?? contact.y + (opts.dy ?? 0) };
            const ms = Math.max(0, opts.ms ?? 0);
            const steps = Math.max(1, Math.round(ms / 16));
            for (let i = 1; i <= steps; i++) {
              const t = i / steps;
              await call('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: contact.x + (to.x - contact.x) * t, y: contact.y + (to.y - contact.y) * t }] });
              if (ms) await sleep(ms / steps);
            }
            contact = to;
            await frame();
            return { phase: 'move', at: [to.x, to.y], delivery: 'platform' };
          }
          if (kind === 'hold') { if (opts.ms) await sleep(opts.ms); return { phase: 'hold', at: [contact.x, contact.y], delivery: 'platform' }; }
          await call('Input.dispatchTouchEvent', { type: kind === 'up' ? 'touchEnd' : 'touchCancel', touchPoints: [] });
          const at = [contact.x, contact.y];
          contact = null;
          await frame();
          return { phase: kind, at, delivery: 'platform' };
        }
        if (kind === 'wheel') await call('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: opts.wheel[0], deltaY: opts.wheel[1] });
        else if (kind === 'hover') await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
        else if (kind === 'contextmenu' || kind === 'dblclick') {
          await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
          for (let clickCount = 1; clickCount <= (kind === 'dblclick' ? 2 : 1); clickCount++) {
            const button = kind === 'contextmenu' ? 'right' : 'left';
            await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button, clickCount });
            await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button, clickCount });
          }
        }
        else if (kind === 'key') {
          const f = await ask({ op: 'focus', id, select: false });
          if (f.error) throw new Error(f.error);
          const key = opts.key === 'Space' ? ' ' : opts.key;
          const code = { ' ': 'Space', Enter: 'Enter', Escape: 'Escape', Tab: 'Tab', Backspace: 'Backspace', ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown', ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight' }[key] ?? (key.length === 1 ? `Key${key.toUpperCase()}` : key);
          const vk = { ' ': 32, Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39 }[key] ?? (key.length === 1 ? key.toUpperCase().charCodeAt(0) : 0);
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
export function jsonLines(readable, writable, hostLines) {
  const waiting = [];
  let failure = null;
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
  const next = () => failure ? Promise.reject(failure) : new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  return {
    next,
    ask: (req) => { const p = next(); if (!failure) writable.write(JSON.stringify(req) + '\n'); return p; },
    fail: (why) => { failure ??= new Error(why); for (const w of waiting.splice(0)) w.reject(failure); },
  };
}

/** One launch, one phone connection; reject other peers before any agent request.
 * The token crosses via the paired device's launch environment, not a public URL. */
export async function phoneBridge() {
  const interfaces = networkInterfaces();
  const address = process.env.EXACT_AGENT_HOST ?? [...(interfaces.en0 ?? []), ...Object.values(interfaces).flat()]
    .find((n) => n.family === 'IPv4' && !n.internal)?.address;
  if (!address) throw new Error('phone agent needs a reachable Mac IPv4 address (EXACT_AGENT_HOST)');
  const token = randomBytes(32).toString('hex');
  const sockets = new Set();
  let accept, fail;
  const ready = new Promise((resolve, reject) => { accept = resolve; fail = reject; });
  const server = createTCPServer((socket) => {
    if (sockets.size >= 8) { socket.destroy(); return; }
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    socket.on('error', () => {});
    socket.setTimeout(5000, () => socket.destroy());
    let buf = '';
    socket.setEncoding('utf8');
    const hello = (chunk) => {
      buf += chunk;
      if (buf.length > 4096) { socket.destroy(); return; }
      if (!buf.includes('\n')) return;
      let announcement;
      try { announcement = JSON.parse(buf); } catch { socket.destroy(); return; }
      if (!announcement || announcement.token !== token || announcement.ready !== true) { socket.destroy(); return; }
      delete announcement.token;
      socket.pause();
      socket.removeListener('data', hello);
      socket.setTimeout(0);
      server.close();
      for (const other of sockets) if (other !== socket) other.destroy();
      accept({ socket, announcement });
    };
    socket.on('data', hello);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, address, resolve); });
  server.on('error', fail);
  return {
    ready, fail,
    env: { EXACT_AGENT_CONNECT: `${address}:${server.address().port}`, EXACT_AGENT_TOKEN: token },
    close() { for (const socket of sockets) socket.destroy(); server.close(); },
  };
}

/** One JSON-lines protocol over stdio on macOS/Linux, or a phone's outbound socket. */
async function openStdio({ host, plan, size, app, env: extra = {}, session, device = false, phone: pick }) {
  const a = resolveApp(app);
  const linux = host === 'linux';
  const sample = host === 'host';
  const artifacts = linux ? null : appleArtifacts(a, { destination: device ? 'ios' : 'macos', host: sample });
  const deviceBundle = artifacts?.bundle;
  const bin = linux ? (process.env.EXACT_LINUX_BIN ?? resolve(a.target, `release/${a.crate('linux')}`)) : (process.env.EXACT_MAC_BIN ?? artifacts.binary);
  if (!existsSync(device ? deviceBundle : bin)) throw new Error(device ? 'run bun host/apple/build.mjs --device first' : linux ? `run cargo build --release -p ${a.crate('linux')} first` : sample ? 'run bun host/apple/build.mjs --host first' : 'run bun host/apple/build.mjs first');
  if (!linux) assertAppleIdentity(a, device ? resolve(deviceBundle, 'ExactIOS') : bin);
  if (device && (plan || extra.EXACT_PLAN || extra.EXACT_ASSETS)) throw new Error('a phone cannot read host-local plan/assets paths; use --url or its embedded app');
  const ph = device ? phone(pick) : null;
  if (device) {
    const installed = spawnSync('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, deviceBundle], { encoding: 'utf8' });
    if (installed.status !== 0) throw new Error(`device install: ${installed.stderr || installed.stdout || installed.error?.message}`);
  }
  const env = { EXACT_ASSETS: a.dir, ...process.env, EXACT_AGENT: '1' };
  if (plan) env.EXACT_PLAN = plan;
  if (linux && size) env.EXACT_SIZE = `${size[0]}x${size[1]}`;
  // @ref LLP 1039 §5 — measure the requested Mac content viewport.
  if (!linux && size) { env.EXACT_WINDOW_WIDTH = String(size[0]); env.EXACT_WINDOW_HEIGHT = String(size[1]); }
  if (linux) {
    // The pinned font: DejaVu Sans from scripts/fixtures/fonts shapes and
    // paints the host's text on every machine, so a pixel fixture recorded
    // here matches on a builder (LLP 1015 §5). The environment still wins.
    env.EXACT_FONTS ??= resolve(ROOT, 'scripts/fixtures/fonts/assets');
    env.EXACT_FONT ??= 'DejaVu Sans';
  }
  Object.assign(env, extra);
  const bridge = device ? await phoneBridge() : null;
  const child = device
    ? spawn('xcrun', ['devicectl', 'device', 'process', 'launch', '--quiet', '--console', '--terminate-existing', '--device', ph.udid,
        '--environment-variables', JSON.stringify({ ...extra, EXACT_AGENT: '1', ...bridge.env }), a.id], { stdio: ['pipe', 'pipe', 'pipe'] })
    : spawn(bin, linux && env.EXACT_LAUNCH_URL ? [env.EXACT_LAUNCH_URL] : [], { env, stdio: ['pipe', 'pipe', 'pipe'] });
  const hostLines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  if (device) child.stdout.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  let lines = device ? null : jsonLines(child.stdout, child.stdin, hostLines);
  const fail = (why) => { lines?.fail(why); bridge?.fail(new Error(why)); };
  child.on('error', (e) => fail(`launch failed: ${e.message}`));
  const exited = new Promise((r) => child.on('exit', (code, signal) => { r(code ?? signal); fail(`the app exited (${code ?? signal}); ` + hostLines.join('\n')); }));
  const close = async () => { bridge?.close(); try { child.stdin.end(); if (device) child.kill('SIGTERM'); } catch {} await Promise.race([exited, sleep(2000)]); if (child.exitCode === null && child.signalCode === null) { try { child.kill('SIGKILL'); } catch {} } await exited; };
  let readyTimeout;
  try {
    const readyLine = device ? bridge.ready.then(({ socket, announcement }) => {
      lines = jsonLines(socket, socket, hostLines);
      socket.on('close', () => lines.fail('the phone agent connection closed; ' + hostLines.slice(-20).join('\n')));
      socket.resume();
      return announcement;
    }) : lines.next();
    const ready = await Promise.race([readyLine, new Promise((_, reject) => {
      readyTimeout = setTimeout(() => reject(new Error('the app never became ready; ' + hostLines.join('\n'))), 20000);
    })]);
    clearTimeout(readyTimeout);
    if (!ready.ready) throw new Error('unexpected first line: ' + JSON.stringify(ready));
    if (ready.error) throw new Error('the app booted with an error: ' + ready.error);
    // The sample host routes by label: the session the caller named, and
    // `s.session = "b"` moves every later request to another.
    const state = { session: session ?? null };
    const ask = async (req, session = state.session) => {
      const reply = lines.ask(session ? { ...req, session } : req);
      if (!device) return reply;
      let timer;
      try {
        return await Promise.race([reply, new Promise((_, reject) => {
          timer = setTimeout(() => {
            bridge.close();
            reject(new Error(`phone ${req.op} did not answer within 45 s; check app health, foreground state and network\n` + hostLines.slice(-20).join('\n')));
          }, 45000);
        })]);
      } finally { clearTimeout(timer); }
    };
    return {
      host, boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      ask,
      async input(id, kind, opts) {
        if (kind === 'key') {
          const session = state.session;
          return nativeKey({id, opts: linux ? {...opts, ownedRelease:false} : opts, ask: req => ask(req, session)});
        }
        const guest = { selector: opts.selector, x: opts.x, y: opts.y, entity: opts.entity, world: opts.world, under: opts.under, phase: opts.phase };
        const phase = ['down', 'move', 'hold', 'up', 'cancel'].includes(kind);
        const r = phase ? await ask({ op: 'tap', phase: kind, ...(id != null ? { id } : {}), x: opts.x, y: opts.y, dx: opts.dx, dy: opts.dy, ms: opts.ms }) : kind === 'contextmenu' || kind === 'dblclick' ? await ask({ op: 'tap', id, [kind]: true }) : kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel, ...(opts.gesture ? { gesture: true } : {}) }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest }) : await ask({ op: 'type', id, text: opts.text, ...guest });
        if (r.error) throw new Error(r.error);
        return r;
      },
      async screenshot(path, window = false) {
        const remote = device ? `${ready.container}/tmp/exact-agent.png` : path;
        const r = await ask({ op: 'screenshot', path: remote, window });
        if (r.error) throw new Error(r.error);
        if (device) {
          const copied = spawnSync('xcrun', ['devicectl', 'device', 'copy', 'from', '--quiet', '--device', ph.udid,
            '--domain-type', 'appDataContainer', '--domain-identifier', a.id, '--source', 'tmp/exact-agent.png', '--destination', resolve(path)], { encoding: 'utf8', timeout: 20000 });
          if (copied.status !== 0) throw new Error(`phone screenshot copy: ${copied.stderr || copied.error || copied.stdout}`);
          r.screenshot = path;
        }
        return r;
      },
      close,
    };
  } catch (e) {
    await close();
    throw e;
  } finally {
    clearTimeout(readyTimeout);
  }
}

// ---------------------------------------------------------------- iOS, over a Unix socket

/** The simulator carrier: the bundle `build.mjs --ios` assembled, installed and launched on a simulator with the agent socket's path in its environment (simctl passes SIMCTL_CHILD_*); then the same JSON lines over that socket (`AgentIOS.swift`). A `simctl launch --console` stays attached for the app's stdout and stderr (its `--stdout=`/`--stderr=` files stay empty on Xcode 26). One app per bundle id per device: a session replaces a running copy; closing hangs up the socket, which ends the app, and kills the pid the app reported if it lingers. */
async function openIOS({ plan, app, env: extra = {}, session, hostFixture = false }) {
  const a = resolveApp(app);
  const bundle = appleArtifacts(a, { destination: 'ios-simulator', host: hostFixture }).bundle;
  const id = hostFixture ? `${a.id}.host` : a.id;
  if (!existsSync(bundle)) throw new Error(hostFixture ? 'run bun host/apple/build.mjs --ios --host first' : 'run bun host/apple/build.mjs --ios first');
  const dev = simulator();
  install(dev, bundle, a, hostFixture);
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
    const ask = (req, session = state.session) => lines.ask(session ? { ...req, session } : req);
    // A held contact on a simulator (LLP 1035.003 §3, candidate 1 — decided
    // 2026-09-10): UIKit synthesizes no touch, so the contact is a real
    // mouse on the Mac's desktop, posted into the Simulator's window by
    // `host/apple/pointer.swift` (built here with swiftc on first use). The
    // window-to-device mapping lives in this one place: the app reports its
    // screen and where its viewport sits on it (`layout.screen`), the helper
    // reports the Simulator window's frame, and a viewport point maps
    // through both. The app receives whatever UIKit delivers from that
    // input; nothing is activated in its place. What this needs from the
    // machine — Accessibility for this terminal, the Simulator window on
    // screen and unobscured — is reported as `unsupported` with the reason
    // when it is missing, never faked.
    let pointer = null;
    let contact = null;
    let contactDesktop = null;
    // The mapping from a viewport point to the desktop, found by observation
    // — a Simulator window carries a bezel and a scale of its own that no
    // frame arithmetic knows: the Mac's pointer is hovered at two desktop
    // points inside the window and the app reports where its viewport saw
    // each (`layout.pointer`); the uniform scale and offset follow. Redone
    // whenever the window's frame changes.
    let mapping = null;
    const calibrate = async (p) => {
      const w = await p.ask({ op: 'window', title: dev.name });
      if (w.error) return { error: w.error };
      const key = `${w.x},${w.y},${w.w},${w.h}`;
      if (mapping?.key === key) return mapping;
      const probe = async (x, y) => {
        const r = await p.ask({ op: 'hover', x, y });
        if (r.error) return { error: r.error };
        await sleep(120);
        const l = await ask({ op: 'layout' });
        return l.pointer ?? null;
      };
      const a = { x: w.x + w.w * 0.5, y: w.y + w.h * 0.45 };
      const b = { x: a.x + w.w * 0.15, y: a.y + w.h * 0.2 };
      const pa = await probe(a.x, a.y);
      if (pa?.error) return pa;
      const pb = await probe(b.x, b.y);
      if (pb?.error) return pb;
      if (!pa || !pb || pa.x === pb.x || pa.y === pb.y) return { error: 'the app saw no pointer hover; is the Simulator window on screen and unobscured?' };
      const sx = (b.x - a.x) / (pb.x - pa.x), sy = (b.y - a.y) / (pb.y - pa.y);
      if (!(sx > 0 && sy > 0) || Math.abs(sx - sy) / sx > 0.1) return { error: `calibration disagrees between axes (${sx.toFixed(3)} vs ${sy.toFixed(3)})` };
      const scale = (sx + sy) / 2;
      mapping = { key, scale, ox: a.x - pa.x * scale, oy: a.y - pa.y * scale, window: w };
      return mapping;
    };
    const helper = async () => {
      if (pointer) return pointer;
      const src = resolve(ROOT, 'host/apple/pointer.swift');
      const bin = resolve(ROOT, 'host/apple/.build/pointer');
      if (!existsSync(bin) || statSync(bin).mtimeMs < statSync(src).mtimeMs) {
        mkdirSync(resolve(ROOT, 'host/apple/.build'), { recursive: true });
        const built = spawnSync('swiftc', ['-O', '-o', bin, src], { encoding: 'utf8' });
        if (built.status !== 0) throw new Error('the desktop pointer did not build: ' + (built.stderr || built.error));
      }
      const child = spawn(bin, [], { stdio: ['pipe', 'pipe', 'pipe'] });
      child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('pointer: ' + l); });
      const io = jsonLines(child.stdout, child.stdin, hostLines);
      pointer = { ask: (req) => io.ask(req), child };
      return pointer;
    };
    let canvasContact = false;
    const phaseSim = async (kind, id, opts) => {
      const p = await helper();
      const unsupported = (reason) => ({ phase: kind, delivery: 'unsupported', reason });
      if (kind === 'down') {
        if (contact) throw new Error('a contact is already down; up it first');
        const trusted = await p.ask({ op: 'trusted' });
        if (!trusted.trusted) return unsupported('the desktop pointer needs Accessibility permission for this terminal (System Settings › Privacy & Security › Accessibility)');
        await p.ask({ op: 'activate' });
        await sleep(200);
      } else if (!contact) throw new Error('no contact is down');
      if (kind === 'hold') { if (opts.ms) await sleep(opts.ms); return { phase: 'hold', at: [contact.x, contact.y], delivery: 'platform' }; }
      if (kind === 'cancel') return { phase: 'cancel', at: [contact.x, contact.y], delivery: 'unsupported', reason: 'a desktop pointer has no cancel; the contact is still down — send up' };
      const m = await calibrate(p);
      if (m.error) return unsupported(m.error);
      const map = (x, y) => ({ x: m.ox + x * m.scale, y: m.oy + y * m.scale });
      if (kind === 'down') {
        const l = await ask({ op: 'layout' });
        const b = l.nodes.find((n) => n.id === id);
        if (!b || (b.w === 0 && b.h === 0)) throw new Error(`view ${id} has no box on screen`);
        const x = opts.x ?? b.x + b.w / 2, y = opts.y ?? b.y + b.h / 2;
        contactDesktop = map(x, y);
        await p.ask({ op: 'down', ...contactDesktop });
        contact = { x, y };
        return { contact: id, phase: 'down', at: [x, y], delivery: 'platform', desktop: [contactDesktop.x, contactDesktop.y] };
      }
      if (kind === 'move') {
        const to = { x: opts.x ?? contact.x + (opts.dx ?? 0), y: opts.y ?? contact.y + (opts.dy ?? 0) };
        const ms = Math.max(0, opts.ms ?? 0);
        const steps = Math.max(1, Math.round(ms / 16));
        for (let i = 1; i <= steps; i++) {
          const t = i / steps;
          contactDesktop = map(contact.x + (to.x - contact.x) * t, contact.y + (to.y - contact.y) * t);
          await p.ask({ op: 'move', ...contactDesktop });
          if (ms) await sleep(ms / steps);
        }
        contact = to;
        return { phase: 'move', at: [to.x, to.y], delivery: 'platform' };
      }
      contactDesktop = map(contact.x, contact.y);
      await p.ask({ op: 'up', ...contactDesktop });
      const at = [contact.x, contact.y];
      contact = null;
      contactDesktop = null;
      return { phase: 'up', at, delivery: 'platform' };
    };
    const closeWithPointer = async () => {
      if (pointer) {
        // Never leave the operator's mouse button down.
        if (contact && contactDesktop) { try { await Promise.race([pointer.ask({ op: 'up', ...contactDesktop }), sleep(1000)]); } catch {} }
        try { pointer.child.stdin.end(); pointer.child.kill('SIGTERM'); } catch {}
      }
      await close();
    };
    return {
      host: hostFixture ? 'host-ios' : 'ios', boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      pointer: true,
      ask,
      async input(id, kind, opts) {
        if (kind === 'key') {
          const session = state.session;
          return nativeKey({id, opts, ask: req => ask(req, session)});
        }
        const guest = { selector: opts.selector, x: opts.x, y: opts.y, entity: opts.entity, world: opts.world, under: opts.under, phase: opts.phase };
        if (['down', 'move', 'hold', 'up', 'cancel'].includes(kind)) {
          if (kind === 'down' || canvasContact) {
            const r = await ask({ op: 'tap', phase: kind, ...(id != null ? { id } : {}), x: opts.x, y: opts.y, dx: opts.dx, dy: opts.dy, ms: opts.ms });
            if (r.error) throw new Error(r.error);
            if (r.delivery !== 'unsupported' || canvasContact) {
              canvasContact = !['up', 'cancel'].includes(kind);
              return r;
            }
          }
          return phaseSim(kind, id, opts);
        }
        const r = kind === 'contextmenu' || kind === 'dblclick' ? await ask({ op: 'tap', id, [kind]: true }) : kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel, ...(opts.gesture ? { gesture: true } : {}) }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest }) : await ask({ op: 'type', id, text: opts.text, ...guest });
        if (r.error) throw new Error(r.error);
        return r;
      },
      async screenshot(path, window = false) {
        const r = await ask({ op: 'screenshot', path, window });
        if (r.error) throw new Error(r.error);
        return r;
      },
      close: closeWithPointer,
    };
  } catch (e) {
    await close();
    throw e;
  }
}

// ---------------------------------------------------------------- the eight operations

/** A convenience over state, screenshot and type; wire replies keep all tags. */
export function worldView(session, name) {
  return {
    async snapshot() {
      const {tick, hash, entities, truncated} = await session.state(`${name}:*`);
      return {tick, hash, entities, truncated};
    },
    state: entity => session.state(`${name}:${entity}`),
    save: path => session.screenshot(path, name, 'save'),
    key: (code, opts = {}) => session.type(name, {...opts, key:code}),
    hold: (code, ms) => session.type(name, {key:code, for:ms}),
  };
}

/** Open a session on `host` ('web' | 'macos' | 'ios' | 'linux'); `url` opens
 * the same app address on each host; `plan` boots a local compiled contract;
 * `env` adds to a native host's environment. @ref LLP 1030.000 §7 */
export async function open({ host = 'web', plan, world, size, env, app, session, url, webDist, device = false, phone: pick, timing = 'agent' } = {}) {
  if (world && (device || !['web','mac','macos','ios'].includes(host))) throw new Error(`world restore unavailable on this host yet: ${host}`);
  if (world && statSync(world).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit');
  if (world && host !== 'web') env = {...env, EXACT_WORLD:resolve(world)};
  if (device && host !== 'ios') throw new Error('--device is supported for the standalone ios client');
  // `timing: 'platform'` (LLP 1035.003 D5, opt-in): the carrier stays and
  // the driver still owns the runner's clock, but UIKit's own transitions,
  // sheet presentations and keyboard animations run at their natural
  // timing — the ordinary app with a socket, for observing an interactive
  // gesture's native motion. The frozen clock is the default the smoke
  // depends on. Replies say `mode: "platform"`.
  if (!['agent', 'platform'].includes(timing)) throw new Error(`timing: agent or platform, not ${timing}`);
  if (timing === 'platform') env = { ...(env ?? {}), EXACT_AGENT_TIMING: 'platform' };
  if (url !== undefined && ['macos', 'mac', 'ios', 'linux', 'host', 'host-ios'].includes(host)) {
    // @ref LLP 1038 D5/D11 — a native scheme/path is a launch location;
    // HTTP(S) keeps the existing development-plan locator form.
    if (/^https?:\/\//i.test(url)) {
      if (plan) throw new Error('a native session takes either a development --url or --plan, not both');
      env = developmentLaunchEnvironment(['--run', '--url', url], env ?? {});
    } else env = { ...(env ?? {}), EXACT_LAUNCH_URL: url };
  }
  const carrier = device ? await openStdio({ host: 'ios', plan, env, app, device, phone: pick }) : host === 'macos' || host === 'mac' ? await openStdio({ host: 'macos', plan, size, env, app }) : host === 'host' ? await openStdio({ host: 'host', plan, env, app, session }) : host === 'host-ios' ? await openIOS({ plan, app, env, session, hostFixture: true }) : host === 'linux' ? await openStdio({ host: 'linux', plan, size, env, app }) : host === 'ios' ? await openIOS({ plan, env, app }) : await openWeb({ plan, world, size, url, app, webDist });
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
    tree: async (target, under) => target == null ? s.op({ op: 'tree' }) : s.op({ op: 'tree', ...await s.target(target), world: true, ...(under != null ? { under } : {}) }),
    /** Simulation conveniences use this receiver so proof proxies record every operation. */
    world(name) { return worldView(this, name); },
    /** Every slot, derive, and resource by name, as typed JSON. */
    state: async (target, under) => s.op({ op: 'state', ...(target != null ? await s.target(target) : {}), ...(under != null ? { under: String(under).replace(/^[^:]+:/, '') } : {}) }),
    /** What happened since the last read: the runner's journal (`lines`, from index `from` up to `next`) and the host's own output (`host`). `dropped` counts lines the journal ring let go before this read caught up. */
    async logs() {
      const r = await s.op({ op: 'logs', since: s.logCursor });
      const dropped = Math.max(0, r.from - s.logCursor);
      s.logCursor = r.next;
      return { lines: r.lines, host: carrier.hostLines.splice(0), from: r.from, next: r.next, dropped, ...(r.world ? { world: r.world } : {}) };
    },
    /** Every on-screen view's box in the viewport (scroll folded in), with its testId and type from the tree. With a target, `node` explains that one node (LLP 1035.002 D1): every row it sets or inherits with where the value came from, its box in each coordinate space the host has, the scroll and clip chains above it, whether it is hidden, inert, in the viewport or clipped away, and what the host mounted for it — observations of the runner's memory and the host's view tree, never a second model. */
    async layout(target, at) {
      const req = { op: 'layout', ...(target != null ? await s.target(target) : {}) };
      if (at) return s.op({ ...req, world: true, x: at[0], y: at[1] });
      if (req.entity !== undefined) return s.op(req);
      const [l, t] = await Promise.all([s.op(req), s.tree()]);
      const by = new Map(t.nodes.map((n) => [n.id, n]));
      for (const n of l.nodes) { const k = by.get(n.id); if (k) { n.type = k.type; if (k.props.testId) n.testId = k.props.testId; } }
      if (l.node) { const k = by.get(l.node.id); if (k?.props.testId) l.node.testId = k.props.testId; }
      return l;
    },
    async target(target) {
      const text = String(target), colon = text.indexOf(':');
      const node = await s.find(target, false);
      if (node) return { id: node.id };
      if (colon < 0) throw new Error(`no view matches ${target}`);
      return { id: (await s.find(text.slice(0, colon))).id, entity: text.slice(colon + 1) };
    },
    /** The node for a target: a testId (first in preorder) or a view id. */
    async find(target, required = true) {
      const t = await s.tree();
      const node = typeof target === 'number' || /^\d+$/.test(String(target)) ? t.nodes.find((n) => n.id === Number(target)) : t.nodes.find((n) => n.props.testId === target);
      if (!node && required) throw new Error(`no view matches ${target}`);
      return node;
    },
    /**
     * What this carrier's input actually is (LLP 1035.003 D2/D3): whether it
     * can hold a contact across requests, and how each form is delivered —
     * `platform` (a real input event through the platform's own path),
     * `recognized` (an already-recognized event injected), `activation` (a
     * hit-test and a direct call), or `unsupported`. iOS activates and
     * injects; it synthesizes no touch (LLP 1008 §9).
     */
    input: host === 'ios' || host === 'host-ios'
      ? { contact: carrier.pointer === true, hold: carrier.pointer === true, delivery: (kind) => (['contextmenu', 'dblclick', 'hover'].includes(kind) ? 'recognized' : ['down', 'move', 'hold', 'up', 'cancel'].includes(kind) ? (carrier.pointer ? 'platform' : 'unsupported') : 'activation') }
      : host === 'linux'
        ? { contact: false, hold: false, delivery: (kind) => (['down', 'move', 'hold', 'up', 'cancel'].includes(kind) ? 'unsupported' : 'platform') }
        : { contact: true, hold: true, delivery: () => 'platform' },
    /** The contact this session holds, `{x, y}` in the viewport's space, or null. */
    contact: null,
    /** A press on the target through the host's input path (an iframe target accepts guest `selector` or `x`/`y`); with `{ wheel: [dx, dy] }`, a wheel over it (dy > 0 scrolls down); with `{ hover: true }`, the pointer moved onto it (a hover — and off whatever it was over); with `{ down: true[, at: [x, y]] }`, a contact goes down on it (at its centre, or at an offset from its corner) and stays down until `pointer('up')` (LLP 1035.003 D1). Every reply says how it was delivered (`delivery`), by which carrier, in which mode. */
    async tap(target, opts = {}) {
      const node = await s.target(target);
      if (node.entity !== undefined) {
        const { entity } = await s.op({ op: 'layout', ...node });
        if (entity?.visible?.inFrustum === false || entity?.visible?.behindCamera === true) throw new Error(`${target} is off screen`);
        const b = entity?.screen;
        if (!b || ![b.x, b.y, b.w, b.h].every(Number.isFinite)) throw new Error(`${target} has no screen box`);
        const x = b.x + b.w / 2, y = b.y + b.h / 2;
        const { hit } = await s.op({ op: 'layout', id: node.id, world: true, x, y });
        if (!hit) throw new Error(`${target} is not hit at ${x},${y}`);
        if (hit.id !== entity.id) throw new Error(`${target} is behind ${hit.name ?? hit.id} at ${x},${y}`);
        if (s.contact) throw new Error('a contact is already down; up or cancel it first');
        const down = await carrier.input(node.id, 'down', { x, y });
        const { phase, ...r } = down.delivery === 'unsupported' ? down : await carrier.input(null, 'up', {});
        return s.tagged({ ...r, tapped: node.id, target, entity: node.entity, at: [x, y], delivery: r.delivery ?? s.input.delivery('down'), carrier: host, mode: timing });
      }
      // @ref LLP 1038 D11 — no native carrier turns browser history into a press.
      if (opts.history !== undefined && host !== 'web') return s.tagged({ tapped: node.id, target, history: opts.history, delivery: 'unsupported', carrier: host, mode: timing });
      // A gesture is the platform's, and only the AppKit carrier can phase
      // one. Refusing beats quietly sending a bare delta: the whole reason
      // this form exists is that a plain wheel tests a path a finger never
      // takes, so a driver must never be told it sent a gesture when it did
      // not (LLP 0382 — fail closed, loudly).
      if (opts.gesture && !(host === 'macos' || host === 'mac')) throw new Error(`${host} cannot phase a wheel; \`gesture\` is the AppKit carrier's`);
      if ((opts.contextmenu || opts.dblclick) && !['web', 'ios'].includes(host)) throw new Error(`${host} does not carry contextmenu/dblclick input`);
      const kind = opts.history !== undefined ? 'history' : opts.down ? 'down' : opts.wheel ? 'wheel' : opts.hover ? 'hover' : opts.contextmenu ? 'contextmenu' : opts.dblclick ? 'dblclick' : 'press';
      if (kind === 'down' && s.contact) throw new Error('a contact is already down; up or cancel it first');
      let at;
      if (kind === 'down' && opts.at) { const b = (await s.layout()).nodes.find((n) => n.id === node.id); if (!b) throw new Error(`view ${node.id} has no box on screen`); at = { x: b.x + opts.at[0], y: b.y + opts.at[1] }; }
      const r = await carrier.input(node.id, kind, { ...opts, ...at });
      if (kind === 'down' && r.delivery !== 'unsupported') s.contact = { x: r.at[0], y: r.at[1] };
      return s.tagged({ ...r, tapped: node.id, target, delivery: r.delivery ?? s.input.delivery(kind), carrier: host, mode: timing });
    },
    /**
     * The held contact's next phase (LLP 1035.003 D1): `move` to `{x, y}` in
     * the viewport or `by` `{dx, dy}`, over `ms` of real time (the platform
     * recognizes velocity from the steps); `hold` for `ms`; `up`; `cancel`.
     * The platform owns hit-testing, recognition, scrolling and animation:
     * the app receives whatever it delivers, and a carrier that cannot hold
     * a contact answers `delivery: "unsupported"` rather than faking one.
     */
    async pointer(phase, opts = {}) {
      if (!['move', 'hold', 'up', 'cancel'].includes(phase)) throw new Error(`pointer: not a phase: ${phase} (move, hold, up, cancel)`);
      if (!s.contact) throw new Error('no contact is down (tap <target> down first)');
      const r = await carrier.input(null, phase, opts);
      if (r.delivery !== 'unsupported') {
        if (phase === 'move') s.contact = { x: r.at[0], y: r.at[1] };
        if (phase === 'up' || phase === 'cancel') s.contact = null;
      }
      return s.tagged({ ...r, phase, delivery: r.delivery ?? s.input.delivery(phase), carrier: host, mode: timing });
    },
    /** Deliver a location to a navigation root (LLP 1038 D11), or set an input's text through the host's text input path; an iframe accepts `{text, selector}` or `{key, selector}` for its guest. */
    async type(target, text) {
      const node = await s.find(target);
      const options = typeof text === 'object' && text !== null ? text : { text };
      const key = options.key;
      if (options.for !== undefined) {
        if (key == null || options.phase != null || !Number.isFinite(options.for) || options.for < 0) throw new Error('type for: expected a key and a nonnegative finite duration, without phase');
        return typeFor({ node, target, options, carrier, clock: spec => s.clock(spec), tagged: reply => s.tagged(reply), delivery: s.input.delivery('key'), host, timing });
      }
      const r = key != null ? await carrier.input(node.id, 'key', { ...options, key: String(key) }) : await carrier.input(node.id, 'type', { ...options, text: String(options.text ?? '') });
      return s.tagged({ ...r, typed: node.id, target, delivery: r.delivery ?? s.input.delivery(key != null ? 'key' : 'type'), carrier: host, mode: timing });
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
    /** Pixels as PNG (second argument true includes the native window), or a canvas carry with `(path, target, "save")`. */
    screenshot: async (path, target = false, form) => {
      if (form === 'save') {
        if (!['web','macos','ios'].includes(s.host) || device) throw new Error(`world save unavailable on this host yet: ${s.host}`);
        const reply = await s.op({op:'screenshot', ...await s.target(target), world:true, form:'save'});
        const {data, ...metadata} = reply;
        if (typeof data !== 'string') throw new Error(`canvas ${target} returned no save bytes`);
        if (reply.bytes > WORLD_LIMIT || data.length > 4 * Math.ceil(WORLD_LIMIT / 3)) throw new Error('world carrier exceeds 256 MiB limit');
        const bytes = Buffer.from(data, 'base64');
        if (bytes.length !== reply.bytes) throw new Error(`canvas ${target} returned a truncated save`);
        writeFileSync(resolve(path), bytes);
        return s.tagged({...metadata, screenshot:resolve(path)});
      }
      if (form !== undefined) throw new Error(`screenshot: unknown form ${form}`);
      return s.tagged(await carrier.screenshot(resolve(path), target));
    },
    /**
     * Every reply carries the runner's `epoch`, `incarnation` and `clock`
     * (LLP 1035.002 D3). A host that answered the operation itself stamps
     * them; the web carrier's input and capture are the driver's own (CDP),
     * so the driver reads the tags after the operation and adds what the
     * reply lacks. An error is left alone.
     */
    async tagged(r) {
      if (r == null || r.error != null || r.epoch != null) return r;
      const tags = await s.op({ op: 'tags' });
      for (const key of Object.keys(tags)) if (r[key] === undefined) r[key] = tags[key];
      return r;
    },
    close: carrier.close,
  };
  return s;
}

// ---------------------------------------------------------------- the CLI

/** Browser-owned key release carries device identity, never a canvas lookup. */
export async function browserKey({id, opts, evaluate, ask, call, frame}) {
  if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new Error(`key: not a phase: ${opts.phase}`);
  const isWorld = await evaluate(`exact.gpu?.wantsInput(${id}) ?? false`);
  const f = isWorld ? await ask({ op: 'focus', id, world: true }) : await evaluate(`(() => { const el = exact.views.get(${id}); el?.focus(); return {ok:document.activeElement === el}; })()`);
  if (f.error || !f.ok) throw new Error(f.error ?? `view ${id} could not take focus`);
  let code = opts.key, key, vk;
  if (/^Key[A-Z]$/.test(code)) { key = code.slice(3).toLowerCase(); vk = code.charCodeAt(3); }
  else if (/^Digit[0-9]$/.test(code)) { key = code.slice(5); vk = code.charCodeAt(5); }
  else {
    const special = { ArrowUp: ['ArrowUp', 38], ArrowDown: ['ArrowDown', 40], ArrowLeft: ['ArrowLeft', 37], ArrowRight: ['ArrowRight', 39], Space: [' ', 32], Enter: ['Enter', 13], Escape: ['Escape', 27], Shift: ['Shift', 16], ShiftLeft: ['Shift', 16], ShiftRight: ['Shift', 16] }[code];
    if (!special) throw new Error(`key: unsupported code ${code}`);
    [key, vk] = special;
    if (code === 'Shift') code = 'ShiftLeft';
  }
  const reply = phase => ({ typed: id, key: opts.key, ...(phase != null ? { phase } : {}), delivery: 'platform' });
  const release = async () => {
    await call('Input.dispatchKeyEvent', { type: 'keyUp', code, key, windowsVirtualKeyCode: vk });
    await frame();
    return reply('up');
  };
  try {
    for (const phase of opts.phase == null ? ['down', 'up'] : [opts.phase]) await call('Input.dispatchKeyEvent', { type: phase === 'down' ? 'keyDown' : 'keyUp', code, key, windowsVirtualKeyCode: vk });
    await frame();
  } catch (error) { if (opts.phase === 'down') error.release = release; throw error; }
  return { ...reply(opts.phase), ...(opts.phase === 'down' ? { release } : {}) };
}

/** Native key carrier shared by stdio, phone and simulator. */
export async function nativeKey({id, opts, ask}) {
  const releaseKey = opts.phase === 'down' && opts.ownedRelease ? randomBytes(16).toString('hex') : undefined;
  const {ownedRelease, ...input} = opts;
  const release = async () => {
    const r = await ask({op:'type', releaseKey, phase:'up'});
    if (r.error) throw new Error(r.error);
    return r;
  };
  try {
    const r = await ask({op:'type', id, ...input, ...(releaseKey ? {releaseKey} : {})});
    if (r.error) throw new Error(r.error);
    return {...r, ...(releaseKey ? {release} : {})};
  } catch (error) { if (releaseKey) error.release = release; throw error; }
}

/** Held-key form: one resolved carrier, including release after a failed clock. */
export async function typeFor({node, target, options, carrier, clock, tagged, delivery, host, timing}) {
  const {for: duration, ...held} = options, key = String(held.key), steps = [];
  let release;
  const send = async phase => {
    const args = [target, {...held, phase}];
    try {
      const result = phase === 'up' && release ? await release() : await carrier.input(node.id, 'key', {...held, key, phase, ownedRelease:true});
      const {release: ownedRelease, ...r} = result;
      if (phase === 'down') release = ownedRelease;
      const reply = await tagged({...r, typed:node.id, target, delivery:r.delivery ?? delivery, carrier:host, mode:timing});
      steps.push({op:'type', args, reply});
    } catch (error) { release ??= error.release; steps.push({op:'type', args, error:error.message}); throw error; }
  };
  let failure;
  try {
    await send('down');
    const args = [`+${duration}`];
    try { steps.push({op:'clock', args, reply:await clock(args[0])}); }
    catch (error) { steps.push({op:'clock', args, error:error.message}); throw error; }
  } catch (error) { failure = error; }
  finally { try { await send('up'); } catch (error) { failure ??= error; } }
  if (failure) { failure.steps = steps; throw failure; }
  return tagged({typed:node.id, target, key, for:duration, delivery:steps[0].reply.delivery, steps});
}
/** Parse the CLI type form without treating an ordinary text suffix as a key. */
export function typeArguments(args) {
  if (args[1] !== 'key' || !args[2]) return [args[0], args.slice(1).join(' ')];
  if (args[3] === 'for') {
    if (args.length !== 5) throw new Error('type key for: expected one duration');
    return [args[0], {key:args[2], for:Number(args[4])}];
  }
  return [args[0], {key:args[2], ...(args[3] != null ? {phase:args[3]} : {})}];
}

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
 *           #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy} [overscroll {ox},{oy}]
 *   logs    "(N earlier lines dropped by the journal ring)" when dropped > 0; the journal lines as they are;
 *           the host's lines indented two spaces; "(nothing new)" when there is nothing
 *   state   the JSON, indented two spaces
 *   others  the JSON on one line
 */
export function render(op, r) {
  const q = JSON.stringify;
  switch (op) {
    case 'tree': {
      const entities = Array.isArray(r.entities) ? r.entities : r.nodes?.some((n) => n.components) ? r.nodes : null;
      if (entities) return [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · tick ${r.tick} · ${entities.length} entities`, ...entities.map((e) => `${'  '.repeat(e.depth ?? 0)}${e.name ?? ''}#${e.id} (${(e.components ?? []).join(', ')})${e.tags?.length ? ' ' + e.tags.join(' ') : ''}`), ...(r.truncated ? ['(truncated)'] : [])].join('\n');
      const lines = [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · ${r.nodes.length} nodes`];
      for (const n of r.nodes) {
        const p = n.props ?? {};
        lines.push(`${'  '.repeat(n.depth)}${n.type}#${n.id}${p.testId != null ? ` [${p.testId}]` : ''}${p.text != null ? ` ${q(p.text)}` : ''}${p.value != null ? ` value=${q(p.value)}` : ''}${p.accessibilityLabel != null ? ` label=${q(p.accessibilityLabel)}` : ''}${n.handlers?.length ? ` (${n.handlers.join(', ')})` : ''}${n.url != null ? ` url=${q(n.url)} loading=${n.loading}` : ''}${n.world ? ` world{${n.world.name}} · ${n.world.entities} entities · tick ${n.world.tick}` : ''}`);
        for (const g of n.guest ?? []) lines.push(`${'  '.repeat(n.depth + g.depth + 1)}[guest] ${g.tag}${g.id != null ? `#${g.id}` : ''}${g.testId != null ? ` [${g.testId}]` : ''}${g.text != null ? ` ${q(g.text)}` : ''}`);
      }
      return lines.join('\n');
    }
    case 'layout': {
      if (r.entity) { const e = r.entity, b = e.screen, p = e.world?.position; return `#${e.id ?? ''} ${e.name ?? ''}${p ? ` world ${Array.isArray(p) ? p.join(',') : [p.x, p.y, p.z].join(',')}` : ''}${b ? ` · screen ${b.x},${b.y} ${b.w}×${b.h}` : ''}${e.depth != null ? ` · depth ${e.depth}` : ''}${e.visible?.inFrustum ? ' · in frustum' : ''}`; }
      if (!r.viewport) return q(r);
      const e = r.env;
      const env = e && Object.values(e).some((v) => v) ? ` · safe-area ${e['safe-area-inset-top']} ${e['safe-area-inset-right']} ${e['safe-area-inset-bottom']} ${e['safe-area-inset-left']} · keyboard ${e['keyboard-inset-height']}` : '';
      // `overscroll` is how far a scroller sits past its own ends — a
      // stretched rubber band, which the offset alone cannot distinguish
      // from an ordinary scroll position. Printed only when there is one.
      const past = (n) => (n.ox != null || n.oy != null ? ` overscroll ${n.ox ?? 0},${n.oy ?? 0}` : '');
      const lines = [`viewport ${r.viewport.w}×${r.viewport.h}${past(r.viewport)}${env} · clock ${r.clock} ms`].concat(r.nodes.map((n) => `#${n.id}${n.testId != null ? ` [${n.testId}]` : ''}${n.type != null ? ` ${n.type}` : ''} ${n.native?.placement === 'window' ? `${n.native.view} · system-owned geometry` : `${n.x},${n.y} ${n.w}×${n.h}${n.sx != null ? ` scroll ${n.sx},${n.sy}` : ''}${past(n)}`}`));
      if (r.node) lines.push(...renderNode(r.node));
      return lines.join('\n');
    }
    case 'logs':
      return [...(r.dropped > 0 ? [`(${r.dropped} earlier lines dropped by the journal ring)`] : []), ...r.lines, ...(r.world ?? []).flatMap((w) => w.lines.map((line) => 'world ' + line)), ...(r.host ?? []).map((l) => '  ' + l)].join('\n') || '(nothing new)';
    case 'state':
      return q(r, null, 2);
    case 'type':
      if (r.steps) return r.steps.map(step => `${step.op} ${step.args.map(a => typeof a === 'string' ? a : q(a)).join(' ')}\n${step.error ? 'ERROR ' + step.error : render(step.op, step.reply)}`).join('\n');
      return q(r);
    default:
      return q(r);
  }
}

/**
 * The block `layout <target>` adds under the listing (LLP 1035.002 D1): the
 * node's identity and site, one line per row with its source, its spaces,
 * the chains above it, its visibility, and what the host mounted. Every
 * part appears only when the host reported it — a space a host cannot
 * observe is absent, never a zero.
 *
 *   node #{id} [{testId}] {Type} · site {n} · instance {keys} · epoch E · incarnation I
 *     {row} = {value} ({authored | inherited from #id | initial}[, applied {…}])
 *     space viewport X,Y W×H · frame X,Y W×H (kernel, in the parent) · window X,Y W×H · screen X,Y W×H · scale S
 *     scroll [viewport | #id] sx,sy · … (outermost first)   clip #id overflow|clip-path · …
 *     visible hidden=… inert=… inViewport=… clipped=…       native key=value …
 *     browser {row}="{the browser's computed value}" …      (the web's oracle beside the kernel's answer)
 */
function renderNode(n) {
  const q = JSON.stringify;
  const box = (b) => (b ? `${b.x},${b.y} ${b.w}×${b.h}` : '—');
  const instance = (n.instance ?? []).map((i) => (i.key !== undefined ? q(i.key) : `arm ${i.arm}`)).join(' / ');
  const out = [`node #${n.id}${n.testId != null ? ` [${n.testId}]` : ''} ${n.type}${n.site != null ? ` · site ${n.site}` : ''}${instance ? ` · instance ${instance}` : ''} · epoch ${n.epoch} · incarnation ${n.incarnation}`];
  // Rows and keys sort by name: a host that answers through a dictionary
  // (AppKit, UIKit) has no order to offer, and the transcript must not
  // depend on which host answered.
  const sorted = (o) => Object.entries(o ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  for (const [row, v] of sorted(n.style)) out.push(`  ${row} = ${typeof v.value === 'string' ? v.value : q(v.value)} (${v.source}${v.from != null ? ` from #${v.from}` : ''}${v.applied != null ? `, applied ${v.applied}` : ''})`);
  const sp = n.space ?? {};
  out.push(`  space viewport ${box(sp.viewport)}${n.frame ? ` · frame ${box(n.frame)} (kernel, in the parent)` : ''}${sp.window ? ` · window ${box(sp.window)}` : ''}${sp.screen ? ` · screen ${box(sp.screen)}` : ''}${sp.capture?.scale != null ? ` · scale ${sp.capture.scale}` : ''}`);
  if (n.scroll?.length) out.push(`  scroll ${n.scroll.map((c) => `${c.id != null ? `#${c.id}` : 'viewport'} ${c.sx},${c.sy}`).join(' · ')}`);
  if (n.clip?.length) out.push(`  clip ${n.clip.map((c) => `#${c.id} ${c.kind}`).join(' · ')}`);
  if (n.visible) out.push(`  visible ${sorted(n.visible).map(([k, v]) => `${k}=${v}`).join(' ')}`);
  if (n.native) out.push(`  native ${sorted(n.native).map(([k, v]) => `${k}=${typeof v === 'string' ? v : q(v)}`).join(' ')}`);
  if (n.browser) out.push(`  browser ${sorted(n.browser).map(([k, v]) => `${k}=${q(v)}`).join(' ')}`);
  return out;
}

/**
 * Run a `test "…"` file (LLP 1017 P7) against a host: `contract test <file>`
 * turns the blocks into steps — the eight operations, plus `expect` lines
 * over their replies — and this drives them through the same session the
 * operations use. One session per file; a failed expect names the test, the
 * line, and what was seen. Returns `{ passed, failed, results }`.
 */
export async function runTests({ host, file, plan, app, size, env, webDist, device = false, phone, url } = {}) {
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
    const s = await open({ host, plan, size, env, app, webDist, device, phone, url });
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
    else if (argv[i] === '--world') flags.world = resolve(argv[++i]);
    else if (argv[i] === '--plan') flags.plan = resolve(argv[++i]);
    else if (argv[i] === '--app') flags.app = argv[++i];
    else if (argv[i] === '--size') flags.size = argv[++i].split('x').map(Number);
    else if (argv[i] === '--test') flags.test = argv[++i];
    else if (argv[i] === '--session') flags.session = argv[++i];
    else if (argv[i] === '--url') flags.url = argv[++i];
    else if (argv[i] === '--device') flags.device = true;
    else if (argv[i] === '--timing') flags.timing = argv[++i];
    else if (argv[i] === '--phone') flags.phone = argv[++i];
    else rest.push(argv[i]);
  }
  const [host, ...ops] = rest;
  if (host && flags.test) {
    const r = await runTests({ host, file: flags.test, plan: flags.plan, app: flags.app, size: flags.size, device: flags.device, phone: flags.phone, url: flags.url });
    for (const t of r.results) {
      console.log(`test "${t.name}": ${t.failures.length ? 'FAIL' : 'ok'}`);
      for (const f of t.failures) console.error('  ' + f);
    }
    console.log(`${r.passed} passed, ${r.failed} failed`);
    return r.failed ? 1 : 0;
  }
  if (!host || !ops.length) {
    console.error('usage: bun scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--app <name>] [--plan <file> | --url <url>] [--world <file>] [--device] [--phone <name|udid>] [--session <label>] [--json] <op> [<op> …]\n  tree | layout | state | logs | screenshot <png> [window] | screenshot <path> <canvas> save | tap <target> [wheel <dx> <dy> [gesture] | hover | history <n> | {"history":n} | contextmenu | dblclick] | type <target> <text…> | type <target> key <Name> [for <ms>] | clock <ms|+ms|settle>\n       bun scripts/agent.mjs <host> --test <file.test.contract>   (LLP 1017 P7: the file\'s `test` blocks, run here)');
    return 2;
  }
  const s = await open({ host, plan: flags.plan, world: flags.world, size: flags.size, app: flags.app, session: flags.session, url: flags.url, device: flags.device, phone: flags.phone, timing: flags.timing });
  try {
    for (const line of ops) {
      const [op, ...args] = line.trim().split(/\s+/);
      let r;
      switch (op) {
        case 'tree': r = await s.tree(args[0], args[1] === 'under' ? args[2] : undefined); break;
        case 'state': r = await s.state(args[0], args[1] === 'under' ? args[2] : undefined); break;
        case 'logs': r = await s.logs(); break;
        case 'layout': r = await s.layout(args[0], args[1] === 'at' ? [Number(args[2]), Number(args[3])] : undefined); break;
        case 'screenshot': r = await s.screenshot(args[0] ?? 'screenshot.png', args[2] === 'save' ? args[1] : args[1] === 'window', args[2]); break;
        case 'tap':
          // The contact's phases (LLP 1035.003 D1) read as `tap move …`,
          // `tap hold`, `tap up`, `tap cancel` only while a contact is down;
          // with none down those words are targets like any other.
          if (s.contact && args[0] === 'move') {
            const by = args[1] === 'by';
            const over = args.indexOf('over');
            const [a, b] = by ? [args[2], args[3]] : [args[1], args[2]];
            r = await s.pointer('move', { ...(by ? { dx: Number(a), dy: Number(b) } : { x: Number(a), y: Number(b) }), ms: over > 0 ? Number(args[over + 1]) : 0 });
          } else if (s.contact && args[0] === 'hold') r = await s.pointer('hold', { ms: Number(args[1] ?? 0) });
          else if (s.contact && (args[0] === 'up' || args[0] === 'cancel')) r = await s.pointer(args[0]);
          else if (args[1] === 'down') r = await s.tap(args[0], { down: true, at: args[2] === 'at' ? [Number(args[3]), Number(args[4])] : undefined });
          else if (args[1] === 'history') r = await s.tap(args[0], { history: Number(args[2]) });
          else if (args[1]?.startsWith('{')) r = await s.tap(args[0], JSON.parse(args.slice(1).join(' ')));
          else r = args[1] === 'wheel' ? await s.tap(args[0], { wheel: [Number(args[2]), Number(args[3])], gesture: args[4] === 'gesture' }) : args[1] === 'hover' ? await s.tap(args[0], { hover: true }) : ['contextmenu', 'dblclick'].includes(args[1]) ? await s.tap(args[0], { [args[1]]: true }) : await s.tap(args[0]);
          break;
        case 'type': r = await s.type(...typeArguments(args)); break;
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
  main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { if (e.steps) console.error(render('type', {steps:e.steps})); console.error(e.message); process.exit(1); });
}
