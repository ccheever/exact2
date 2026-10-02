// Firefox and WebKit carrier for scripts/agent.mjs. Chrome deliberately stays
// in agent.mjs on its direct CDP pipe; importing Playwright is deferred until
// one of these browsers is selected.
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { launchFacts, refuseStale, warnStale, webChanges } from './agent-launch.mjs';
import { builtAppMatches, jsTargetBuild, serveBuildTree, serveStatic } from '../host/web/serve.mjs';
import { resolveApp, webDist as defaultWebDist } from './app.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
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

const quote = value => "'" + String(value).replaceAll("'", "'\\''") + "'";
async function assertDist(dist, app) {
  if (!await builtAppMatches(dist, app)) throw new Error(`web dist is not a complete build for selected app ${app.id}; stale receipt ${resolve(dist, '.exact-build.json')}; run EXACT_APP_DIR=${quote(app.dir)} EXACT_WEB_DIST=${quote(resolve(dist))} bun host/web/build.mjs ${app.crate('web')}`);
}

async function files({ plan, pageURL, app, webDist }) {
  const selected = resolveApp(app), dist = resolve(webDist ?? defaultWebDist());
  if (!pageURL) {
    await assertDist(dist, selected);
    const js = jsTargetBuild(dist), env = process.env.EXACT_APP_DIR || webDist || process.env.EXACT_WEB_DIST ? `EXACT_APP_DIR=${selected.dir} EXACT_WEB_DIST=${dist} ` : '';
    const command = `${env}bun host/web/build.mjs ${selected.crate('web')}${js ? '' : ' --wasm'}`, changed = webChanges(dist, selected);
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
  // Playwright exposes no phased touchscreen API outside Chromium. For an
  // app-owned contact (touch-action is not auto), dispatch matching touch
  // and pointer records with the agent clock. Install before app listeners.
  globalThis.__exactAgentTouchTime = null;
  for (const type of ['pointerdown','pointermove','pointerup','pointercancel','touchstart','touchmove','touchend','touchcancel']) addEventListener(type, event => {
    if ((event.pointerType === 'touch' || type.startsWith('touch')) && Number.isFinite(globalThis.__exactAgentTouchTime)) {
      try { Object.defineProperty(event, 'timeStamp', { value: globalThis.__exactAgentTouchTime }); } catch {}
    }
  }, true);
  // These engines' Playwright protocols expose a trusted tap but no phased
  // touch. A recognized contact is used only where CSS or pointer capture has
  // taken native scrolling out of the gesture. Give that
  // contact pointer-capture semantics without changing real pointers.
  const captures = new Map(), synthetic = new Set(), active = new Map(), targets = new Map();
  globalThis.__exactAgentSyntheticPointers = { captures, synthetic, active, targets };
  const set = Element.prototype.setPointerCapture, has = Element.prototype.hasPointerCapture, release = Element.prototype.releasePointerCapture;
  Element.prototype.setPointerCapture = function(id) { if (synthetic.has(id)) captures.set(id, this); else return set.call(this, id); };
  Element.prototype.hasPointerCapture = function(id) { return synthetic.has(id) ? captures.get(id) === this : has.call(this, id); };
  Element.prototype.releasePointerCapture = function(id) { if (synthetic.has(id)) captures.delete(id); else return release.call(this, id); };
`;

async function waitForBoot(page, why = 'the page never booted') {
  try { await page.waitForFunction(() => document.getElementById('exact-root')?.dataset.bootMs != null, null, { timeout: 30000 }); }
  catch { throw new Error(why); }
  await page.evaluate(() => globalThis.exact.ready);
  return Number(await page.locator('#exact-root').getAttribute('data-boot-ms'));
}

const keyName = code => {
  if (code.length === 1) return code;
  if (/^Key[A-Z]$/.test(code)) return code;
  if (/^Digit[0-9]$/.test(code)) return code;
  const key = { Space: ' ', Enter: 'Enter', Escape: 'Escape', Tab: 'Tab', Backspace: 'Backspace', ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown', ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight', Shift: 'Shift', ShiftLeft: 'ShiftLeft', ShiftRight: 'ShiftRight' }[code];
  if (!key) throw new Error(`key: unsupported code ${code}`);
  return key;
};

/** Open Firefox or WebKit through Playwright. The page still owns Exact's
 * deterministic runner/motion clock; Playwright carries only browser IO. */
export async function openPlaywrightWeb({ browser: name, plan, world, size, url: pageURL, app, webDist, onProcess, reuse, storage, facts: givenFacts }) {
  if (!['firefox', 'webkit'].includes(name)) throw new Error(`browser: chrome, firefox or webkit, not ${name}`);
  const facts = givenFacts ?? launchFacts({});
  if (reuse) await reuse.close(); // Chrome reuse is intentionally not crossed with another engine.
  const hosted = await files({ plan, pageURL, app, webDist });
  let browserServer, browser, context, page;
  const hostLines = [];
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
    context = await browser.newContext({ viewport: { width: size[0], height: size[1] }, screen: { width: size[0], height: size[1] }, deviceScaleFactor: 1, hasTouch: true, colorScheme: 'light', reducedMotion: 'no-preference', contrast: 'no-preference' });
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
    await page.addInitScript(init);
    if (world && !plan) {
      const encoded = worldFile(world).toString('base64');
      await page.addInitScript(value => { globalThis.exactWorldCarry = Uint8Array.from(atob(value), c => c.charCodeAt(0)); }, encoded);
    }
    const address = new URL(hosted.url);
    address.searchParams.set('agent', '1');
    for (const [key, value] of Object.entries(facts)) address.searchParams.set(key, value);
    if (storage !== undefined) address.searchParams.set('storage', storage);
    await page.goto(address.href);
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
      if (req.op === 'clock' && await page.evaluate(() => typeof ImageDecoder === 'undefined' && [...document.images].some(i => /\.(gif|webp)(?:[?#]|$)/i.test(i.currentSrc || i.src)))) throw new Error(`${name} clock refuses: this engine has no ImageDecoder, so an animated GIF/WebP would run on wall time`);
      return JSON.parse(await page.evaluate(req => globalThis.exact.agentSettled(req).then(JSON.stringify), req));
    };
    let contact = null;
    const heldKeys = new Map();
    const focus = async (id, select = true) => {
      const r = await ask({ op: 'focus', id, select });
      if (r.error) throw new Error(r.error);
      return r;
    };
    const directFocus = async id => {
      const result = await page.evaluate(id => {
        const el = globalThis.exact.views.get(id);
        el?.focus();
        return { ok: document.activeElement === el };
      }, id);
      if (!result.ok) throw new Error(`view ${id} could not take focus`);
    };
    const browserKey = async (id, opts) => {
      if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new Error(`key: not a phase: ${opts.phase}`);
      const isWorld = await page.evaluate(id => globalThis.exact.gpu?.wantsInput(id) ?? false, id);
      if (isWorld) { const r = await ask({ op: 'focus', id, world: true }); if (r.error || !r.ok) throw new Error(r.error ?? `view ${id} could not take focus`); }
      else await directFocus(id);
      const key = keyName(opts.key), reply = phase => ({ typed: id, key: opts.key, ...(phase != null ? { phase } : {}), delivery: 'platform' });
      const release = async () => { await page.keyboard.up(key); heldKeys.delete(opts.key); await frame(); return reply('up'); };
      try {
        for (const phase of opts.phase == null ? ['down', 'up'] : [opts.phase]) {
          await page.keyboard[phase](key);
          if (phase === 'down') heldKeys.set(opts.key, key); else heldKeys.delete(opts.key);
        }
        await frame();
      } catch (error) { if (opts.phase === 'down') error.release = release; throw error; }
      return { ...reply(opts.phase), ...(opts.phase === 'down' ? { release } : {}) };
    };
    const syntheticTouch = async (type, points, time) => page.evaluate(({ type, points, time }) => {
      globalThis.__exactAgentTouchTime = time;
      const state = globalThis.__exactAgentSyntheticPointers;
      const changed = [], eventTargets = [];
      for (const point of points) {
        if (type === 'pointerdown') state.synthetic.add(point.id);
        const target = state.captures.get(point.id) ?? state.targets.get(point.id) ?? document.elementFromPoint(point.x, point.y);
        if (!target) throw new Error(`touch point (${point.x}, ${point.y}) is outside the viewport`);
        if (type === 'pointerdown') state.targets.set(point.id, target);
        eventTargets.push(target);
        const event = new PointerEvent(type, { bubbles: true, composed: true, cancelable: true, pointerId: point.id, pointerType: 'touch', isPrimary: point.primary, button: type === 'pointerdown' ? 0 : -1, buttons: type === 'pointerup' || type === 'pointercancel' ? 0 : 1, clientX: point.x, clientY: point.y, width: 1, height: 1, pressure: type === 'pointerup' || type === 'pointercancel' ? 0 : 0.5 });
        try { Object.defineProperty(event, 'timeStamp', { value: time }); } catch {}
        target.dispatchEvent(event);
        const touch = new Touch({ identifier: point.id, target, clientX: point.x, clientY: point.y, screenX: point.x, screenY: point.y, pageX: point.x + scrollX, pageY: point.y + scrollY, radiusX: 1, radiusY: 1, force: type === 'pointerup' || type === 'pointercancel' ? 0 : 0.5 });
        changed.push(touch);
        if (type === 'pointerup' || type === 'pointercancel') state.active.delete(point.id); else state.active.set(point.id, touch);
      }
      const touchType = {pointerdown:'touchstart',pointermove:'touchmove',pointerup:'touchend',pointercancel:'touchcancel'}[type];
      const touches = [...state.active.values()], touchEvent = new TouchEvent(touchType, { bubbles:true, composed:true, cancelable:true, touches, targetTouches:touches.filter(touch => eventTargets[0].contains(touch.target)), changedTouches:changed });
      try { Object.defineProperty(touchEvent, 'timeStamp', { value: time }); } catch {}
      eventTargets[0].dispatchEvent(touchEvent);
      if (type === 'pointerup' || type === 'pointercancel') for (const point of points) { state.captures.delete(point.id); state.synthetic.delete(point.id); state.targets.delete(point.id); }
    }, { type, points, time });
    const phasedTouch = async (type, points, time) => {
      await syntheticTouch(type, points, time);
    };
    const carrier = {
      host: 'web', browser: name, boot, hostLines, evaluate, launchFacts: facts,
      async gpuMs() { const ms = await page.locator('#exact-root').getAttribute('data-gpu-ms'); return ms == null ? null : Number(ms); },
      async reset() {
        if (contact) {
          await syntheticTouch('pointercancel', [{ id: 31, x: contact.x, y: contact.y, primary: true }], contact.t).catch(() => {});
          contact = null;
        }
        for (const key of heldKeys.values()) await page.keyboard.up(key).catch(() => {});
        heldKeys.clear();
        await page.evaluate(async () => { sessionStorage.clear(); localStorage.clear(); await Promise.all((await indexedDB.databases?.() ?? []).map(x => x.name && new Promise(ok => { const r = indexedDB.deleteDatabase(x.name); r.onsuccess = r.onerror = r.onblocked = ok; }))); });
        await context.clearCookies(); hostLines.length = 0; await page.goto(address.href); this.boot = await waitForBoot(page, 'the reused page never booted');
      },
      ask,
      async prefer(media, pageFacts) {
        const unsupported = Object.entries(media).find(([key, value]) => (key === 'prefers-reduced-transparency' && value !== 'no-preference') || (key === 'prefers-contrast' && !['more', 'no-preference'].includes(value)));
        if (unsupported) throw new Error(`${name} prefer cannot emulate ${unsupported[0]} ${unsupported[1]} through Playwright`);
        const options = {};
        if (media['prefers-color-scheme']) options.colorScheme = media['prefers-color-scheme'];
        if (media['prefers-reduced-motion']) options.reducedMotion = media['prefers-reduced-motion'];
        if (media['prefers-contrast']) options.contrast = media['prefers-contrast'];
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
        const x = box ? box.x + box.w / 2 : contact?.x, y = box ? box.y + box.h / 2 : contact?.y;
        if (kind === 'press' || kind === 'key' || kind === 'type') {
          const request = kind === 'press' ? { op: 'tap', id, selector: opts.selector, x: opts.x, y: opts.y }
            : { op: 'type', id, selector: opts.selector, ...(kind === 'key' ? { key: opts.key } : { text: opts.text }) };
          const guest = await ask(request);
          if (guest.guest === true || guest.handled === true) { if (guest.error) throw new Error(guest.error); await frame(); return { ...guest, at: [x, y] }; }
        }
        if (kind === 'key' && (opts.phase != null || await page.evaluate(id => globalThis.exact.gpu?.wantsInput(id) || globalThis.exact.views.get(id)?.matches('button, a[href], [role="button"], [role="link"]') || false, id))) return browserKey(id, opts);
        let deliveredAt = [x, y];
        if (kind === 'down') {
          if (contact) throw new Error('a contact is already down; use `tap up` first');
          const px = opts.x ?? x, py = opts.y ?? y;
          const viewport = page.viewportSize();
          if (px < 0 || py < 0 || px >= viewport.width || py >= viewport.height) throw new Error(`${name} held touch unsupported: its contact point (${px}, ${py}) is outside the viewport`);
          const clock = (await ask({ op: 'tags' })).clock;
          contact = { x: px, y: py, t: clock, elapsed: 0 };
          await phasedTouch('pointerdown', [{ id: 31, x: px, y: py, primary: true }], clock);
          const owns = await page.evaluate(({ id, x, y }) => {
            if (globalThis.__exactAgentSyntheticPointers.captures.has(id)) return true;
            for (let e = document.elementFromPoint(x, y); e; e = e.parentElement) if (getComputedStyle(e).touchAction !== 'auto') return true;
            return false;
          }, { id: 31, x: px, y: py });
          if (!owns) {
            await syntheticTouch('pointercancel', [{ id: 31, x: px, y: py, primary: true }], clock);
            contact = null;
            throw new Error(`${name} held touch unsupported: Playwright cannot phase the native scrolling contact at this point`);
          }
          deliveredAt = [px, py];
        }
        else if (kind === 'move') {
          if (!contact) throw new Error('no contact is down');
          const to = { x: opts.x ?? contact.x + (opts.dx ?? 0), y: opts.y ?? contact.y + (opts.dy ?? 0) }, steps = Math.max(1, Math.round(Math.max(0, opts.ms ?? 0) / 16));
          const from = contact;
          for (let i = 1; i <= steps; i++) {
            contact.elapsed += (opts.ms || 16) / steps;
            await phasedTouch('pointermove', [{ id: 31, x: from.x + (to.x - from.x) * i / steps, y: from.y + (to.y - from.y) * i / steps, primary: true }], contact.t + contact.elapsed);
          }
          contact.x = to.x; contact.y = to.y; deliveredAt = [to.x, to.y];
        } else if (kind === 'hold') { if (!contact) throw new Error('no contact is down'); contact.elapsed += opts.ms ?? 0; deliveredAt = [contact.x, contact.y]; }
        else if (kind === 'cancel') {
          if (!contact) throw new Error('no contact is down');
          deliveredAt = [contact.x, contact.y]; await syntheticTouch('pointercancel', [{ id: 31, x: contact.x, y: contact.y, primary: true }], contact.t + contact.elapsed + 8); contact = null;
        }
        else if (kind === 'up') { if (!contact) throw new Error('no contact is down'); deliveredAt = [contact.x, contact.y]; await phasedTouch('pointerup', [{ id: 31, x: contact.x, y: contact.y, primary: true }], contact.t + contact.elapsed + 8); contact = null; }
        else if (kind === 'wheel') {
          await page.mouse.move(x, y); await page.mouse.wheel(opts.wheel[0], opts.wheel[1]); deliveredAt = [x, y];
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
        else if (kind === 'press') await page.mouse.click(x, y);
        else if (kind === 'type') { await focus(id); await page.keyboard.insertText(opts.text); }
        else if (kind === 'key') return browserKey(id, opts);
        else if (kind === 'pinch') {
          const [cx, cy] = opts.at ? [box.x + opts.at[0], box.y + opts.at[1]] : [x, y], d = Math.max(8, Math.min(box.w, box.h) * 0.3), clock = (await ask({ op: 'tags' })).clock;
          const owns = await page.evaluate(({ x, y }) => { for (let e = document.elementFromPoint(x, y); e; e = e.parentElement) if (getComputedStyle(e).touchAction !== 'auto') return true; return false; }, { x: cx, y: cy });
          if (!owns) throw new Error(`${name} pinch unsupported: Playwright cannot phase the native scrolling contacts at this point`);
          const fingers = k => [{ id: 41, x: cx - d * k / 2, y: cy, primary: true }, { id: 42, x: cx + d * k / 2, y: cy, primary: false }];
          await phasedTouch('pointerdown', fingers(1), clock);
          for (let i = 1; i <= 8; i++) await phasedTouch('pointermove', fingers(1 + (opts.pinch - 1) * i / 8), clock + i * 16);
          await phasedTouch('pointerup', fingers(opts.pinch), clock + 136);
          await frame(); return { pinch: opts.pinch, at: [cx, cy], delivery: 'recognized' };
        }
        await frame();
        return ['down', 'move', 'hold', 'up', 'cancel'].includes(kind) ? { phase: kind, at: deliveredAt, delivery: 'recognized' } : { at: kind === 'wheel' ? deliveredAt : [x, y] };
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
