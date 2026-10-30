// @exact/motion's conformance suite on the web (LLP 1133.013 §10, §13 stage
// 1): the fixture app (`test/fixture`) made with `exact new` outside the
// repo, depending on this package by `file:`, built for the web, and driven
// in each engine with the web implementation bound. No agent: a substitute
// would prove nothing about the implementation.
//
// Chromium reads the Generic Sensor classes, fed by the DevTools protocol's
// sensor overrides (`Emulation.setSensorOverrideReadings`); Firefox and
// WebKit have no such class and read `devicemotion`, which the suite
// synthesizes (WebKit's constructor is not exposed on the Mac, so there the
// event is a plain `Event` with the fields). Neither proves a browser's own
// sign or units: that is the phone checklist's (`test/phone-checklist.md`).
//
// usage: bun packages/motion/test/web.mjs [chromium] [firefox] [webkit]
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

// The page's side of a synthesized `devicemotion` stream at 60 Hz: `__motion(reading)` sets what it repeats.
const synthesized = () => {
  let reading = null;
  globalThis.__motion = r => { reading = r; };
  setInterval(() => {
    if (!reading) return;
    const fields = { accelerationIncludingGravity: reading.withGravity, acceleration: reading.acceleration, rotationRate: reading.rotationRate, interval: 1000 / 60 };
    let e;
    try { e = new DeviceMotionEvent('devicemotion', fields); } catch { e = Object.assign(new Event('devicemotion'), fields); }
    dispatchEvent(e);
  }, 1000 / 60);
};

