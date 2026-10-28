#!/usr/bin/env bun
/**
 * Harmony's background drives (LLP 1133.017 §11), under the agent against a
 * `snapback4 dev --memory` server of this run's own, behind a proxy this
 * script holds so a request can be made to hang (drive 4's stand-in server
 * that never answers):
 *
 * 1. Outbox: a write while the backend fails is pending and registers
 *    `sync outbox:ada`; `tap @ref` is refused `offline` while the page is,
 *    then, online, is a warm wake that delivers it and retires the entry.
 * 2. Cold outbox: as 1, then persona Grace, the session ended, and a new one
 *    launched with the entry (`--wake`): Ada's message is delivered, the view
 *    still Grace's.
 * 3. A mention: the switch asks for notification permission (granted by
 *    the drive) and registers `periodicsync catch-up`, refused until due;
 *    after Grace posts `@ada`, the catch-up posts one notification for
 *    #general (`@exact/notifications`), which navigates to the room; the
 *    next catch-up posts nothing.
 * 4. An expired catch-up: the backend hangs; past the 30 s budget the wake
 *    expires, nothing is posted and the mark stays, so the next posts it.
 *
 * In a browser (`chromium`, `firefox`, `webkit`), with the web implementation
 * bound and no agent: drive 1, the browser's offline switch for the network,
 * and drive 3's hidden switch.
 *
 * usage: bun packages/background/test/harmony.mjs [web|macos|ios|chromium|firefox|webkit]
 *   (macos and ios build the app against this run's proxy, on port 47613)
 */
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const APP = resolve(ROOT, 'apps/harmony/app'), BACKEND = resolve(APP, '..');
const GENERAL = '01HARMNY00000000000000000E';
const ENGINES = ['chromium', 'firefox', 'webkit'];
const asked = process.argv[2] ?? 'web', engine = ENGINES.includes(asked) ? asked : null;
// An engine drives the web build in that browser with the implementation bound; the agent's sessions check the server.
const host = engine ? 'web' : asked;
const { open, faultRequest } = await import(resolve(ROOT, 'scripts/agent.mjs'));
const { resetBackends, freshRows, storeBase } = await import(resolve(ROOT, 'scripts/agent-test.mjs'));
const { resolveApp } = await import(resolve(ROOT, 'scripts/app.mjs'));

