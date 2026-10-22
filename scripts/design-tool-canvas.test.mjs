// The design tool's canvas on a real dev loop: the frame, builds and pins (T2), and inspect from the tool (T2b)
// (split from design-tool.test.mjs, so a lane runs them beside the others).
import { expect, onTestFinished, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { request } from 'node:http';
import { spawnOwned } from './test-children.mjs';
import { checkoutPort, takePort } from './port-lock.mjs';
import { ROOT, TOOL_DIST, TOOL_ORIGIN, drive, lines, stop, toolTurns } from './design-tool-harness.mjs';
import { suiteBrowser } from './agent-launch.mjs';

toolTurns();

/** A build that holds no stream, as Linux's does, on the dev loop at
 * `base`: it registers as `id`, polls its commands, and reports each pin's
 * scheme as applied with the pin's revision, its mode always its own
 * (dark-style=dim: an app's own choice beats a pin). After its first pin
 * it reloads once (a new document, a new nonce, the same id) and keeps the
 * frames it got. */
async function polledBuild(base, id) {
  let nonce = '', ack = 0, stopped = false, reloaded = false;
  const frames = [];
  const report = body => fetch(`${base}/__dev/report`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ nonce, ...body }) });
  const hello = async () => {
    ({ nonce } = await (await fetch(`${base}/__dev/hello`)).json());
    ack = 0;
    expect((await report({ kind: 'register', id, host: 'linux', capabilities: ['pin'] })).status).toBe(200);
  };
  await hello();
  const loop = (async () => {
    while (!stopped) {
      const answer = await fetch(`${base}/__dev/commands?nonce=${nonce}&ack=${ack}`);
      if (answer.status === 404) { await hello(); continue; }
      const { seq, frames: got } = await answer.json();
      ack = seq;
      for (const f of got.filter(f => f.kind === 'pin')) {
        frames.push({ ...f, reloaded, at: Date.now() });
        await report({ kind: 'status', scheme: f.pin?.scheme ?? 'light', contrast: 'no-preference', mode: { 'dark-style': 'dim' }, rootFontSize: 16, revision: f.revision });
        if (f.pin && !reloaded) { reloaded = true; await hello(); }
      }
      await new Promise(ok => setTimeout(ok, 200));
    }
  })();
  return { frames, stop: async () => { stopped = true; await loop; } };
}

