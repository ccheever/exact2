// Stage 6's kinds in the design tool on a real dev loop (split from design-tool-scrub.test.mjs, so a lane runs it
// beside the others).
import { expect, onTestFinished, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, ROOT, TOOL_ORIGIN, edit, lines, signals, stop, timed, toolTurns } from './design-tool-harness.mjs';

toolTurns();

/** Signal 0 of `looked` once the server lists the frame's build running a design and the plan it runs (the loop's
 * check of a live edit reads it, LLP 1122 T2b), so a live edit is proposed live (polled until the drive ends). */
function framed(looked) {
  let done = false;
  const polling = (async () => {
    while (!done) {
      const listed = await fetch(`${TOOL_ORIGIN}/__tamber/token`).then(r => r.json()).then(({ token }) => fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).then(r => r.json()).catch(() => null);
      if (listed?.ok?.builds?.some(b => b.host === 'web-js' && b.design && b.planDigest)) { looked.fire(0); return; }
      await Bun.sleep(100);
    }
  })();
  onTestFinished(async () => { done = true; await polling; });
  return polling;
}
/** The drive's wait for the frame's build (4 s at most, as before): once listed, the tool's next read has it (`clock
 * +1` runs the follow task that asks it, as it opens the stream, whose first ask is answered at once), or, the stream
 * open already, its push (`clock held`: the next reply, the push or, landed already, a 3 s heartbeat). */
const ready = looked => [looked.pause(4000, 0), 'clock +1', 'clock data', 'clock +1', 'clock data', 'clock held changes 10s'];

/** Stage 6's kinds in the tool on the JS target's dev loop: a theme-fixture
 * copy whose card reads a length, a shadow, a font-weight number and a
 * duration token, with an icon. A typed length field's scrub previews on the
 * frame's build with no reload and no file written, and Keep writes one
 * line; a motion token says Rebuilds before the gesture and offers no scrub;
 * a font-weight token rebuilds, since the JS target writes it inline; the
 * icon is listed. (The wasm loop builds only a workspace app's crate, so a
 * copy can't run there: the natives' half is the clone's gate.) */
