// @exact/clipboard's stage 1 fixture (LLP 1133.006 §9 item 7), driven: an
// app `exact new` made outside the repo, depending on this package by
// `file:`, imports it from data code only, and writes a PNG by bytes and by
// path, an APNG whose default image is not part of its animation, a PNG over
// the pixel bound, bytes that are no PNG, text, an A/B race, two items and
// an unsupported type.
//
// - The web, bound, in Chromium, whose headless clipboard is its own (the
//   machine's pasteboard is saved first and checked unchanged after): the
//   answers, and what `navigator.clipboard.read()` gives back.
// - Under the agent on each host named (`web`, `macos`, `ios`): the answers
//   and `state.extensions.clipboard`, where the implementation runs as its
//   own substitute; the APNG's written PNG hashes as the still green frame's
//   does, not as the red default image's.
//
// usage: bun packages/clipboard/test/drive.mjs [web] [macos] [ios] [--keep]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, root = resolve(here, '../../..');
const hosts = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

const scratch = mkdtempSync(join(tmpdir(), 'exact-clipboard-'));
const app = join(scratch, 'clipapp');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
cpSync(join(here, 'fixture'), app, { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@exact/clipboard': `file:${resolve(here, '..')}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);

// The machine's pasteboard's change count, which reads none of its contents.
const pasteboard = () => process.platform === 'darwin' ? run('osascript', ['-l', 'JavaScript', '-e', "ObjC.import('AppKit'); $.NSPasteboard.generalPasteboard.changeCount"]).trim() : '';

// Each press, its answer, and what the last write shows (null: unchanged).
const presses = [
  ['png-bytes', 'ok', (l) => l.types.join() === 'image/png,text/plain' && l.text === 'a gradient' && l.image.width === 64 && l.image.height === 48],
  ['png-path', 'ok', (l) => l.types.join() === 'image/png' && l.image.width === 64],
  ['green', 'ok', (l) => l.image.width === 4],
  ['red', 'ok', (l) => l.image.width === 4],
  ['apng', 'ok', (l) => l.image.width === 4 && l.image.height === 3],
  ['huge', 'DataError', null],
  ['not-png', 'DataError', null],
  ['two', 'NotAllowedError', null],
  ['jpeg', 'NotAllowedError', null],
  ['race', 'AbortError ok', (l) => l.types.join() === 'text/plain' && l.text === 'B'],
  ['text', 'ok', (l) => l.text === 'exact clipboard fixture'],
];
/** The checks over each press's answer and last write, `lasts` by press. */
const judge = (where, answers, lasts) => {
  for (const [press, said, shows] of presses) {
    check(where, `${press} answers ${said}`, answers[press] === said, answers[press]);
    if (shows) check(where, `${press}: the last write shows it`, (() => { try { return shows(lasts[press]); } catch { return false; } })(), JSON.stringify(lasts[press]));
  }
  const sha = (p) => lasts[p]?.image?.sha256;
  check(where, 'the APNG writes its frame 0 (green), not its default image (red)', sha('apng') && sha('apng') === sha('green') && sha('apng') !== sha('red'), `${sha('apng')} green ${sha('green')} red ${sha('red')}`);
};

if (hosts.includes('web')) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const origin = `http://localhost:${server.port}`;
  const before = pasteboard();
  const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  const browser = await chromium.launch();
  try {
    const context = await browser.newContext();
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
    const page = await context.newPage();
    await page.goto(origin);
    const answers = {}, lasts = {};
    for (const [press] of presses) {
      await page.getByTestId(press).click();
      const end = Date.now() + 5000;
      let got = 'none';
      while (Date.now() < end && (got = await page.getByTestId(`${press}-said`).innerText()) === 'none') await page.waitForTimeout(50);
      answers[press] = got;
      lasts[press] = await page.evaluate(() => globalThis.exact.direct.state().clipboard?.last);
      if (press === 'apng') {
        // What the browser's clipboard holds: the green frame.
        const pixel = await page.evaluate(async () => {
          const [item] = await navigator.clipboard.read();
          const bitmap = await createImageBitmap(await item.getType('image/png'));
          const c = new OffscreenCanvas(bitmap.width, bitmap.height), x = c.getContext('2d');
          x.drawImage(bitmap, 0, 0);
          return [...x.getImageData(0, 0, 1, 1).data].join(',');
        });
        check('web', 'the browser\'s clipboard holds the APNG\'s green frame', pixel === '0,255,0,255', pixel);
      }
    }
    judge('web', answers, lasts);
    const text = await page.evaluate(() => navigator.clipboard.readText());
    check('web', 'the browser\'s clipboard holds the last text', text === 'exact clipboard fixture', text);
  } finally {
    await browser.close();
    server.stop();
  }
  check('web', 'the machine\'s pasteboard is untouched', pasteboard() === before, 'it changed');
}

/** Each press under the agent on `host`, then its state; then `copyText`, and the journal. */
const agentDrive = (host) => {
  const ops = [...presses.flatMap(([press]) => [`tap ${press}`, 'clock settle', 'state']), 'tap copy-text', 'clock settle', 'logs'];
  const before = pasteboard();
  const lines = run('bun', ['exact.mjs', 'agent', host, '--storage', 'clip', '--json', ...ops], app).trim().split('\n').map((l) => JSON.parse(l));
  // Nothing of the drive's reaches the machine's pasteboard, a simulator's synced one included (a person copying meanwhile would move it too).
  if (host !== 'web') check(host, 'the machine\'s pasteboard is untouched', pasteboard() === before, 'its change count moved');
  const journal = JSON.stringify(lines.at(-1));
  check(host, 'copyText under the agent writes the drive\'s clipboard, not the person\'s', /copyText: 22 characters (to the drive's pasteboard|kept for the agent)/.test(journal), journal.slice(-400));
  const states = lines.filter((o) => o && o.slots !== undefined), answers = {}, lasts = {};
  const mutation = (press) => press.replace(/-(\w)/g, (_, c) => c.toUpperCase());
  presses.forEach(([press], i) => {
    answers[press] = states[i]?.slots?.[mutation(press)]?.said ?? 'none';
    lasts[press] = states[i]?.extensions?.clipboard?.last;
  });
  judge(host, answers, lasts);
  const clipboard = states.at(-1)?.extensions?.clipboard;
  check(host, 'the implementation is its own substitute under the agent', clipboard?.substituting === true && clipboard?.calls?.write >= 8, JSON.stringify(clipboard));
  if (host === 'macos') console.log(`macos: the drive's pasteboard after the PNG with text: ${JSON.stringify(states[0]?.extensions?.clipboard?.written)}`);
};
for (const host of ['web', 'macos', 'ios']) if (hosts.includes(host)) agentDrive(host);

if (!process.argv.includes('--keep')) rmSync(scratch, { recursive: true, force: true });
else console.log(`kept ${scratch}`);
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
