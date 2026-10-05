#!/usr/bin/env bun
// The agent API's driver (LLP 1012, 1079): the ten operations —
//   tree · screenshot · tap · type · state · layout · logs · clock · prefer · perf
// — against a running app on either host, from one script, with the clock in
// the driver's hands: nothing moves between two calls unless a call moved it.
//
// Usage:  bun scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--browser chrome|firefox|webkit] [--plan <file>] [--world <file>] [--url <page>] [--session <label>] [--open <document> …] [--size <w>x<h>] [--json] <op> [<op> …]
//   Web defaults to Chrome; EXACT_WEB_BROWSER selects the same option. Install
//   the other engines with: bunx playwright@1.63.0 install firefox webkit
//   tree | layout | state | logs | screenshot <png> [window] | screenshot <png|apng> over <ms> every <ms> | screenshot <path> <canvas> save
//   tap <target> [wheel <dx> <dy> [gesture] [modifiers <Control…>] | drop <path…> | into <key> [block <v>] [inline <v>] | hover | history <n> | {"history":n} | contextmenu | mouse | dblclick | modifiers <Shift+Meta…> | pinch <scale> [at <x> <y>] | drag <dx> <dy> [from <x> <y>] [mouse] [press <ms>] [over <ms>] [hold <ms>] [during "<op>" …, inside the hold under --touch platform]] | type <target> <text…> | type <target> key <Name> | type <target> copy|cut|paste <text…>
//   tap <target> down [at <x> <y>] · tap move [by] <x> <y> [over <ms>] · tap hold <ms> · tap up · tap cancel   (a held contact, LLP 1035.003 D1)
//   tap @N <choice> | type @N <value>   (a held device request, by ticket: LLP 1069.007 D4)
//   tap @<id> <choice> | type @<id> <value>   (by the node it answers at, or its capability: files F11)
//   clock <ms|+ms|+ms real|settle|data> | resize <w>x<h> | prefer <media feature or page fact> <value> […] | perf [<target>] [during "<op>" …] | perf frames [late <n>]
//   bun scripts/agent.mjs trace <file>   (a development session's trace, LLP 1079 D5: no app runs)
// A target is a testId or a view id; each op is one argument (quote it).
// `tap … wheel <dx> <dy> gesture` sends the wheel as a trackpad's gesture —
// began, changed, and the zero-delta lift that ends it (LLP 1033 D4a, macOS
// only); `tap … hover` moves the pointer onto the target (LLP 1005 §3). --device: build/install first with build.mjs --device; no Mac-local plan/assets paths.
import { Cdp, copyCdpFailureContext, chromium, closeWindowsBrowser, retainCleanupError, removeBrowserProfile, driveStore, traceLocators, parseFlags, launchFacts, launchEnvironment, refuseStale, unchecked, depInfoChanges, packagedBuildChanges, receiptChanges, webChanges, bakedPlans } from './agent-launch.mjs';
export { Cdp } from './agent-launch.mjs';
import { sourceMapReaders, identifyInspectedNode, render, perfOp, readTrace, renderTrace, layoutArgs } from './agent-inspect.mjs';
import { LAUNCH_MEDIA, preferGroups, preferOp, preferWeb } from './agent-prefer.mjs';
import { axTree } from './agent-ax.mjs';
export { sourceMapReader, identifyInspectedNode, render } from './agent-inspect.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
const WORLD_LIMIT = 256 * 1024 * 1024;
function worldFile(path) {
  if (statSync(path).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  const bytes = readFileSync(path);
  if (bytes.length > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  return bytes;
}
import { connect } from 'node:net';
import { contactSheet, decodePng, encodeApng, encodePng } from './png.mjs';
/** Film's bounds (LLP 1012.001.000 D2): a drive's pictures, not a recording, decoded in memory at once. */
const FILM_FRAMES = 240, FILM_PIXELS = 64e6;
import { tmpdir } from 'node:os';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { openTouches, realTap } from '../host/apple/touches.mjs';
import { dragTap } from './agent-drag.mjs';
import { runTests } from './agent-test.mjs';
import { cdpKey, browserKey, nativeKey, typeFor, ticketOf, holdOf, heldTicket, typeArguments, pickedPaths, mouseContact, withHeldModifiers } from './agent-keys.mjs';
export { cdpKey, browserKey, nativeKey, typeFor, ticketOf, holdOf, heldTicket, typeArguments, pickedPaths, mouseContact } from './agent-keys.mjs';
import { appleArtifacts, assertAppleIdentity, bundleId, install } from '../host/apple/build.mjs';
import { crashReports, developmentLaunchEnvironment, phone, phoneBridge, showSimulator, simulator } from '../host/apple/devices.mjs';
import { builtAppMatches, jsTargetBuild, serveBuildTree, serveStatic } from '../host/web/serve.mjs';
import { bakeOutput, bakeTarget, linuxBinary, linuxBuild, resolveApp, webDist as defaultWebDist } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// A completed operation must release its deadline too, so an otherwise closed
// driver does not stay alive until a losing timeout expires.
async function waitAtMost(operation, ms, onTimeout) {
  let timer;
  const deadline = new Promise(resolve => { timer = setTimeout(resolve, ms); }).then(onTimeout);
  try { return await Promise.race([operation, deadline]); }
  finally { clearTimeout(timer); }
}

/** Browser-process diagnostics that do not describe the page or Exact. Page
 * exceptions and console errors arrive over CDP separately and remain logs. */
export function browserDiagnosticNoise(line) {
  return /crashpad|updater|gcm|VERBOSE|DevTools listening/i.test(line)
    // Linux without a session bus: Chrome's dbus client reports it on every launch.
    || /:ERROR:dbus\/(bus|object_proxy)\.cc:\d+\] (Failed to connect to the bus|Failed to call method: org\.freedesktop\.DBus)/.test(line)
    || /CVDisplayLinkCreateWithCGDisplay failed|CVReturn:\s*-6670/i.test(line)
    // The browser process checking the renderer's paint-timing report
    // against itself (two paints in one frame, image before first): its
    // bookkeeping, not the page's. The page's own errors come over CDP.
    || /\bpage_load_metrics_update_dispatcher\.cc:\d+\] Invalid first_\w+ [\d.]+ s for \w+ [\d.]+ s$/.test(line);
}

// ---------------------------------------------------------------- web

/** Every desktop carrier's viewport unless a drive names one (LLP 1012.001.000 D8, Charlie 2026-09-30: 900, the page's and the conformance run's), so one drive gives one set of numbers on every host. A phone or simulator is its device's size. */
export const VIEWPORT = [420, 900];

export { LAUNCH_MEDIA, PREFERENCES, PAGE_FACTS, FOLD_FACTS, displayFeatures } from './agent-prefer.mjs'; // `prefer`'s tables and the web carrier's CDP path
/** An app made by `exact new` builds itself: its own `exact.mjs web-build`, when the dist is its default one. */
/** An `exact new` app's own command for a native build (`mac`, `ios`), else null. */
const ownAppleBuild = (app, verb) => app.dir && !resolve(app.dir).startsWith(ROOT + '/') && existsSync(resolve(app.dir, 'exact.mjs'))
  ? `(cd '${String(app.dir).replaceAll("'", "'\\''")}' && bun exact.mjs ${verb})` : null;
const ownWebBuild = (app, dist) => app.dir && app.target && existsSync(resolve(app.dir, 'exact.mjs')) && resolve(dist) === resolve(app.target, 'web-dist')
  ? `(cd '${String(app.dir).replaceAll("'", "'\\''")}' && bun exact.mjs web-build)` : null;
/** Refuse to drive anything but a complete, authenticated build of the
 * selected app. The build marker binds every public runtime artifact. */
export async function assertWebDistApp(dist, app) {
  const shellQuote = value => "'" + String(value).replaceAll("'", "'\\''") + "'";
  if (!await builtAppMatches(dist, app)) throw new Error(`web dist is not a complete build for selected app ${app.id}; stale receipt ${resolve(dist, ".exact-build.json")}; run ${ownWebBuild(app, dist) ?? `EXACT_APP_DIR=${shellQuote(app.dir)} EXACT_WEB_DIST=${shellQuote(resolve(dist))} bun host/web/build.mjs ${app.crate('web')}`}`);
}

