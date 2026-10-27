// @exact/device's conformance suite on the web (LLP 1133.031 §12 stage 1):
// the fixture app (`test/fixture`) made with `exact new` outside the repo,
// depending on this package by `file:`, built for the web, and driven in
// each engine with the web implementation bound. No agent: a substitute
// would prove nothing about the implementation.
//
// - Chromium answers the page's own objects: headless Chrome's battery is
//   the specification's no-battery one, and desktop Chrome gives no `type`
//   (the module answers `unknown`) and does not take one from the DevTools
//   protocol; `saveData` is set through it (`Emulation.setDataSaverOverride`).
//   The suite prints what this Chrome gives, for the RFC's record.
// - A level is answered in hundredths, and a change finer than that
//   announces nothing; a `type`'s change is announced, and a type the
//   interface does not name is `unknown` (a battery and a connection whose
//   events the suite raises, as Chrome on Android gives a `type`).
// - The last reader gone, each source's listeners are removed.
// - Firefox and WebKit have neither API: all three sources fail
//   `NotSupportedError`.
// - `powerState` fails `NotSupportedError` everywhere on the web.
//
// usage: bun packages/device/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

// The build writes under the app's target directory, which CARGO_TARGET_DIR moves (scripts/app.mjs).
const dist = join(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(app, 'target'), 'web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;

// Counts the listeners added to and removed from the page's battery and connection, for the release check.
const count = () => {
  globalThis.__device = { added: 0, removed: 0 };
  for (const proto of [globalThis.BatteryManager?.prototype, globalThis.NetworkInformation?.prototype].filter(Boolean)) {
    const add = proto.addEventListener ?? EventTarget.prototype.addEventListener, remove = proto.removeEventListener ?? EventTarget.prototype.removeEventListener;
    proto.addEventListener = function (...a) { globalThis.__device.added++; return add.apply(this, a); };
    proto.removeEventListener = function (...a) { globalThis.__device.removed++; return remove.apply(this, a); };
  }
};

// A battery whose level and charging the suite sets, raising the spec's events: `__battery.set(level, charging)`.
const fakeBattery = () => {
  const battery = new EventTarget();
  Object.assign(battery, { level: 0.5, charging: false });
  globalThis.__battery = {
    reads: 0,
    set(level, charging) {
      if (battery.level !== level) { battery.level = level; battery.dispatchEvent(new Event('levelchange')); }
      if (battery.charging !== charging) { battery.charging = charging; battery.dispatchEvent(new Event('chargingchange')); }
    },
  };
  navigator.getBattery = () => { globalThis.__battery.reads++; return Promise.resolve(battery); };
  // A connection whose `type` the suite sets, as Chrome on Android and ChromeOS gives one: `__connection.set(type)`.
  const connection = new EventTarget();
  Object.assign(connection, { type: 'cellular', saveData: false });
  Object.defineProperty(navigator, 'connection', { value: connection, configurable: true });
  globalThis.__connection = { set(type) { connection.type = type; connection.dispatchEvent(new Event('change')); } };
};

const { check, failed } = checker();

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(100); }
    return got;
  };
  try {
    const context = await browser.newContext();
    await context.addInitScript(count);
    const page = await context.newPage();
    await page.goto(origin);
    const power = await until(page, 'power', t => t !== '' && !t.startsWith('('));
    check(engine, 'powerState fails NotSupportedError', power === 'NotSupportedError', power);
    if (engine === 'chromium') {
      const battery = await until(page, 'battery', t => t !== '' && !t.startsWith('('));
      const real = await page.evaluate(async () => { const b = await navigator.getBattery(); return `${b.level} ${b.charging}`; });
      check(engine, 'getBattery answers the page\'s battery', battery === real && battery === '1 true', `${battery} beside the page's ${real}`);
      const network = await until(page, 'network', t => t !== '' && !t.startsWith('('));
      const given = await page.evaluate(() => `${navigator.connection.type ?? 'undefined'} ${navigator.connection.saveData}`);
      console.log(`RECORD chromium desktop: navigator.connection.type ${given.split(' ')[0]}, saveData ${given.split(' ')[1]}; the module answers "${network}"`);
      check(engine, 'connection answers, unknown where Chrome gives no type', network === `${given.split(' ')[0] === 'undefined' ? 'unknown' : given.split(' ')[0]} ${given.split(' ')[1]}`, `${network} beside ${given}`);

      // Desktop Chromium has no `type`, and the protocol's `connectionType` does not set one; a throttle changes
      // `downlink` and raises `change`, which answers nothing new. Data Saver is set through the protocol.
      const cdp = await context.newCDPSession(page);
      await cdp.send('Network.enable');
      await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 100, downloadThroughput: 50000, uploadThroughput: 50000, connectionType: 'cellular3g' });
      await page.waitForTimeout(300);
      const throttled = await page.evaluate(() => `${navigator.connection.type ?? 'undefined'} ${navigator.connection.downlink}`);
      console.log(`RECORD chromium: with connectionType cellular3g and a throttle, type ${throttled.split(' ')[0]}, downlink ${throttled.split(' ')[1]}`);
      check(engine, 'a change to what the module does not answer leaves its answer', await text(page, 'network') === network, await text(page, 'network'));
      let saveData = 'not offered';
      try {
        await cdp.send('Emulation.setDataSaverOverride', { dataSaverEnabled: true });
        saveData = await page.evaluate(() => String(navigator.connection.saveData));
      } catch (e) { saveData = `refused (${e.message.split('\n')[0]})`; }
      console.log(`RECORD chromium: Emulation.setDataSaverOverride sets saveData: ${saveData}`);
      if (saveData === 'true') {
        // The override raises no `change` of its own; the one a real switch raises is raised here.
        await page.evaluate(() => navigator.connection.dispatchEvent(new Event('change')));
        const saving = await until(page, 'network', t => t.endsWith('true'));
        check(engine, 'saveData set through the protocol is announced', saving === 'unknown true', saving);
      }

      // The last reader gone: every listener the module added is removed.
      await page.getByTestId('toggle').click();
      await until(page, 'battery', t => t === 'off');
      await page.waitForTimeout(300);
      const listeners = await page.evaluate(() => globalThis.__device);
      check(engine, 'the last reader gone, the listeners are removed', listeners.added === 3 && listeners.removed === 3, JSON.stringify(listeners));
      await context.close();

      // Hundredths: a battery whose events the suite raises.
      const faked = await browser.newContext();
      await faked.addInitScript(fakeBattery);
      const fake = await faked.newPage();
      await fake.goto(origin);
      check(engine, 'a level is answered in hundredths', await until(fake, 'battery', t => t === '0.5 false') === '0.5 false');
      const reads = await fake.evaluate(() => { globalThis.__battery.set(0.504, false); return globalThis.__battery.reads; });
      await fake.waitForTimeout(500);
      const after = await fake.evaluate(() => globalThis.__battery.reads);
      check(engine, 'a change within a hundredth announces nothing', after === reads && await text(fake, 'battery') === '0.5 false', `${reads} reads, then ${after}`);
      await fake.evaluate(() => globalThis.__battery.set(0.42, true));
      const moved = await until(fake, 'battery', t => t === '0.42 true');
      check(engine, 'a level and a charger change are announced', moved === '0.42 true', moved);
      check(engine, 'a connection\'s type is answered', await until(fake, 'network', t => t === 'cellular false') === 'cellular false');
      await fake.evaluate(() => globalThis.__connection.set('wifi'));
      check(engine, 'a type\'s change is announced', await until(fake, 'network', t => t === 'wifi false') === 'wifi false');
      await fake.evaluate(() => globalThis.__connection.set('satellite'));
      check(engine, 'a type the interface does not name is unknown', await until(fake, 'network', t => t === 'unknown false') === 'unknown false');
      await faked.close();
    } else {
      const errors = await Promise.all(['battery', 'network'].map(id => until(page, id, t => t !== '' && !t.startsWith('('))));
      check(engine, 'getBattery and connection fail NotSupportedError', errors.every(e => e === 'NotSupportedError'), errors.join(' '));
      await context.close();
    }
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