let failures = 0;
const check = (drive, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${asked} ${drive}: ${name}${ok ? '' : `: ${typeof detail === 'string' ? detail : JSON.stringify(detail)}`.slice(0, 2000)}`);
};

// ---- the backend: a temporary store, behind a proxy that can hang
const step = (what, cmd, args, opts) => {
  const r = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (r.status !== 0) throw new Error(`${what}: ${cmd} ${args.join(' ')} exited ${r.status}`);
};
if (!existsSync(resolve(BACKEND, 'node_modules/snapback4/package.json'))) step('the backend\'s packages', process.execPath, ['install', '--frozen-lockfile'], { cwd: BACKEND });
step('the Snapback device', process.execPath, [resolve(ROOT, 'snapback4/web/build.mjs'), 'assets/snapback4.wasm'], { cwd: APP });
const { resolveBinary } = await import(pathToFileURL(resolve(BACKEND, 'node_modules/snapback4/dist/bin-path.js')).href);
const server = spawn(resolveBinary(), ['dev', '--port', '0', '--memory', '--no-watch'], { cwd: BACKEND, stdio: ['ignore', 'pipe', 'inherit'], env: { ...process.env, SNAPBACK4_SUPERVISOR_PID: String(process.pid) } });
const upstream = new URL(await new Promise((ok, fail) => {
  const lines = createInterface({ input: server.stdout });
  lines.once('line', (line) => { lines.close(); server.stdout.resume(); ok(line.trim()); });
  server.once('exit', (code) => fail(new Error(`snapback4 dev exited ${code}`)));
})).origin;
const status = await (await fetch(`${upstream}/dev/status`)).json();
if (status.pid !== server.pid || status.temporary !== true) throw new Error(`${upstream} is not this run's temporary store`);

let hang = false;
const hung = [];
const proxy = Bun.serve({ port: host === 'web' ? 0 : 47613, idleTimeout: 0, async fetch(req) {
  const url = new URL(req.url), to = `${upstream}${url.pathname}${url.search}`;
  const go = async () => {
    const r = await fetch(to, { method: req.method, headers: req.headers, body: ['GET', 'HEAD'].includes(req.method) ? undefined : await req.arrayBuffer(), redirect: 'manual' });
    return new Response(r.body, { status: r.status, headers: r.headers });
  };
  if (hang && !url.pathname.startsWith('/dev/')) return new Promise((ok) => hung.push(() => ok(go())));
  return go();
} });
const origin = `http://127.0.0.1:${proxy.port}`;
const release = () => { hang = false; for (const go of hung.splice(0)) go(); };
const seed = freshRows(resolve(APP, 'app.test.contract'), '../seed.json');
const fresh = () => resetBackends([origin], seed);
const as = (persona) => ({ 'content-type': 'application/json', 'x-snapback-persona': persona });
const post = async (persona, roomId, body) => {
  const r = await fetch(`${upstream}/m/sendMessage`, { method: 'POST', headers: as(persona), body: JSON.stringify({ id: randomUUID(), args: { roomId, body } }) });
  const sent = await r.json().catch(() => null);
  if (!r.ok || sent?.state !== 'sent') throw new Error(`${persona}'s send answered ${r.status}: ${JSON.stringify(sent)}`);
};

// ---- the build, against the proxy
const work = resolve(ROOT, 'target', 'background-harmony');
mkdirSync(work, { recursive: true });
const dir = mkdtempSync(resolve(work, `${host}-`)), dist = resolve(dir, 'web-dist');
const env = { ...process.env, EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: APP };
if (host === 'web') step('the web build', process.execPath, ['host/web/build.mjs', resolveApp(APP).crate('web')], { cwd: ROOT, env });
else step(`the ${host} build`, process.execPath, ['host/apple/build.mjs', ...(host === 'ios' ? ['--ios'] : []), resolveApp(APP).crate('apple')], { cwd: ROOT, env });

// The driver runs in this process, and checks the build against its environment.
Object.assign(process.env, { EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: APP });
// Storage is refused under the agent without a scratch store: each session names one, a new one unless it continues.
let stores = 0;
const session = (storage = `harmony-bg-${process.pid}-${++stores}`) => open({ host, app: APP, webDist: dist, epoch: 'now', storage, env: { EXACT_BACKEND_ORIGIN: origin } });
const state = async (s) => s.op({ op: 'state' });
const latest = (st) => st.derives?.latest ?? {};
const entry = (st, kind, tag) => (st.backgroundSync ?? []).find((e) => e.kind === kind && e.tag === tag);
const tapRef = async (s, ref) => s.op({ op: 'tap', ticket: ref }).catch((e) => ({ error: e.message }));
const offline = async (s) => { await s.op(faultRequest('fail', ['fetch', origin], [origin])); await s.prefer({ online: 'false' }); };
const online = async (s) => { await s.op(faultRequest('pass', ['fetch', origin], [origin])); await s.prefer({ online: 'true' }); };
const settle = async (s) => { await s.clock('settle'); await s.clock('data'); };
const journal = async (s) => (await s.logs({ all: true })).lines.join('\n');
const count = (text, pattern) => (text.match(pattern) ?? []).length;
/** A message written while the backend fails: pending, and the read after it registers the outbox. */
async function writeOffline(s, body) {
  await s.clock('data');
  await s.tap('room-general').catch(async (e) => { const st = await state(s); throw new Error(`${e.message}\n${JSON.stringify({ failed: st.failed, resources: Object.keys(st.resources ?? {}) })}\n${(await journal(s)).slice(-4000)}`); });
  await settle(s);
  await offline(s);
  await s.type('composer', body);
  await s.tap('send');
  await s.clock('data');
  await s.clock('+10000');
  await s.clock('data');
  return state(s);
}
/** The message in #general as a fresh session of Ada's sees it. */
async function delivered(body) {
  const s = await session();
  try {
    await s.clock('data');
    await s.tap('room-general');
    await settle(s);
    const m = latest(await state(s));
    return m.body === body && m.pending === false ? null : m;
  } finally { await s.close(); }
}

/** Drives 1 and 3 in a browser (stage 1): the page's `online` event is the wake. */
async function inBrowser() {
  // The build as the agent's web host serves it (its headers included).
  const { serveBuildTree } = await import(resolve(ROOT, 'host/web/serve.mjs'));
  const pages = createServer((req, res) => serveBuildTree(dist, req, res));
  await new Promise((ok) => pages.listen(0, '127.0.0.1', ok));
  const engines = await import(resolve(ROOT, 'node_modules/playwright-core/index.mjs'));
  const browser = await engines[engine].launch();
  const tags = (page) => page.evaluate(() => { try { return JSON.parse(localStorage.getItem('exact.background.sync') ?? '[]').map(([tag]) => tag).join(' '); } catch { return '?'; } });
  const until = async (read, want, ms = 20000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await read())) return got; await Bun.sleep(200); }
    return got;
  };
  try {
    await fresh();
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
    page.on('console', (m) => { if (m.type() === 'error') console.log(`console: ${m.text()}`); });
    await page.goto(`http://127.0.0.1:${pages.address().port}/`);
    await page.getByTestId('room-general').click({ timeout: 30000 });
    // Everything the room loads, loaded before the network goes (a chunk is fetched at its first use).
    await page.getByTestId('composer').waitFor({ timeout: 30000 });
    await page.waitForTimeout(2000);
    await context.setOffline(true);
    const body = `Sent from ${engine}'s outbox`;
    await page.getByTestId('composer').fill(body);
    await page.getByTestId('send').click();
    const waiting = await until(() => tags(page), (t) => t === 'outbox:ada');
    check('1 outbox', 'offline, the failed round registers sync outbox:ada', waiting === 'outbox:ada', waiting === 'outbox:ada' ? '' : JSON.stringify({ waiting, shown: (await page.evaluate(() => document.body.innerText)).includes(body), composer: await page.getByTestId('composer').inputValue().catch((e) => e.message) }));
    check('1 outbox', 'the message is not on the server yet', (await delivered(body)) !== null, 'delivered while offline');
    await context.setOffline(false);
    const retired = await until(() => tags(page), (t) => t === '');
    check('1 outbox', 'at the page\'s online event the wake runs and the tag is retired', retired === '', await page.evaluate(() => localStorage.getItem('exact.background.sync')));
    const one = await until(() => delivered(body), (m) => m === null, 30000);
    check('1 outbox', 'the message is delivered', one === null, one);
    await page.getByTestId('tab-you').click();
    await page.getByTestId('switch-ada').waitFor({ timeout: 10000 });
    check('3 mention', 'the web hides the switch: periodic sync is `later`', (await page.getByTestId('notify-mentions').count()) === 0, 'shown');
    await context.close();
  } finally {
    await browser.close();
    pages.close();
  }
}

