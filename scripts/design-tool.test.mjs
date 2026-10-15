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
      'type name Ada', 'tap save-name', 'clock data', 'tap more-Colour', 'tap more-Tone', 'tree dials'], { cwd: ROOT, encoding: 'utf8' });
    expect(drive.status, drive.stderr).toBe(0);
    const tree = drive.stdout.split('\n').filter(l => l.startsWith('{')).map(l => JSON.parse(l)).at(-1);
    // The panel as drawn: each group's dials in screen order, "More"'s
    // after its button, and each one's control (its testId's last word),
    // label, value, range and step, options and help, all read off the
    // screen.
    const byId = new Map(tree.nodes.map(n => [n.id, n]));
    const kids = n => n.children.map(c => byId.get(c));
    const texts = n => [n, ...kids(n).flatMap(texts)].filter(m => m.type === 'Text');
    const drawn = tree.nodes.filter(n => /^group-/.test(n.props.testId ?? '')).map(g => {
      const dials = kids(g).filter(n => /^dial-[^-]+$/.test(n.props.testId ?? '') || /^more-/.test(n.props.testId ?? ''));
      return { group: g.props.testId.slice(6), dials: dials.map(n => {
        if (n.props.testId.startsWith('more-')) return 'more';
        const name = n.props.testId.slice(5);
        const of = suffix => tree.nodes.find(m => m.props.testId === `dial-${name}-${suffix}`);
        const control = ['color', 'slider', 'stepper', 'select'].find(c => of(c));
        return { name, control, label: texts(kids(n)[0])[0].props.text, value: of('value').props.text, bounds: of('bounds')?.props.text ?? '',
          options: control === 'select' ? kids(of('select')).map(o => o.props.text) : [], help: of('help')?.props.text ?? '' };
      }) };
    });
    const dial = (name, control, label, value, bounds = '', options = [], help = '') => ({ name, control, label, value, bounds, options, help });
    expect(drawn).toEqual([
      { group: 'Colour', dials: [
        dial('accent', 'color', 'Accent', '#1083feff'),
        dial('lift', 'slider', 'Lift', '0.1', '0 – 0.5 · step 0.01'),
        'more',
        dial('paint', 'select', 'paint', '#f00', '', ['#1083fe', '#f00'], 'brand.red is not a value a dial can take (a dial is a literal), so it is not offered.'),
      ] },
      { group: 'Tone', dials: [
        // hue is declared after chroma, and drawn first: priority 1.
        dial('hue', 'stepper', 'hue', '200', '0 – 360 · step 5'),
        dial('chroma', 'slider', 'chroma', '0.1', '0 – 0.3 · step 0.01'),
        'more',
        // A range in scientific notation, its step fine enough for it.
        dial('fine', 'slider', 'fine', '0.001', '0 – 0.005 · step 0.001'),
        dial('warmth', 'slider', 'Warmth', '20', '0 – 60 · step 1'),
      ] },
    ]);
    // And the panel describe gives, by the tool's own reading of each
    // dial's metadata, is the one drawn: every dial but `none`, in order.
    const { panelOf } = await import('../apps/design-tool/app.ts');
    const panel = panelOf(described.ir.dials, new Set(described.ir.tokens.map(t => t.name)));
    expect(drawn.map(g => g.dials.filter(d => d !== 'more').map(d => [d.name, d.control]))).toEqual(panel.map(g => [...g.dials, ...g.more].map(c => [c.name, c.control])));
    expect(JSON.stringify(tree)).not.toContain('dial-shade');
  } finally {
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

/** `bun scripts/agent.mjs web --url <url>` running `ops`, each reply parsed
 * (spawned, not run synchronously: the proxy below answers in this
 * process). */
async function drive(url, storage, ops) {
  const child = spawn(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', '--app', 'design-tool', '--url', url, '--storage', storage, '--json', ...ops], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  child.stderr.on('data', d => { said = (said + d).slice(-4000); });
  const out = lines(child);
  const code = await new Promise(ok => child.once('close', ok));
  if (code !== 0) throw new Error(`the drive failed: ${said}`);
  return out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
}

/** The tool on `app` through `tamber tool --via` to a proxy in front of
 * the app's design server: the proxy records each write's file as the
 * server left it (read once its reply is in, so in the order written), and
 * calls `before(request)` before forwarding a request, so a hand edit lands
 * exactly between two of the tool's requests, with no timing. */
async function proxied(app, before = () => {}) {
  const serve = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', app], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const origin = new URL(await lines(serve).until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
  const writes = [];
  const proxy = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: async req => {
    const url = new URL(req.url);
    const body = req.method === 'POST' ? await req.text() : undefined;
    let request = {};
    try { request = url.pathname === '/__tamber' && body ? JSON.parse(body) : {}; } catch { /* not ours */ }
    await before(request);
    const headers = new Headers(req.headers);
    headers.delete('host'); headers.delete('content-length');
    const res = await fetch(`${origin}${url.pathname}${url.search}`, { method: req.method, headers, body, redirect: 'manual' });
    const bytes = await res.arrayBuffer();
    if (request.method === 'transaction' || request.method === 'undo') {
      const reply = JSON.parse(new TextDecoder().decode(bytes));
      writes.push({ method: request.method, ok: !!reply.ok, error: reply.error, after: readFileSync(join(app, 'app.tamber'), 'utf8') });
    }
    const out = new Headers(res.headers);
    out.delete('content-encoding'); out.delete('content-length');
    return new Response(bytes, { status: res.status, headers: out });
  } });
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', `http://127.0.0.1:${proxy.port}`, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
  return { url, writes, close: async () => { await stop(tool); proxy.stop(true); await stop(serve); } };
}

