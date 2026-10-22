// The design tool's live scrubs on a real dev loop: a token's on the frame's build (T2), and stage 6's kinds on the
// JS loop (split from design-tool.test.mjs, so a lane runs them beside the others).
import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, ROOT, TOOL_ORIGIN, edit, lines, stop, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('T2: a scrub chosen on a token previews on the frame\'s build with no reload and no file written, Keep writes one line, Cancel restores, and an edit that rebuilds has no scrub', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-scrub-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-design-tool-scrub-dist-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes(`127.0.0.1:${port}/`), 600_000);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    // What the dev loop serves its builds now (`/__dev/design`), the frame's
    // build (its id, and the design it reports running), how many pages
    // have connected, and the file: read while the drive holds each moment
    // (a `tree` op, then a real-clock pause), waiting there until the frame
    // reports the design the loop serves.
    const seen = [];
    const connects = () => devSaid.seen.filter(l => l.startsWith('page connected')).length;
    const look = async () => {
      const end = Date.now() + 5000;
      for (;;) {
        const design = await fetch(`${base}/__dev/design`);
        const snapshot = design.status === 200 ? await design.json() : null;
        const builds = (await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': (await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json()).token }, body: '{"method":"hello"}' })).json()).ok.builds;
        const frame = builds.find(b => b.host === 'web-js');
        if ((snapshot && frame?.design === snapshot.design) || Date.now() > end) {
          seen.push({ connects: connects(), snapshot, frame, file: readFileSync(join(app, 'app.tamber'), 'utf8') });
          return;
        }
        await Bun.sleep(100);
      }
    };
    // Real time in one step, not several: a jump that crosses one of the tool's timers first waits on what
    // is in flight, up to 20 s (the agent's bound; the server holds the tool's changes poll 25 s), and a
    // `+N real` step counts that wait as time passed.
    const wait = ['clock +4000 real', 'clock data'];
    const drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-scrub-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', ...wait,
      // accent.fill, the default branch: a value, its proposal live (the
      // moment before any scrub), then scrubbed to another.
      'tap cell-accent.fill-0-light', 'clock data', 'type branch-value #ff0000', 'type branch-value key Enter', 'clock data', 'state', 'tree proposal', 'clock +6000 real',
      // (A scrub with no move for 30 s ends: no long waits while one is open.)
      'tap scrub-start', 'clock data', 'type branch-value #ff1100', 'type branch-value key Enter', 'clock data', 'state',
      'tree scrub', 'clock +6000 real',
      'tap scrub-keep', 'clock data', ...wait, 'state', 'tree undo-bar', 'clock +2000 real',
      // Undone, in one step.
      'tap undo', 'clock data', ...wait, 'state',
      // Again, cancelled.
      'tap cell-accent.fill-0-light', 'clock data', 'type branch-value #00ff00', 'type branch-value key Enter', 'clock data',
      'tap scrub-start', 'clock data', 'tree scrub', 'clock +6000 real', 'tap scrub-cancel', 'clock data', ...wait, 'state',
      // A role override added is structure: it rebuilds, and offers no
      // scrub. (Its value is live since LLP 1121 stage 5; a token no view
      // names, in this copy with no data crate, is in no plan, so adding
      // one is live: D7's retention.)
      'type new-name -exact-label', 'type new-value #ff0000', 'tap add-token', 'clock data', 'state',
      'tree proposal', 'clock +6000 real',
    ], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // Looked at each `tree` op: before the scrub, during it, after Keep,
    // during the second, and after its cancel (before the last edit applies).
    for (const n of [1, 2, 3, 4, 5]) {
      await out.until(() => out.seen.filter(l => l.startsWith('{"nodes"')).length >= n, 600_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
      await look();
    }
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const replies = out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const states = replies.filter(r => r.resources);
    const trees = replies.filter(r => r.nodes);
    const [before, during, kept, cancelling, after] = seen;
    expect(states[0].resources.proposal).toMatchObject({ ok: true, active: true, chip: 'live' });
    expect(states[1].resources.scrub).toMatchObject({ ok: true, live: true, ready: true });
    expect(states[1].resources.scrub.open).toMatch(/^scrub-/);
    expect(trees[1].nodes.some(n => n.props.text === 'Previewing: the running copies show this value, and the files are unchanged until Keep.')).toBe(true);
    // While it previews: the frame's build runs the scrub's design (the one
    // the loop serves, with the scrubbed value), not the committed one; the
    // same build, no page connected since before the scrub (no reload); the
    // file untouched.
    expect(before.frame.design).toBe(before.snapshot?.design ?? before.frame.design);
    expect(JSON.stringify(during.snapshot)).toContain('ff1100');
    expect(during.frame.design).toBe(during.snapshot.design);
    expect(during.frame.design).not.toBe(before.frame.design);
    expect(during.frame.id).toBe(before.frame.id);
    expect(during.connects).toBe(before.connects);
    expect(during.file).toBe(original);
    // Keep: one line written, the value previewed; Undo: the bytes back.
    expect(states[2].slots.written).toMatchObject({ ok: true });
    const line = '  accent.fill = #1d9bf0';
    expect(edit(original, kept.file)).toEqual({ at: original.split('\n').indexOf(line), removed: [line], added: [line.replace('#1d9bf0', '#ff1100')] });
    // The frame runs the kept design, the same build, no reload.
    expect(JSON.stringify(kept.snapshot)).toContain('ff1100');
    expect(kept.frame.design).toBe(kept.snapshot.design);
    expect([kept.frame.id, kept.connects]).toEqual([before.frame.id, before.connects]);
    expect(states[2].resources.changes.undo).toBe(1);
    expect(states[3].resources.changes.undo).toBe(0);
    // The second scrub previews too; Cancel: nothing written, and the frame
    // back on the committed design (the original's, after the undo).
    expect(JSON.stringify(cancelling.snapshot)).toContain('00ff00');
    expect(cancelling.frame.design).toBe(cancelling.snapshot.design);
    expect(cancelling.file).toBe(original);
    expect(states[4].slots.written).toMatchObject({ ok: true, note: 'Cancelled: nothing was written, and the running copies have the committed value back.' });
    expect(after.file).toBe(original);
    expect(after.frame.design).toBe(before.frame.design);
    expect([cancelling.frame.id, after.frame.id]).toEqual([before.frame.id, before.frame.id]);
    expect(after.connects).toBe(before.connects);
    // The role override added: rebuilds, and Scrub live is not offered. (A
    // set-token edit that rebuilds says why it has no scrub: the stub's
    // contract test. Not applied.)
    expect(states[5].resources.proposal).toMatchObject({ ok: true, chip: 'rebuild' });
    expect(trees[4].nodes.some(n => n.props.testId === 'chip' && n.props.text === 'Rebuilds')).toBe(true);
    expect(trees[4].nodes.some(n => n.props.testId === 'scrub-start')).toBe(false);
  } finally {
    if (tool) await stop(tool);
    await stop(dev);
    rmSync(app, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);

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
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes(`127.0.0.1:${port}/`), 600_000);
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
    const wait = ['clock +4000 real', 'clock data'];
    const drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-kinds-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', ...wait,
      // The length, by its amount field: proposed live, then scrubbed.
      'tap cell-space.card-0-light', 'clock data', 'type part-amount-field 14', 'clock data', 'state', 'tree typed', 'clock +6000 real',
      'tap scrub-start', 'clock data', 'type part-amount-field 16', 'clock data', 'state', 'tree scrub', 'clock +6000 real',
      'tap scrub-keep', 'clock data', ...wait, 'state', 'tree undo-bar', 'clock +2000 real',
      // The motion token: Rebuilds before the gesture, and no scrub.
      'tap cell-duration.card-0-light', 'clock data', 'type branch-value 300ms', 'type branch-value key Enter', 'clock data', 'state', 'tree branch', 'clock +2000 real',
      // The font-weight token: inline on the JS target.
      'tap cell-weight.title-0-light', 'clock data', 'type part-amount-field 700', 'clock data', 'state', 'tree icons', 'clock +2000 real',
    ], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // Looked at each `tree` op: before the scrub, during it, after Keep.
    for (const n of [1, 2, 3]) {
      await out.until(() => out.seen.filter(l => l.startsWith('{"nodes"')).length >= n, 600_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
      await look();
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