async function kindsOnTheJsLoop() {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-kinds-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const contract = readFileSync(join(app, 'app.contract'), 'utf8');
  const card = contract
    .replace('transition="background-color 400ms" padding=12', 'transition="background-color duration.card" padding="space.card"')
    .replace('box-shadow="0 2px 8px color-mix(in srgb, border.hairline 60%, transparent)"', 'box-shadow="shadow.card"')
    .replace('text "A card" color="text.label"', 'text "A card" color="text.label" font-weight="weight.title"');
  expect(card.match(/space\.card|shadow\.card|weight\.title|duration\.card/g)?.length).toBe(4);
  writeFileSync(join(app, 'app.contract'), card);
  writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('  AccentColor = #1d9bf0\n', [
    '  AccentColor = #1d9bf0',
    '  space.card = 12px',
    '  shadow.card: box-shadow = 0px 2px 8px border.hairline, inset 0px 0px 0px 1px border.hairline',
    '  weight.title = 600',
    '  duration.card = 400ms',
    '',
    'icons',
    '  icon.star = icon(sf "star", material "star")',
    '',
  ].join('\n')));
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-design-tool-kinds-dist-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...timed({ POLL: 3_000 }), EXACT_APP_DIR: app, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes(`127.0.0.1:${port}/`), 600_000);
    // Its design reader built: before it is, a live edit is proposed as a rebuild (theme2 10:01), and this test scrubs.
    await devSaid.until(l => l === 'design reader ready', 600_000);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const seen = [];
    const connects = () => devSaid.seen.filter(l => l.startsWith('page connected')).length;
    // The frame's build and the file, once the frame runs the design the
    // loop serves (or after 5 s).
    const look = async () => {
      const end = Date.now() + 5000;
      for (;;) {
        const design = await fetch(`${base}/__dev/design`);
        const snapshot = design.status === 200 ? await design.json() : null;
        const builds = (await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': (await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json()).token }, body: '{"method":"hello"}' })).json()).ok.builds;
        const frame = builds.find(b => b.host === 'web-js');
        // Before any edit the loop has no snapshot to give (204): the frame runs the build's own design.
        const synced = !!frame && (snapshot ? frame.design === snapshot.design : design.status === 204 && !!frame.design);
        if (synced || Date.now() > end) {
          seen.push({ synced, connects: connects(), snapshot, frame, file: readFileSync(join(app, 'app.tamber'), 'utf8') });
          return;
        }
        await Bun.sleep(100);
      }
    };
    const wait = ['clock +4000 real', 'clock data'], looked = signals('kinds');
    onTestFinished(looked.drop);
    framed(looked);
    const drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-kinds-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', ...ready(looked),
      // The length, by its amount field: proposed live, then scrubbed.
      'tap cell-space.card-0-light', 'clock data', 'type part-amount-field 14', 'clock data', 'state', 'tree typed', looked.pause(6000, 1),
      'tap scrub-start', 'clock data', 'type part-amount-field 16', 'clock data', 'state', 'tree scrub', looked.pause(6000, 2),
      // Kept, the file's change and then the rebuilt frame's registration reach the tool on its poll: `clock held`
      // waits for each push (`clock +1` runs the follow task that asks the next).
      'tap scrub-keep', 'clock data', 'clock +1', 'clock held', 'clock +1', 'clock held', ...wait, 'state', 'tree undo-bar', looked.pause(2000, 3),
      // The motion token: Rebuilds before the gesture, and no scrub.
      'tap cell-duration.card-0-light', 'clock data', 'type branch-value 300ms', 'type branch-value key Enter', 'clock data', 'state', 'tree branch', 'clock +2000 real',
      // The font-weight token: inline on the JS target.
      'tap cell-weight.title-0-light', 'clock data', 'type part-amount-field 700', 'clock data', 'state', 'tree icons',
    ], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // Looked at each `tree` op: before the scrub, during it, after Keep; each look done, the drive goes on.
    for (const n of [1, 2, 3]) {
      await out.until(() => out.seen.filter(l => l.startsWith('{"nodes"')).length >= n, 600_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
      await look();
      looked.fire(n);
    }
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const replies = out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const states = replies.filter(r => r.resources);
    const trees = replies.filter(r => r.nodes);
    const [before, during, kept] = seen;
    // Each look found the frame running the design the loop serves.
    expect(seen.map(l => l.synced)).toEqual([true, true, true]);
    expect(states[0].resources.proposal).toMatchObject({ ok: true, active: true, chip: 'live' });
    expect(trees[0].nodes.some(n => n.props.testId === 'part-amount-field')).toBe(true);
    expect(states[1].resources.scrub).toMatchObject({ ok: true, live: true, ready: true });
    // While it previews: the frame runs the scrubbed design, the same build,
    // no reload, the file untouched.
    expect(during.frame.id).toBe(before.frame.id);
    expect(during.connects).toBe(before.connects);
    expect(during.frame.design).not.toBe(before.frame.design);
    expect(JSON.stringify(during.snapshot)).toContain('16px');
    expect(during.file).toBe(original);
    // Keep: one line, the value previewed; the same build, no reload.
    const line = '  space.card = 12px';
    expect(edit(original, kept.file)).toEqual({ at: original.split('\n').indexOf(line), removed: [line], added: ['  space.card = 16px'] });
    expect([kept.frame.id, kept.connects]).toEqual([before.frame.id, before.connects]);
    // Motion: said before the gesture, proposed as a rebuild, no scrub.
    expect(trees[3].nodes.some(n => n.props.testId === 'branch-rebuilds')).toBe(true);
    expect(states[3].resources.proposal).toMatchObject({ ok: true, chip: 'rebuild' });
    // A font-weight number: the JS target writes it inline, so it rebuilds.
    expect(states[4].resources.proposal).toMatchObject({ ok: true, active: true, chip: 'rebuild' });
    // The icon, read only.
    expect(trees[4].nodes.some(n => n.props.testId === 'icon-icon.star-sources' && n.props.text === 'sf star · material star')).toBe(true);
  } finally {
    if (tool) await stop(tool);
    await stop(dev);
    rmSync(app, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}

test('stage 6 kinds on the JS loop: a typed length scrubs live and Keep writes one line, motion says Rebuilds first, an inline font-weight token rebuilds, icons listed', () => kindsOnTheJsLoop(), 900_000);