async function openWeb({ browser = 'chrome', plan, world, size = VIEWPORT, url: pageURL, app, webDist, onProcess, reuse, storage, fresh = false, facts, env }) {
  if (browser !== 'chrome') {
    const { openPlaywrightWeb } = await import('./agent-playwright.mjs');
    return openPlaywrightWeb({ browser, plan, world, size, url: pageURL, app, webDist, onProcess, reuse, storage, facts });
  }
  if (reuse) {
    if (reuse.browser === 'chrome' && JSON.stringify(reuse.launchFacts) === JSON.stringify(facts)) {
      try { await reuse.reset(); return reuse; }
      catch (error) { await reuse.close(); throw error; }
    }
    await reuse.close();
  }
  const selected = resolveApp(app);
  const dist = resolve(webDist ?? defaultWebDist());
  if (!pageURL) {
    await assertWebDistApp(dist, selected);
    const js = jsTargetBuild(dist), env = process.env.EXACT_APP_DIR || webDist || process.env.EXACT_WEB_DIST ? `EXACT_APP_DIR=${selected.dir} EXACT_WEB_DIST=${dist} ` : '';
    const command = (js && ownWebBuild(selected, dist)) || `${env}bun host/web/build.mjs ${selected.crate('web')}${js ? '' : ' --wasm'}`, changed = webChanges(dist, selected);
    refuseStale('web', resolve(dist, '.exact-build.json'), changed.all, command);
  }
  // A JS-target build (LLP 1071) is served as its tree. It compiles one plan
  // ahead of time, so `--plan` is a JS build of that plan (host/web-js/build.mjs
  // --plan, over the app's data sources: its Rust module from the dist, when
  // it has one), served instead of the app's; on the wasm target the page
  // boots the app and swaps the plan in (`exact.reload`).
  const js = !pageURL && jsTargetBuild(dist);
  let served = dist, planBuild = null;
  if (js && plan) {
    planBuild = mkdtempSync(resolve(tmpdir(), 'exact-agent-plan-'));
    const b = spawnSync(process.execPath, [resolve(ROOT, 'host/web-js/build.mjs'), selected.name, '--plan', resolve(plan), '--out', planBuild, '--render', 'none',
      ...(existsSync(resolve(dist, 'rust/wasm/app.module.wasm')) ? ['--data', dist] : [])], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: selected.dir }, encoding: 'utf8', maxBuffer: 64 << 20 });
    if (b.status !== 0) { rmSync(planBuild, { recursive: true, force: true }); throw new Error(`--plan ${plan}: its JS build failed: ${(b.stderr ?? '').trim().split('\n').slice(-3).join(' ')}`); }
    served = planBuild;
  }
  const server = createServer((req, res) => {
    if (req.url === '/__plan' && plan) { res.writeHead(200, { 'content-type': 'application/octet-stream' }); res.end(readFileSync(plan)); return; }
    if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    if (js) return serveBuildTree(served, req, res);
    serveStatic(dist, req, res);
  });
  // A named store's profile and origin are kept between drives, as a native scratch store is; `fresh` empties it first (a test's).
  // Under the drive's env, as `runTests` places and removes it (review b5-c 3).
  const kept = storage === undefined ? null : driveStore(selected.id, storage, env);
  if (kept && fresh) await removeBrowserProfile(kept.profile);
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(kept && !pageURL ? kept.port : 0, '127.0.0.1', ok); }).catch((e) => {
    throw e.code === 'EADDRINUSE' ? new Error(`--storage ${storage}: its page's port ${kept.port} is in use: another drive of this store is open, and a store is one drive's at a time`) : e;
  });
  const port = server.address().port;
  const chrome = chromium().executable;
  const profile = kept ? (mkdirSync(kept.profile, { recursive: true }), kept.profile) : mkdtempSync(resolve(tmpdir(), 'exact-agent-'));
  let child;
  try {
    child = spawn(chrome, [
    '--headless=new', '--remote-debugging-pipe', `--window-size=${size[0]},${size[1]}`, '--hide-scrollbars',
    '--enable-unsafe-webgpu', '--disable-smooth-scrolling', `--user-data-dir=${profile}`, '--no-sandbox',
    '--disable-extensions', '--disable-background-networking', '--disable-component-update', '--no-first-run',
    '--no-default-browser-check',
    // The profile is a throwaway: a mock keychain and a plain password
    // store keep Chrome from asking the login keychain, which a shell
    // without keychain access refuses (errSecInteractionNotAllowed).
    '--use-mock-keychain', '--password-store=basic', 'about:blank',
  ], { detached: true, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'], windowsHide:true });
    // Bun exposes null stdio on a failed spawn; wait before constructing CDP.
    // A pid is a started child: Bun 1.4.2 can drop the 'spawn' event of the
    // first child with extra pipes a test process starts, so only a child
    // without one waits for the event.
    if (!child.pid) await new Promise((ok, fail) => { child.once('spawn', ok); child.once('error', fail); });
  } catch (error) {
    server.close();
    if (!kept) rmSync(profile, {recursive:true, force:true});
    throw new Error(`web carrier unavailable: ${chrome}: ${error.code}; set CHROME to an installed browser`);
  }
  onProcess?.(child);
  const hostLines = [], launchTail = [];
  let partial = ''; // a line split across stderr chunks
  const line = (l) => {
    if (!l) return;
    if (!browserDiagnosticNoise(l)) hostLines.push('chrome: ' + l);
    launchTail.push(l); if (launchTail.length > 8) launchTail.shift(); // unfiltered, for a launch that fails
  };
  child.stderr.on('data', (d) => { const parts = (partial + d).split('\n'); partial = parts.pop(); parts.forEach(line); });
  child.stderr.on('end', () => { line(partial); partial = ''; });
  const stdioClosed = new Promise((r) => child.on('close', r)); // after 'exit', once stderr is drained
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const exited = new Promise((r) => {
    child.on('exit', (code, signal) => { cdp.fail(`Chrome exited (${code ?? signal})`); r(); });
    child.on('error', error => { cdp.fail(`web carrier unavailable: ${chrome}: ${error.code}; set CHROME to an installed browser`); r(); });
  });
  const close = async () => {
    try {
      if (process.platform === 'win32') await closeWindowsBrowser(child, cdp, exited, profile);
      else {
        try { process.kill(-child.pid, 'SIGKILL'); } catch {}
        await waitAtMost(exited, 2000);
      }
    } finally { server.close(); }
    if (child.exitCode === null && child.signalCode === null)
      throw new Error(`Chrome ${child.pid} did not exit; owned profile retained at ${profile}`);
    if (!kept) await removeBrowserProfile(profile);
    if (planBuild) rmSync(planBuild, { recursive: true, force: true });
  };
  try {
    // A Chrome that dies at launch says why only on its stderr; the pipe just closes.
    const { targetInfos } = await cdp.send('Target.getTargets').catch(async (error) => {
      await waitAtMost(stdioClosed, 1000);
      const said = [...launchTail, ...(partial ? [partial] : [])].slice(-6).map(l => '  chrome: ' + l).join('\n'); // and an unterminated last line
      const alive = child.exitCode === null && child.signalCode === null;
      throw copyCdpFailureContext(error, new Error(`Chrome ${alive ? 'did not answer' : 'did not start'} (${error.message}); CHROME=${chrome}\n${said || '  (it printed nothing)'}${alive ? '' : '\nSet CHROME to a browser that runs here.'}`));
    });
    const target = targetInfos.find((t) => t.type === 'page') ?? (await cdp.send('Target.createTarget', { url: 'about:blank' }));
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    const heldKeys = new Map();
    const call = async (method, params, timeoutMs) => {
      const reply = await cdp.send(method, withHeldModifiers(method, params, heldKeys), sessionId, timeoutMs);
      if (method === 'Input.dispatchKeyEvent') {
        if (params.type === 'keyUp') heldKeys.delete(params.code);
        else if (params.type === 'keyDown' || params.type === 'rawKeyDown') heldKeys.set(params.code, params);
      }
      return reply;
    };
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
      // A wheel past the page's edge bounces (macOS elastic overscroll) on the
      // compositor's own clock, not the drive's: a screenshot after it caught
      // the page 1-10 px low in some runs. Off, on every target.
      addEventListener('DOMContentLoaded', () => { document.documentElement.style.overscrollBehavior = 'none'; });
    ` });
    // The viewport exactly: Chrome will not make a window narrower than 500.
    await call('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
    const evaluate = async (expression, timeoutMs) => {
      const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, timeoutMs);
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
    for (const [key, value] of Object.entries(facts)) page.searchParams.set(key, value);
    if (storage !== undefined) page.searchParams.set('storage', storage);
    // Display preferences are the agent's from launch, never the machine's (LLP 1069.007 D2); `prefer` changes them.
    const emulated = { ...LAUNCH_MEDIA };
    await call('Emulation.setEmulatedMedia', { features: Object.entries(emulated).map(([name, value]) => ({ name, value })) });
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
    if (plan && !js) {
      const carry = world ? `exact.worldCarry = Uint8Array.from(atob(${JSON.stringify(worldFile(world).toString('base64'))}), c => c.charCodeAt(0));` : '';
      // Another plan is a new document: its autofocus runs (LLP 1035.000 D9).
      await evaluate(`fetch('/__plan').then((r) => r.arrayBuffer()).then((b) => { ${carry} return exact.reload(new Uint8Array(b), true); })`);
    }
    const frame = () => waitAtMost(evaluate('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))'), 250);
    // The one contact this carrier may hold (LLP 1035.003 D1), and whether
    // Chrome's touch emulation is on — switched on by the first contact.
    let touch = false;
    let contact = null;
    const ask = async (req) => {
      // The existing resize input uses Chrome's real viewport and resize event.
      if (req.op === 'tap' && req.resize !== undefined) {
        const pair = req.resize;
        if (Object.keys(req).some(k => !['op', 'resize'].includes(k)) || !Array.isArray(pair) || pair.length !== 2
          || !pair.every(n => Number.isInteger(n) && n >= 64 && n <= 4096) || pair[0] * pair[1] > 8388608) return { error: 'tap resize needs exactly two integer dimensions in 64...4096, area <= 8388608, and no other input fields' };
        await call('Emulation.setDeviceMetricsOverride', { width: pair[0], height: pair[1], deviceScaleFactor: 1, mobile: false });
        await frame();
        return { resized: pair, viewport: await evaluate('[innerWidth, innerHeight]'), delivery: 'browser-viewport' };
      }
      // A settle runs up to the page's 20 s deadline twice (requests, then rounds).
      const timeout = req.op === 'clock' && (req.settle || req.data) ? 60000 : undefined;
      return JSON.parse(await evaluate(`exact.agentSettled(${JSON.stringify(req)}).then((r) => JSON.stringify(r))`, timeout));
    };
    return {
      host: 'web', browser: 'chrome', boot: Number(boot), hostLines, evaluate, call, launchFacts: facts,
      async gpuMs() {
        const ms = await evaluate("document.getElementById('exact-root')?.dataset.gpuMs ?? null");
        return ms == null ? null : Number(ms);
      },
      /** A fresh document on this page: its origin's storage emptied, or `keep`ing it (a test's `reload`, mail F19). */
      async reset({ keep = false } = {}) {
        // Release browser-owned input while its original document still exists.
        for (const key of heldKeys.values()) await call('Input.dispatchKeyEvent', {...key, type:'keyUp', text:undefined});
        if (contact?.mouse) await call('Input.dispatchMouseEvent', {type:'mouseReleased', x:contact.x, y:contact.y, button:'left', buttons:0, clickCount:1});
        else if (contact) await call('Input.dispatchTouchEvent', {type:'touchCancel', touchPoints:[]});
        await frame();
        contact = null;
        if (touch) await call('Emulation.setTouchEmulationEnabled', {enabled:false});
        touch = false;
        await evaluate('sessionStorage.clear()');
        if (!keep) await call('Storage.clearDataForOrigin', {origin:page.origin, storageTypes:'all'});
        await call('Page.navigate', {url:'about:blank'});
        const deadline = Date.now() + 30000;
        while (!await evaluate("location.href === 'about:blank'").catch(() => false)) {
          if (Date.now() > deadline) throw new Error('the reused page never left its old document');
          await sleep(15);
        }
        hostLines.length = 0;
        await call('Page.navigate', {url:page.href});
        let boot;
        while ((boot = await evaluate("document.getElementById('exact-root')?.dataset.bootMs ?? null").catch(() => null)) == null) {
          if (Date.now() > deadline) throw new Error('the reused page never booted');
          await sleep(15);
        }
        await evaluate('exact.ready');
        await call('Page.resetNavigationHistory');
        this.boot = Number(boot);
      },
      ask,
      async reveal(id) { const r = await ask({ op: 'reveal', id }); if (r.scrolled) await frame(); return r; },
      prefer: (media, page, fold) => preferWeb({ media, page, fold, emulated, call, evaluate, frame, ask }), // the browser's emulation through CDP, the glue's substitute where it offers none (agent-prefer.mjs)
      async input(id, kind, opts) {
        if (kind === 'key') cdpKey(opts.key); // Refuse before focus or browser input.
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
        const point = ['contextmenu', 'mouse'].includes(kind) ? opts.at : null;
        // The middle of the box, unless the target is not there: then the middle of the first of its client rects that is —
        // a wrapped inline run's line, where its box's middle falls between two lines, in the paragraph's half-leading (reader).
        const line = r && !point && ['press', 'contextmenu', 'dblclick', 'hover'].includes(kind) ? await evaluate(`(() => { const el = exact.views.get(${id}), on = (x, y) => { const h = document.elementFromPoint(x, y); return !!h && (el === h || el.contains(h)); }, b = el?.getBoundingClientRect(); if (!el || on(b.x + b.width / 2, b.y + b.height / 2)) return null; for (const c of el.getClientRects()) if (c.width && c.height && on(c.x + c.width / 2, c.y + c.height / 2)) return [c.x + c.width / 2, c.y + c.height / 2]; return null; })()`) : null;
        const x = line ? line[0] : r ? r.x + (point?.[0] ?? r.w / 2) : contact?.x, y = line ? line[1] : r ? r.y + (point?.[1] ?? r.h / 2) : contact?.y;
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
        if (id != null && ['press', 'contextmenu', 'dblclick', 'mouse'].includes(kind)) {
          // An ancestor under the point takes the press itself, never the target's handlers: a miss, unless the target passes its
          // presses through (`pointer-events: none`), as a person's would be (reader: a tap between a run's lines reported success).
          const why = await evaluate(`(() => { const el = exact.views.get(${id}), hit = document.elementFromPoint(${x}, ${y}), name = hit && (hit.dataset?.view ? 'node #' + hit.dataset.view : hit.tagName.toLowerCase()); return !el ? null : !hit ? 'its middle is outside the viewport; scroll it into view first' : el === hit || el.contains(hit) || hit.contains(el) && getComputedStyle(el).pointerEvents === 'none' ? null : hit.contains(el) ? name + ', which holds it, is under the point and would take the press: no box the target draws is there' : name + ' covers its middle'; })()`);
          if (why) throw new Error(`tap #${id} at (${x}, ${y}): ${why}`);
        }
        if (kind === 'mouse' && !await evaluate(`exact.views.get(${id})?.localName === 'canvas' || !!exact.views.get(${id})?.querySelector('canvas')`)) throw new Error('mouse requires a canvas target');
        if (kind === 'down' || kind === 'move' || kind === 'hold' || kind === 'up' || kind === 'cancel') {
          // A held contact (LLP 1035.003 D1) is a finger here: CDP touch
          // events under touch emulation, enabled the first time a contact
          // is used. Chrome recognizes, scrolls and flings from them exactly
          // as it would from a hand; a timed move is delivered as steps on
          // real time so its velocity is real too. Each event carries the
          // contact's own timestamp (LLP 1057 §10.6): a lift follows the last
          // move by one frame, as a finger's does, however long the driver
          // takes between ops; `tap hold <ms>` is how a pause is said.
          // `mouse` (a drag's, files diary F10): the left button instead, with
          // touch emulation off, so the page's pointer is `fine` and a
          // desktop path is what runs — and nothing scrolls by the contact.
          const mouse = kind === 'down' ? !!opts.mouse : !!contact?.mouse;
          if (mouse && touch) { await call('Emulation.setTouchEmulationEnabled', { enabled: false }); touch = false; await frame(); }
          if (!mouse && !touch) { await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 2 }); touch = true; }
          const button = (type, at, t, buttons) => call('Input.dispatchMouseEvent', { type, x: at.x, y: at.y, button: 'left', buttons, clickCount: type === 'mouseMoved' ? 0 : 1, timestamp: t });
          if (kind === 'down') {
            if (contact) throw new Error('a contact is already down; use `tap up` first');
            const px = opts.x ?? x, py = opts.y ?? y;
            // CDP wants epoch seconds; performance.timeOrigin plus the
            // runner's clock makes the DOM event's timeStamp that clock,
            // independent of how long the carrier took between operations.
            const t = (await evaluate('performance.timeOrigin') + (await ask({ op: 'tags' })).clock) / 1000;
            if (mouse) { await button('mouseMoved', { x: px, y: py }, t, 0); await button('mousePressed', { x: px, y: py }, t, 1); }
            else await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: px, y: py }], timestamp: t });
            contact = { x: px, y: py, t, mouse };
            await frame();
            return { contact: id, phase: 'down', at: [px, py], delivery: 'platform', ...(mouse ? { pointer: 'mouse' } : {}) };
          }
          if (!contact) throw new Error('no contact is down');
          if (kind === 'move') {
            const to = { x: opts.x ?? contact.x + (opts.dx ?? 0), y: opts.y ?? contact.y + (opts.dy ?? 0) };
            const ms = Math.max(0, opts.ms ?? 0);
            const steps = Math.max(1, Math.round(ms / 16));
            for (let i = 1; i <= steps; i++) {
              const t = i / steps;
              contact.t += (ms || 16) / steps / 1000;
              const at = { x: contact.x + (to.x - contact.x) * t, y: contact.y + (to.y - contact.y) * t };
              if (mouse) await button('mouseMoved', at, contact.t, 1);
              else await call('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [at], timestamp: contact.t });
              if (ms) await sleep(ms / steps);
            }
            contact = { ...to, t: contact.t, mouse };
            await frame();
            return { phase: 'move', at: [to.x, to.y], delivery: 'platform' };
          }
          if (kind === 'hold') { if (opts.ms) { await sleep(opts.ms); contact.t += opts.ms / 1000; } return { phase: 'hold', at: [contact.x, contact.y], delivery: 'platform' }; }
          // A mouse has no cancel: its button comes up where it is.
          if (mouse) await button('mouseReleased', contact, contact.t + 0.008, 0);
          else await call('Input.dispatchTouchEvent', { type: kind === 'up' ? 'touchEnd' : 'touchCancel', touchPoints: [], timestamp: contact.t + 0.008 });
          const at = [contact.x, contact.y];
          contact = null;
          await frame();
          return { phase: kind, at, delivery: 'platform' };
        }
        if (kind === 'pinch') { // two fingers spread from d to d·scale about the middle (LLP 1057.001 §5)
          if (!touch) { await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 2 }); touch = true; }
          const [cx, cy] = opts.at ? [r.x + opts.at[0], r.y + opts.at[1]] : [x, y], d = Math.max(8, Math.min(r.w, r.h) * 0.3), fingers = (k) => [0, 1].map((i) => ({ x: cx + (i ? 1 : -1) * d * k / 2, y: cy, id: i }));
          await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: fingers(1) });
          for (let i = 1; i <= 8; i++) { await call('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: fingers(1 + (opts.pinch - 1) * i / 8) }); await sleep(16); }
          await call('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
          await frame();
          return { pinch: opts.pinch, at: [cx, cy], delivery: 'platform' };
        }
        // CDP's modifier bits: Alt 1, Control 2, Meta 4, Shift 8.
        if (kind === 'wheel') await call('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: opts.wheel[0], deltaY: opts.wheel[1], modifiers: String(opts.modifiers ?? '').split('+').reduce((m, k) => m | ({ Alt: 1, Control: 2, Meta: 4, Shift: 8 }[k] ?? 0), 0) });
        else if (kind === 'drop') { // the files' bytes as a drag's DataTransfer, its dragover then its drop at the point, as a browser delivers a drag from the desktop
          const files = opts.drop.map(p => ({ name: basename(p), bytes: readFileSync(p).toString('base64') }));
          const heard = await evaluate(`(() => { const el = exact.views.get(${id}), dt = new DataTransfer(); for (const f of ${JSON.stringify(files)}) dt.items.add(new File([Uint8Array.from(atob(f.bytes), c => c.charCodeAt(0))], f.name)); const at = { clientX: ${x}, clientY: ${y}, bubbles: true, cancelable: true, dataTransfer: dt }; const over = new DragEvent('dragover', at); el.dispatchEvent(over); const drop = new DragEvent('drop', at); el.dispatchEvent(drop); return { accepted: over.defaultPrevented, taken: drop.defaultPrevented }; })()`);
          if (!heard.taken) throw new Error(`view ${id} took no drop (dragover ${heard.accepted ? 'accepted' : 'refused'})`);
        }
        else if (kind === 'hover') await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
        else if (kind === 'contextmenu' || kind === 'dblclick' || kind === 'mouse') {
          await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
          for (let clickCount = 1; clickCount <= (kind === 'dblclick' ? 2 : 1); clickCount++) {
            const button = kind === 'contextmenu' ? 'right' : 'left';
            // `buttons` is what PointerEvent.buttons reports; CDP leaves it 0 unless told.
            await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button, buttons: button === 'right' ? 2 : 1, clickCount });
            await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button, buttons: 0, clickCount });
          }
        }
        else if (kind === 'clipboard') {
          // What ⌘C, ⌘X or ⌘V delivers at the focus, a paste carrying the
          // text as the clipboard's (whose own contents are left alone);
          // a field takes a paste's text as its default, as the browser's paste would.
          const f = await ask({ op: 'focus', id, select: false });
          if (f.error) throw new Error(f.error);
          const heard = await evaluate(`(() => { const el = document.activeElement?.closest?.('[data-view]') ? document.activeElement : exact.views.get(${id}), dt = new DataTransfer(); if (${JSON.stringify(opts.clipboard)} === 'paste') dt.setData('text/plain', ${JSON.stringify(opts.text ?? '')}); const ev = new ClipboardEvent(${JSON.stringify(opts.clipboard)}, { clipboardData: dt, bubbles: true, cancelable: true }); el.dispatchEvent(ev); return { editable: el.matches('input, textarea, [contenteditable]'), prevented: ev.defaultPrevented }; })()`);
          if (opts.clipboard === 'paste' && heard.editable && !heard.prevented) await call('Input.insertText', { text: opts.text ?? '' });
        }
        else if (kind === 'key') {
          const f = await ask({ op: 'focus', id, select: false });
          if (f.error) throw new Error(f.error);
          // A chord's modifiers ride on the key; Enter types a newline in a textarea.
          const { key, code, vk, modifiers, text, location } = cdpKey(opts.key);
          await call('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode: vk, modifiers, location, ...(text ? { text } : {}) });
          await call('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: vk, modifiers, location });
        }
        else if (kind === 'press' && await evaluate(`(() => { const hit = document.elementFromPoint(${x}, ${y}), host = hit?.closest('[data-gpu-input]'), el = ${id == null ? 'null' : `exact.views.get(${id})`}; return !!host && (hit === host || hit.localName === 'canvas') && (!el || el === host || el.contains(host) || host.contains(el)); })()`)) {
          // A tap on a world's canvas is a finger, as a held contact is here and
          // every tap is on iOS and Linux, so a proof leaves one world everywhere.
          if (!touch) { await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 2 }); touch = true; }
          await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
          await call('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
        }
        else if (kind === 'press') {
          // CDP's bits for the modifiers held: Alt 1, Control 2, Meta 4, Shift 8.
          const modifiers = String(opts.modifiers ?? '').split('+').reduce((m, k) => m | ({ Alt: 1, Control: 2, Meta: 4, Shift: 8 }[k] ?? 0), 0);
          // The page can move between the aim above and the press (a reply lands on real time while the
          // agent's clock stands still): record what the press reached and refuse a tap that missed.
          // Judged at the press itself: the tap's own action may replace the node right after.
          // A fresh listener each tap, removed in `finally`, so a failed dispatch leaves none behind.
          const arm = `(() => { if (window.__exactPress) removeEventListener('pointerdown', window.__exactPress, true); window.__exactPressed = null; window.__exactPress = (e) => { if (window.__exactPressed) return; const el = exact.views.get(${id}), hit = e.target; window.__exactPressed = !el ? 'nothing: the target was gone at the press' : el === hit || el.contains(hit) || hit.contains(el) && getComputedStyle(el).pointerEvents === 'none' ? 'ok' : hit.closest?.('[data-testid]') ? '\"' + hit.closest('[data-testid]').dataset.testid + '\"' : hit.closest?.('[data-view]') ? 'node #' + hit.closest('[data-view]').dataset.view : hit.localName; }; addEventListener('pointerdown', window.__exactPress, true); })()`;
          const disarm = `(() => { removeEventListener('pointerdown', window.__exactPress, true); window.__exactPress = null; const r = window.__exactPressed; window.__exactPressed = null; return r; })()`;
          if (id != null) await evaluate(arm);
          let reached = 'ok';
          try {
            await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, modifiers });
            await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1, modifiers });
            await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1, modifiers });
          } finally { if (id != null) reached = await evaluate(disarm).catch(() => 'ok'); }
          // null: armed but never pressed — the press reached no element of this page (an iframe moved over
          // the target, say); undefined: the press loaded a new document, which is the tap's own doing.
          const missed = reached === 'ok' || reached === undefined ? null : reached ?? 'no element of this page';
          if (missed) throw new Error(`tap #${id} at (${x}, ${y}) landed on ${missed}: the page moved between aiming and the press; \`clock settle\` first, then tap again`);
        } else if (kind === 'type') {
          const f = await ask({ op: 'focus', id });
          if (f.error) throw new Error(f.error);
          await call('Input.insertText', { text: opts.text });
        }
        await frame();
        return { at: [x, y] };
      },
      async screenshot(path) {
        const tags = await ask({ op: 'tags' });
        if (tags.error) throw new Error(tags.error);
        await frame();
        // Images in the viewport finish loading first, up to 3 s: a picture
        // of grey placeholders is not the page (LLP 1054.000 R9). `clock
        // settle` stays the app's clock alone; this wait is the picture's.
        const pending = await waitAtMost(evaluate(`(async () => {
          const on = (i) => { const b = i.getBoundingClientRect(); return b.bottom > 0 && b.right > 0 && b.top < innerHeight && b.left < innerWidth; };
          const left = () => [...document.images].filter((i) => !i.complete && on(i));
          const end = performance.now() + 3000;
          while (left().length && performance.now() < end) await new Promise((r) => setTimeout(r, 25)); await globalThis.exact.imageFrames?.();
          return left().length;
        })()`), 3500);
        await frame();
        const { data } = await call('Page.captureScreenshot', { format: 'png' });
        writeFileSync(path, Buffer.from(data, 'base64'));
        return { ...tags, screenshot: path, w: size[0], h: size[1], ...(pending ? { imagesPending: pending } : {}) };
      },
      close,
    };
  } catch (e) {
    try { await close(); } catch (failure) { retainCleanupError(e, failure); }
    throw e;
  }
}

