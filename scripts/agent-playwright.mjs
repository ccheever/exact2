// Firefox and WebKit carrier for scripts/agent.mjs. Chrome deliberately stays
// in agent.mjs on its direct CDP pipe; importing Playwright is deferred until
// one of these browsers is selected.
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchFacts, refuseStale, staleError, warnStale, webChanges, withFaults } from './agent-launch.mjs';
import { deliverClipboard, pasteChord } from './agent-keys.mjs';
import { builtAppMatches, jsTargetBuild, serveBuildTree, serveStatic } from '../host/web/serve.mjs';
import { resolveApp, webBuildCommand, webDist as defaultWebDist } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const INSTALL = 'bunx playwright@1.63.0 install firefox webkit';
const WORLD_LIMIT = 256 * 1024 * 1024;

const FIREFOX_PREFS = {
  'general.smoothScroll': false,
  'general.smoothScroll.mouseWheel': false,
  'mousewheel.system_scroll_override.enabled': false,
  'mousewheel.default.delta_multiplier_x': 100,
  'mousewheel.default.delta_multiplier_y': 100,
  'widget.gtk.overlay-scrollbars.enabled': true,
  'ui.prefersReducedTransparency': 0,
  // TouchEvent for the synthetic held-touch path without Playwright's hasTouch,
  // which would also make (pointer: coarse) true where Chrome's oracle is fine.
  'dom.w3c_touch_events.enabled': 1,
};

