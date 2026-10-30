// @exact/nfc's conformance suite on the web (LLP 1133.035 §12 stage 1): the
// fixture app built for the web and driven in each engine with the web
// implementation bound, as in production.
//
// Only Chrome on Android ships `NDEFReader`, so in Chromium the page gets a
// fake one before it loads: `__nfc.tap(records, serial)` brings a tag into
// the field, which settles the waiting write or lock and is read by every
// scanning reader. Firefox and WebKit have none and answer `available` false
// and `NotSupportedError`.
//
// usage: bun packages/nfc/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from '../../contacts/test/fixture.mjs';
import { fake } from './fake.mjs';

const args = process.argv.slice(2), engines = args.length ? args : ['chromium', 'firefox', 'webkit'];
const { check, failed } = checker();
const fixture = makeFixture('nfc');

run('bun', ['exact.mjs', 'web-build', '--render', 'none'], fixture.app);
const dist = join(fixture.app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
// 127.0.0.1 is a secure context, which Web NFC needs.
const origin = `http://127.0.0.1:${server.port}`;
const hex = s => [...new TextEncoder().encode(s)].map(b => b.toString(16).padStart(2, '0')).join('');
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = async (page, id) => (await page.getByTestId(id).allInnerTexts().catch(() => []))[0] ?? '';
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(50); }
    return got;
  };
  const tap = (page, records, serial) => page.evaluate(([r, s]) => globalThis.__nfc.tap(r, s), [records, serial]);
  try {
    const context = await browser.newContext(), page = await context.newPage(), errors = [];
    page.on('pageerror', e => errors.push(e.message));
    const faked = engine === 'chromium';
    if (faked) await page.addInitScript(fake);
    await page.goto(origin);
    if (!faked) {
      const status = await until(page, 'status', t => t.startsWith('false'));
      check(engine, 'no NDEFReader: unavailable', status === 'false unknown denied app', status);
      await page.getByTestId('scan').click();
      const e = await until(page, 'scan-error', t => t !== '');
      check(engine, 'a scan fails NotSupportedError', e.startsWith('NotSupportedError'), e);
      await page.getByTestId('write').click();
      const w = await until(page, 'write-error', t => t !== '');
      check(engine, 'a write fails NotSupportedError', w.startsWith('NotSupportedError'), w);
    } else {
      const status = await until(page, 'status', t => t.startsWith('true'));
      check(engine, 'status: available, radio unknown, granted, the app\'s UI', status === 'true unknown granted app', status);

      // A `once` scan ends with the reading that ended it; the root hears it.
      await page.getByTestId('scan').click();
      await until(page, 'scanned', t => t === 'scanning');
      await page.waitForFunction(() => globalThis.__nfc.scanning() === 1);
      await tap(page, [{ recordType: 'url', data: 'https://example.com/a' }], '04:a1:b2');
      const heard = await until(page, 'heard', t => t !== 'none');
      check(engine, 'the root hears the reading, its URL decoded', heard === 'r1 04:a1:b2 url=https://example.com/a', heard);
      const ended = await until(page, 'scanned', t => t.startsWith('read'));
      check(engine, 'a once scan ends `read` with the reading', ended === 'read r1', ended);
      check(engine, 'the reader stops with the last scan', await page.evaluate(() => globalThis.__nfc.scanning()) === 0, '');

      // Data code reads the reading's records with their bytes.
      await page.getByTestId('records').click();
      const bytes = await until(page, 'record-bytes', t => t !== '-');
      check(engine, 'ndefRecords answers the bytes', bytes === `url=${hex('https://example.com/a')}`, bytes);

      // A scan that is not `once` hears each tag until it is stopped.
      await page.getByTestId('scan-all').click();
      await until(page, 'scanned', t => t === 'scanning');
      await page.waitForFunction(() => globalThis.__nfc.scanning() === 1);
      await tap(page, [{ recordType: 'text', data: 'shelf', encoding: 'utf-8', lang: 'en' }], '04:c3');
      const second = await until(page, 'heard', t => t.startsWith('r2'));
      check(engine, 'a text record reads as its text', second === 'r2 04:c3 text=shelf', second);
      await page.evaluate(() => globalThis.__nfc.unreadable());
      const unread = await until(page, 'unread', t => t !== 'none');
      check(engine, 'a tag with no NDEF is a readingerror', /^t\d+$/.test(unread), unread);
      check(engine, 'the scan still waits', await text(page, 'scanned') === 'scanning', await text(page, 'scanned'));
      await page.getByTestId('stop-scan').click();
      const stopped = await until(page, 'scanned', t => t !== 'scanning');
      check(engine, 'abortScan ends the scan `stopped`', stopped === 'stopped', stopped);

      // A write waits for the next tag and writes the message.
      await page.getByTestId('write').click();
      await until(page, 'written', t => t === 'writing');
      await tap(page, [], '04:d4');
      const written = await until(page, 'written', t => t.startsWith('written'));
      check(engine, 'a write settles against the tag', written.startsWith('written'), written);
      const [first] = await page.evaluate(() => globalThis.__nfc.written());
      check(engine, 'the message is the URL record', first?.kind === 'write' && first.records[0]?.recordType === 'url' && new TextDecoder().decode(new Uint8Array(first.records[0].data)) === 'https://example.com/fixture', JSON.stringify(first));
      check(engine, 'a written tag is not also read', await text(page, 'heard') === second, await text(page, 'heard'));

      // abortWrite fails the waiting write AbortError.
      await page.getByTestId('write').click();
      await until(page, 'written', t => t === 'writing');
      await page.getByTestId('stop-write').click();
      const aborted = await until(page, 'write-error', t => t !== '');
      check(engine, 'abortWrite fails the write AbortError', aborted.startsWith('AbortError'), aborted);

      // Data code writes bytes.
      await page.getByTestId('write-data').click();
      await until(page, 'data', t => t === 'writing');
      await tap(page, [], '04:e5');
      const data = await until(page, 'data', t => t !== 'writing');
      const all = await page.evaluate(() => globalThis.__nfc.written());
      const own = all.at(-1)?.records[0];
      check(engine, 'a write of bytes from data code', data.startsWith('written') && own?.recordType === 'example.com:fixture' && JSON.stringify(own.data) === '[1,2,3]', `${data} ${JSON.stringify(own)}`);

      // makeReadOnly waits for a tag as a write does.
      await page.getByTestId('lock').click();
      await until(page, 'written', t => t === 'writing');
      await tap(page, [], '04:f6');
      const locked = await until(page, 'written', t => t !== 'writing');
      check(engine, 'makeReadOnly settles against the tag', locked.startsWith('written') && (await page.evaluate(() => globalThis.__nfc.written())).at(-1)?.kind === 'lock', locked);

      // The tier 2 command is iOS's alone.
      await page.getByTestId('command').click();
      const command = await until(page, 'commanded', t => t !== '-');
      check(engine, 'sendCommand fails NotSupportedError', command === 'error NotSupportedError', command);
    }
    check(engine, 'no page errors', !errors.length, errors.join('; '));
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
fixture.remove();
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
