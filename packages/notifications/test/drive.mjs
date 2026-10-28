// The conformance drive of @exact/notifications (LLP 1133.004 stage 1): the
// fixture app (`fixture/`), made by `exact new` outside the repo and
// depending on this package by `file:`, driven by the agent on each target
// named, and bound to its implementation in Chromium.
//
// - Bound, in Chromium with the permission granted: the status reads
//   `granted`, a post and a data-code post go out, the badge is set where the
//   browser has it, and a script's `dispatchEvent` click raises nothing.
// - Under the agent on every target: the post waits on the permission
//   ticket, then is listed silent with its data; `tap @ref` raises
//   `notificationclick` with the tag and data; the badge is a number, a flag
//   or none; a post for later is delivered once the clock passes it; and a
//   relaunch with `--notification '<entry>'` carries the entry's data.
//
// usage: bun packages/notifications/test/drive.mjs [web] [macos] [ios] [--sim <udid>] [--keep]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : null;
const hosts = ['web', 'macos', 'ios'].filter((h) => argv.includes(h));
if (!hosts.length) hosts.push('web');
const run = (cmd, args, cwd) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

const scratch = mkdtempSync(join(tmpdir(), 'exact-notifications-')), app = join(scratch, 'notifyfixture');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@exact/notifications': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);

if (hosts.includes('web')) await bound();
for (const host of hosts) substituted(host);

async function bound() {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  // The build's target directory: the app's own, or `CARGO_TARGET_DIR`'s.
  const dist = join(process.env.CARGO_TARGET_DIR ?? join(app, 'target'), 'web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const origin = `http://127.0.0.1:${server.port}`;
  const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  // The new headless mode: the old one's Notification.permission is `denied` whatever is granted.
  const browser = await chromium.launch({ channel: 'chromium' });
  const text = (page, testId) => page.getByTestId(testId).innerText().catch(() => '');
  const until = async (page, testId, want, ms = 5000) => {
    const end = Date.now() + ms;
    let got = '';
    while (Date.now() < end) { got = await text(page, testId); if (want(got)) return got; await page.waitForTimeout(50); }
    return got;
  };
  try {
    const context = await browser.newContext();
    await context.grantPermissions(['notifications'], { origin });
    // Each notification the page makes, kept where the drive can reach it.
    await context.addInitScript(() => {
      const Real = globalThis.Notification;
      if (typeof Real !== 'function') return;
      globalThis.__posted = [];
      globalThis.Notification = class extends Real { constructor(title, options) { super(title, options); globalThis.__posted.push({ title, options, n: this }); } };
    });
    const page = await context.newPage();
    await page.goto(origin);
    const status = await until(page, 'status', (t) => t === 'granted');
    check('web', 'the status reads the browser\'s permission', status === 'granted', status);
    await page.getByTestId('post').click();
    await page.waitForFunction(() => globalThis.__posted?.length === 1, null, { timeout: 5000 }).catch(() => {});
    const posted = await page.evaluate(() => (globalThis.__posted ?? []).map((p) => ({ title: p.title, ...p.options })));
    check('web', 'a silent post with its data goes out', posted[0]?.title === 'Build finished' && posted[0]?.silent === true && posted[0]?.data === '/thread/3' && posted[0]?.tag === 'thread-3', JSON.stringify(posted));
    await page.evaluate(() => globalThis.__posted[0].n.dispatchEvent(new Event('click')));
    await page.waitForTimeout(200);
    check('web', 'a script\'s click on it raises nothing', (await text(page, 'opened')) === 'none', await text(page, 'opened'));
    await page.getByTestId('post-data').click();
    const data = await until(page, 'data', (t) => t !== 'none');
    check('web', 'data code posts, after reading the status, and the send resolves undefined', data === 'posted', data);
    const badge = await text(page, 'badge-kind');
    await page.getByTestId('count').click();
    await page.waitForTimeout(200);
    check('web', `the badge where the browser has it (badge ${badge})`, badge === 'true' || badge === 'false', badge);
  } finally {
    await browser.close();
    server.stop();
  }
}

function substituted(host) {
  const flags = [...(sim && host === 'ios' ? ['--sim', sim] : []), ...(host === 'web' ? [] : ['--storage', 'notifications', '--reset-storage'])];
  const agent = (...ops) => JSON.parse(`[${run('bun', ['exact.mjs', 'agent', host, '--json', ...flags, ...ops], app).trim().split('\n').filter((l) => l.startsWith('{')).join(',')}]`);
  const [, held] = agent('tap post', 'state');
  const ticket = (held.pending ?? []).find((p) => p.device?.capability === 'permission')?.ticket;
  check(host, 'a post while prompt waits on the permission ticket', ticket != null && held.notifications?.length === 0, JSON.stringify(held.pending));
  const listed = agent('tap post', `tap @${ticket} allow`, 'state').at(-1);
  const entry = listed.notifications?.[0];
  check(host, 'allowed, the post is listed silent with its data', entry?.silent === true && entry?.data === '/thread/3' && entry?.delivered === true, JSON.stringify(listed.notifications));
  const clicked = agent('tap post', `tap @${ticket} allow`, `tap @${entry?.ref}`, 'clock settle', 'state').at(-1);
  check(host, 'tap @ref raises notificationclick with the tag and data', clicked.slots?.opened === 'thread-3 /thread/3', JSON.stringify(clicked.slots));
  const badges = agent('tap count', 'state', 'tap flag', 'state', 'tap clear', 'state');
  check(host, 'the badge is a number, a flag, then none', badges[1].badge === 3 && badges[3].badge === 'flag' && badges[5].badge === null, JSON.stringify([badges[1].badge, badges[3].badge, badges[5].badge]));
  const later = agent('--permission', 'notifications=granted', 'tap later', 'state', 'clock +61000', 'state');
  check(host, 'a post for later is delivered once the clock passes it', later[1].notifications?.[0]?.delivered === false && later[3].notifications?.[0]?.delivered === true, JSON.stringify([later[1].notifications, later[3].notifications]));
  const launched = agent('--permission', 'notifications=granted', '--notification', JSON.stringify(entry ?? {}), 'clock settle', 'state').at(-1);
  check(host, 'a relaunch\'s --notification carries the entry\'s data to the handler', launched.slots?.opened === 'thread-3 /thread/3', JSON.stringify(launched.slots));
}

if (!argv.includes('--keep')) rmSync(scratch, { recursive: true, force: true });
else console.log(`kept ${app}`);
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
