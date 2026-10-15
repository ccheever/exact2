// The design tool (LLP 1122 T1) driven as a person would, against a real
// design server: `bun exact.mjs tamber tool` on a copy of the theme fixture
// serves it at its own URL, and the agent drives that page (`agent web
// --url`). The tool shows the server's hello, and an agent's write arrives
// on its long poll attributed. Its stub-server tests are
// apps/design-tool/app.test.contract.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { TOOL_ORIGIN } from './tamber-server.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** Lines of `child`'s stdout as they come; `until(pass)` waits for one. */
function lines(child) {
  const seen = [];
  let buf = '';
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', d => { buf += d; const all = buf.split('\n'); buf = all.pop(); seen.push(...all); });
  const until = async (pass, ms) => {
    const end = Date.now() + ms;
    while (!seen.some(pass)) { if (Date.now() > end || child.exitCode !== null) throw new Error(`not seen: ${seen.join('\n')}`); await Bun.sleep(50); }
    return seen.find(pass);
  };
  return { seen, until };
}

const stop = async child => {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exit = new Promise(ok => child.once('exit', ok));
  child.kill('SIGTERM');
  if (!(await Promise.race([exit.then(() => true), Bun.sleep(15_000).then(() => false)]))) child.kill('SIGKILL');
};

test('the tool opens on the theme fixture, shows its hello, and an agent\'s write arrives attributed; a second tool reuses it or is refused', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  let drive, other;
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
    drive = spawn(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', '--app', 'design-tool', '--url', url, '--storage', `design-tool-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'state'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // The first answer is in (the third op's reply, `clock data`): the next
    // poll will carry what comes after it.
    await out.until(() => out.seen.filter(l => l.startsWith('{')).length >= 3, 300_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const write = spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', TOOL_ORIGIN, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, encoding: 'utf8' });
    expect(write.status, write.stdout + write.stderr).toBe(0);
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const replies = out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const { hello, changes } = replies.findLast(r => r.resources).resources;
    expect(hello).toMatchObject({ ok: true, protocol: 'tamber/1.1', app });
    expect(hello.files.map(f => f.path)).toEqual(['app.tamber']);
    expect(changes.recent[0]).toMatchObject({ kind: 'changed', who: 'agent:codex', files: ['app.tamber'], tokens: ['surface.card'] });
    // One tool per machine: while it runs, another app's is refused, naming
    // the design the port serves, and this app's reuses it (its build is
    // this checkout's).
    other = mkdtempSync(join(tmpdir(), 'exact-design-tool-other-'));
    for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(other, f));
    const again = target => spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', target, '--no-open'], { cwd: ROOT, encoding: 'utf8' });
    const refused = again(other);
    expect(refused.status, refused.stderr).toBe(1);
    expect(refused.stderr).toContain(`already serves the design of ${app}; stop it first`);
    const reused = again(app);
    expect(reused.status, reused.stderr).toBe(0);
    expect(reused.stdout).toContain('already running');
  } finally {
    if (drive) await stop(drive);
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
    if (other) rmSync(other, { recursive: true, force: true });
  }
}, 900_000);

test('--via reaches only a loopback dev loop serving this app, and forwards to it', async () => {
  const copy = () => {
    const dir = mkdtempSync(join(tmpdir(), 'exact-design-tool-via-'));
    for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(dir, f));
    return dir;
  };
  const [mine, other] = [copy(), copy()];
  const run = args => spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', ...args, '--no-open'], { cwd: ROOT, encoding: 'utf8' });
  // The design server for `mine` alone, as a dev loop would hold it.
  const serve = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', mine], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool;
  try {
    const at = new URL(await lines(serve).until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
    for (const [via, said] of [['http://example.com', 'loopback URL'], [TOOL_ORIGIN, 'loopback URL'], ['http://localhost:9', 'loopback URL']]) {
      const r = run([mine, '--via', via]);
      expect(r.status, r.stderr).toBe(2);
      expect(r.stderr).toContain(said);
    }
    const wrong = run([other, '--via', at]);
    expect(wrong.status).toBe(1);
    expect(wrong.stderr).toContain(`the design of ${mine}`);
    tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', mine, '--via', at, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const { token } = await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json();
    const hello = await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    expect(hello.ok.app).toBe(mine);
    expect((await fetch(`${TOOL_ORIGIN}/__tamber/tool/`)).status).toBe(200);
  } finally {
    if (tool) await stop(tool);
    await stop(serve);
    rmSync(mine, { recursive: true, force: true });
    rmSync(other, { recursive: true, force: true });
  }
}, 900_000);