function unavailable(name, error) {
  const message = String(error?.message ?? error);
  const libraries = [...new Set([...message.matchAll(/\b(lib[\w.+-]+\.so(?:\.\d+)*)/g)].map(m => m[1]))];
  if (libraries.length) return new Error(`web carrier unavailable: ${name} missing system libraries: ${libraries.join(', ')}`);
  if (/Executable doesn't exist|download new browsers/i.test(message)) return new Error(`web carrier unavailable: ${name} not installed: ${INSTALL}`);
  return new Error(`web carrier unavailable: ${name}: ${message.replace(/\s+/g, ' ').trim()}`);
}

/** Launch once without an app so a conformance run reports an engine-level
 * installation or library failure once, before iterating its apps. */
export async function probePlaywrightBrowser(name) {
  if (!['firefox', 'webkit'].includes(name)) throw new Error(`browser: firefox or webkit, not ${name}`);
  let server;
  try {
    const playwright = await import('playwright-core');
    server = await playwright[name].launchServer({ headless: true, ...(name === 'firefox' ? { firefoxUserPrefs: FIREFOX_PREFS } : {}) });
  } catch (error) { throw unavailable(name, error); }
  await server.close();
}

function worldFile(path) {
  if (statSync(path).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  const bytes = readFileSync(path);
  if (bytes.length > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  return bytes;
}

async function assertDist(dist, app) {
  if (!await builtAppMatches(dist, app)) throw staleError(`web dist is not a complete build for selected app ${app.id}; stale receipt ${resolve(dist, '.exact-build.json')}; run ${webBuildCommand(app, dist)}`);
}

async function files({ plan, pageURL, app, webDist }) {
  const selected = resolveApp(app), dist = resolve(webDist ?? defaultWebDist());
  if (!pageURL) {
    await assertDist(dist, selected);
    const js = jsTargetBuild(dist), env = process.env.EXACT_APP_DIR || webDist || process.env.EXACT_WEB_DIST ? `EXACT_APP_DIR=${selected.dir} EXACT_WEB_DIST=${dist} ` : '';
    const command = webBuildCommand(selected, dist, js ? '' : ' --wasm', `${env}bun host/web/build.mjs ${selected.crate('web')}${js ? '' : ' --wasm'}`), changed = webChanges(dist, selected);
    refuseStale('web', resolve(dist, '.exact-build.json'), changed.app, command);
    warnStale('web', resolve(dist, '.exact-build.json'), changed.shared, `if they matter, run ${command}`);
  }
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
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  return { js, server, planBuild, url: pageURL ?? `http://127.0.0.1:${server.address().port}/` };
}

const init = `
  addEventListener('click', event => {
    if (event.isTrusted) { performance.clearMarks('exact-agent-input'); performance.mark('exact-agent-input', {startTime: event.timeStamp}); }
  }, true);
  addEventListener('DOMContentLoaded', () => {
    document.documentElement.style.overscrollBehavior = 'none';
    const style = document.createElement('style');
    style.textContent = '*{scrollbar-width:none!important}*::-webkit-scrollbar{display:none!important}';
    document.head.append(style);
  });
`;

async function waitForBoot(page, why = 'the page never booted') {
  try { await page.waitForFunction(() => document.getElementById('exact-root')?.dataset.bootMs != null, null, { timeout: 30000 }); }
  catch { throw new Error(why); }
  await page.evaluate(() => globalThis.exact.ready);
  return Number(await page.locator('#exact-root').getAttribute('data-boot-ms'));
}

// A key by its `key` name or its code, on every target (as scripts/agent.mjs `cdpKey`).
const keyName = (code) => {
  if (code.length === 1) return code;
  if (/^Key[A-Z]$/.test(code)) return code;
  if (/^Digit[0-9]$/.test(code)) return code;
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;
  // Playwright's keyboard names F1–F12 only; Chrome's CDP and the native hosts press F13–F24.
  if (/^F(1[3-9]|2[0-4])$/.test(code)) throw new Error(`key: ${code} is not on Playwright's keyboard (F1–F12); Chrome, macOS, iOS and Linux press it`);
  const key = { Space: ' ', Enter: 'Enter', Escape: 'Escape', Tab: 'Tab', Backspace: 'Backspace', Delete: 'Delete', Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown', ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown', ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight', Shift: 'Shift', ShiftLeft: 'ShiftLeft', ShiftRight: 'ShiftRight', Control: 'Control', Alt: 'Alt', Meta: 'Meta' }[code];
  if (!key) throw new Error(`key: unsupported key ${code}`);
  return key;
};
// A chord (`Shift+Enter`, `Meta+s`, `+`), Playwright's own syntax: the keys
// to hold down in order, modifiers first.
const chordKeys = (chord) => {
  const held = [];
  let rest = chord;
  for (let m; (m = /^(Shift|Control|Alt|Meta)\+(.+)$/.exec(rest)); rest = m[2]) held.push(m[1]);
  return [...held, keyName(rest)];
};

/** Hold a chord (`Shift`, `Shift+Meta`) around `act` and release it in
 * reverse, including when `act` throws. Playwright's mouse has no modifier
 * field; the keys are what make `shiftKey` true (drums R13). */
export async function withHeldKeys(keyboard, held, act) {
  const names = String(held ?? '').split('+').filter(Boolean);
  for (const key of names) await keyboard.down(key);
  try { return await act(); }
  finally { for (const key of [...names].reverse()) await keyboard.up(key).catch(() => {}); }
}

/** A key focuses its target when the target can take it. A button, a link, a
 * world and a held key require that (Chrome's `browserKey` throws otherwise).
 * Any other key leaves the focus where it is and still presses
 * (drums R13; docs/contract-for-agents.md). */
export async function focusForKey(id, required, focus) {
  const result = await focus(id);
  if (required && !result?.ok) throw new Error(result?.error ?? `view ${id} could not take focus`);
}

/** Mouse phases. `mouse` is named on down only; later phases keep that button.
 * A touch phase is refused: Playwright has no trusted touch stream, and a
 * synthetic one is not the same input (drums R13). */
export function playwrightPointer({ name, move, down, up, wait }) {
  let contact = null;
  const refused = (kind) => new Error(`${name} ${kind} unsupported: Playwright cannot produce trusted phased touches; synthetic dispatchEvent input is not equal input`);
  const phase = async (kind, opts, point) => {
    const mouse = kind === 'down' ? !!opts.mouse : !!contact;
    if (!mouse) throw refused(kind);
    if (kind === 'down') {
      if (contact) throw new Error('a contact is already down; use `tap up` first');
      const px = opts.x ?? point?.x, py = opts.y ?? point?.y;
      await move(px, py);
      await down();
      contact = { x: px, y: py };
      return { contact: opts.id, phase: 'down', at: [px, py], delivery: 'platform', pointer: 'mouse' };
    }
    if (!contact) throw new Error('no contact is down');
    if (kind === 'move') {
      const to = { x: opts.x ?? contact.x + (opts.dx ?? 0), y: opts.y ?? contact.y + (opts.dy ?? 0) };
      const ms = Math.max(0, opts.ms ?? 0), steps = Math.max(1, Math.round(ms / 16));
      for (let i = 1; i <= steps; i++) {
        const t = i / steps;
        await move(contact.x + (to.x - contact.x) * t, contact.y + (to.y - contact.y) * t);
        if (ms && !opts.virtual) await wait(ms / steps);
      }
      contact = to;
      return { phase: 'move', at: [to.x, to.y], delivery: 'platform' };
    }
    if (kind === 'hold') {
      if (opts.ms && !opts.virtual) await wait(opts.ms);
      return { phase: 'hold', at: [contact.x, contact.y], delivery: 'platform' };
    }
    // A mouse has no cancel: the button comes up where it is.
    const at = [contact.x, contact.y];
    await up();
    contact = null;
    return { phase: kind, at, delivery: 'platform' };
  };
  phase.release = async () => { if (!contact) return; await up().catch(() => {}); contact = null; };
  return phase;
}

/** Open Firefox or WebKit through Playwright. The page still owns Exact's
 * deterministic runner/motion clock; Playwright carries only browser IO. */
export async function openPlaywrightWeb({ browser: name, plan, world, size, url: pageURL, app, webDist, onProcess, reuse, storage, facts: givenFacts, parity = '' }) {
  if (!['firefox', 'webkit'].includes(name)) throw new Error(`browser: chrome, firefox or webkit, not ${name}`);
  const facts = givenFacts ?? launchFacts({});
  if (reuse) await reuse.close(); // Chrome reuse is intentionally not crossed with another engine.
  const hosted = await files({ plan, pageURL, app, webDist });
  let browserServer, browser, context, page;
  // Only Chrome keeps a named store's profile between drives (agent.mjs `openWeb`); here it is this drive's own.
  const hostLines = storage === undefined ? [] : [`${name}: --storage ${storage} is this drive's own profile, emptied when it ends; Chrome keeps a store between drives`];
  const closeFiles = () => {
    hosted.server.close();
    if (hosted.planBuild) rmSync(hosted.planBuild, { recursive: true, force: true });
  };
  try {
    const playwright = await import('playwright-core');
    browserServer = await playwright[name].launchServer({ headless: true,
      ...(name === 'firefox' ? { firefoxUserPrefs: FIREFOX_PREFS } : {}) });
    onProcess?.(browserServer.process());
    browser = await playwright[name].connect(browserServer.wsEndpoint());
    context = await browser.newContext({ viewport: { width: size[0], height: size[1] }, screen: { width: size[0], height: size[1] }, deviceScaleFactor: 1, hasTouch: false, colorScheme: 'light', reducedMotion: 'no-preference', contrast: 'no-preference' });
    page = await context.newPage();
  } catch (error) {
    await browser?.close().catch(() => {});
    await browserServer?.close().catch(() => {});
    closeFiles();
    throw unavailable(name, error);
  }
  const close = async () => { await browser.close().catch(() => {}); await browserServer.close().catch(() => {}); closeFiles(); };
  try {
    page.on('console', msg => hostLines.push(`console.${msg.type()}: ${msg.text()}`));
    page.on('pageerror', error => hostLines.push(`exception: ${error.stack ?? error.message}`));
    await page.addInitScript(init + parity);
    if (world && !plan) {
      const encoded = worldFile(world).toString('base64');
      await page.addInitScript(value => { globalThis.exactWorldCarry = Uint8Array.from(atob(value), c => c.charCodeAt(0)); }, encoded);
    }
    const address = new URL(hosted.url);
    address.searchParams.set('agent', '1');
    for (const [key, value] of Object.entries(facts)) address.searchParams.set(key, value);
    if (storage !== undefined) address.searchParams.set('storage', storage);
    await page.goto(address.href, { waitUntil: 'commit' });
    let boot = await waitForBoot(page, `the page never booted; ${hostLines.join('\n')}`);
    if (await page.evaluate(() => matchMedia('(prefers-reduced-transparency: reduce)').matches)) throw new Error(`${name} cannot emulate the required prefers-reduced-transparency: no-preference launch fact`);
    if (plan && !hosted.js) {
      const carry = world ? worldFile(world).toString('base64') : null;
      await page.evaluate(async ({ carry }) => {
        if (carry) globalThis.exact.worldCarry = Uint8Array.from(atob(carry), c => c.charCodeAt(0));
        const bytes = await fetch('/__plan').then(r => r.arrayBuffer());
        await globalThis.exact.reload(new Uint8Array(bytes), true);
      }, { carry });
    }
    const frame = () => page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true)))));
    const evaluate = expression => page.evaluate(expression);
    const ask = async req => {
      if (req.op === 'tap' && req.resize !== undefined) {
        const pair = req.resize;
        if (Object.keys(req).some(k => !['op', 'resize'].includes(k)) || !Array.isArray(pair) || pair.length !== 2
          || !pair.every(n => Number.isInteger(n) && n >= 64 && n <= 4096) || pair[0] * pair[1] > 8388608) return { error: 'tap resize needs exactly two integer dimensions in 64...4096, area <= 8388608, and no other input fields' };
        await page.setViewportSize({ width: pair[0], height: pair[1] }); await frame();
        return { resized: pair, viewport: await page.evaluate(() => [innerWidth, innerHeight]), delivery: 'browser-viewport' };
      }
      // The tab closed with its `beforeunload` run (agent.mjs's Chrome carrier): a prevented one's dialog answered "Stay".
      if (req.op === 'tap' && req.close !== undefined) {
        if (Object.keys(req).some(k => !['op', 'close'].includes(k)) || req.close !== true) return { error: 'tap close takes no other input fields' };
        const asked = new Promise(ok => page.once('dialog', ok)), gone = new Promise(ok => page.once('close', () => ok(null)));
        await page.close({ runBeforeUnload: true });
        const dialog = await Promise.race([asked, gone, new Promise(ok => setTimeout(() => ok(undefined), 5000))]);
        if (dialog === undefined) return { error: 'the page neither closed nor asked to stay within 5 s of closing it' };
        if (dialog === null) return { closed: true, delivery: 'browser-window', native: 'page.close' };
        await dialog.dismiss(); await frame();
        return { closed: false, kept: 'a `beforeunload` called `preventDefault()`: the browser asked to leave, answered "Stay"', delivery: 'browser-window', native: 'page.close' };
      }
      if (req.op === 'clock' && await page.evaluate(() => typeof ImageDecoder === 'undefined' && [...document.images].some(i => /\.(gif|webp)(?:[?#]|$)/i.test(i.currentSrc || i.src)))) throw new Error(`${name} clock refuses: this engine has no ImageDecoder, so an animated GIF/WebP would run on wall time`);
      return JSON.parse(await page.evaluate(req => globalThis.exact.agentSettled(req).then(JSON.stringify), req));
    };
    const heldKeys = new Map();
    const emulated = { 'prefers-color-scheme': 'light', 'prefers-reduced-motion': 'no-preference', 'prefers-contrast': 'no-preference' };
    const focus = async (id, select = true) => {
      const r = await ask({ op: 'focus', id, select });
      if (r.error) throw new Error(r.error);
      return r;
    };
    const directFocus = (id, required) => focusForKey(id, required, view => page.evaluate(view => {
      const el = globalThis.exact.views.get(view);
      el?.focus();
      return { ok: document.activeElement === el };
    }, view));
    const browserKey = async (id, opts, requireFocus) => {
      if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new Error(`key: not a phase: ${opts.phase}`);
      const isWorld = await page.evaluate(id => globalThis.exact.gpu?.wantsInput(id) ?? false, id);
      if (isWorld) { const r = await ask({ op: 'focus', id, world: true }); if (r.error || !r.ok) throw new Error(r.error ?? `view ${id} could not take focus`); }
      else await directFocus(id, requireFocus);
      const keys = chordKeys(opts.key), reply = phase => ({ typed: id, key: opts.key, ...(phase != null ? { phase } : {}), delivery: 'platform' });
      const up = async () => { for (const key of [...keys].reverse()) await page.keyboard.up(key); };
      const release = async () => { await up(); heldKeys.delete(opts.key); await frame(); return reply('up'); };
      try {
        for (const phase of opts.phase == null ? ['down', 'up'] : [opts.phase]) {
          if (phase === 'down') { for (const key of keys) await page.keyboard.down(key); heldKeys.set(opts.key, keys); }
          else { await up(); heldKeys.delete(opts.key); }
        }
        await frame();
      } catch (error) { if (opts.phase === 'down') error.release = release; throw error; }
      return { ...reply(opts.phase), ...(opts.phase === 'down' ? { release } : {}) };
    };
    const pointer = playwrightPointer({
      name,
      move: (x, y) => page.mouse.move(x, y),
      down: () => page.mouse.down(),
      up: () => page.mouse.up(),
      wait: (ms) => page.waitForTimeout(ms),
    });
    const carrier = {
      host: 'web', browser: name, phasedTouch: false, boot, hostLines, evaluate, launchFacts: facts,
      async gpuMs() { const ms = await page.locator('#exact-root').getAttribute('data-gpu-ms'); return ms == null ? null : Number(ms); },
      /** A fresh document on this page: its origin's storage emptied, or `keep`ing it (a test's `reload`, mail F19). */
      async reset({ keep = false, failFetch } = {}) {
        await pointer.release();
        for (const keys of heldKeys.values()) for (const key of [...keys].reverse()) await page.keyboard.up(key).catch(() => {});
        heldKeys.clear();
        await page.evaluate(async (keep) => { sessionStorage.clear(); if (keep) return; localStorage.clear(); await Promise.all((await indexedDB.databases?.() ?? []).map(x => x.name && new Promise(ok => { const r = indexedDB.deleteDatabase(x.name); r.onsuccess = r.onerror = r.onblocked = ok; }))); }, keep);
        if (!keep) await context.clearCookies();
        hostLines.length = 0; await page.goto(withFaults(keep ? page.url() : address.href, failFetch), { waitUntil: 'commit' }); this.boot = await waitForBoot(page, 'the reused page never booted');
      },
      ask,
      async reveal(id) { const r = await ask({ op: 'reveal', id }); if (r.scrolled) await frame(); return r; },
      async prefer(media, pageFacts) {
        const unsupported = Object.entries(media).find(([key, value]) => (key === 'prefers-reduced-transparency' && value !== 'no-preference') || (key === 'prefers-contrast' && !['more', 'no-preference'].includes(value)));
        if (unsupported) throw new Error(`${name} prefer cannot emulate ${unsupported[0]} ${unsupported[1]} through Playwright`);
        Object.assign(emulated, media);
        const options = { colorScheme: emulated['prefers-color-scheme'], reducedMotion: emulated['prefers-reduced-motion'], contrast: emulated['prefers-contrast'] };
        if (Object.keys(options).length) { await page.emulateMedia(options); await frame(); }
        const pageReply = Object.keys(pageFacts).length ? await ask({ op: 'prefer', page: pageFacts }) : null;
        if (pageReply?.error) throw new Error(pageReply.error);
        if (pageReply) await frame();
        const current = await page.evaluate(() => {
          const choices = {
            'prefers-reduced-motion': ['reduce', 'no-preference'], 'prefers-reduced-transparency': ['reduce', 'no-preference'],
            'prefers-contrast': ['more', 'less', 'custom', 'no-preference'], 'prefers-color-scheme': ['dark', 'light'],
          };
          return Object.fromEntries(Object.entries(choices).map(([key, values]) => [key, values.find(value => matchMedia(`(${key}: ${value})`).matches) ?? values.at(-1)]));
        });
        return { media: current, ...(pageReply ? { page: pageReply.page } : {}) };
      },
      async input(id, kind, opts) {
        if (kind === 'history') { const reply = await ask({ op: 'tap', id, history: opts.history }); if (reply.error) throw new Error(reply.error); await frame(); return reply; }
        const box = id == null ? null : (await ask({ op: 'layout' })).nodes.find(n => n.id === id);
        if (id != null && (!box || (box.w === 0 && box.h === 0))) throw new Error(`view ${id} has no box on screen`);
        const point = kind === 'contextmenu' ? opts.at : null;
        const x = box ? box.x + (point?.[0] ?? box.w / 2) : undefined, y = box ? box.y + (point?.[1] ?? box.h / 2) : undefined;
        if (kind === 'press' || kind === 'key' || kind === 'type') {
          const request = kind === 'press' ? { op: 'tap', id, selector: opts.selector, x: opts.x, y: opts.y }
            : { op: 'type', id, selector: opts.selector, ...(kind === 'key' ? { key: opts.key } : { text: opts.text }) };
          const guest = await ask(request);
          if (guest.guest === true || guest.handled === true) { if (guest.error) throw new Error(guest.error); await frame(); return { ...guest, at: [x, y] }; }
        }
        if (kind === 'key' && (opts.phase != null || await page.evaluate(id => globalThis.exact.gpu?.wantsInput(id) || globalThis.exact.views.get(id)?.matches('button, a[href], [role="button"], [role="link"]') || false, id))) return browserKey(id, opts, true);
        if (id != null && ['press', 'contextmenu', 'dblclick'].includes(kind)) {
          const why = await page.evaluate(({ id, x, y }) => { const el = globalThis.exact.views.get(id), hit = document.elementFromPoint(x, y); return !el ? null : !hit ? 'its middle is outside the viewport; scroll it into view first' : el === hit || el.contains(hit) || hit.contains(el) ? null : `${hit.dataset?.view ? `node #${hit.dataset.view}` : hit.tagName.toLowerCase()} covers its middle`; }, { id, x, y });
          if (why) throw new Error(`tap #${id} at (${x}, ${y}): ${why}`);
        }
        let deliveredAt = [x, y];
        if (['down', 'move', 'hold', 'up', 'cancel'].includes(kind)) {
          const reply = await pointer(kind, { ...opts, id }, { x, y });
          if (kind !== 'hold') await frame();
          return reply;
        }
        else if (kind === 'wheel') {
          await withHeldKeys(page.keyboard, opts.modifiers, async () => { await page.mouse.move(x, y); await page.mouse.wheel(opts.wheel[0], opts.wheel[1]); }); deliveredAt = [x, y];
          let same = 0, previous = '';
          for (let i = 0; i < 30 && same < 2; i++) {
            await page.evaluate(() => new Promise(requestAnimationFrame));
            const current = await page.evaluate(() => JSON.stringify([scrollX, scrollY, ...[...document.querySelectorAll('[data-scroll=true]')].flatMap(e => [e.scrollLeft, e.scrollTop])]));
            same = current === previous ? same + 1 : 0; previous = current;
          }
        }
        else if (kind === 'hover') await page.mouse.move(x, y);
        else if (kind === 'contextmenu') await page.mouse.click(x, y, { button: 'right' });
        else if (kind === 'dblclick') await page.mouse.dblclick(x, y);
        else if (kind === 'press') await withHeldKeys(page.keyboard, opts.modifiers, () => page.mouse.click(x, y));
        else if (kind === 'type') { await focus(id); await page.keyboard.insertText(opts.text); }
        else if (kind === 'key') return browserKey(id, opts, false);
        else if (kind === 'clipboard') {
          // Playwright has no modifier-bit field, so the chord is two keys.
          // A failed `v` releases the modifier; `deliverClipboard` releases both
          // if the paste event throws. A string `evaluate` is an expression
          // (playwright-core evaluateExpression, isFunction false).
          const modifier = pasteChord().startsWith('Meta') ? 'Meta' : 'Control';
          await deliverClipboard({
            id, opts, ask,
            evaluate: expression => page.evaluate(expression),
            keyDown: async () => {
              await page.keyboard.down(modifier);
              try { await page.keyboard.down('v'); }
              catch (error) { await page.keyboard.up(modifier).catch(() => {}); throw error; }
            },
            keyUp: async () => { await page.keyboard.up('v'); await page.keyboard.up(modifier); },
            insertText: text => page.keyboard.insertText(text),
          });
        }
        else if (kind === 'pinch') throw new Error(`${name} pinch unsupported: Playwright cannot produce trusted phased touches; synthetic dispatchEvent input is not equal input`);
        await frame();
        return { at: kind === 'wheel' ? deliveredAt : [x, y] };
      },
      async screenshot(path) {
        await frame();
        const pending = await page.evaluate(async () => { const on = i => { const b = i.getBoundingClientRect(); return b.bottom > 0 && b.right > 0 && b.top < innerHeight && b.left < innerWidth; }; const left = () => [...document.images].filter(i => !i.complete && on(i)); const end = performance.now() + 3000; while (left().length && performance.now() < end) await new Promise(r => setTimeout(r, 25)); await globalThis.exact.imageFrames?.(); return left().length; });
        await frame(); await page.screenshot({ path, type: 'png' });
        const viewport = page.viewportSize();
        return { screenshot: path, w: viewport.width, h: viewport.height, ...(pending ? { imagesPending: pending } : {}) };
      },
      close,
    };
    return carrier;
  } catch (error) { await close(); throw error; }
}
