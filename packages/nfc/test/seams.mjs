// The seam is public (LLP 1133.001 D8, LLP 1133.035 §12 stage 4): two tests
// that need nothing an outside author lacks, in Chromium with the fake
// `NDEFReader`.
//
// - Out of the tree: the package, packed as a registry would carry it
//   (`bun pm pack`), installed into the fixture app `exact new` made outside
//   the repo, builds unchanged and reads a tag.
// - Replaced: a fixture implementation of @exact/nfc outside the tree, a
//   direct dependency of the app, replaces the endorsed web implementation
//   (LLP 1133.001 D4).
//
// usage: bun packages/nfc/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from '../../contacts/test/fixture.mjs';
import { fake } from './fake.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-nfc-seams-'));

/** The fixture app with `dependencies` beside its own, installed again. */
function fixtureWith(dependencies) {
  const fixture = makeFixture('nfc');
  const file = join(fixture.app, 'package.json'), manifest = JSON.parse(readFileSync(file, 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, ...dependencies };
  writeFileSync(file, JSON.stringify(manifest, null, 2));
  run('bun', ['install'], fixture.app);
  return fixture;
}

/** A built app's page in Chromium with the fake reader: `f(page, until, tap)`'s result. */
async function inPage(app, f) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const browser = await chromium.launch();
  try {
    const page = await (await browser.newContext()).newPage();
    await page.addInitScript(fake);
    await page.goto(`http://127.0.0.1:${server.port}`);
    const text = id => page.getByTestId(id).innerText({ timeout: 10000 }).catch(() => '');
    const until = async (id, want, ms = 10000) => {
      const end = Date.now() + ms;
      let got;
      while (Date.now() < end) { if (want(got = await text(id))) return got; await page.waitForTimeout(50); }
      return got;
    };
    return await f(page, until, id => page.getByTestId(id).click());
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarball, installed as any registry package is.
run('bun', ['pm', 'pack', '--destination', join(scratch, 'packed')], resolve(root, 'packages/nfc'));
const tarball = join(scratch, 'packed', readdirSync(join(scratch, 'packed')).find(f => f.startsWith('exact-nfc-')));
const packed = fixtureWith({ '@exact/nfc': `file:${tarball}` });
const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/nfc/package.json'), 'utf8'));
check('packed', 'the tarball is installed, not a link into the checkout', installed.name === '@exact/nfc' && readFileSync(join(packed.app, 'package.json'), 'utf8').includes(tarball), JSON.stringify(installed));
const packedRun = await inPage(packed.app, async (page, until, tap) => {
  const status = await until('status', t => t.startsWith('true'));
  await tap('scan');
  await until('scanned', t => t === 'scanning');
  await page.waitForFunction(() => globalThis.__nfc.scanning() === 1);
  await page.evaluate(() => globalThis.__nfc.tap([{ recordType: 'url', data: 'https://example.com/packed' }], '04:01'));
  return { status, heard: await until('heard', t => t !== 'none'), scanned: await until('scanned', t => t.startsWith('read')) };
});
check('packed', 'it builds unchanged and reads a tag', packedRun.status === 'true unknown granted app' && packedRun.heard === 'r1 04:01 url=https://example.com/packed' && packedRun.scanned === 'read r1', JSON.stringify(packedRun));
packed.remove();

// Replaced: an implementation outside the tree, which knows a radio and reads its own tag.
const replacement = join(scratch, 'nfc-web');
mkdirSync(replacement, { recursive: true });
writeFileSync(join(replacement, 'package.json'), JSON.stringify({ name: '@fixture/nfc-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(replacement, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/nfc@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(replacement, 'index.js'), `export function implementation({ emit }) {
  return {
    nfcStatus() { return { available: true, radio: 'on', state: 'granted', readerUI: 'app' }; },
    async scan() {
      emit('reading', [{ id: 'fixture', serialNumber: 'ff', apple: null, android: null, records: [] }]);
      return { reason: 'read', reading: 'fixture', message: null };
    },
    async abortScan() {},
  };
}
`);
const replaced = fixtureWith({ '@fixture/nfc-web': `file:${replacement}` });
const replacedRun = await inPage(replaced.app, async (page, until, tap) => {
  const status = await until('status', t => t.startsWith('true'));
  await tap('scan');
  return { status, heard: await until('heard', t => t !== 'none'), scanned: await until('scanned', t => t.startsWith('read')) };
});
check('replaced', "a direct dependency's implementation answers nfcStatus", replacedRun.status === 'true on granted app', JSON.stringify(replacedRun));
check('replaced', 'and its scan and reading', replacedRun.heard.trim() === 'fixture ff' && replacedRun.scanned === 'read fixture', JSON.stringify(replacedRun));
replaced.remove();

rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
