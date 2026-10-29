// @exact/gamepad's conformance suite on the web (LLP 1133.022 §10): the
// fixture app (`test/fixture`) made with `exact new` outside the repo,
// depending on this package by `file:`, built for the web, and driven in
// each engine with the web implementation bound. No agent: a substitute
// would prove nothing about the implementation.
//
// The Gamepad specification has no automation for pads, so the page's
// `navigator.getGamepads` is replaced before the app loads by pads the suite
// connects, presses and disconnects, each with a `vibrationActuator` that
// records what it was asked and settles as Chromium's does: `complete`
// after its span, `preempted` when another effect or a reset reaches the
// pad first. It proves the implementation's reading of the API, not a
// browser's own; that is the device checklist's (`test/device-checklist.md`).
//
// usage: bun packages/gamepad/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;

// The page's pads: `__pads.connect(index, id, rumbles)`, `.press(index, button)`,
// `.disconnect(index)`, and `.played`, what their motors were asked.
const fakePads = () => {
  const slots = [null, null, null, null], played = [];
  const event = (type, gamepad) => {
    const e = new Event(type);
    Object.defineProperty(e, 'gamepad', { value: gamepad });
    dispatchEvent(e);
  };
  const actuator = index => {
    let running = null;
    const end = how => { if (running) { const r = running; running = null; clearTimeout(r.timer); r.resolve(how); } };
    return {
      effects: ['dual-rumble'],
      playEffect(type, params) {
        end('preempted');
        played.push({ index, type, ...params });
        return new Promise(resolve => {
          running = { resolve, timer: setTimeout(() => end('complete'), (params.startDelay ?? 0) + (params.duration ?? 0)) };
        });
      },
      reset() { end('preempted'); played.push({ index, type: 'reset' }); return Promise.resolve('complete'); },
    };
  };
  globalThis.__pads = {
    played,
    connect(index, id, rumbles) {
      const buttons = Array.from({ length: 17 }, () => ({ pressed: false, touched: false, value: 0 }));
      const pad = { index, id, mapping: 'standard', connected: true, timestamp: performance.now(), buttons, axes: [0, 0, 0, 0], vibrationActuator: rumbles ? actuator(index) : null };
      slots[index] = pad;
      event('gamepadconnected', pad);
    },
    press(index, button) {
      const pad = slots[index];
      pad.buttons = pad.buttons.map((b, i) => (i === button ? { pressed: true, touched: true, value: 1 } : { pressed: false, touched: false, value: 0 }));
      pad.timestamp = performance.now();
      setTimeout(() => { pad.buttons = pad.buttons.map(() => ({ pressed: false, touched: false, value: 0 })); }, 200);
    },
    disconnect(index) {
      const pad = slots[index];
      slots[index] = null;
      pad.connected = false;
      event('gamepaddisconnected', pad);
    },
  };
  Object.defineProperty(Navigator.prototype, 'getGamepads', { configurable: true, value: () => slots.slice() });
};

const { check, failed } = checker();

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(50); }
    return got;
  };
  try {
    const context = await browser.newContext();
    await context.addInitScript(fakePads);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin);
    const pads = (f, ...a) => page.evaluate(([f, a]) => globalThis.__pads[f](...a), [f, a]);

    const none = await until(page, 'list', t => t === 'true 0 none');
    check(engine, 'no pad: available, an empty list', none === 'true 0 none', none);

    await pads('connect', 0, 'DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)', true);
    await pads('connect', 1, 'Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b13)', true);
    const listed = await until(page, 'pads', t => t.split(' ').length === 2);
    check(engine, 'two pads, each by its family with its labels and effects',
      listed === '0:playstation:Square:dual-rumble 1:xbox:X:dual-rumble', listed);
    const came = await until(page, 'came', t => t.trim().split(' ').length === 2);
    check(engine, 'gamepadconnected for each', came.trim() === '0:playstation 1:xbox', came);

    await page.getByTestId('buzz').click();
    const unused = await until(page, 'played-error', t => t !== '');
    check(engine, 'no pad in use: a send for the current pad fails NotFoundError', unused === 'NotFoundError', unused);

    await pads('press', 1, 2);
    const inUse = await until(page, 'list', t => t.endsWith(' 1'));
    check(engine, 'a press makes its pad the current one', inUse === 'true 2 1', inUse);

    await page.getByTestId('buzz').click();
    const done = await until(page, 'played', t => t === 'complete');
    check(engine, 'dual-rumble on the current pad completes', done === 'complete', done);
    const asked = await page.evaluate(() => globalThis.__pads.played);
    check(engine, 'its motors were asked as the call said',
      asked.length === 1 && asked[0].index === 1 && asked[0].duration === 200 && asked[0].strongMagnitude === 1 && asked[0].weakMagnitude === 0.5, JSON.stringify(asked));

    await page.getByTestId('long').click();
    await page.waitForTimeout(200);
    await page.getByTestId('stop').click();
    const preempted = await until(page, 'played', t => t === 'preempted');
    check(engine, 'a reset preempts the running effect', preempted === 'preempted', preempted);
    const stopped = await until(page, 'stopped', t => t !== '');
    check(engine, 'and completes itself', stopped === 'complete', stopped);

    await pads('disconnect', 1);
    const went = await until(page, 'went', t => t.trim() === '1');
    check(engine, 'gamepaddisconnected, and the list follows', went.trim() === '1' && (await until(page, 'list', t => t === 'true 1 none')) === 'true 1 none', `${went} ${await text(page, 'list')}`);

    check(engine, 'no page errors', !errors.length, errors.join('; '));
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
