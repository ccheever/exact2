// LLP 1133.017 stage 0's acceptance, through this module outside the tree:
// an app `exact new` made outside the repo depends on a copy of it by
// `file:`, as on any installed package, and its event `tide` is raised only
// by a wake.
//
// - On the web, bound to its implementation: a registration is a button the
//   implementation places; a click raises `tide` through the context's
//   `wakes.raise`, which answers the invocation's outcome: `completed` once
//   the handler's request lands, `expired` when it does not within the
//   declared 5 s budget.
// - Under the agent, substituted: registrations are listed in `state.tides`;
//   `tap @ref` on a periodic one is refused until due, then is a warm wake,
//   which stops the page's timers and refuses `copyText`; a relaunch with
//   `--wake '<entry>'` is a cold wake; past the budget it expires.
//
// usage: bun contract/cli/tests/tide/seams.mjs [--keep]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, root = resolve(here, '../../../..');
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

// The handler's request: answered at once, after `slow` ms of real time, or
// (`hang`) held until the suite lets go.
let hang = false, slow = 0;
const hung = [];
const pulls = Bun.serve({ port: 0, fetch(req) {
  const cors = { 'access-control-allow-origin': '*' };
  if (req.method === 'OPTIONS') return new Response(null, { headers: { ...cors, 'access-control-allow-headers': '*' } });
  if (slow) return Bun.sleep(slow).then(() => new Response('7', { headers: cors }));
  if (!hang) return new Response('7', { headers: cors });
  return new Promise((resolve) => hung.push(() => resolve(new Response('7', { headers: cors }))));
} });
const pullOrigin = `http://127.0.0.1:${pulls.port}`;

const scratch = mkdtempSync(join(tmpdir(), 'exact-tide-'));
const pkg = join(scratch, 'tide');
cpSync(here, pkg, { recursive: true, filter: (src) => src !== join(here, 'app') && !src.startsWith(join(here, 'app') + '/') && !src.endsWith('seams.mjs') });
const app = join(scratch, 'tideapp');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
cpSync(join(here, 'app'), app, { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__PULL_ORIGIN__', pullOrigin));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@fixture/tide': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);

// The web, bound.
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
  return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const browser = await chromium.launch();
const text = (page, testId) => page.getByTestId(testId).innerText().catch(() => '');
const until = async (read, want, ms = 8000, page) => {
  const end = Date.now() + ms;
  let got = '';
  while (Date.now() < end) { got = await read(); if (want(got)) return got; await (page?.waitForTimeout(50) ?? Bun.sleep(50)); }
  return got;
};
try {
  const page = await browser.newPage();
  await page.goto(origin);
  await page.getByTestId('once').click();
  const tide = page.getByTestId('tide-o1');
  await tide.waitFor({ timeout: 5000 });
  await tide.click();
  const outcome = () => tide.getAttribute('data-outcome').catch(() => null);
  const done = await until(outcome, (o) => o === 'completed', 8000, page);
  check('web', 'the implementation\'s wake completes once the handler\'s request lands', done === 'completed', done);
  check('web', 'the handler heard the event and its request answered', (await text(page, 'heard')) === 'o1' && (await text(page, 'pulled')) === '7', `${await text(page, 'heard')} ${await text(page, 'pulled')}`);
  hang = true;
  await tide.click();
  const expired = await until(outcome, (o) => o === 'expired', 9000, page);
  check('web', 'a wake whose request never lands expires at its budget', expired === 'expired', expired);
  hang = false;
  for (const release of hung.splice(0)) release();
} finally {
  await browser.close();
  server.stop();
}

// Under the agent: the declared substitute.
// Spawned, not run: this process's server answers the handler's request meanwhile.
const drive = async (ops) => {
  const child = Bun.spawn(['bun', 'exact.mjs', 'agent', 'web', '--json', ...ops], { cwd: app, stdout: 'pipe', stderr: 'pipe' });
  const [out, err, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  return { out, err, code };
};
const agent = async (...ops) => {
  const { out, err, code } = await drive(ops);
  if (code !== 0) throw new Error(`agent ${ops.join(' ')}: ${out}\n${err}`);
  return JSON.parse(`[${out.trim().split('\n').join(',')}]`);
};
const listed = await agent('tap periodic', 'state');
const entry = (listed[1].tides ?? []).find((e) => e.kind === 'every');
check('agent', 'a periodic registration is listed with its interval', entry?.tag === 'e1' && entry?.every === 60000 && entry?.lastRun === null, JSON.stringify(listed[1].tides));
const early = await drive(['tap periodic', `tap @${entry?.ref}`]);
check('agent', 'tap @ref before its interval is refused, not due', early.code !== 0 && /not due until/.test(early.err), early.err);
slow = 1500;
const warm = await agent('tap periodic', 'clock +60000', 'state', `tap @${entry?.ref}`, 'clock +3000', 'state', 'clock settle', 'state', 'logs');
const [before, during, after] = [warm[2], warm[5], warm[7]], journal = JSON.stringify(warm[8]);
check('agent', 'tap @ref when due is a warm wake: the handler hears the tag', after.slots?.heard === 'e1' && after.wake?.outcome === 'completed' && after.wake?.cold === false, JSON.stringify(after.wake));
slow = 0;
check('agent', 'no timer fires while the wake runs unseen', during.wake?.outcome === null && during.slots?.ticks === before.slots?.ticks, `${JSON.stringify(during.wake)} ${before.slots?.ticks} ${during.slots?.ticks}`);
check('agent', 'copyText is refused during the wake', journal.includes('copyText: refused: during a wake'), journal);
check('agent', 'the registration stays, with its run', (after.tides ?? []).some((e) => e.tag === 'e1' && typeof e.lastRun === 'number'), JSON.stringify(after.tides));
const cold = await agent('--wake', JSON.stringify(entry ?? {}), 'clock settle', 'state', 'logs');
check('agent', 'a relaunch with --wake is a cold wake that completes', cold.at(-2).wake?.cold === true && cold.at(-2).wake?.outcome === 'completed' && cold.at(-2).slots?.heard === 'e1', JSON.stringify(cold.at(-2).wake));
hang = true;
const expire = await agent('tap once', 'state', 'logs');
const once = (expire[1].tides ?? []).find((e) => e.kind === 'once');
const late = await agent('tap once', `tap @${once?.ref}`, 'clock +6000', 'state', 'logs');
check('agent', 'past its budget with its request in flight, the wake expires', late.at(-2).wake?.outcome === 'expired' && JSON.stringify(late.at(-1)).includes('wake tide o1: expired'), JSON.stringify(late.at(-2).wake) + JSON.stringify(late.at(-1)));
hang = false;
for (const release of hung.splice(0)) release();
pulls.stop();

if (!process.argv.includes('--keep')) rmSync(scratch, { recursive: true, force: true });
else console.log(`kept ${scratch}`);
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