test('T2: the canvas frames the dev loop\'s page, sandboxed, at the size chosen; builds are listed with their effective facts, a pin on one moves only it, a reloaded build gets it back, and the tool gone hands it back', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-canvas-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-design-tool-canvas-dist-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool, other, driving = true, saidPlan = null;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes(`127.0.0.1:${port}/`), 600_000);
    // Its page built (a build that failed frames an error page, which never registers as the frame's build).
    expect(devSaid.seen.filter(l => l.includes('does not build'))).toEqual([]);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    other = await polledBuild(base, 'linux-1');
    // A build's report reaches the tool on its long poll. (Real time in one step, not several: a jump
    // that crosses one of the tool's timers first waits on what is in flight, up to 20 s (the agent's
    // bound; the server holds the tool's changes poll 25 s), and a `+N real` step counts that wait as
    // time passed.)
    const wait = ['clock +4000 real', 'clock data'];
    // The frame's build says the plan it runs (LLP 1122 T2b, the JS target), read while the drive has it open.
    saidPlan = (async () => {
      while (driving) {
        const listed = await fetch(`${TOOL_ORIGIN}/__tamber/token`).then(r => r.json()).then(({ token }) => fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).then(r => r.json()).catch(() => null);
        const page = listed?.ok?.builds?.find(b => b.host === 'web-js' && b.planDigest);
        if (page) return page.planDigest;
        await Bun.sleep(300);
      }
      return null;
    })();
    const replies = await drive(url, `design-tool-canvas-${process.pid}`, ['type name Ada', 'tap save-name', 'clock data', 'tree canvas', 'layout', 'type canvas-size tablet', 'layout', 'type canvas-size desktop', 'layout',
      // The framed page's first registration, on a loaded machine (another run beside this one) later than a pin's.
      'clock +18000 real', 'clock data', 'state',
      // Dark and Black pinned on the other build only.
      'type pin-build linux-1', 'type pin-scheme dark', 'type pin-mode-dark-style black', 'tap pin-apply', 'clock data', ...wait, 'state',
      // Open past one presence lease (35 s; a real-clock step is at least
      // its time): its polls keep the pin.
      'clock +36000 real', 'clock data', 'state',
    ]);
    const closed = Date.now();
    const [tree] = replies.filter(r => r.nodes && !('x' in (r.nodes[0] ?? {})));
    const of = id => tree.nodes.find(n => n.props.testId === id);
    // Framed as localhost: another site than the tool's, its own process.
    expect(of('canvas-frame')).toMatchObject({ type: 'WebView', url: base.replace('127.0.0.1', 'localhost') });
    expect(of('canvas-open').type).toBe('Pressable');
    // Its size, inside a 1px border: a phone's, a tablet's, a desktop's.
    const sizes = replies.filter(r => r.nodes?.some(n => 'x' in n)).map(r => r.nodes.find(n => n.testId === 'canvas-frame')).map(n => [n.w - 2, n.h - 2]);
    expect(sizes).toEqual([[390, 844], [820, 1180], [1280, 800]]);
    // The sandbox the tool built: scripts, its own origin, forms, popups
    // (sandboxed too), dialogs and downloads, never navigating the tool.
    expect(readFileSync(resolve(TOOL_DIST, 'app.plan'), 'latin1')).toContain('allow-scripts allow-same-origin allow-forms allow-popups allow-modals allow-downloads');
    // The framed page ran: it opened its dev stream (while the tool was
    // open: the drive waits on the real clock for it).
    await devSaid.until(l => l.startsWith('page connected'), 1);
    // The page itself lets the tool's origin frame it.
    expect((await fetch(`${base}/`, { headers: { accept: 'text/html' } })).headers.get('content-security-policy')).toBe(`frame-ancestors 'self' ${TOOL_ORIGIN}`);
    // It answers on [::1] too, where `localhost` (the canvas's) goes first.
    // (A request there names localhost, as the browser's does.)
    // (Where this machine has IPv6.)
    const viaSix = await new Promise((ok, fail) => request({ host: '::1', port, path: '/', headers: { host: `localhost:${port}`, accept: 'text/html' } }, res => { res.resume(); ok([res.statusCode, res.headers['content-security-policy']]); }).on('error', e => (['EADDRNOTAVAIL', 'EAFNOSUPPORT', 'ENETUNREACH'].includes(e.code) ? ok(null) : fail(e))).end());
    if (viaSix) expect(viaSix).toEqual([200, `frame-ancestors 'self' ${TOOL_ORIGIN}`]);
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.hello).toMatchObject({ page: base, frame: base.replace('127.0.0.1', 'localhost') });
    // The frame's build and the other, each with the facts it reports.
    const builds = states[0].resources.changes.builds;
    const frame = builds.find(b => b.host === 'web-js');
    expect(frame).toMatchObject({ pin: '', caughtUp: true });
    // It said the plan it runs: dist's app.plan, whose map the loop keeps.
    driving = false;
    const plan = createHash('sha256').update(readFileSync(join(dist, 'app.plan'))).digest('hex');
    expect(await saidPlan).toBe(plan);
    expect(JSON.parse(readFileSync(join(dist, 'app.plan.map.json'), 'utf8')).digest).toBe(plan);
    expect(['light', 'dark']).toContain(frame.scheme);
    expect(builds.find(b => b.id === 'linux-1')).toMatchObject({ host: 'linux', scheme: 'light', mode: 'dark-style=dim', pin: '' });
    expect(states[1].slots.pinned).toMatchObject({ ok: true, note: 'Pinned on 1 build(s).' });
    // The pinned build shows its effective mode, its own Dim, beside the
    // pin's Black; the frame's build is as it was.
    const after = new Map(states[1].resources.changes.builds.map(b => [b.id, b]));
    expect(after.get('linux-1')).toMatchObject({ scheme: 'dark', mode: 'dark-style=dim', pin: 'dark, dark-style=black', caughtUp: true });
    expect(after.get(frame.id)).toEqual(frame);
    // Reloaded, the build was sent its pin again.
    expect(states[2].resources.changes.builds.find(b => b.id === 'linux-1')).toMatchObject({ pin: 'dark, dark-style=black', caughtUp: true });
    expect(other.frames.filter(f => f.pin).map(f => [f.reloaded, f.pin])).toEqual([[false, { scheme: 'dark', mode: { 'dark-style': 'black' } }], [true, { scheme: 'dark', mode: { 'dark-style': 'black' } }]]);
    // The tool closed (its page gone, no unload request): the pin holds a
    // while (its last long poll counts 35 s), then the server hands it back.
    const until = Date.now() + 60_000;
    while (other.frames.at(-1)?.pin && Date.now() < until) await new Promise(ok => setTimeout(ok, 500));
    expect(other.frames.at(-1)).toMatchObject({ pin: null });
    const held = other.frames.at(-1).at - closed;
    expect(held).toBeGreaterThan(10_000);
    // The lease (35 s) and the server's sweep (5 s), with a little room.
    expect(held).toBeLessThan(45_000);
    // While the tool was open, more than a lease went by with no hand-back.
    const pinnedAt = other.frames.find(f => f.pin).at;
    expect(closed - pinnedAt).toBeGreaterThan(36_000);
    expect(other.frames.filter(f => !f.pin && f.at > pinnedAt && f.at < closed)).toEqual([]);
  } finally {
    // The poll beside the drive ends before the tool goes.
    driving = false;
    await saidPlan?.catch(() => null);
    await other?.stop();
    if (tool) await stop(tool);
    await stop(dev);
    rmSync(app, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);
test('T2b: inspect from the tool selects a view in the canvas frame, shows its source and rows, and opens its token on the branch that won', async () => {
  // This checkout's apps/theme-fixture (the wasm loop builds the app's crate, so no copy will do), in turn with another
  // run here: its design server takes the app's .exact/tamber.lock, which refuses a second writer.
  const giveFixture = await takePort(1000 + checkoutPort(resolve(ROOT, 'apps/theme-fixture')), { label: `T2b on ${ROOT}/apps/theme-fixture` });
  onTestFinished(giveFixture);
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-design-tool-inspect-dist-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--wasm', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  // Site isolation on, as desktop Chrome has it: the pinned headless shell (the default browser without CHROME) runs a
  // cross-site frame in its parent's process unless told, so the check below could never pass there.
  const browser = await suiteBrowser('chromium', { args: ['--site-per-process'] });
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes('plan ready'), 600_000);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', resolve(ROOT, 'apps/theme-fixture'), '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const page = await browser.newPage({ viewport: { width: 1400, height: 1200 } });
    await page.goto(url);
    await page.locator('[data-testid="name"]').fill('Ada');
    await page.locator('[data-testid="save-name"]').click();
    await page.locator('[data-testid^="build-web-"]').first().waitFor({ timeout: 60_000 });
    await page.locator('[data-testid="inspect-toggle"]').click();
    await page.locator('[data-testid="inspect-result"]').waitFor();
    expect(await page.locator('[data-testid="inspect-result"]').innerText()).toMatch(/^Inspecting on 1 build\(s\)/);
    // The frame shows the banner; a click on Swap pins there (through the
    // page's cover, at its place) selects it.
    const frame = page.frames().find(f => f.url().startsWith(base.replace('127.0.0.1', 'localhost')));
    await frame.locator('#exact-inspect').waitFor();
    // Another site than the tool's: the browser runs it out of the tool's
    // process (an iframe target of its own, with site isolation on, above).
    // Its target's URL can trail the frame's first navigation, so it is asked
    // again for a while; a frame in the tool's own process never shows as one.
    const cdp = await page.context().newCDPSession(page), here = base.replace('127.0.0.1', 'localhost');
    let isolated = false;
    for (const end = Date.now() + 10_000; Date.now() < end; await Bun.sleep(200)) {
      if ((await cdp.send('Target.getTargets')).targetInfos.some(t => t.type === 'iframe' && t.url.startsWith(here))) { isolated = true; break; }
    }
    expect(isolated).toBe(true);
    const at = await frame.locator('[data-testid="swap"]').boundingBox();
    await page.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
    const line = readFileSync(resolve(ROOT, 'apps/theme-fixture/app.contract'), 'utf8').split('\n').findIndex(l => l.includes('text "Swap pins"')) + 1;
    await page.locator('[data-testid="selection-source"]').waitFor({ timeout: 60_000 });
    expect(await page.locator('[data-testid="selection-source"]').innerText()).toBe(`app.contract:${line} · App`);
    // Its colour is inherited: a link to the page's token, not its edit;
    // opened, the grid's cell for it, in the column the copy shows.
    expect(await page.locator('[data-testid="selrow-link-color"]').innerText()).toBe('Open text.primary');
    expect(await page.locator('[data-testid="selrow-edit-color"]').count()).toBe(0);
    // The page is light (headless Chrome's default) in Black: the light
    // column, its default branch.
    expect(await page.locator('[data-testid="selection-build"]').innerText()).toMatch(/ · Light$/);
    await page.locator('[data-testid="selrow-link-color"]').click();
    await page.waitForFunction(() => /source site\(s\)/.test(document.querySelector('[data-testid="impact-count"]')?.innerText ?? ''), null, { timeout: 60_000 });
    expect(await page.locator('[data-testid="impact-count"]').innerText()).toMatch(/^text\.primary: read at [1-9]\d* source site\(s\), statically/);
    await page.locator('[data-testid="branch-label"]').waitFor();
    expect(await page.locator('[data-testid="branch-label"]').innerText()).toBe('text.primary · default (Light)');
    // Stop: the frame leaves inspect mode.
    await page.locator('[data-testid="inspect-toggle"]').click();
    await frame.locator('#exact-inspect').waitFor({ state: 'detached' });
  } finally {
    await browser.close();
    if (tool) await stop(tool);
    await stop(dev);
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);