/** Where two texts differ, in order: the first line that differs, the
 * lines taken out there and the lines put in. */
const edit = (a, b) => {
  const [x, y] = [a.split('\n'), b.split('\n')];
  let i = 0, j = 0;
  while (i < x.length && i < y.length && x[i] === y[i]) i++;
  while (j < x.length - i && j < y.length - i && x[x.length - 1 - j] === y[y.length - 1 - j]) j++;
  return { at: i, removed: x.slice(i, x.length - j), added: y.slice(i, y.length - j) };
};

test('a dial edit and a Dim cell edit each write one line, undo gives the bytes back, and a stale undo says who changed the file', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-edit-'));
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8');
  writeFileSync(join(app, 'app.tamber'), original);
  // A second design file that sorts before the app's: a write to the app's
  // design still carries the app's digest.
  writeFileSync(join(app, 'accent.tamber'), original.replace('tamber dials', 'tamber accent'));
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  let undos = 0;
  // The second undo finds the file edited by hand just before it.
  const tool = await proxied(app, request => {
    if (request.method === 'undo' && ++undos === 2) writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('#101820', '#111111'));
  });
  try {
    const replies = await drive(tool.url, `design-tool-edit-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // A dial: propose (its chip and diff), apply, undo.
      'type dial-hue-stepper 210', 'clock data', 'state', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data',
      // The Dim dark column of surface.page: its `when`, not the default.
      'clock +1', 'clock data', 'tap cell-surface.page-2-dark', 'clock data', 'state',
      'type branch-value light-dark(#ffffff, #101820)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      // A hand edit, then undo: refused, naming it.
      'clock +1', 'clock data', 'tap undo', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const proposal = states[0].resources.proposal;
    expect(proposal).toMatchObject({ ok: true, active: true, chip: 'rebuild' });
    expect(proposal.diff).toContain('+  hue: number = 210');
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['transaction', true], ['undo', true], ['transaction', true], ['undo', false]]);
    const hue = '  hue: number = 200  in 0..360  step=5 control=stepper group=Tone priority=1';
    expect(edit(original, tool.writes[0].after)).toEqual({ at: original.split('\n').indexOf(hue), removed: [hue], added: [hue.replace('200', '210')] });
    expect(tool.writes[1].after).toBe(original);
    const branch = states[1].resources.branch;
    expect(branch).toMatchObject({ when: 'dark-style=dim', own: 'dark-style=dim density=regular', exact: false, expression: false });
    const dim = '    when dark-style=dim: light-dark(#ffffff, #15202b)';
    expect(edit(original, tool.writes[2].after)).toEqual({ at: original.split('\n').indexOf(dim), removed: [dim], added: [dim.replace('#15202b', '#101820')] });
    expect(states.at(-1).derives.stack.error).toContain('app.tamber was changed on disk');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('expressions are kept unless pinned, "only this mode" adds a when, redo writes it again, and tokens are added, renamed and removed; undo restores an unformatted line', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-ops-'));
  // Not in fmt's layout: undo must give these bytes back, not fmt's.
  const primary = '  text.primary   =   light-dark(#0f1419, #e7e9ea)';
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').replace('  text.primary = light-dark(#0f1419, #e7e9ea)', primary);
  expect(original).toContain(primary);
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const tool = await proxied(app);
  try {
    const replies = await drive(tool.url, `design-tool-ops-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // accent.soft follows accent and lift: its inputs, and a pin to write
      // a value, which the checker refuses (lift would move no colour),
      // saying why.
      'tap cell-accent.soft-0-light', 'clock data', 'state', 'tap pin', 'state',
      'type branch-value #123456', 'type branch-value key Enter', 'clock data', 'state', 'tap cancel-edit',
      // A light cell passes over the under-dark `when`s, and has no "only
      // this mode" to offer.
      'tap cell-surface.page-0-light', 'clock data', 'state',
      // The compact dark cell of surface.card has no `when` of its own.
      'tap cell-surface.card-1-dark', 'clock data', 'tap only-this-mode', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data', 'tap redo', 'clock data', 'tap undo', 'clock data',
      'clock +1', 'clock data', 'type new-name surface.raised', 'type new-value light-dark(#ffffff, #202020)', 'tap add-token', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.raised-0-light', 'clock data', 'type rename-to surface.lifted', 'tap rename', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.lifted-0-light', 'clock data', 'tap remove', 'clock settle', 'tap confirm-remove-yes', 'clock settle', 'clock data', 'tap apply', 'clock data',
      // The unformatted line itself, edited and undone.
      'clock +1', 'clock data', 'tap cell-text.primary-0-light', 'clock data', 'type branch-value light-dark(#000000, #e7e9ea)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.branch).toMatchObject({ token: 'accent.soft', expression: true, inputs: [], dials: [{ name: 'accent', group: 'Colour', more: false }, { name: 'lift', group: 'Colour', more: false }] });
    expect(states[1].slots.pinning).toBe(true);
    expect(states[2].resources.proposal.ok).toBe(false);
    expect(states[2].resources.proposal.error).toContain('tamber-dial-inert');
    expect(states[3].resources.branch).toMatchObject({ token: 'surface.page', when: '', own: '', exact: true });
    const w = tool.writes;
    expect(w.map(x => [x.method, x.ok])).toEqual([
      ['transaction', true], ['undo', true], ['transaction', true], ['undo', true],
      ['transaction', true], ['transaction', true], ['transaction', true],
      ['transaction', true], ['undo', true],
    ]);
    const lines = original.split('\n');
    const card = '    when dark-style=dim: light-dark(#f1f3f5, #1e2732)';
    const mode = { at: lines.indexOf(card) + 1, removed: [], added: ['    when dark-style=black density=compact: light-dark(#f1f3f5, #16181c)'] };
    expect(edit(original, w[0].after)).toEqual(mode);
    expect(w[1].after).toBe(original);
    expect(w[2].after).toBe(w[0].after);
    expect(w[3].after).toBe(original);
    expect(edit(original, w[4].after)).toMatchObject({ removed: [], added: ['  surface.raised = light-dark(#ffffff, #202020)'] });
    expect(edit(w[4].after, w[5].after)).toMatchObject({ removed: ['  surface.raised = light-dark(#ffffff, #202020)'], added: ['  surface.lifted = light-dark(#ffffff, #202020)'] });
    expect(w[6].after).toBe(original);
    const at = lines.indexOf(primary);
    const changedPrimary = edit(original, w[7].after);
    expect(changedPrimary.at).toBe(at);
    expect(changedPrimary.removed).toEqual([primary]);
    expect(changedPrimary.added).toHaveLength(1);
    expect(changedPrimary.added[0]).toContain('#000000');
    expect(w[8].after).toBe(original);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('controls from metadata that does not fit, steps, select values, column labels, branches, inputs and contrast over a background', async () => {
  const { controlOf, columnsOf, contrast, branchOf, inputsOf } = await import('../apps/design-tool/app.ts');
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
  // A step fine enough for a range in scientific notation; a length's range in px.
  expect(dial('number', {}, '0', ['0', '5e-3']).step).toBe(0.001);
  expect(controlOf({ name: 'gap', kind: 'length', value: '12px', range: ['0px', '24px'], meta: {} })).toMatchObject({ control: 'slider', number: 12, min: 0, max: 24, step: 1, bounds: '0 – 24 · step 1' });
  // A select's colours folded as the server folds them; an option naming a
  // token (no literal) is left out, said so; with none left, the kind's own.
  expect(dial('color', { control: 'select', options: '#f00|rgb(16, 131, 254)' })).toMatchObject({ value: 'rgb(16, 131, 254)', options: ['#f00', 'rgb(16, 131, 254)'] });
  expect(controlOf({ name: 'd', kind: 'color', value: '#ff0000ff', range: null, meta: { control: 'select', options: 'brand|brand.red' } }, new Set(['brand']))).toMatchObject({ control: 'color', help: 'control=select does not fit this color dial; its own control is shown. brand, brand.red are not a value a dial can take (a dial is a literal), so they are not offered.' });
  // A light cell passes over a `when` naming an under-dark axis, as the
  // checker does; "only this mode" names every axis, and a light column
  // with an under-dark axis has none to offer.
  const axes = [{ name: 'dark-style', values: ['black', 'dim'], under: 'dark' }, { name: 'density', values: ['regular', 'compact'], under: null }];
  const [light1, dark3] = [columnsOf(axes).find(c => c.id === '1-light'), columnsOf(axes).find(c => c.id === '3-dark')];
  const whens = [{ when: [{ axis: 'dark-style', value: 'black' }, { axis: 'density', value: 'compact' }], value: 'a' }, { when: [{ axis: 'density', value: 'compact' }], value: 'b' }];
  expect(branchOf(axes, light1, whens)).toMatchObject({ when: 'density=compact', index: 1, own: '', tie: '' });
  expect(branchOf(axes, dark3, whens)).toMatchObject({ when: 'density=compact', own: 'dark-style=dim density=compact' });
  expect(branchOf(axes, dark3, [...whens, { when: [{ axis: 'dark-style', value: 'dim' }], value: 'c' }]).tie).toBe('when density=compact and when dark-style=dim');
  // A value's names as the checker reads them: a role by its bare name in
  // any case; no word inside a hex colour.
  expect(inputsOf('LABEL', ['-exact-label'], [])).toEqual({ tokens: ['-exact-label'], dials: [] });
  expect(inputsOf('oklch(from accent calc(l + lift) c h)', ['accent.soft'], ['accent', 'lift'])).toEqual({ tokens: [], dials: ['accent', 'lift'] });
  expect(inputsOf('#accent0', ['accent0'], [])).toEqual({ tokens: [], dials: [] });
  // Two axes with like-named values make two labels.
  const cols = columnsOf([{ name: 'a', values: ['normal', 'compact'], under: null }, { name: 'b', values: ['normal', 'compact'], under: null }]);
  expect(new Set(cols.map(c => c.label)).size).toBe(cols.length);
  // Text over a background, composited; a translucent background has no one ratio.
  expect(contrast('#00000000', '#ffffffff')).toBe(1);
  expect(contrast('#000000', '#ffffff')).toBe(21);
  expect(contrast('#000000', '#ffffff80')).toBe('translucent background');
  expect(contrast('oklch(0.5 0 0)', '#ffffff')).toBe('not a colour');
});