test('every control on the dials panel comes from the design as describe gives it', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-dials-'));
  cpSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), join(app, 'app.tamber'));
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const { token } = await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json();
    const ask = async request => (await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) })).json()).ok;
    const described = await ask({ method: 'describe' });
    // The stub's canned replies are still what the real server says.
    const canned = JSON.parse(readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.json'), 'utf8'));
    expect(canned.describe.ir).toEqual(described.ir);
    for (const t of described.ir.tokens) {
      expect(canned.resolve[t.name]).toEqual(await ask({ method: 'resolve', token: t.name }));
      expect(canned.explain[t.name]).toEqual(await ask({ method: 'explain', token: t.name }));
    }
    const drive = spawnSync(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', '--app', 'design-tool', '--url', url, '--storage', `design-tool-dials-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', 'state design', 'tree dials'], { cwd: ROOT, encoding: 'utf8' });
    expect(drive.status, drive.stderr).toBe(0);
    const replies = drive.stdout.split('\n').filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const panel = replies.find(r => r.groups);
    // The panel for stub/dials.tamber, as its metadata declares it: groups
    // in the order they first appear, each by priority, 3 behind "More",
    // `none` not drawn, and each control's range, step and value.
    const pick = c => ({ name: c.name, control: c.control, label: c.label, value: c.value, min: c.min, max: c.max, step: c.step });
    expect(panel.groups.map(g => ({ name: g.name, dials: g.dials.map(pick), more: g.more.map(pick) }))).toEqual([
      { name: 'Colour', dials: [
        { name: 'accent', control: 'color', label: 'Accent', value: '#1083feff', min: 0, max: 1, step: 1 },
        { name: 'lift', control: 'slider', label: 'Lift', value: '0.1', min: 0, max: 0.5, step: 0.01 },
      ], more: [] },
      { name: 'Tone', dials: [
        { name: 'hue', control: 'stepper', label: 'hue', value: '200', min: 0, max: 360, step: 5 },
        { name: 'chroma', control: 'slider', label: 'chroma', value: '0.1', min: 0, max: 0.3, step: 0.01 },
      ], more: [
        { name: 'warmth', control: 'slider', label: 'Warmth', value: '20', min: 0, max: 60, step: 1 },
      ] },
    ]);
    // Every dial describe gives, but `none`, is on it.
    const expected = described.ir.dials.filter(d => d.meta.control !== 'none').map(d => ({ name: d.name, control: d.meta.control ?? (d.kind === 'color' ? 'color' : 'slider'), more: d.meta.priority === '3' }));
    expect(panel.groups.flatMap(g => [...g.dials, ...g.more]).map(c => c.name).sort()).toEqual(expected.map(e => e.name).sort());
    // And on screen: each first-rank dial's control, by its testId, and no other.
    const tree = JSON.stringify(replies.at(-1));
    for (const e of expected) expect(tree.includes(`dial-${e.name}-${e.control}`)).toBe(!e.more);
    expect(tree).not.toContain('dial-shade');
  } finally {
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

/** `bun scripts/agent.mjs web --url <url>` running `ops`, each reply parsed;
 * `at[i]`, when given, runs as reply `i` arrives (a hand edit, a write by
 * another client), while the drive goes on. */
async function drive(url, storage, ops, at = {}) {
  const child = spawn(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', '--app', 'design-tool', '--url', url, '--storage', storage, '--json', ...ops], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  child.stderr.on('data', d => { said = (said + d).slice(-4000); });
  const out = lines(child);
  const done = new Set();
  while (child.exitCode === null) {
    const replies = out.seen.filter(l => l.startsWith('{'));
    for (const i of Object.keys(at)) if (!done.has(i) && replies.length > Number(i)) { done.add(i); at[i](); }
    await Bun.sleep(20);
  }
  if (child.exitCode !== 0) throw new Error(`the drive failed: ${said}`);
  return out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
}

/** The lines two texts differ in: [removed, added]. */
const changed = (a, b) => {
  const [x, y] = [a.split('\n'), b.split('\n')];
  return [x.filter(l => !y.includes(l)), y.filter(l => !x.includes(l))];
};

test('a dial edit and a Dim cell edit each write one line, undo gives the bytes back, and a stale undo says who changed the file', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-edit-'));
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8');
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const read = () => readFileSync(join(app, 'app.tamber'), 'utf8');
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const storage = `design-tool-edit-${process.pid}`;
    let afterDial, afterUndo, afterCell;
    const replies = await drive(url, storage, [
      'type name Ada', 'tap save-name', 'clock data',
      // A dial: propose (its chip and diff), apply, undo.
      'type dial-hue-stepper 210', 'clock data', 'state', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data',
      // The Dim dark column of surface.page: its `when`, not the default.
      'tap cell-surface.page-2-dark', 'clock data', 'state',
      'type branch-value light-dark(#ffffff, #101820)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      // A hand edit, then undo: refused, naming it.
      'clock +1500 real', 'tap undo', 'clock data', 'state',
    ], {
      7: () => { afterDial = read(); },
      11: () => { afterUndo = read(); },
      19: () => { afterCell = read(); writeFileSync(join(app, 'app.tamber'), read().replace('#101820', '#111111')); },
    });
    const states = replies.filter(r => r.resources);
    const proposal = states[0].resources.proposal;
    expect(proposal).toMatchObject({ ok: true, active: true, chip: 'rebuild' });
    expect(proposal.diff).toContain('+  hue: number = 210');
    expect(changed(original, afterDial)).toEqual([['  hue: number = 200  in 0..360  step=5 control=stepper group=Tone priority=1'], ['  hue: number = 210  in 0..360  step=5 control=stepper group=Tone priority=1']]);
    expect(afterUndo).toBe(original);
    const branch = states[1].resources.branch;
    expect(branch).toMatchObject({ when: 'dark-style=dim', own: 'dark-style=dim density=regular', exact: false, expression: false });
    expect(changed(original, afterCell)).toEqual([['    when dark-style=dim: light-dark(#ffffff, #15202b)'], ['    when dark-style=dim: light-dark(#ffffff, #101820)']]);
    const last = replies.at(-1);
    expect(last.derives.stack.error).toContain('app.tamber was changed on disk');
  } finally {
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('expressions are kept unless pinned, "only this mode" adds a when, and tokens are added, renamed and removed; undo restores an unformatted file', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-ops-'));
  // Not in fmt's layout: undo must give these bytes back, not fmt's.
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').replace('  text.primary = ', '  text.primary   =   ');
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const read = () => readFileSync(join(app, 'app.tamber'), 'utf8');
  const snaps = {};
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const replies = await drive(url, `design-tool-ops-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // accent.soft follows accent and lift: its inputs, and a pin to write a value.
      'tap cell-accent.soft-0-light', 'clock data', 'state', 'tap pin', 'state',
      // surface.inset's compact light cell shows its compact `when`; the
      // compact dark cell of surface.card has none of its own.
      'tap cell-surface.card-1-dark', 'clock data', 'tap only-this-mode', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data',
      'type new-name surface.raised', 'type new-value light-dark(#ffffff, #202020)', 'tap add-token', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.raised-0-light', 'clock data', 'type rename-to surface.lifted', 'tap rename', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.lifted-0-light', 'clock data', 'tap remove', 'clock settle', 'tap confirm-remove-yes', 'clock settle', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'state',
    ], {
      14: () => { snaps.mode = read(); },
      18: () => { snaps.undone = read(); },
      24: () => { snaps.added = read(); },
      32: () => { snaps.renamed = read(); },
      44: () => { snaps.removed = read(); },
    });
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.branch).toMatchObject({ token: 'accent.soft', expression: true, inputs: ['accent', 'lift'] });
    expect(states[1].slots.pinning).toBe(true);
    expect(changed(original, snaps.mode)).toEqual([[], ['    when dark-style=black density=compact: light-dark(#f1f3f5, #16181c)']]);
    expect(snaps.undone).toBe(original);
    expect(changed(original, snaps.added)).toEqual([[], ['  surface.raised = light-dark(#ffffff, #202020)']]);
    expect(changed(snaps.added, snaps.renamed)).toEqual([['  surface.raised = light-dark(#ffffff, #202020)'], ['  surface.lifted = light-dark(#ffffff, #202020)']]);
    expect(snaps.removed).toBe(original);
  } finally {
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('controls from metadata that does not fit, steps, select values, column labels and contrast over a background', async () => {
  const { controlOf, columnsOf, contrast } = await import('../apps/design-tool/app.ts');
  const dial = (kind, meta, value = kind === 'color' ? '#1083feff' : '0.001', range = kind === 'color' ? null : ['0', '0.005']) => controlOf({ name: 'd', kind, value, range, meta });
  // A control that does not fit its kind is the kind's own, said so.
  expect(dial('color', { control: 'stepper' })).toMatchObject({ control: 'color', help: 'control=stepper does not fit this color dial; its own control is shown.' });
  expect(dial('number', { control: 'slider' }, '1', null)).toMatchObject({ control: 'stepper' });
  expect(dial('number', { control: 'select' })).toMatchObject({ control: 'slider' });
  expect(dial('number', { control: 'none' })).toBeNull();
  // No step: one fine enough for the value; a bad step or priority is ignored.
  expect(dial('number', {}).step).toBe(0.001);
  expect(dial('number', { step: '-1', priority: '1.5' })).toMatchObject({ step: 0.001, priority: 2 });
  expect(dial('number', { step: '2px' }).step).toBe(0.001);
  // A select's value matched by meaning, else listed first.
  expect(dial('color', { control: 'select', options: '#ff0000|#1083fe' })).toMatchObject({ control: 'select', value: '#1083fe', options: ['#ff0000', '#1083fe'] });
  expect(dial('color', { control: 'select', options: '#ff0000' })).toMatchObject({ value: '#1083feff', options: ['#1083feff', '#ff0000'] });
  // Two axes with like-named values make two labels.
  const cols = columnsOf([{ name: 'a', values: ['normal', 'compact'], under: null }, { name: 'b', values: ['normal', 'compact'], under: null }]);
  expect(new Set(cols.map(c => c.label)).size).toBe(cols.length);
  // Text over a background, composited; a translucent background has no one ratio.
  expect(contrast('#00000000', '#ffffffff')).toBe(1);
  expect(contrast('#000000', '#ffffff')).toBe(21);
  expect(contrast('#000000', '#ffffff80')).toBe('translucent background');
  expect(contrast('oklch(0.5 0 0)', '#ffffff')).toBe('not a colour');
});