try {
  if (engine) await inBrowser();
  else {
    await fresh();
    let s = await session();
    try {
      await settle(s);
      await s.prefer({ 'visibility-state': 'hidden' });
      await settle(s);
      const before = (await state(s)).slots.sampledAt;
      await s.clock('+10000');
      await settle(s);
      check('0 visibility', 'the hidden page stops its UI clock', (await state(s)).slots.sampledAt === before, (await state(s)).slots.sampledAt);
      await s.prefer({ 'visibility-state': 'visible' });
      await s.clock('+1');
      await settle(s);
      check('0 visibility', 'returning to the page refreshes its screen data', (await state(s)).slots.sampledAt > before, (await state(s)).slots.sampledAt);
    } finally { await s.close(); }

    // 1. Outbox
    await fresh();
    s = await session();
    try {
      const st = await writeOffline(s, 'Sent from the outbox');
      const e = entry(st, 'sync', 'outbox:ada');
      check('1 outbox', 'a failed round with a write on the device registers sync outbox:ada', e != null && latest(st).pending === true, { listed: st.backgroundSync, latest: latest(st) });
      const refused = await tapRef(s, e?.ref);
      check('1 outbox', 'tap @ref is refused while offline', /offline/.test(refused.error ?? ''), refused);
      await online(s);
      const woke = await tapRef(s, e?.ref);
      await settle(s);
      const after = await state(s);
      check('1 outbox', 'online, the tap is a warm wake that completes', !woke.error && after.wake?.outcome === 'completed' && after.wake?.cold === false, after.wake ?? woke);
      check('1 outbox', 'the entry is retired', !entry(after, 'sync', 'outbox:ada'), after.backgroundSync);
    } finally { await s.close(); }
    const one = await delivered('Sent from the outbox');
    check('1 outbox', 'the message is delivered', one === null, one);

    // 2. Cold outbox: the entry, as listed, handed to a new session whose view shows Grace.
    await fresh();
    const store = `harmony-bg-${process.pid}-cold`;
    s = await session(store);
    let listed;
    try {
      const st = await writeOffline(s, 'Sent while Grace looked');
      listed = entry(st, 'sync', 'outbox:ada');
      await online(s);
      await s.tap('tab-you');
      await s.tap('switch-grace');
      await s.clock('data');
    } finally { await s.close(); }
    s = await session(store);
    try {
      const launched = await s.op({ op: 'launchEntry', launch: 'wake', entry: JSON.stringify(listed ?? {}) }).catch((e) => ({ error: e.message }));
      await settle(s);
      const st = await state(s), lines = await journal(s);
      check('2 cold outbox', 'the cold wake completes', !launched.error && st.wake?.cold === true && st.wake?.outcome === 'completed' && lines.includes('wake sync outbox:ada: completed'), st.wake ?? launched);
      check('2 cold outbox', 'the view still shows Grace', st.slots?.persona === 'grace', st.slots?.persona);
    } finally { await s.close(); }
    const two = await delivered('Sent while Grace looked');
    check('2 cold outbox', 'Ada\'s message is delivered', two === null, two);

    // 3. A mention, and 4. an expired catch-up
    if (host === 'web') {
      // The web's periodic cell is `later` (D17): the switch is hidden, and shown by a preferred status, it registers nothing.
      s = await session();
      try {
        await s.clock('data');
        await s.tap('tab-you');
        await s.clock('data');
        const ids = async () => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
        check('3 mention', 'the web hides the switch: backgroundSyncStatus().periodic is false', !(await ids()).has('notify-mentions'), (await state(s)).resources?.background);
        await s.prefer({ extension: `background.backgroundSyncStatus=${JSON.stringify({ available: true, periodic: true, sync: true })}` });
        await s.clock('data');
        await s.prefer({ 'permission.notifications': 'granted' });
        await s.tap('notify-mentions');
        await s.clock('data');
        const st = await state(s), lines = await journal(s);
        check('3 mention', 'shown anyway, the switch\'s registration is refused NotSupportedError', !entry(st, 'periodicsync', 'catch-up') && /registerPeriodicSync: refused.*NotSupportedError/.test(lines), lines.split('\n').filter((l) => /registerPeriodicSync/.test(l)));
      } finally { await s.close(); }
    } else {
      await fresh();
      s = await session(`harmony-bg-${process.pid}-mention`);
      try {
        await s.clock('data');
        await s.tap('tab-you');
        await s.clock('data');
        await s.prefer({ 'permission.notifications': 'granted' });
        await s.tap('notify-mentions');
        await s.clock('data');
        let st = await state(s);
        const catchUp = entry(st, 'periodicsync', 'catch-up');
        check('3 mention', 'the switch registers periodicsync catch-up every 30 minutes', catchUp?.minInterval === 1800000 && st.derives?.notifying === true, { listed: st.backgroundSync, notifying: st.derives?.notifying });
        const early = await tapRef(s, catchUp?.ref);
        check('3 mention', 'tap @ref before its interval is refused, not due', /not due/.test(early.error ?? ''), early);
        await s.clock('+1800000');
        await post('grace', GENERAL, '@ada can you look at the release notes?');
        await tapRef(s, catchUp?.ref);
        await settle(s);
        st = await state(s);
        const notes = st.notifications ?? [];
        check('3 mention', 'one notification for #general with the message, opening the room', notes.length === 1 && notes[0].tag === GENERAL && /release notes/.test(notes[0].body ?? '') && notes[0].navigate === `/c/${GENERAL}`, notes);
        await s.clock('+1800000');
        await tapRef(s, catchUp?.ref);
        await settle(s);
        st = await state(s);
        check('3 mention', 'the next catch-up posts nothing: the mark moved', st.wake?.outcome === 'completed' && count(await journal(s), /showNotification: listed/g) === 1, st.wake);

        // 4. The backend hangs past the budget.
        await post('grace', GENERAL, '@ada one more thing');
        await s.clock('+1800000');
        hang = true;
        await tapRef(s, catchUp?.ref);
        await s.clock('+30000');
        st = await state(s);
        let lines = await journal(s);
        check('4 expired', 'past the budget the catch-up expires and posts nothing', st.wake?.outcome === 'expired' && lines.includes('wake periodicsync catch-up: expired') && count(lines, /showNotification: listed/g) === 1, st.wake);
        release();
        await settle(s);
        await s.clock('+1800000');
        await tapRef(s, catchUp?.ref);
        await settle(s);
        st = await state(s);
        lines = await journal(s);
        check('4 expired', 'the mark did not move: the next catch-up posts it', (st.notifications ?? []).some((n) => /one more thing/.test(n.body ?? '')), { notes: st.notifications, wake: st.wake });
      } finally { release(); await s.close(); }
    }
  }
} finally {
  release();
  proxy.stop(true);
  server.kill('SIGTERM');
  rmSync(dir, { recursive: true, force: true });
  // This run's scratch stores (the web's live in the browser's profile).
  const base = storeBase('com.example.harmony', host);
  let kept = [];
  try { kept = base ? readdirSync(base) : []; } catch { /* none */ }
  for (const name of kept.filter((n) => n.startsWith(`harmony-bg-${process.pid}-`))) rmSync(resolve(base, name), { recursive: true, force: true });
}
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