// ---------------------------------------------------------------- macOS and Linux, over stdio

/** JSON lines over a duplex: each request is answered by the next line the app writes; unmatched lines are host output (`hostLines`). `fail` rejects every pending request (the app is gone). */
export function jsonLines(readable, writable, hostLines) {
  const waiting = [];
  let failure = null;
  // A large reply (a `state` carrying a 16 MiB surface record) arrives in
  // many chunks: hold them and look for the newline in each new chunk only.
  let parts = [];
  readable.setEncoding('utf8');
  readable.on('data', (d) => {
    let i;
    while ((i = d.indexOf('\n')) >= 0) {
      parts.push(d.slice(0, i));
      const line = parts.join('');
      parts = [];
      d = d.slice(i + 1);
      const w = waiting.shift();
      if (!w) { hostLines.push('app: ' + line); continue; }
      try { w.resolve(JSON.parse(line)); } catch { w.reject(new Error('unreadable reply: ' + line)); }
    }
    if (d) parts.push(d);
  });
  const next = () => failure ? Promise.reject(failure) : new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  return {
    next,
    ask: (req) => { const p = next(); if (!failure) writable.write(JSON.stringify(req) + '\n'); return p; },
    fail: (why) => { failure ??= new Error(why); for (const w of waiting.splice(0)) w.reject(failure); },
  };
}

/** Why a native app stopped answering, for the error that ends the run: its pid alive or gone, how it exited, any crash report since launch, and the last 20 lines it wrote — not all of them since the last `logs`. */
export function hangup({ what, pid = null, exit = null, reports = [], hostLines = [] }) {
  const alive = pid == null ? null : (() => { try { process.kill(pid, 0); return true; } catch { return false; } })();
  const status = exit ? (exit.signal ? `killed by ${exit.signal}` : `exit code ${exit.code}`) : alive == null ? 'no pid known' : `pid ${pid} ${alive ? 'still running' : 'gone'}`;
  return [`${what} (${status})`, ...reports.map((f) => `crash report: ${f}`), ...hostLines.slice(-20)].join('\n');
}
/** A native reply's deadline: past `clock settle`'s own 20 s bound, so only a wedged app reaches it (a phone has 45 s). */
const REPLY_MS = 120000;

/** A refused boot; a format refusal usually means a host binary built before the plan's format changed. */
const bootRefusal = (error) => 'the app booted with an error: ' + error +
  (/UnsupportedVersion|FormatDigestMismatch|KernelSchemaMismatch/.test(error) ? ' (the plan and the host binary were built from different formats: rebuild the host)' : '');