const { check, failed } = checker();
// Chromium rounds a Generic Sensor's readings (to 0.1 m/s² for an accelerometer's), so the readings fed are on that grid.
const near = (text, want, within = 1e-6) => {
  const got = String(text).split(' ').map(Number);
  return got.length === want.length && got.every((g, i) => Math.abs(g - want[i]) <= within);
};

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
    if (engine === 'chromium') await context.grantPermissions(['accelerometer', 'gyroscope'], { origin });
    else await context.addInitScript(synthesized);
    // Where `requestPermission` is called: inside the press's dispatch (`window.event` is set only there), with
    // the gesture's activation, as WebKit requires (D11).
    await context.addInitScript(() => {
      const ask = globalThis.DeviceMotionEvent?.requestPermission;
      if (typeof ask !== 'function') return;
      DeviceMotionEvent.requestPermission = function (...a) {
        globalThis.__asked = { event: globalThis.event?.type ?? null, active: navigator.userActivation?.isActive ?? null };
        return ask.apply(this, a);
      };
    });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    let cdp = null;
    // Each sensor's reading, repeated at 60 Hz while `feeding`: Chromium's virtual sensor reports a reading when one is set.
    const readings = { gravity: [0, 0, 9.8], accelerometer: [0, 0, 9.8], gyroscope: [0, 0, 0] };
    let feeding = true;
    if (engine === 'chromium') {
      cdp = await context.newCDPSession(page);
      for (const type of Object.keys(readings)) await cdp.send('Emulation.setSensorOverrideEnabled', { enabled: true, type, metadata: { available: true } });
    }
    const feed = async () => {
      while (feeding) {
        if (cdp) for (const [type, [x, y, z]] of Object.entries(readings)) await cdp.send('Emulation.setSensorOverrideReadings', { type, reading: { xyz: { x, y, z } } }).catch(() => {});
        else {
          const [gx, gy, gz] = readings.gravity, [rx, ry, rz] = readings.gyroscope.map(r => r * 180 / Math.PI);
          await page.evaluate(r => globalThis.__motion?.(r), { withGravity: { x: gx, y: gy, z: gz }, acceleration: { x: 0, y: 0, z: 0 }, rotationRate: { alpha: rz, beta: rx, gamma: ry } }).catch(() => {});
        }
        await new Promise(r => setTimeout(r, cdp ? 1000 / 60 : 100));
      }
    };
    await page.goto(origin);
    const feeder = feed();

    const status = await until(page, 'status', t => t === 'true granted');
    check(engine, 'state granted', status === 'true granted', status);
    const flat = await until(page, 'gravity', t => near(t, [0, 0, 9.8]));
    check(engine, 'face up, gravity reads +z in m/s²', near(flat, [0, 0, 9.8]), flat);
    readings.gravity = [1, 2, 9];
    const tilted = await until(page, 'gravity', t => near(t, [1, 2, 9]));
    check(engine, 'a new reading reaches the reader', near(tilted, [1, 2, 9]), tilted);
    if (engine === 'chromium') {
      // The screen turned a quarter counter-clockwise (`screen.orientation.angle` 90): x follows the screen's right edge.
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 400, deviceScaleFactor: 1, mobile: true, screenOrientation: { type: 'landscapePrimary', angle: 90 } });
      await page.getByTestId('frame').click();
      const screen = await until(page, 'gravity', t => near(t, [-2, 1, 9]));
      check(engine, 'the screen frame at 90° turns x and y', near(screen, [-2, 1, 9]), screen);
      await page.getByTestId('frame').click();
    }
    readings.gyroscope = [0.5, 0, 0];
    const spin = await until(page, 'spin', t => near(t, [0.5, 0, 0], 1e-3));
    check(engine, 'the gyroscope in rad/s', near(spin, [0.5, 0, 0], 1e-3), spin);

    if (engine === 'chromium') {
      // A batch: a 60 Hz reader polled each 100 ms holds about six samples an answer, none dropped.
      await page.waitForTimeout(1000);
      const batches = [];
      for (let i = 0; i < 5; i++) { batches.push(await text(page, 'batch')); await page.waitForTimeout(100); }
      const counts = batches.map(b => b.split(' ').map(Number));
      // The protocol's feed is only about 60 Hz: an answer may find fewer samples, never more than its interval holds.
      check(engine, 'a 64-sample reader at 60 Hz gets the samples since its last answer',
        counts.every(([, n, dropped]) => n <= 10 && dropped === 0) && counts.filter(([, n]) => n >= 2).length >= 3 && batches.every(b => b.endsWith('false')), JSON.stringify(batches));
      const counted = (await text(page, 'counted')).split(' ').map(Number);
      check(engine, 'a data-code reader of a 64-sample batch', counted[0] >= 2 && counted[0] <= 10 && counted[1] === 0, counted.join(' '));
      // Hidden and shown: the next answer marks the gap.
      const visibility = state => page.evaluate(s => { Object.defineProperty(document, 'visibilityState', { value: s, configurable: true }); document.dispatchEvent(new Event('visibilitychange', { bubbles: true })); }, state);
      await visibility('hidden');
      await page.waitForTimeout(300);
      await visibility('visible');
      const gap = await until(page, 'batch', t => t.endsWith('true'));
      check(engine, 'after the page was hidden, the next answer reads interrupted', gap.endsWith('true'), gap);
    }

    await page.getByTestId('ask').click();
    const asked = await until(page, 'asked', t => t !== '');
    check(engine, 'requestMotionPermission answers the state', asked === 'granted', `${asked} ${await text(page, 'asked-error')}`);
    if (engine === 'chromium') {
      const where = await page.evaluate(() => globalThis.__asked);
      check(engine, 'the permission is asked in the press\'s dispatch, with its activation', where?.event != null && where.active === true, JSON.stringify(where));
    }
    check(engine, 'no page errors', !errors.length, errors.join('; '));
    feeding = false;
    await feeder;
    await context.close();
    if (engine === 'chromium') {
      const mobile = await browser.newContext();
      await mobile.addInitScript(() => {
        for (const name of ['Accelerometer', 'LinearAccelerationSensor', 'GravitySensor', 'Gyroscope']) {
          Object.defineProperty(globalThis, name, { value: undefined, configurable: true });
        }
        globalThis.DeviceMotionEvent = class extends Event {
          constructor(type, fields) { super(type); Object.assign(this, fields); }
          static requestPermission() { return Promise.resolve('granted'); }
        };
        globalThis.DeviceOrientationEvent = class extends Event {
          static requestPermission() { return Promise.resolve('granted'); }
        };
        DeviceOrientationEvent.prototype.webkitCompassHeading = 0;
      });
      await mobile.addInitScript(synthesized);
      const phone = await mobile.newPage();
      await phone.goto(origin);
      const denied = await until(phone, 'gravity-error', t => t === 'NotAllowedError');
      check(engine, 'a sensor read before the mobile permission fails', denied === 'NotAllowedError', denied);
      await phone.evaluate(() => __motion({ withGravity: { x: 0, y: 0, z: -9.8 },
        acceleration: { x: 0, y: 0, z: 0 }, rotationRate: { alpha: 0, beta: 0, gamma: 0 } }));
      await phone.getByTestId('ask').click();
      const resumed = await until(phone, 'gravity', t => near(t, [0, 0, 9.8]));
      check(engine, 'granting mobile permission restarts a failed sensor reader', near(resumed, [0, 0, 9.8]), resumed);
      const counted = await until(phone, 'counted', t => !t.startsWith('error') && Number(t.split(' ')[0]) > 0);
      check(engine, 'granting mobile permission restarts a data-code sensor reader',
        !counted.startsWith('error') && Number(counted.split(' ')[0]) > 0, counted);
      await mobile.close();
    }
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
