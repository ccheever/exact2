// The design tool's live scrubs on a real dev loop: a token's on the frame's build (T2) (split from
// design-tool.test.mjs, so a lane runs it beside the others; stage 6's kinds are design-tool-kinds.test.mjs).
import { expect, onTestFinished, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, HEARTBEAT_RACE, ROOT, TOOL_ORIGIN, edit, lines, signals, stop, timed, toolTurns } from './design-tool-harness.mjs';

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

test('T2: a scrub chosen on a token previews on the frame\'s build with no reload and no file written, Keep writes one line, Cancel restores, and an edit that rebuilds has no scrub', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-scrub-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-design-tool-scrub-dist-'));
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
    // What the dev loop serves its builds now (`/__dev/design`), the frame's
    // build (its id, and the design it reports running), how many pages
    // have connected, and the file: read while the drive holds each moment
    // (a `tree` op, then a real-clock pause the look ends, its signal),
    // waiting there until the frame reports the design the loop serves.
    const seen = [];
    const connects = () => devSaid.seen.filter(l => l.startsWith('page connected')).length;
    const look = async (first) => {
      const end = Date.now() + 5000;
      for (;;) {
        const design = await fetch(`${base}/__dev/design`);
        const snapshot = design.status === 200 ? await design.json() : null;
        const builds = (await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': (await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json()).token }, body: '{"method":"hello"}' })).json()).ok.builds;
        const frame = builds.find(b => b.host === 'web-js');
        // Before any scrub the loop has no snapshot to give (204): the first look's frame runs the build's own design.
        if ((snapshot ? frame?.design === snapshot.design : first && design.status === 204 && !!frame?.design) || Date.now() > end) {
          seen.push({ connects: connects(), snapshot, frame, file: readFileSync(join(app, 'app.tamber'), 'utf8') });
          return;
        }
        await Bun.sleep(100);
      }
    };
    const wait = ['clock +4000 real', 'clock data'], looked = signals('scrub');
    onTestFinished(looked.drop);
    // The frame's build registered (the server lists it): the drive's signal 0, before its first edit.
    framed(looked);
    const drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-scrub-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', ...ready(looked),
      // accent.fill, the default branch: a value, its proposal live (the
      // moment before any scrub), then scrubbed to another.
      'tap cell-accent.fill-0-light', 'clock data', 'type branch-value #ff0000', 'type branch-value key Enter', 'clock data', 'state', 'tree proposal', looked.pause(6000, 1),
      // (A scrub with no move for 30 s ends: no long waits while one is open.)
      'tap scrub-start', 'clock data', 'type branch-value #ff1100', 'type branch-value key Enter', 'clock data', 'state',
      'tree scrub', looked.pause(6000, 2),
      'tap scrub-keep', 'clock data', ...wait, 'state', 'tree undo-bar', looked.pause(2000, 3),
      // Undone, in one step.
      'tap undo', 'clock data', ...wait, 'state',
      // Again, cancelled.
      'tap cell-accent.fill-0-light', 'clock data', 'type branch-value #00ff00', 'type branch-value key Enter', 'clock data',
      'tap scrub-start', 'clock data', 'tree scrub', looked.pause(6000, 4), 'tap scrub-cancel', 'clock data', ...wait, 'state',
      // A role override added is structure: it rebuilds, and offers no
      // scrub. (Its value is live since LLP 1121 stage 5; a token no view
      // names, in this copy with no data crate, is in no plan, so adding
      // one is live: D7's retention.)
      'type new-name -exact-label', 'type new-value #ff0000', 'tap add-token', 'clock data', 'state',
      'tree proposal', looked.pause(6000, 5),
    ], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // Looked at each `tree` op: before the scrub, during it, after Keep,
    // during the second, and after its cancel (before the last edit applies);
    // each look done, the drive goes on.
    for (const n of [1, 2, 3, 4, 5]) {
      await out.until(() => out.seen.filter(l => l.startsWith('{"nodes"')).length >= n, 600_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
      await look(n === 1);
      looked.fire(n);
    }
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const replies = out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const states = replies.filter(r => r.resources);
    const trees = replies.filter(r => r.nodes);
    const [before, during, kept, cancelling, after] = seen;
    expect(states[0].resources.proposal, HEARTBEAT_RACE).toMatchObject({ ok: true, active: true, chip: 'live' });
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
