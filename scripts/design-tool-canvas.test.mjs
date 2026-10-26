// The design tool's canvas on a real dev loop: the frame, builds and pins (T2) (split from design-tool.test.mjs, so a
// lane runs it beside the others; inspect from the tool, T2b, is design-tool-inspect.test.mjs).
import { expect, onTestFinished, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { request } from 'node:http';
import { spawnOwned } from './test-children.mjs';
import { HEARTBEAT_RACE, ROOT, SLOW, TOOL_DIST, TOOL_ORIGIN, drive, lines, signals, stop, timed, toolTurns } from './design-tool-harness.mjs';

toolTurns();

// T2's server clocks (tamber-timing.mjs's test knobs; the real ones in the slow lane, EXACT_SLOW_TESTS=1): the long
// poll's wait, a POST client's presence, a GET long poll's (the lease a closed tool's pin is held for), the sweep. As
// the real ones are, chosen so the lease's window below (from a GET long poll's lease, less a wait and a sweep) lies
// past a POST client's lease and a sweep: a pin handed back on the POST lease fails it.
const CLOCKS = SLOW ? { POLL: 25_000, PRESENT: 35_000, LONG_PRESENT: 90_000, SWEEP: 5_000 } : { POLL: 3_000, PRESENT: 4_000, LONG_PRESENT: 12_000, SWEEP: 1_000 };

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
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...timed(CLOCKS), EXACT_APP_DIR: app, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool, other, driving = true, saidPlan = null;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes(`127.0.0.1:${port}/`), 600_000);
    // Its page built (a build that failed frames an error page, which never registers as the frame's build).
    expect(devSaid.seen.filter(l => l.includes('does not build'))).toEqual([]);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', base, '--no-open'], { cwd: ROOT, env: timed(CLOCKS), stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    other = await polledBuild(base, 'linux-1');
    // A build's report reaches the tool on its long poll.
    const wait = ['clock +4000 real', 'clock data'], registered = signals('canvas');
    onTestFinished(registered.drop);
    // The frame's build says the plan it runs (LLP 1122 T2b, the JS target), read while the drive has it open; its
    // registration is the drive's signal to go on.
    saidPlan = (async () => {
      while (driving) {
        const listed = await fetch(`${TOOL_ORIGIN}/__tamber/token`).then(r => r.json()).then(({ token }) => fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).then(r => r.json()).catch(() => null);
        const page = listed?.ok?.builds?.find(b => b.host === 'web-js' && b.planDigest);
        if (page) { registered.fire(1); return page.planDigest; }
        await Bun.sleep(300);
      }
      return null;
    })();
    const replies = await drive(url, `design-tool-canvas-${process.pid}`, ['type name Ada', 'tap save-name', 'clock data', 'tree canvas', 'layout', 'type canvas-size tablet', 'layout', 'type canvas-size desktop', 'layout',
      // The framed page's first registration, on a loaded machine (another run beside this one) later than a pin's:
      // once the server lists it, the tool's next read has it (`clock +1` runs the follow task that asks it, as it
      // opens the stream, whose first ask is answered at once), or, the stream open already, its push (`clock held`:
      // the next reply, the push or, landed already, a heartbeat).
      registered.pause(18000, 1), 'clock +1', 'clock data', 'clock +1', 'clock data', 'clock held changes 10s', 'state',
      // Dark and Black pinned on the other build only.
      'type pin-build linux-1', 'type pin-scheme dark', 'type pin-mode-dark-style black', 'tap pin-apply', 'clock data', ...wait, 'state',
      // Open past one presence lease (35 s, the slow lane's; a real-clock
      // step is at least its time): its polls keep the pin.
      `clock +${CLOCKS.PRESENT + 1000} real`, 'clock data', 'state',
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
    expect(frame, HEARTBEAT_RACE).toMatchObject({ pin: '', caughtUp: true });
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
    // while (its last long poll by cursor counts 90 s, LLP 1122 D4), then
    // the server hands it back.
    const until = Date.now() + CLOCKS.LONG_PRESENT + 30_000;
    while (other.frames.at(-1)?.pin && Date.now() < until) await new Promise(ok => setTimeout(ok, 500));
    expect(other.frames.at(-1)).toMatchObject({ pin: null });
    const held = other.frames.at(-1).at - closed;
    // The lease, 90 s after a long poll by cursor's last answer (at most a 25 s wait before the close), so past the
    // POST form's 35 s; and the server's sweep (5 s), with a little room: 60 s to 100 s at the real clocks.
    expect(CLOCKS.LONG_PRESENT - CLOCKS.POLL - CLOCKS.SWEEP).toBeGreaterThan(CLOCKS.PRESENT + CLOCKS.SWEEP);
    expect(held).toBeGreaterThan(CLOCKS.LONG_PRESENT - CLOCKS.POLL - CLOCKS.SWEEP);
    expect(held).toBeLessThan(CLOCKS.LONG_PRESENT + 2 * CLOCKS.SWEEP);
    // While the tool was open, more than a lease went by with no hand-back.
    const pinnedAt = other.frames.find(f => f.pin).at;
    expect(closed - pinnedAt).toBeGreaterThan(CLOCKS.PRESENT + 1000);
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
