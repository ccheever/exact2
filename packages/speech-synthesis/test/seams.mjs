// The seam is public (LLP 1133.001 D8, LLP 1133.012 stage 3): the package,
// packed as a registry carries it (`bun pm pack`) and installed into an app
// `exact new` made outside the repo, builds unchanged and speaks, in WebKit.
// Synthesis consumes no other package's object, so there is no replacement
// test here; recognition's seams test replaces its implementation on a
// media-capture track.
//
// usage: bun packages/speech-synthesis/test/seams.mjs
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-speech-synthesis-seams-'));
run('bun', ['pm', 'pack', '--destination', scratch], pkg);
const tarball = join(scratch, readdirSync(scratch).find((f) => f.startsWith('exact-speech-synthesis') && f.endsWith('.tgz')));
const { app } = makeFixture({ synthesis: `file:${tarball}` });
process.on('exit', () => {
  rmSync(scratch, { recursive: true, force: true });
  rmSync(dirname(app), { recursive: true, force: true });
});
const installed = JSON.parse(readFileSync(join(app, 'node_modules/@exact/speech-synthesis/package.json'), 'utf8')).name;
check('packed', 'the tarball is installed, not a link into the checkout', installed === '@exact/speech-synthesis' && !readFileSync(join(app, 'package.json'), 'utf8').includes(`file:${pkg}`), installed);

run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
  return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
} });
const browser = await webkit.launch();
try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.port}`);
  const text = (id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(() => '');
  const until = async (id, want, ms = 15000) => {
    const end = Date.now() + ms;
    let got = '';
    while (Date.now() < end && !want(got = await text(id))) await page.waitForTimeout(100);
    return got;
  };
  await page.getByTestId('say').click();
  const first = await until('first', (t) => t.startsWith('ended'));
  check('packed', 'it builds unchanged and speaks an utterance to its end', first === 'ended end -', first);
} finally {
  await browser.close();
  server.stop();
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