/** One JSON-lines protocol over stdio on desktop hosts, or a phone's outbound socket. */
async function openStdio({ host, plan, world, size, app, env: extra = {}, session, documents = [], device = false, phone: pick, onProcess }) {
  const a = resolveApp(app);
  const linux = host === 'linux';
  const windows = host === 'windows', portable = linux || windows;
  const sample = host === 'host';
  const artifacts = portable ? null : appleArtifacts(a, { destination: device ? 'ios' : 'macos', host: sample });
  const deviceBundle = artifacts?.bundle;
  const bin = windows ? (process.env.EXACT_WINDOWS_BIN ?? resolve(a.dir, `dist-windows/${a.crate('windows')}.exe`))
    : linux ? (process.env.EXACT_LINUX_BIN ?? linuxBinary(a)) : (process.env.EXACT_MAC_BIN ?? artifacts.binary);
  if (!existsSync(device ? deviceBundle : bin)) throw new Error(device ? 'run bun host/apple/build.mjs --device first' : windows ? 'run bun host/windows/build.mjs first' : linux ? `run ${linuxBuild(a).join(' ')} first` : sample ? 'run bun host/apple/build.mjs --host first' : `run ${ownAppleBuild(a, 'mac') ?? `bun host/apple/build.mjs ${a.crate('apple')}`} first`);
  if (!portable) assertAppleIdentity(a, device ? resolve(deviceBundle, 'ExactIOS') : bin);
  if (windows && process.env.EXACT_WINDOWS_BIN) unchecked('windows', 'EXACT_WINDOWS_BIN');
  else if (windows) {
    const receipt = resolve(bakeOutput(a), `windows-${bakeTarget('windows')}.build.json`);
    refuseStale('windows', bin, packagedBuildChanges(receipt, dirname(bin)), `bun host/windows/build.mjs ${a.crate('windows')}`);
  }
  else if (linux && process.env.EXACT_LINUX_BIN) unchecked('linux', 'EXACT_LINUX_BIN');
  else if (linux) refuseStale('linux', bin, depInfoChanges(bin), linuxBuild(a).join(' '));
  else if (!device && process.env.EXACT_MAC_BIN) unchecked(host, 'EXACT_MAC_BIN');
  else if (!device) {
    const receipt = [resolve(bin, '..', 'receipt.json'), resolve(deviceBundle, 'Contents/Resources/receipt.json')].find(existsSync);
    if (receipt) refuseStale(sample ? 'sample host' : 'macos', receipt, receiptChanges(receipt, a), (!sample && ownAppleBuild(a, 'mac')) || `bun host/apple/build.mjs ${a.crate('apple')}${sample ? ' --host' : ''}`);
  }
  if (device && (plan || extra.EXACT_PLAN || extra.EXACT_ASSETS)) throw new Error('a phone cannot read host-local plan/assets paths; use --url or its embedded app');
  const ph = device ? phone(pick) : null;
  if (device) {
    const installed = spawnSync('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, deviceBundle], { encoding: 'utf8' });
    if (installed.status !== 0) throw new Error(`device install: ${installed.stderr || installed.stdout || installed.error?.message}`);
    if (world) {
      const copied = spawnSync('xcrun', ['devicectl', 'device', 'copy', 'to', '--quiet', '--device', ph.udid,
        '--domain-type', 'appDataContainer', '--domain-identifier', a.id, '--source', resolve(world), '--destination', 'tmp/exact-agent.world'], { encoding: 'utf8', timeout: 45000 });
      if (copied.status !== 0) throw new Error(`phone world copy: ${copied.stderr || copied.error || copied.stdout}`);
      extra = { ...extra, EXACT_WORLD: '~/tmp/exact-agent.world' };
    }
  }
  const env = { EXACT_ASSETS: windows ? dirname(bin) : linux ? a.dir : artifacts.capture, ...process.env, EXACT_AGENT: '1' };
  if (plan) env.EXACT_PLAN = plan;
  if (portable && size) env.EXACT_SIZE = `${size[0]}x${size[1]}`;
  // @ref LLP 1039 §5 — measure the requested Mac content viewport.
  if (!portable && size) { env.EXACT_WINDOW_WIDTH = String(size[0]); env.EXACT_WINDOW_HEIGHT = String(size[1]); }
  if (portable) {
    // The pinned font: DejaVu Sans from scripts/fixtures/fonts shapes and
    // paints the host's text on every machine, so a pixel fixture recorded
    // here matches on a builder (LLP 1015 §5). The environment still wins.
    env.EXACT_PAINTER ??= 'cpu';
    if (windows) env.EXACT_GPU_RENDER ??= '1';
    env.EXACT_FONTS ??= resolve(ROOT, 'scripts/fixtures/fonts/assets');
    env.EXACT_FONT ??= 'DejaVu Sans';
  }
  Object.assign(env, extra);
  const bridge = device ? await phoneBridge() : null;
  const launched = Date.now(); let closing = false;
  const child = device
    ? spawn('xcrun', ['devicectl', 'device', 'process', 'launch', '--quiet', '--console', '--terminate-existing', '--device', ph.udid,
        '--environment-variables', JSON.stringify({ ...(size ? { EXACT_WINDOW_WIDTH: env.EXACT_WINDOW_WIDTH, EXACT_WINDOW_HEIGHT: env.EXACT_WINDOW_HEIGHT } : {}), ...extra, EXACT_AGENT: '1', ...bridge.env }), a.id], { stdio: ['pipe', 'pipe', 'pipe'] })
    : spawn(bin, portable && env.EXACT_LAUNCH_URL ? [env.EXACT_LAUNCH_URL] : documents, { env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide:true });
  onProcess?.(child);
  const hostLines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  if (device) child.stdout.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  let lines = device ? null : jsonLines(child.stdout, child.stdin, hostLines);
  const fail = (why) => { lines?.fail(why); bridge?.fail(new Error(why)); };
  child.on('error', (e) => fail(`launch failed: ${e.message}`));
  const exited = new Promise((r) => child.on('exit', (code, signal) => { r(code ?? signal); fail(closing ? 'the app was closed' : hangup({ what: 'the app exited', exit: { code, signal }, hostLines, reports: device ? [] : crashReports(basename(bin), launched) })); }));
  const close = async () => { closing = true; bridge?.close(); try { child.stdin.end(); if (device) child.kill('SIGTERM'); } catch {} await waitAtMost(exited, 2000); if (child.exitCode === null && child.signalCode === null) { try { child.kill('SIGKILL'); } catch {} } await exited; };
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
    if (ready.error) throw new Error(bootRefusal(ready.error));
    // The sample host routes by label: the session the caller named, and
    // `s.session = "b"` moves every later request to another.
    const state = { session: session ?? null };
    const heldButton = mouseContact();
    const ask = (req, session = state.session) => waitAtMost(lines.ask(session ? { ...req, session } : req), device ? 45000 : REPLY_MS, () => {
      const why = device ? `phone ${req.op} did not answer within 45 s; check app health, foreground state and network\n` + hostLines.slice(-20).join('\n')
        : hangup({ what: `${req.op} did not answer within ${REPLY_MS / 1000} s`, pid: child.pid, hostLines });
      if (device) bridge.close(); else lines.fail(why);
      throw new Error(why);
    });
    return {
      host, boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      ask, reveal: (id) => ask({ op: 'reveal', id }),
      async input(id, kind, opts) {
        if (kind === 'key') {
          const session = state.session;
          return nativeKey({id, opts: portable ? {...opts, ownedRelease:false} : opts, ask: req => ask(req, session)});
        }
        const guest = { selector: opts.selector, x: opts.x, y: opts.y, entity: opts.entity, world: opts.world, under: opts.under, phase: opts.phase };
        const phase = ['down', 'move', 'hold', 'up', 'cancel'].includes(kind);
        // A contact that went down with `mouse` (a drag's) holds the left button through its phases (review A1).
        const r = phase ? await heldButton.ask(kind, opts, (button) => ask({ op: 'tap', phase: kind, ...(id != null ? { id } : {}), x: opts.x, y: opts.y, dx: opts.dx, dy: opts.dy, ms: opts.ms, ...button })) : ['contextmenu', 'dblclick', 'mouse'].includes(kind) ? await ask({ op: 'tap', id, [kind]: true, ...(['contextmenu', 'mouse'].includes(kind) && opts.at !== undefined ? {at: opts.at} : {}) }) : kind === 'pinch' ? await ask({ op: 'tap', id, pinch: opts.pinch, at: opts.at }) : kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel, ...(opts.gesture ? { gesture: true } : {}), ...(opts.modifiers ? { modifiers: opts.modifiers } : {}) }) : kind === 'drop' ? await ask({ op: 'tap', id, drop: opts.drop }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest, ...(opts.modifiers ? { modifiers: opts.modifiers } : {}) }) : await ask({ op: 'type', id, text: opts.text, ...(opts.clipboard ? { clipboard: opts.clipboard } : {}), ...guest });
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
async function openIOS({ plan, app, size, env: extra = {}, session, hostFixture = false, touch = 'agent', onProcess }) {
  const a = resolveApp(app);
  const bundle = appleArtifacts(a, { destination: 'ios-simulator', host: hostFixture }).bundle;
  const id = hostFixture ? `${a.id}.host` : a.id;
  if (!existsSync(bundle)) throw new Error(hostFixture ? 'run bun host/apple/build.mjs --ios --host first' : `run ${ownAppleBuild(a, 'ios') ?? `bun host/apple/build.mjs --ios ${a.crate('apple')}`} first`);
  refuseStale('ios', resolve(bundle, 'receipt.json'), receiptChanges(resolve(bundle, 'receipt.json'), a), (!hostFixture && ownAppleBuild(a, 'ios')) || `bun host/apple/build.mjs --ios ${a.crate('apple')}${hostFixture ? ' --host' : ''}`);
  const dev = simulator();
  showSimulator(dev, true); // a person watching sees what is driven, and keeps the focus
  // Real touches (LLP 1080.000, `--touch platform`): the runner starts first, so its own launch never backgrounds the app.
  const touches = touch === 'platform' ? await openTouches({ udid: dev.udid, appId: id ?? bundleId(a.crate('apple')), appPath: bundle, onProcess }) : null;
  try { install(dev, bundle, a, hostFixture); } catch (e) { await touches?.close(); throw e; }
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ios-'));
  const sock = resolve(dir, 'agent.sock');
  const env = { EXACT_ASSETS: appleArtifacts(a,{destination:'ios-simulator',host:hostFixture}).capture, EXACT_AGENT: '1', EXACT_AGENT_SOCKET: sock, ...(plan ? { EXACT_PLAN: plan } : {}), ...(size ? {EXACT_WINDOW_WIDTH:String(size[0]), EXACT_WINDOW_HEIGHT:String(size[1])} : {}), ...extra };
  const childEnv = { ...process.env };
  for (const [k, v] of Object.entries(env)) childEnv[`SIMCTL_CHILD_${k}`] = v;
  const launched = Date.now(); let closing = false;
  const console_ = spawn('xcrun', ['simctl', 'launch', '--console', '--terminate-running-process', dev.udid, id ?? bundleId(a.crate('apple'))], { env: childEnv, stdio: ['ignore', 'pipe', 'pipe'] });
  onProcess?.(console_);
  const hostLines = [];
  for (const stream of [console_.stdout, console_.stderr]) stream.on('data', (d) => { for (const l of String(d).split('\n')) if (l && !/^com\.exact\.\w+: \d+$/.test(l)) hostLines.push('app: ' + l); });
  let consoleDone = false;
  const consoleExited = new Promise((r) => console_.on('exit', (code, signal) => { consoleDone = true; r({ code, signal }); }));
  let pid = null;
  // The app binds the socket once its first frame is applied: connect when it appears.
  let socket = null;
  const t = Date.now();
  while (!socket) {
    if (consoleDone) { rmSync(dir, { recursive: true, force: true }); await touches?.close(); throw new Error('the simulator launch exited before opening its agent socket; ' + hostLines.join('\n')); }
    if (Date.now() - t > 20000) {
      try { console_.kill('SIGKILL'); } catch {}
      await consoleExited;
      rmSync(dir, { recursive: true, force: true });
      await touches?.close();
      throw new Error('the app never opened its agent socket; ' + hostLines.join('\n'));
    }
    socket = await new Promise((ok) => { const s = connect(sock); s.once('connect', () => ok(s)); s.once('error', () => { s.destroy(); ok(null); }); });
    if (!socket) await sleep(50);
  }
  socket.on('error', () => {});
  const lines = jsonLines(socket, socket, hostLines);
  const exited = new Promise((r) => socket.on('close', async () => {
    const exit = closing ? null : await waitAtMost(consoleExited, 1000);
    lines.fail(closing ? 'the app was closed' : hangup({ what: 'the app hung up', pid, exit, hostLines, reports: crashReports(hostFixture ? 'ExactHostIOS' : 'ExactIOS', launched) })); r();
  }));
  const close = async () => {
    closing = true;
    try { socket.end(); } catch {}
    await waitAtMost(exited, 2000);
    let touchFailure = null;
    try { await touches?.close(); } catch (e) { touchFailure = e; }
    if (pid) { try { process.kill(pid, 'SIGKILL'); } catch {} }
    await waitAtMost(consoleExited, 1000);
    if (!consoleDone) {
      try { console_.kill('SIGKILL'); } catch {}
      await consoleExited;
    }
    rmSync(dir, { recursive: true, force: true });
    if (touchFailure) throw touchFailure;
  };
  try {
    const ready = await waitAtMost(lines.next(), 20000, () => { throw new Error('the app never became ready; ' + hostLines.join('\n')); });
    if (!ready.ready) throw new Error('unexpected first line: ' + JSON.stringify(ready));
    if (ready.error) throw new Error(bootRefusal(ready.error));
    pid = ready.pid ?? null;
    // The sample host routes by label, as the macOS one does over stdio.
    const state = { session: session ?? null };
    const ask = (req, session = state.session) => waitAtMost(lines.ask(session ? { ...req, session } : req), REPLY_MS, () => {
      const why = hangup({ what: `${req.op} did not answer within ${REPLY_MS / 1000} s`, pid, hostLines }); lines.fail(why); throw new Error(why);
    });
    let canvasContact = false;
    return {
      host: hostFixture ? 'host-ios' : 'ios', boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state,
      touches,
      ask, reveal: (id) => ask({ op: 'reveal', id }),
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
          // A held contact needs a touch that stays down across requests;
          // XCTest's public touches each press and lift (LLP 1080.000 D3).
          return { phase: kind, delivery: 'unsupported', reason: 'no held contact across requests on iOS (LLP 1080.000 P3)' };
        }
        // Under `--touch platform` a press is a real touch or nothing: a guest
        // (iframe) or world press would be dispatched by the host and must
        // never be reported as platform input (LLP 1080.000 D8).
        // One whole real gesture (LLP 1080.000 §11): only the runner makes one.
        if (kind === 'drag') return realTap({ ask, touches, id, at: opts.at, drag: opts.drag, abandon: (why) => lines.fail(why) });
        if (touches && kind === 'press') {
          if (Object.values(guest).some((v) => v != null)) throw new Error('unsupported under --touch platform: a press into an iframe guest or a world entity reaches no real touch yet (LLP 1080.000 stage 1)');
          return realTap({ ask, touches, id, abandon: (why) => lines.fail(why) });
        }
        const r = kind === 'contextmenu' || kind === 'dblclick' ? await ask({ op: 'tap', id, [kind]: true }) : kind === 'pinch' ? await ask({ op: 'tap', id, pinch: opts.pinch, at: opts.at }) : kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel, ...(opts.gesture ? { gesture: true } : {}), ...(opts.modifiers ? { modifiers: opts.modifiers } : {}) }) : kind === 'drop' ? await ask({ op: 'tap', id, drop: opts.drop }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest, ...(opts.modifiers ? { modifiers: opts.modifiers } : {}) }) : await ask({ op: 'type', id, text: opts.text, ...(opts.clipboard ? { clipboard: opts.clipboard } : {}), ...guest });
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
    try { await close(); } catch (failure) { e.message += `; ${failure.message}`; }
    throw e;
  }
}
const CLOCK_STEP_MS = 1000, CLOCK_BUDGET_MS = 3000, CLOCK_SPAN_MS = 600_000, REAL_STEP_MS = 50;
/** The next step of a split clock: aimed at CLOCK_BUDGET_MS of wall clock from the last step's cost, growing at most 4x and never past CLOCK_SPAN_MS of world time. */
export const clockSpan = (span, elapsedMs) => Math.max(CLOCK_STEP_MS, Math.min(span * 4, CLOCK_SPAN_MS, span * CLOCK_BUDGET_MS / Math.max(1, elapsedMs)));
// ---------------------------------------------------------------- the eight operations
/** A convenience over state, screenshot and type; wire replies keep all tags. */
export function worldView(session, name) {
  return {
    /** The first page of entities (512), or every page with {all:true}, read at one tick and hash. */
    async snapshot({all = false} = {}) {
      const page = {limit:5000};
      const first = all ? await session.state(`${name}:*`, undefined, false, false, page) : await session.state(`${name}:*`);
      const {tick, hash} = first, entities = [...first.entities];
      for (let r = first; all && r.truncated;) {
        r = await session.state(`${name}:*`, undefined, false, false, {...page, from:r.next});
        if (r.tick !== tick || r.hash !== hash) throw new Error('world changed while paging; capture on the agent clock with no concurrent drive');
        entities.push(...r.entities);
      }
      return {tick, hash, entities, truncated: all ? false : first.truncated};
    },
    /** Every resource's value, as `state world:* resources` reads them. */
    async resources() { return (await session.state(`${name}:*`, undefined, false, false, {limit:1, resources:true})).resources; },
    state: entity => session.state(`${name}:${entity}`),
    save: path => session.screenshot(path, name, 'save'),
    run: ms => {
      if (!Number.isFinite(ms) || ms < 0) throw new Error('run duration must be finite and nonnegative');
      // Like Sim::run, establish the current epoch after deferred assets settle
      // before moving time. Otherwise a newly ready world can eat the first seek.
      return session.clock('+0').then(() => session.clock(`+${ms}`));
    },
    settle: async () => (await session.clock('settle')).settled === true,
    tap: code => session.type(name, {key:code}),
    key_down: code => session.type(name, {key:code, phase:'down'}),
    key_up: code => session.type(name, {key:code, phase:'up'}),
    async local_position(entity) { return (await this.get(entity, 'Transform'))?.position; },
    async global_position(entity) {
      try { return (await session.layout(`${name}:${entity}`)).entity?.world?.position; }
      catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    async get(entity, component) {
      try {
        return (await session.state(`${name}:${entity}`)).entity?.components?.[component];
      } catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    hold: (code, ms) => session.type(name, {key:code, for:ms}),
  };
}
/** Explain a refused placed-child tap using the world's own visibility. */
export async function tapRefusal(session, target, error) {
  if (error.transport) return error; // the carrier failed: no diagnostic read can answer
  try {
    for (const canvas of (await session.tree()).nodes.filter(n => n.world)) {
      const canvasName = canvas.props?.testId ?? canvas.id;
      const outline = await session.tree(canvasName);
      if (!outline.entities?.some(entity => entity.name === target)) continue;
      const owner = `${canvasName}:${target}`;
      const state = await session.state(owner).catch(() => null);
      if (!state?.entity?.placed?.hidden) continue;
      const box = await session.layout(owner);
      const reason = box.entity?.visible?.behindCamera ? 'hidden (behind the camera)' : 'hidden';
      error.message = `${target} is ${reason}: \`layout ${owner}\` (with --json before the quoted operation) shows visibility and any available screen box; layout ${target} shows the child when mounted`;
      return error;
    }
  } catch { /* Preserve the original refusal if the diagnostic target also vanished. */ }
  return error;
}
/** Open a session on `host` ('web' | 'macos' | 'ios' | 'linux'); `url` opens
 * the same app address on each host; `plan` boots a local compiled contract;
 * `env` adds to a native host's environment. @ref LLP 1030.000 §7 */
export async function open({onProcess, host = 'web', browser, plan, world, size, env, app, session, documents, url, webDist, reuse, device = false, phone: pick, timing = 'agent', touch = 'agent', storage, seed, locale, timeZone, epoch } = {}) {
  browser ??= 'chrome';
  if (!['chrome', 'firefox', 'webkit'].includes(browser)) throw new Error(`browser: chrome, firefox or webkit, not ${browser}`);
  const facts = launchFacts({seed, locale, timeZone, epoch, env});
  env = {...env, ...launchEnvironment(facts)};
  if (world && !['web','mac','macos','ios','linux','windows'].includes(host)) throw new Error(`world restore unavailable on this host yet: ${host}`);
  if (world && statSync(world).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  if (world && host !== 'web' && !device) env = {...env, EXACT_WORLD:resolve(world)};
  if (device && host !== 'ios') throw new Error('--device is supported for the standalone ios client');
  if (host !== 'web' && browser !== 'chrome') throw new Error(`--browser is only supported by the web carrier, not ${host}`);
  // `timing: 'platform'` (LLP 1035.003 D5, opt-in): the carrier stays and the driver still owns the runner's clock,
  // but UIKit's own transitions, sheet presentations and keyboard animations run at their natural timing — the
  // ordinary app with a socket, for observing an interactive gesture's native motion. The frozen clock is the
  // default the smoke depends on. Replies say `mode: "platform"`.
  if (!['agent', 'platform'].includes(timing)) throw new Error(`timing: agent or platform, not ${timing}`);
  if (timing === 'platform') env = { ...(env ?? {}), EXACT_AGENT_TIMING: 'platform' };
  // `touch: 'platform'` (LLP 1080.000, opt-in): a plain `tap` on an iOS simulator is a real touch from the XCTest runner, `delivery: platform`.
  if (!['agent', 'platform'].includes(touch) || (touch === 'platform' && (device || !['ios', 'host-ios'].includes(host)))) throw new Error(`touch: platform is an iOS simulator's (LLP 1080.000), not ${device ? 'a phone' : host}'s`);
  // A drive has no app storage unless it names a scratch store apart from the app's real files (`--storage <name>`), kept
  // between drives: a tree under the cache base on native; on the web, Chrome's profile for it and its page's origin
  // (agent-launch.mjs `webStore`; Firefox and WebKit open a fresh one each drive). EXACT_AGENT_STORAGE_FRESH empties it.
  if (storage !== undefined && (!/^[A-Za-z0-9._-]+$/.test(storage) || ['.', '..'].includes(storage))) throw new Error("--storage: one name of letters, digits, '.', '-' or '_'");
  if (storage !== undefined && host !== 'web') env = { ...(env ?? {}), EXACT_AGENT_STORAGE: storage };
  // The CLI's `390x844` is accepted from a script too; anything else names the two forms.
  if (typeof size === 'string') {
    const m = size.match(/^(\d+)x(\d+)$/);
    if (!m) throw new Error(`size: [width, height] or "<width>x<height>", not ${JSON.stringify(size)}`);
    size = [Number(m[1]), Number(m[2])];
  }
  if (url !== undefined && ['macos', 'mac', 'ios', 'linux', 'windows', 'host', 'host-ios'].includes(host)) {
    // @ref LLP 1038 D5/D11 — a native scheme/path is a launch location;
    // HTTP(S) keeps the existing development-plan locator form.
    if (/^https?:\/\//i.test(url)) {
      if (plan) throw new Error('a native session takes either a development --url or --plan, not both');
      env = developmentLaunchEnvironment(['--run', '--url', url], env ?? {});
    } else env = { ...(env ?? {}), EXACT_LAUNCH_URL: url };
  }
  const carrier = device ? await openStdio({ host: 'ios', plan, world, size, env, app, device, phone: pick, onProcess })
    // `documents` are the Mac's command line, a terminal's route in (LLP
    // 1033 D3); each window's session then routes by its label (LLP 1069.010).
    : host === 'macos' || host === 'mac' ? await openStdio({ host: 'macos', plan, size: size ?? VIEWPORT, env, app, session, documents, onProcess })
    : host === 'host' ? await openStdio({ host: 'host', plan, env, app, session, onProcess })
    : host === 'host-ios' ? await openIOS({ plan, app, env, session, hostFixture: true, touch, onProcess })
    : host === 'linux' ? await openStdio({ host: 'linux', plan, size: size ?? VIEWPORT, env, app, onProcess })
    : host === 'windows' ? await openStdio({ host: 'windows', plan, size: size ?? VIEWPORT, env, app, onProcess })
    : host === 'ios' ? await openIOS({ plan, env, app, size, touch, onProcess })
    : await openWeb({ browser, plan, world, size, url, app, webDist, onProcess, reuse, storage, fresh: env?.EXACT_AGENT_STORAGE_FRESH === '1', facts, env });
  const mapLocator = plan ?? (url && /^https?:\/\//i.test(url) ? url : env?.EXACT_DEV_PLAN ?? process.env.EXACT_DEV_PLAN)
    // The dist the carrier serves — an app outside the repo's own (shop F2), never Caltrain's by default.
    ?? (carrier.host === 'web' ? resolve(webDist ?? defaultWebDist(), 'app.plan') : null);
  // Without a plan of the drive's own, a native host runs its bake's: the maps a development bake left (LLP 1012.001.000 D6).
  const baked = () => { const a = resolveApp(app); const bin = host === 'windows'
    ? process.env.EXACT_WINDOWS_BIN ?? resolve(a.dir, `dist-windows/${a.crate('windows')}.exe`)
    : process.env.EXACT_LINUX_BIN ?? linuxBinary(a); return bakedPlans(bin, bakeOutput(a)); };
  const sourceMaps = sourceMapReaders(mapLocator ? [mapLocator] : carrier.host !== 'web' ? baked() : []);
  const s = {
    carrier,
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
    /** Await the current page's GPU load time, or null before loading/on native. */
    gpuMs: carrier.gpuMs,
    /** The host's wire: one request as the carrier sends it, views addressed by `id`. The methods (`tap`, `type`, …)
     * are the operations a person performs; a request the wire would answer by doing nothing is refused (habits F8). */
    async op(req) {
      if (!req || typeof req !== 'object' || typeof req.op !== 'string') throw new Error('op: a request is {op: "<name>", …}');
      const input = req.op === 'tap' || req.op === 'type';
      if (input && req.target !== undefined) throw new Error(`op: the wire's ${req.op} addresses a view by \`id\`, not \`target\`; s.${req.op}(${JSON.stringify(req.target)}, …) finds a target and delivers the input as a person would`);
      const r = await carrier.ask(req);
      if (r.error) throw Object.assign(new Error(`${req.op}: ${r.error}`), {reply:r});
      // A web page's own tap or type acts only for an iframe guest, history, a list's row or a control's value:
      // the carrier's browser input is the rest, which only the methods reach.
      if (input && carrier.host === 'web' && req.resize === undefined && !['tapped', 'typed', 'handled', 'history', 'into', 'resized', 'guest', 'value', 'delivery', 'phase'].some((k) => r[k] != null && r[k] !== false))
        throw Object.assign(new Error(`op: the web page's ${req.op} delivered nothing to view ${req.id}; s.${req.op}(…) delivers browser input`), {reply:r});
      return r;
    },
    /** Every live node in preorder, or one target and its descendants; {shallow:true} reads only the target's record, retaining its real child ids. An iframe also carries url, loading, and a reachable guest outline (@ref LLP 1020 D4). A canvas whose row carries a world summary answers with its world instead: `tree <canvas> [under <entity>]` is the world's outline (@ref llp/1046.001-agent-interface-to-a-game.rfc.md D2). */
    async tree(target, options = {}) {
      const under = typeof options === "string" ? {under:options} : {};
      if (typeof target === "string" && target.startsWith("world:")) return s.op({op:"tree", ...await s.target(target), world:true, ...under});
      const {shallow = false, ax = false} = typeof options === "object" && options ? options : {};
      if (ax) return axTree(s, target, options); // LLP 1080.002: the platform's accessibility tree
      const req = { op: 'tree' };
      if (target != null) req.target = typeof target === 'number' || /^\d+$/.test(String(target)) ? Number(target) : target;
      if (shallow !== false) req.shallow = shallow;
      const reply = await s.op(req);
      const canvas = target != null && shallow === false ? reply.nodes?.find((n) => n.id === req.target || n.props?.testId === req.target) : null;
      return canvas?.world ? s.op({op:"tree", id:canvas.id, world:true, ...under}) : reply;
    },
    world(name) { return worldView(this, name); },
    /** Every slot, derive, and resource by name, as typed JSON. */
    state: async (target, under, pose = false, busy = false, page = {}) => s.op({ op: 'state', ...page, ...(busy ? { busy:true } : {}), ...(pose ? { pose: true } : {}), ...(target != null ? await s.target(target) : {}), ...(under != null ? { under: String(under).replace(/^[^:]+:/, '') } : {}) }),
    /** What happened since the last read: the runner's journal (`lines`, from index `from` up to `next`) and the host's own output (`host`). `dropped` counts lines the journal ring let go before this read caught up. */
    async logs() {
      const r = await s.op({ op: 'logs', since: s.logCursor });
      const dropped = Math.max(0, r.from - s.logCursor);
      s.logCursor = r.next;
      return { lines: r.lines, host: carrier.hostLines.splice(0), from: r.from, next: r.next, dropped, ...(r.world ? { world: r.world } : {}) };
    },
    /** `perf [<target>]` (LLP 1079 D2): the plan sites under a view (every root's without one), each with its work across its instances' lifetimes and its source line; `{frames:true}` is the host's presented frames (D4). */
    async perf(target, frames) {
      if (frames) return s.op({ op: 'perf', frames: true, ...(frames.late != null ? { late: frames.late } : {}) });
      const r = await s.op({ op: 'perf', ...(target != null ? { target: /^\d+$/.test(String(target)) ? Number(target) : target } : {}) });
      if (await sourceMaps.refresh()) for (const site of r.sites) { const n = { planDigest: r.plan, site: site.site }; sourceMaps.attach(n); site.source = n.sourceMap; }
      return { target: target ?? null, ...r };
    },
    /** Every on-screen view's box in the viewport (scroll folded in), with its testId and type from the tree. With a target, `node` explains that one node (LLP 1035.002 D1): every row it sets or inherits with where the value came from, its box in each coordinate space the host has, the scroll and clip chains above it, whether it is hidden, inert, in the viewport or clipped away, and what the host mounted for it — observations of the runner's memory and the host's view tree, never a second model. */
    async layout(target, at, inspect = {}) { // inspect: {native: {depth, limit}} | {agree: true, limit} (LLP 1080.001)
      // The host labels disagreements with testIds from the walk's own snapshot (no second read to join).
      if (inspect.agree) return s.op({ op: 'layout', agree: true, ...(inspect.limit != null ? { limit: inspect.limit } : {}) });
      if (typeof target === "string" && target.startsWith("world:")) return s.op({op:"layout", ...await s.target(target), ...(at ? {world:true,x:at[0],y:at[1]} : {})});
      // `layout <canvas> at <x> <y>` is the world's pick (@ref llp/1046.001-agent-interface-to-a-game.rfc.md D2).
      if (target != null && at) return s.op({op:"layout", id:(await s.find(target)).id, world:true, x:at[0], y:at[1]});
      const req = { op: 'layout', ...(inspect.native ? { native: inspect.native } : {}) };
      if (target != null) {
        req.id = (await s.find(target)).id;
        if (await sourceMaps.refresh()) req.plan = true;
        const reply = await s.op(req);
        identifyInspectedNode(reply, target);
        // The JS target keeps no runner to name its plan: it is the plan this carrier serves.
        if (reply.node && carrier.host === 'web' && reply.node.site != null && !reply.node.planDigest && mapLocator && existsSync(mapLocator)) reply.node.planDigest = createHash('sha256').update(readFileSync(mapLocator)).digest('hex');
        if (reply.node) sourceMaps.attach(reply.node);
        return reply;
      }
      const [l, t] = await Promise.all([s.op(req), s.tree()]);
      const by = new Map(t.nodes.map((n) => [n.id, n]));
      for (const n of l.nodes) { const k = by.get(n.id); if (k) { n.type = k.type; if (k.props.testId) n.testId = k.props.testId; } }
      return l;
    },
    async target(target) {
      const text = String(target), colon = text.indexOf(':');
      const node = await s.find(target, false);
      if (node) return { id: node.id };
      if (colon < 0) throw new Error(`no view matches ${target}; tree lists live targets, and a virtualized list's row outside its window comes in by key: \`tap <list> into <key>\` (feed F10)${await s.inFlight()}`);
      return { id: (await s.find(text.slice(0, colon))).id, entity: text.slice(colon + 1) };
    },
    /** The node for a target: a testId (first in preorder on a selected route; a covered screen's copy only when no active one carries it) or a view id. */
    async find(target, required = true) {
      if (target == null) throw new Error(`no view matches ${target}`);
      const t = await s.op(required ? {op:'tree', target, shallow:true} : {op:'tree'});
      const matches = typeof target === 'number' || /^\d+$/.test(String(target)) ? t.nodes.filter((n) => n.id === Number(target)) : t.nodes.filter((n) => n.props.testId === target);
      const node = matches.find((n) => !n.inactive) ?? matches[0];
      if (!node && required) throw new Error(`no view matches ${target}; tree lists live targets, and a virtualized list's row outside its window comes in by key: \`tap <list> into <key>\` (feed F10)${await s.inFlight()}`);
      return node;
    },
    /** For a miss: the requests still in flight, whose answers may bring the view (authoring bench: a drive's first op ran before a stored list loaded). */
    async inFlight() {
      if (host !== 'web') return ''; // a native read on the error path could time out and close the transport
      const st = await s.op({ op: 'state' }).catch(() => null);
      const pending = (st?.pending ?? []).map((p) => p.name).filter(Boolean);
      return pending.length ? `; ${pending.length} request${pending.length === 1 ? ' is' : 's are'} still in flight (${[...new Set(pending)].join(', ')}): \`clock data\` waits for data, and \`state\` shows what is still pending` : '';
    },
    /** What this carrier's input actually is (LLP 1035.003 D2/D3): whether it can hold a contact across requests,
     * and how each form is delivered — `platform` (a real input event through the platform's own path),
     * `recognized` (an already-recognized event injected), `activation` (a hit-test and a direct call),
     * `presenter` (seekable native recognition without OS input injection), or `unsupported`. iOS activates and
     * injects; it synthesizes no touch (LLP 1008 §9). */
    input: host === 'ios' || host === 'host-ios'
      ? { contact: false, hold: false, delivery: (kind) => (['contextmenu', 'dblclick', 'hover', 'pinch'].includes(kind) ? 'recognized' : ['down', 'move', 'hold', 'up', 'cancel'].includes(kind) ? 'unsupported' : ['press', 'drag'].includes(kind) && carrier.touches ? 'platform' : kind === 'drag' ? 'unsupported' : 'activation') }
      : host === 'linux' || host === 'windows'
        ? { contact: true, hold: true, delivery: (kind) => (['down', 'move', 'hold', 'up', 'cancel', 'contextmenu', 'mouse'].includes(kind) ? 'presenter' : 'platform') }
        : { contact: true, hold: true, delivery: () => 'platform' },
    /** The contact this session holds, `{x, y}` in the viewport's space, or null. */
    contact: null,
    /** A press on the target through the host's input path (an iframe target accepts guest `selector` or `x`/`y`); with `{ wheel: [dx, dy] }`, a wheel over it (dy > 0 scrolls down); with `{ hover: true }`, the pointer moved onto it (a hover — and off whatever it was over); with `{ down: true[, at: [x, y]] }`, a contact goes down on it (at its centre, or at an offset from its corner) and stays down until `pointer('up')` (LLP 1035.003 D1). Every reply says how it was delivered (`delivery`), by which carrier, in which mode. */
    // Canvas {mouse:true,at:[x,y]} is one primary mouse click on web/Windows/Linux;
    // {contextmenu:true,at:[x,y]} is secondary. Points are target-relative; plain
    // canvas taps and held down/move/up contacts remain fingers (LLP 1015.000).
    async tap(target, opts = {}) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      if (holdOf(target)) return s.answer('tap', target, opts.choice);
      let node;
      try { node = await s.target(target); }
      catch (error) { throw await tapRefusal(s, target, error); }
      if (opts.mouse) {
        if (!['web', 'linux', 'windows'].includes(host)) throw new Error(`${host} does not carry explicit mouse clicks`);
        if (node.entity !== undefined || ['contextmenu','dblclick','down','drag','wheel','hover','pinch','history','into','gesture'].some(key => opts[key] !== undefined)) throw new Error('mouse cannot be combined with another input mode or an entity target');
      }
      if (node.entity !== undefined) {
        if (opts.drag) throw new Error(`drag: ${target} is a world entity; a drag starts on a view`);
        const { entity } = await s.op({ op: 'layout', ...node });
        if (entity?.visible?.inFrustum === false || entity?.visible?.behindCamera === true) throw new Error(`${target} is ${entity.visible.behindCamera ? 'hidden (behind the camera)' : 'off screen'}: layout ${target} shows the placed box`);
        const b = entity?.screen;
        if (!b || ![b.x, b.y, b.w, b.h].every(Number.isFinite)) throw new Error(`${target} has no screen box; layout ${target} shows visibility; state world:* shows camera/components`);
        const x = b.x + b.w / 2, y = b.y + b.h / 2;
        const { hit } = await s.op({ op: 'layout', id: node.id, world: true, x, y });
        if (!hit) throw new Error(`${target} is not hit at ${x},${y}; layout ${target} shows its box; layout ${String(target).split(':')[0]} at ${x} ${y} shows the pick`);
        if (hit.id !== entity.id) throw new Error(`${target} is behind ${hit.name ?? hit.id} at ${x},${y}; layout ${target} shows its box and layout ${String(target).split(':')[0]}:${hit.name ?? hit.id} shows the blocker`);
        if (s.contact) throw new Error('a contact is already down; use `tap up` or `tap cancel` first');
        const down = await carrier.input(node.id, 'down', { x, y });
        const { phase, ...r } = down.delivery === 'unsupported' ? down : await carrier.input(null, 'up', {});
        return s.landed({ ...r, tapped: node.id, target, entity: node.entity, at: [x, y], delivery: r.delivery ?? s.input.delivery('down'), carrier: host, mode: timing });
      }
      // @ref LLP 1038 D11 — no native carrier turns browser history into a press.
      if (opts.history !== undefined && host !== 'web') return s.tagged({ tapped: node.id, target, history: opts.history, delivery: 'unsupported', carrier: host, mode: timing });
      // A gesture is the platform's, and only the AppKit carrier can phase
      // one. Refusing beats quietly sending a bare delta: the whole reason
      // this form exists is that a plain wheel tests a path a finger never
      // takes, so a driver must never be told it sent a gesture when it did
      // not (LLP 0382 — fail closed, loudly).
      if (opts.gesture && !(host === 'macos' || host === 'mac')) throw new Error(`${host} cannot phase a wheel; \`gesture\` is the AppKit carrier's`);
      if ((opts.contextmenu || opts.dblclick) && !['web', 'ios', 'macos', 'mac', ...(opts.dblclick ? [] : ['linux', 'windows'])].includes(host)) throw new Error(`${host} does not carry contextmenu/dblclick input`);
      if ((opts.contextmenu || opts.mouse) && s.contact) throw new Error('mouse/contextmenu requires the held contact to be released');
      if ((opts.contextmenu || opts.mouse) && opts.at !== undefined) {
        if (!['web', 'linux', 'windows'].includes(host)) throw new Error(`${host} does not carry contextmenu at an explicit point`);
        if (!Array.isArray(opts.at) || opts.at.length !== 2 || !opts.at.every(Number.isFinite)) throw new Error('mouse/contextmenu at needs two finite numbers');
        const layout = await s.layout(), b = layout.nodes.find(n => n.id === node.id), [x, y] = opts.at;
        if (!b || x < 0 || y < 0 || x >= b.w || y >= b.h) throw new Error('mouse/contextmenu at must be inside the target box');
        if (layout.viewport && (b.x + x < 0 || b.y + y < 0 || b.x + x >= layout.viewport.w || b.y + y >= layout.viewport.h)) throw new Error('mouse/contextmenu at is outside the viewport');
      }
      if (opts.pinch !== undefined && !(opts.pinch > 0 && Number.isFinite(opts.pinch))) throw new Error('pinch: expected a positive finite scale');
      if (opts.pinch !== undefined && !['web', 'ios', 'macos', 'mac'].includes(host)) return s.tagged({ tapped: node.id, target, pinch: opts.pinch, delivery: 'unsupported', reason: `${host} has no pinch (LLP 1057.001 §4)`, carrier: host, mode: timing });
      // @ref LLP 1070.000 §5 — a virtualized list's row brought into view by
      // key: the runner's request, the same on every carrier, not an input.
      if (opts.into) {
        const r = await s.op({ op: 'tap', id: node.id, into: opts.into });
        return s.tagged({ ...r, tapped: node.id, target, delivery: 'runner', carrier: host, mode: timing });
      }
      // A target out of view is scrolled into it first, as a person (and Playwright's actionability) would: ledger F7, shop F11.
      // An explicit contact point (`drag … from`, `down at`) is the author's: a visible one is not moved; one out of view
      // brings its view's middle in, as an unspecified point would (the host's reveal scrolls a view, not a point).
      const explicit = opts.drag?.from ?? opts.at, pointOut = async () => {
        const l = await s.layout(), b = l.nodes.find((n) => n.id === node.id), vp = l.viewport;
        if (!b || !vp) return false;
        const x = b.x + explicit[0], y = b.y + explicit[1];
        return x < 0 || y < 0 || x > vp.w || y > vp.h;
      };
      const scrolled = opts.wheel || opts.history !== undefined || (explicit && !await pointOut()) ? null : await s.reveal(node.id);
      // @ref LLP 1080.000 §11 — one whole gesture: press, one straight drag, hold, lift.
      if (opts.drag) return dragTap({ s, carrier, node, target, host, timing, tapRefusal, scrolled }, opts.drag);
      if (opts.modifiers !== undefined && !String(opts.modifiers).split('+').filter(Boolean).every(k => ['Shift', 'Control', 'Alt', 'Meta'].includes(k))) throw new Error(`tap … modifiers: ${opts.modifiers} is not Shift, Control, Alt and Meta joined by +`);
      const kind = opts.history !== undefined ? 'history' : opts.pinch !== undefined ? 'pinch' : opts.down ? 'down' : opts.drop ? 'drop' : opts.wheel ? 'wheel' : opts.hover ? 'hover' : opts.contextmenu ? 'contextmenu' : opts.mouse ? 'mouse' : opts.dblclick ? 'dblclick' : 'press';
      if (kind === 'down' && s.contact) throw new Error('a contact is already down; use `tap up` or `tap cancel` first');
      let at;
      if (kind === 'down' && opts.at) { const b = (await s.layout()).nodes.find((n) => n.id === node.id); if (!b) throw new Error(`view ${node.id} has no box on screen`); at = { x: b.x + opts.at[0], y: b.y + opts.at[1] }; }
      let r;
      try { r = await carrier.input(node.id, kind, { ...opts, ...at }); }
      catch(error) { throw await tapRefusal(s, target, error); }
      if (r.error) r.error = (await tapRefusal(s, target, new Error(r.error))).message;
      if (kind === 'down' && r.delivery !== 'unsupported') {
        s.contact = r.contact === false ? null : { x: r.at[0], y: r.at[1] };
        if (Number.isFinite(r.clock)) s.now = r.clock;
      }
      return s.landed({ ...r, tapped: node.id, target, ...(scrolled ? { scrolled } : {}), delivery: r.delivery ?? s.input.delivery(kind), carrier: host, mode: r.mode ?? timing });
    },
    /** Scroll view `id` into view when its middle is out of it — its nearest scroll containers, then the page (the host's `reveal`): `{from, to}` where its middle moved, or null when it was in view. */
    async reveal(id) {
      const r = await carrier.reveal(id);
      if (r.error) throw new Error(r.error);
      return r.scrolled ? { from: r.from, to: r.to } : null;
    },
    /** The held contact's next phase (LLP 1035.003 D1): `move` to `{x, y}` in the viewport or `by` `{dx, dy}`,
     * over `ms` of real time on platform carriers or seekable time on Linux's presenter; `hold` for `ms`; `up`; `cancel`.
     * The platform owns hit-testing, recognition, scrolling and animation: the app receives whatever it delivers,
     * and a carrier that cannot hold a contact answers `delivery: "unsupported"` rather than faking one. */
    async pointer(phase, opts = {}) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      if (!['move', 'hold', 'up', 'cancel'].includes(phase)) throw new Error(`pointer: not a phase: ${phase} (move, hold, up, cancel)`);
      if (carrier.phasedTouch === false) return carrier.input(null, phase, opts);
      if (!s.contact && ['up','cancel'].includes(phase)) {
        const state = await s.state();
        const contacts = (state.world ?? []).flatMap(w => (w.input?.controlContacts ?? []).filter(c=>c.id<4294967292).map(c=>({...c,canvas:w.canvas})));
        if (contacts.length === 1) return s.op({op:'tap',id:contacts[0].canvas,contact:contacts[0].id,phase});
        if (contacts.length > 1) throw new Error('multiple restored contacts; release one with tap and its contact ID');
      }
      if (!s.contact) throw new Error('no contact is down (tap <target> down first)');
      const r = await carrier.input(null, phase, opts);
      if (r.delivery !== 'unsupported') {
        if (Number.isFinite(r.clock)) s.now = r.clock;
        if (r.contact === false || phase === 'up' || phase === 'cancel') s.contact = null;
        else if (phase === 'move') s.contact = { x: r.at[0], y: r.at[1] };
      }
      return s.landed({ ...r, phase, delivery: r.delivery ?? s.input.delivery(phase), carrier: host, mode: r.mode ?? timing });
    },
    /** Deliver a location to a navigation root (LLP 1038 D11), or set an input's text through the host's text input path; an iframe accepts `{text, selector}` or `{key, selector}` for its guest. */
    async type(target, text) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      if (holdOf(target)) return s.answer('type', target, typeof text === 'object' && text !== null ? JSON.stringify(text) : text);
      const node = await s.find(target);
      const options = typeof text === 'object' && text !== null ? text : { text };
      const key = options.key;
      if (options.for !== undefined) {
        if (key == null || options.phase != null || !Number.isFinite(options.for) || options.for < 0) throw new Error('type for: expected a key and a nonnegative finite duration, without phase');
        return typeFor({ node, target, options, carrier, clock: spec => s.clock(spec), tagged: reply => s.landed(reply), delivery: s.input.delivery('key'), host, timing });
      }
      // A field out of view is scrolled into it, as for a tap; a root (a navigation root's location) is the page.
      const scrolled = node.depth === 0 ? null : await s.reveal(node.id);
      const r = key != null ? await carrier.input(node.id, 'key', { ...options, key: String(key) }) : await carrier.input(node.id, options.clipboard ? 'clipboard' : 'type', { ...options, text: String(options.text ?? '') });
      return s.landed({ ...r, typed: node.id, target, ...(scrolled ? { scrolled } : {}), delivery: r.delivery ?? s.input.delivery(key != null ? 'key' : 'type'), carrier: host, mode: timing });
    },
    /** Answer held device request `@N` (LLP 1069.007 D4), resolved before any view: `tap @N <choice>` (`cancel`, or a choice the capability declares) or `type @N <value>` (a fixture path, a URL, JSON). `@<id>` names the hold by the node its answer arrives at (a picker's `id`) or by its capability (`@open-directory`, `@pick`), whatever its ticket (files F11). The hold is consumed once; a stale ticket is refused by name. The reply says `delivery: "substituted"`. */
    async answer(op, target, value) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      if (value == null || value === '') throw new Error(`${op} ${target}: expected ${op === 'tap' ? 'a choice (cancel, …)' : 'a value'}; state shows the hold under pending`);
      const pending = ticketOf(target) != null && op === 'tap' ? [] : (await s.op({ op: 'state' })).pending ?? [];
      const ticket = ticketOf(target) ?? heldTicket(pending, target);
      if (op === 'tap') return s.landed(await s.op({ op, ticket, choice: String(value) }));
      // A picker's answer is files on this machine (LLP 1069.002 D9): each
      // made absolute here, where the host copies it from; the browser is
      // handed the bytes, as a real picker hands it a File.
      const held = pending.find(p => p.ticket === ticket);
      // An export's answer is where the copy goes (LLP 1069.010 D3): a
      // path on this machine; the browser hands back the bytes to write.
      if (held?.device?.capability === 'export') {
        const to = resolve(String(value));
        const r = await s.op({ op, ticket, text: to });
        if (typeof r.bytes === 'string') { writeFileSync(to, Buffer.from(r.bytes, 'base64')); delete r.bytes; }
        return s.landed(r);
      }
      // A document picker's answer is paths on this machine (LLP 1069.010
      // D2), minted as the person's choice; the browser is handed each
      // file's bytes, or a folder's tree, or a name to save to.
      const documents = ['open-file', 'open-directory', 'save-file'];
      if (documents.includes(held?.device?.capability)) {
        const paths = String(value).split('\n').map((p) => p.trim()).filter(Boolean).map((p) => resolve(p));
        const req = { op, ticket, text: paths.join('\n') };
        if (host === 'web') {
          const tree = (dir, prefix = '') => readdirSync(dir, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? tree(resolve(dir, e.name), `${prefix}${e.name}/`) : e.isFile() ? [{ path: `${prefix}${e.name}`, bytes: readFileSync(resolve(dir, e.name)).toString('base64') }] : []);
          req.files = paths.map((p) => held.device.capability === 'open-directory' ? { name: basename(p), files: tree(p) }
            : held.device.capability === 'save-file' ? { name: basename(p), bytes: '' } : { name: basename(p), bytes: readFileSync(p).toString('base64') });
        }
        return s.landed(await s.op(req));
      }
      if (held?.device?.capability !== 'pick') return s.landed(await s.op({ op, ticket, text: String(value) }));
      const paths = pickedPaths(value).map(p => resolve(p));
      const missing = paths.find(p => !existsSync(p) || !statSync(p).isFile());
      if (missing) throw new Error(`type ${target}: no such file ${missing}`);
      const req = { op, ticket, text: paths.join('\n') };
      if (host === 'web') req.files = paths.map(p => ({ name: basename(p), bytes: readFileSync(p).toString('base64') }));
      return s.landed(await s.op(req));
    },
    /** Move the clock: to an absolute millisecond, by '+N', or to 'settle' — a fixed point at which nothing is in flight (`settled: false` if timers keep starting motion). Timers fire on the way, each at its own time; motion is seeked, never played. The clock lands where the runner says; a timer's refusal is the error. */
    async clock(spec = 'settle') {
      // @ref LLP 1080.000 §12 — an Apple host takes its clock over at the wall under platform timing (and reports a clock it already has otherwise); a seek counts from where it stands. Settle needs no count.
      if (spec !== 'settle' && spec !== 'data' && ['ios', 'host-ios', 'macos', 'host'].includes(carrier.host)) s.now = (await s.op({ op: 'clock', take: true })).clock;
      // `data`: the app's data lands (activation, every request in flight, each `then`), the clock unmoved and no timer fired.
      if (spec === 'data') { const r = await s.op({ op: 'clock', data: true }); if (r.settled === false) r.diagnostic = r.reason === 'data' ? 'clock data gave up on the data module\'s activation at its bound (20 s); logs say why' : 'clock data gave up on requests still in flight at its bound (20 s); state shows them under pending'; return r; }
      // `+N real`: N ms of real time pass, the clock moving with them a step
      // at a time, so what runs on real time — a playing video (LLP 1042 §3),
      // a store, the network — goes on beside the app's timers (jukebox F14).
      const real = /^\+(\d+(?:\.\d+)?)\s+real$/.exec(String(spec));
      if (real) {
        const from = s.now, end = from + Number(real[1]), t0 = performance.now();
        for (;;) {
          const to = Math.min(end, from + performance.now() - t0), r = await s.op({ op: 'clock', to });
          s.now = r.clock;
          if (to >= end) return { ...r, real: Math.round(performance.now() - t0) };
          await new Promise(ok => setTimeout(ok, Math.min(REAL_STEP_MS, end - to)));
        }
      }
      const req = { op: 'clock' };
      if (spec === 'settle') req.settle = true;
      else if (typeof spec === 'string' && spec.startsWith('+')) req.to = s.now + Number(spec.slice(1));
      else req.to = Number(spec);
      if (!req.settle && !Number.isFinite(req.to)) throw new Error(`clock: not a time: ${spec}; use clock +100, clock +100 real, clock settle or clock data; state shows the current clock`);
      // A long seek steps by wall clock (each operation ~CLOCK_BUDGET_MS of Chrome's 15 s window, from CLOCK_STEP_MS of world
      // time, at most 4x a step and CLOCK_SPAN_MS), so a cheap hour is a few presents, not 3,600; the final reply is the seek's.
      let span = CLOCK_STEP_MS;
      for (let t0; !req.settle && req.to - s.now > span; span = clockSpan(span, performance.now() - t0)) t0 = performance.now(), s.now = (await s.op({ op: 'clock', to: s.now + span })).clock;
      const r = await s.op(req); // an intermediate step's pending state is stale by now; a failed step threw
      s.now = r.clock;
      // A store's or the network's reply comes on real time, which a jump does not wait for unless a timer fires first (calendar F10).
      if (!req.settle && r.inflight) r.diagnostic = `${r.inflight} request${r.inflight === 1 ? '' : 's'} still in flight on real time (a store's, a worker's, the network's): \`clock +N\` moved the clock without waiting for ${r.inflight === 1 ? 'it' : 'them'}; \`clock settle\` lands them, and \`state\` lists them under pending`;
      if (req.settle && r.settled === false) r.diagnostic = r.reason === 'device' ? `clock settle stops at held device requests (${(r.tickets ?? []).map(t => '@' + t).join(' ')}); state shows them under pending; answer with tap @N <choice> or type @N <value>` : r.reason === 'requests' ? 'clock settle gave up on requests still in flight at its bound (20 s native); state shows them under pending, and logs a `request N` with no `fulfil N`' : `clock settle did not reach quiescence: ${JSON.stringify(r.world ?? r)}; state world:* busy shows moving values and busy reasons; state shows held input; logs shows reload/refusals`;
      return r;
    },
    /** The window to `w`x`h` points (reader: repagination): the browser's viewport, a macOS window, the Linux presenter. The wire's
     * `tap` with `resize` (LLP 1041 §8: an input, not an operation); an iOS app's viewport is the device's, and it says so. */
    async resize(w, h) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      return s.landed({ ...(await s.op({ op: 'tap', resize: [w, h] })), carrier: host });
    },
    /** The device facts by their web names (LLP 1061 D5; LLP 1069.000 D6), grouped on the wire as LLP 1069.007 D2 groups them. `media`: `{"prefers-reduced-motion": "reduce"}`, `"prefers-reduced-transparency"` likewise, `"prefers-contrast": "more"|"less"|"custom"|"no-preference"`, `"prefers-color-scheme": "dark"|"light"` (the system's; an app's `setScheme` still wins). `page`: `"visibility-state": "visible"|"hidden"`, `online`, `can-share` and `can-open-files` `"true"|"false"`, `"root-font-size"` in px (what `rem` follows). `fold` (LLP 1078 D7): `posture folded|continuous`, `segments <cols>x<rows>`, `gap <points>` — a host without a fold splits its viewport evenly with the gap centred on each divider; a host with a real fold refuses ("the device decides"). Unnamed facts stay. The reply is what the host now reports, by group. */
    async prefer(facts) {
      if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
      const { media, page, fold } = preferGroups(facts);
      if (carrier.prefer) return s.tagged(await carrier.prefer(media, page, fold));
      return s.op(preferOp(media, page, fold));
    },
    /** Pixels as PNG (second argument true includes the native window), or a canvas carry with `(path, target, "save")`, or film: `(path, {over, every})` (LLP 1012.001.000 D2). */
    screenshot: async (path, target = false, form) => {
      // `screenshot shots/home.png` makes `shots/`, as a shell redirect would not but every agent expects.
      mkdirSync(dirname(resolve(path)), { recursive: true });
      if (target && typeof target === 'object') return s.film(path, target);
      if (form === 'save') {
        if (!['web','macos','ios','linux','windows'].includes(s.host)) throw new Error(`world save unavailable on this host yet: ${s.host}`);
        const reply = await s.op({op:'screenshot', ...await s.target(target), world:true, form:'save'});
        const {data, ...metadata} = reply;
        if (typeof data !== 'string') throw new Error(`canvas ${target} returned no save bytes; inspect state and state ${target}:* for the refusal reason`);
        if (reply.bytes > WORLD_LIMIT || data.length > 4 * Math.ceil(WORLD_LIMIT / 3)) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
        const bytes = Buffer.from(data, 'base64');
        if (bytes.length !== reply.bytes) throw new Error(`canvas ${target} returned a truncated save; inspect state and logs; retry screenshot checkpoint.world ${target} save`);
        writeFileSync(resolve(path), bytes);
        return s.tagged({...metadata, screenshot:resolve(path)});
      }
      if (form !== undefined) throw new Error(`screenshot: unknown form ${form}`);
      if (typeof target === 'string') return s.op({op:'screenshot', ...await s.target(target), world:true, path:resolve(path)});
      return s.tagged(await carrier.screenshot(resolve(path), target));
    },
    /** Film on the agent's clock (LLP 1012.001.000 D2): a frame, `clock +every`, a frame … through `over`. A `.apng` path is an animated PNG (for a person to play); any other is one PNG of the frames in a grid (for an agent to look at). Every frame is also kept at full size beside it, `<path>.frames/<i>-<clock>ms.png`. The clock lands at the last frame. A step that fails stops the film: what was taken is written, and the error says so. */
    async film(path, { over, every }) {
      const frames = Math.floor(over / every) + 1;
      if (!(Number.isFinite(over) && over >= 0 && Number.isFinite(every) && every >= 1)) throw new Error('screenshot over <ms> every <ms>: over ≥ 0, every ≥ 1 ms');
      if (frames > FILM_FRAMES) throw new Error(`screenshot over ${over} every ${every}: ${frames} frames, at most ${FILM_FRAMES}; take a longer every or a shorter over`);
      // Under platform timing UIKit's transitions run on their own clock: frames would not be the clock's.
      if (timing === 'platform') throw new Error('screenshot over: film is the agent clock\'s, and --timing platform leaves UIKit\'s motion on its own');
      const out = resolve(path), dir = out + '.frames', animated = /\.apng$/i.test(out), images = [], at = [];
      rmSync(dir, { recursive: true, force: true });
      mkdirSync(dir, { recursive: true });
      let last, failure;
      for (let i = 0; i < frames && !failure; i++) {
        try {
          if (i) await s.clock('+' + every);
          const file = resolve(dir, `${String(i).padStart(3, '0')}-${s.now}ms.png`);
          last = await s.screenshot(file);
          const image = decodePng(readFileSync(file));
          if (images.length && (image.width !== images[0].width || image.height !== images[0].height)) throw new Error(`frame ${i} is ${image.width}×${image.height}, frame 0 ${images[0].width}×${images[0].height}; film needs one size`);
          if (frames * image.width * image.height > FILM_PIXELS) throw new Error(`${frames} frames of ${image.width}×${image.height} px exceed ${FILM_PIXELS / 1e6}M pixels; take a longer every, a shorter over, or a smaller --size`);
          images.push(image); at.push(s.now);
        } catch (error) { failure = Object.assign(new Error(`screenshot over: stopped at frame ${i} (clock ${s.now}) of ${frames}: ${error.message}`), { reply: error.reply }); }
      }
      if (images.length) writeFileSync(out, animated ? encodeApng(images, every) : encodePng(contactSheet(images)));
      else rmSync(dir, { recursive: true, force: true });
      if (failure) { failure.message += images.length ? `; the ${images.length} frames taken are in ${out} and ${dir}` : ''; throw failure; }
      const { screenshot, ...tags } = last;
      return { ...tags, screenshot: out, frames, every, over, at, dir, form: animated ? 'animated' : 'sheet' };
    },
    /** An input's end (LLP 1012 §2): the `then` of each answer the input settled lands before the reply, as a click
     * handler's state is there for a test's next line — the clock unmoved and no timer fired (trivia F3, kanban F19).
     * A reply still on real time (a store's, the network's) stays for a `clock` step. The reply's tags are read after it. */
    async landed(r) {
      if (r == null || r.error != null || r.delivery === 'unsupported') return s.tagged(r);
      const l = await carrier.ask({ op: 'clock', land: true });
      if (l.error) return { ...r, error: l.error };
      delete r.epoch; delete r.incarnation; delete r.clock;
      return s.tagged(r);
    },
    /** Every reply carries the runner's `epoch`, `incarnation` and `clock` (LLP 1035.002 D3). A host that answered
     * the operation itself stamps them; the web carrier's input and capture are the driver's own (CDP), so the driver
     * reads the tags after the operation and adds what the reply lacks. An error is left alone. */
    async tagged(r) {
      if (r == null || r.error != null || r.epoch != null) return r;
      const tags = await s.op({ op: 'tags' });
      for (const key of Object.keys(tags)) if (r[key] === undefined) r[key] = tags[key];
      return r;
    },
    close: carrier.close,
  };
  // Under platform timing this seed is the runner's clock; on the Apple hosts
  // `s.clock` rebases it on the host's (LLP 1035.003 D5, LLP 1080.000 §12).
  if (timing === 'platform' && carrier.host !== 'web') { const { clock } = await s.op({ op: 'tags' }); if (Number.isFinite(clock)) s.now = clock; }
  return s;
}
// ---------------------------------------------------------------- the CLI
// Authored tests (LLP 1017 P7): agent-test.mjs drives a file's `test` blocks through `open`.
export { runTests, textOf } from './agent-test.mjs';
async function main(argv) {
  const { flags, rest } = parseFlags(argv);
  const [host, ...ops] = rest;
  const browser = flags.browser ?? (host === 'web' ? process.env.EXACT_WEB_BROWSER : undefined);
  if (host && flags.test) {
    if (flags.touch && flags.touch !== 'agent') throw new Error('--test runs its own sessions, without real touches: --touch platform is not supported with --test (LLP 1080.000)');
    const r = await runTests({ host, browser, file: flags.test, plan: flags.plan, app: flags.app, size: flags.size, device: flags.device, phone: flags.phone, url: flags.url, seed: flags.seed, locale: flags.locale, timeZone: flags.timeZone, epoch: flags.epoch, storage: flags.storage });
    for (const t of r.results) {
      console.log(`test "${t.name}": ${t.failures.length ? 'FAIL' : 'ok'}`);
      for (const f of t.failures) console.error('  ' + f);
    }
    console.log(`${r.passed} passed, ${r.failed} failed`);
    return r.failed ? 1 : 0;
  }
  // A trace a person's session saved (LLP 1079 D5), read back with no app running.
  if (host === 'trace' && ops.length === 1) { const t = await readTrace(ops[0], traceLocators); console.log(flags.json ? JSON.stringify(t) : renderTrace(t)); return 0; }
  if (!host || !ops.length) {
    console.error('usage: bun scripts/agent.mjs <web|macos|ios|linux|host|host-ios> [--browser chrome|firefox|webkit] [--app <name>] [--plan <file> | --url <url>] [--world <file>] [--device] [--phone <name|udid>] [--session <label>] [--open <document>] [--storage <name>] [--seed <n>] [--locale <tag>] [--time-zone <zone>] [--epoch <ISO|ms>] [--size <w>x<h>] [--json] <op> [<op> …]\n  desktop carriers open 420x900 unless --size names another viewport; tap and type scroll a target out of view into it first; web defaults to chrome; EXACT_WEB_BROWSER selects the same option. Install the other engines outside the repo: bunx playwright@1.63.0 install firefox webkit\n  tree | layout | state | logs | screenshot <png> [window] | screenshot <png|apng> over <ms> every <ms> | screenshot <path> <canvas> save | tap <target> [wheel <dx> <dy> [gesture] [modifiers <Control…>] | drop <path…> | hover | history <n> | {"history":n} | contextmenu | mouse | dblclick | modifiers <Shift+Meta…> | pinch <scale> [at <x> <y>] | drag <dx> <dy> [from <x> <y>] [mouse: the left button, desktop pointers] [press <ms>] [over <ms>] [hold <ms>] [during "<op>" …: after the move, the finger down]] | tap <target> down [at <x> <y>], then tap move [by] <x> <y> [over <ms>] | tap hold <ms> | tap up | tap cancel | type <target> <text…> (a select, date, time, range or checkbox: its value) | type <target> key <Name> [for <ms>] | type <target> copy | cut | paste <text…> (the clipboard event at the focus) | tap @N|@<id> <choice> | type @N|@<id> <value> | clock <ms|+ms|+ms real|settle|data> | resize <w>x<h> (the window: web, macOS, Linux) | prefer <media feature, page fact, posture folded|continuous, segments <cols>x<rows> [gap <points>]> […] | perf [<target>] [during "<op>" …] | perf frames [late <n>]\n       bun scripts/agent.mjs trace <file>   (a development session\'s trace, LLP 1079 D5)\n       bun scripts/agent.mjs <host> --test <file.test.contract>   (LLP 1017 P7: the file\'s `test` blocks, run here)');
    return 2;
  }
  const s = await open({ host, browser, plan: flags.plan, world: flags.world, size: flags.size, app: flags.app, session: flags.session, documents: flags.open, url: flags.url, device: flags.device, phone: flags.phone, timing: flags.timing, touch: flags.touch, storage: flags.storage, seed: flags.seed, locale: flags.locale, timeZone: flags.timeZone, epoch: flags.epoch });
  let at = 0;
  // One op line; `perf … during "<op>" …` drives its own through here.
  const step = async (line) => {
      const [op, ...args] = line.trim().split(/\s+/);
      let r;
      switch (op) {
        case 'tree': r = args[0] === '--ax' ? await s.tree(args[1], {ax: true}) : await s.tree(args[0], args[1] === 'under' ? args[2] : undefined); break;
        case 'state': r = await s.state(args[0], args[1] === 'under' ? args[2] : undefined, args[1] === 'pose', args[1] === 'busy', {...(args.includes('from') ? {from:Number(args[args.indexOf('from') + 1])} : {}), ...(args.includes('limit') ? {limit:Number(args[args.indexOf('limit') + 1])} : {}), ...(args.includes('resources') ? {resources:true} : {})}); break;
        case 'logs': r = await s.logs(); break;
        case 'layout': r = await s.layout(...layoutArgs(args)); break;
        case 'screenshot':
          if (args[1] === 'over') {
            if (args[3] !== 'every' || args.length !== 5) throw new Error('screenshot <png|apng> over <ms> every <ms>');
            r = await s.screenshot(args[0], { over: Number(args[2]), every: Number(args[4]) });
          } else r = await s.screenshot(args[0] ?? 'screenshot.png', args[2] === 'save' ? args[1] : args[1] === 'window', args[2]);
          break;
        case 'tap':
          // The contact's phases (LLP 1035.003 D1) read as `tap move …`,
          // `tap hold`, `tap up`, `tap cancel` only while a contact is down;
          // with none down those words are targets like any other.
          if (holdOf(args[0])) r = await s.tap(args[0], { choice: args[1] }); // `tap @7 cancel` is the hold, even while a contact is down
          else if (s.contact && args[0] === 'move') {
            const by = args[1] === 'by';
            const over = args.indexOf('over');
            const [a, b] = by ? [args[2], args[3]] : [args[1], args[2]];
            r = await s.pointer('move', { ...(by ? { dx: Number(a), dy: Number(b) } : { x: Number(a), y: Number(b) }), ms: over > 0 ? Number(args[over + 1]) : 0 });
          } else if (s.contact && args[0] === 'hold') r = await s.pointer('hold', { ms: Number(args[1] ?? 0) });
          else if (s.contact && (args[0] === 'up' || args[0] === 'cancel')) r = await s.pointer(args[0]);
          else if (args[1] === 'down') r = await s.tap(args[0], { down: true, at: args[2] === 'at' ? [Number(args[3]), Number(args[4])] : undefined });
          else if (args[1] === 'history') r = await s.tap(args[0], { history: Number(args[2]) });
          else if (args[1] === 'drag') {
            // tap <target> drag <dx> <dy> [from <x> <y>] [mouse] [press <ms>] [over <ms>] [hold <ms>] [during "<op>" …, inside the hold under --touch platform]
            const drag = { dx: Number(args[2]), dy: Number(args[3]) };
            for (let i = 4; i < args.length;) {
              if (args[i] === 'during') {
                // The rest of the line is quoted ops and nothing else; reads and the clock only (a tap would be a second finger).
                const after = line.indexOf(' drag ') + 1, rest = line.slice(after + line.slice(after).search(/\sduring\s+"/) + ' during '.length);
                const ops = [...rest.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => JSON.parse(`"${m[1]}"`));
                if (!ops.length || rest.replace(/"((?:[^"\\]|\\.)*)"/g, '').trim()) throw new Error('tap … drag … during: the last option, each op quoted, as hold 300 during "state" "clock +600" (a hold the touch runner needs)');
                // A filmed screenshot (`over … every`) loops on the clock: not one bounded read.
                const refused = ops.find((op) => { const w = op.trim().split(/\s+/); return !['tree', 'layout', 'state', 'logs', 'screenshot', 'clock'].includes(w[0]) || (w[0] === 'screenshot' && w[2] === 'over'); });
                if (refused) throw new Error(`tap … drag … during: ${JSON.stringify(refused)} is not a read or the clock`);
                drag.during = ops.map((op) => async () => (await step(op))[1]);
                break;
              }
              if (args[i] === 'from') { drag.from = [Number(args[i + 1]), Number(args[i + 2])]; i += 3; }
              else if (args[i] === 'mouse') { drag.mouse = true; i += 1; }
              else if (['press', 'over', 'hold'].includes(args[i])) { drag[args[i]] = Number(args[i + 1]); i += 2; }
              else throw new Error(`tap … drag: unknown option ${args[i]}; from <x> <y>, mouse, press <ms>, over <ms>, hold <ms>, during "<op>" …`);
            }
            r = await s.tap(args[0], { drag });
          }
          else if (args[1] === 'pinch') r = await s.tap(args[0], { pinch: Number(args[2]), ...(args[3] === 'at' ? { at: [Number(args[4]), Number(args[5])] } : {}) });
          else if (args[1]?.startsWith('{')) r = await s.tap(args[0], JSON.parse(args.slice(1).join(' ')));
          else if (args[1] === 'into') {
            // tap <list> into <key> [block <v>] [inline <v>]
            const into = { key: String(args[2] ?? '') };
            for (let i = 3; i + 1 < args.length; i += 2) {
              if (!['block', 'inline'].includes(args[i])) throw new Error(`tap … into: unknown option ${args[i]}; block <start|center|end|nearest> and inline <…>`);
              into[args[i]] = args[i + 1];
            }
            r = await s.tap(args[0], { into });
          }
          // A click with modifiers held (gallery F20): `tap <target> modifiers Shift+Meta`.
          else if (args[1] === 'modifiers' && args[2] && args.length === 3) r = await s.tap(args[0], { modifiers: args[2] });
          // Files dragged in from outside, each an absolute path (studio diary R19): `tap <target> drop <path…>`.
          else if (args[1] === 'drop' && args.length > 2) r = await s.tap(args[0], { drop: args.slice(2).map(p => resolve(p)) });
          // A wheel with the modifiers held (a pinch is a Control-held wheel; studio diary R3): `… wheel <dx> <dy> [gesture] [modifiers Control]`.
          else r = args[1] === 'wheel' ? await s.tap(args[0], { wheel: [Number(args[2]), Number(args[3])], gesture: args[4] === 'gesture', ...(args.indexOf('modifiers') > 3 ? { modifiers: args[args.indexOf('modifiers') + 1] } : {}) }) : args[1] === 'hover' ? await s.tap(args[0], { hover: true }) : ['contextmenu', 'dblclick', 'mouse'].includes(args[1]) ? await s.tap(args[0], { [args[1]]: true }) : await s.tap(args[0]);
          break;
        case 'type': r = await s.type(...typeArguments(args)); break;
        case 'clock': r = await s.clock(args.join(' ') || 'settle'); break;
        case 'resize': { const m = /^(\d+)x(\d+)$/.exec(args.join('')); if (!m) throw new Error('resize <w>x<h>, as resize 800x600'); r = await s.resize(Number(m[1]), Number(m[2])); break; }
        case 'prefer': r = await s.prefer(Object.fromEntries(args.flatMap((a, i) => i % 2 ? [] : [[a, args[i + 1]]]))); break;
        case 'perf': r = await perfOp(s, args, line, step); break;
        default: throw new Error(`unknown op: ${op} (tree, layout, state, logs, screenshot, tap, type, clock, resize, prefer, perf)`);
      }
      return [op, r];
  };
  // A drive that names no scratch store has no storage, and a source's write fails only in the
  // journal; say so the first time, beside the op that caused it (authoring bench, LLP 1087).
  // The web carrier only: its journal read is an in-page call, where a native carrier's
  // could time out and fail the transport for the ops after it.
  let peek = 0, probes = 0, warned = host !== 'web' || flags.json || flags.storage !== undefined;
  const storageNote = async () => {
    if (warned || ++probes > 20) return; // a missing store shows at the first writes
    try {
      const j = await s.op({ op: 'logs', since: peek });
      if (!Array.isArray(j?.lines)) return;
      peek = j.next;
      if (j.lines.some((l) => /unavailable in agent mode/.test(typeof l === 'string' ? l : JSON.stringify(l)))) {
        warned = true;
        console.error('note: a data source was refused storage: this drive names no scratch store, so writes do nothing; pass --storage <name> (docs/agent-pitfalls.md)');
      }
    } catch { warned = true; } // advice only: never fail a drive over it
  };
  try {
    for (const [k, line] of ops.entries()) {
      at = k + 1;
      const [op, r] = await step(line);
      console.log(flags.json ? JSON.stringify(r) : render(op, r));
      if (['tap', 'type', 'clock'].includes(op)) await storageNote();
    }
    return 0;
  } catch (e) {
    e.message = `op ${at}/${ops.length} \`${ops[at - 1]?.trim()}\`: ${e.message}`;
    throw e;
  } finally {
    await s.close();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { if (e.steps) console.error(render('type', {steps:e.steps})); console.error(e.message); process.exit(e.stale ? 3 : 1); });
}
