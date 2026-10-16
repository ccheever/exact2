// The design tool (LLP 1122 T1) driven as a person would, against a real
// design server: `bun exact.mjs tamber tool` on a copy of the theme fixture
// serves it at its own URL, and the agent drives that page (`agent web
// --url`). The tool shows the server's hello, and an agent's write arrives
// on its long poll attributed. Its stub-server tests are
// apps/design-tool/app.test.contract.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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
    expect(hello).toMatchObject({ ok: true, protocol: 'tamber/1.2', app });
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
      'type name Ada', 'tap save-name', 'clock data', 'tap more-Colour', 'tap more-Tone', 'tree dials',
      // The sliders' own range and step, which the page keeps a value to:
      // a step of 0.001 to 0.005, a value past the end held at it.
      'type dial-fine-slider 0.004', 'state', 'type dial-fine-slider 1', 'state', 'type dial-lift-slider 0.374', 'state', 'type dial-chroma-slider -1', 'state'], { cwd: ROOT, encoding: 'utf8' });
    expect(drive.status, drive.stderr).toBe(0);
    const out = drive.stdout.split('\n').filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const tree = out.find(r => r.nodes);
    expect(out.filter(r => r.slots).map(r => [r.slots.editTarget, r.slots.editValue])).toEqual([['fine', '0.004'], ['fine', '0.005'], ['lift', '0.37'], ['chroma', '0']]);
    // The panel as drawn: each group's dials in screen order, "More"'s
    // after its button, and each one's control (its testId's last word),
    // label, value, range and step, options and help, all read off the
    // screen.
    const byId = new Map(tree.nodes.map(n => [n.id, n]));
    const kids = n => n.children.map(c => byId.get(c));
    const texts = n => [n, ...kids(n).flatMap(texts)].filter(m => m.type === 'Text');
    const drawn = tree.nodes.filter(n => /^group-/.test(n.props.testId ?? '')).map(g => {
      // A group's own children are its dials' boxes and its More button (a
      // dial's name may hold a hyphen, so no pattern on the name).
      const dials = kids(g).filter(n => /^(dial|more)-/.test(n.props.testId ?? ''));
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
  if (code !== 0) throw new Error(`the drive failed: ${said}\n${out.seen.slice(-3).join('\n').slice(0, 3000)}`);
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
    if (request.method === 'transaction' || request.method === 'undo' || request.method === 'accept') {
      const reply = JSON.parse(new TextDecoder().decode(bytes));
      writes.push({ method: request.method, ok: !!reply.ok, error: reply.error, after: readFileSync(join(app, 'app.tamber'), 'utf8') });
    }
    const out = new Headers(res.headers);
    out.delete('content-encoding'); out.delete('content-length');
    return new Response(bytes, { status: res.status, headers: out });
  } });
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', `http://127.0.0.1:${proxy.port}`, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
  return { url, origin, writes, close: async () => { await stop(tool); proxy.stop(true); await stop(serve); } };
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
  const primary = '  text.primary   =   light-dark(#0f1419, #e7e9ea)  pair=surface.page';
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').replace('  text.primary = light-dark(#0f1419, #e7e9ea)  pair=surface.page', primary);
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
      // accent.fill follows accent, which accent.soft still follows: its pin
      // is taken, one line, and undone to the bytes.
      'tap cell-accent.fill-0-light', 'clock data', 'tap pin', 'type branch-value #2070f0', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data',
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
    const pin = w.splice(0, 2);
    expect(pin.map(x => [x.method, x.ok])).toEqual([['transaction', true], ['undo', true]]);
    const fill = '  accent.fill = accent';
    expect(edit(original, pin[0].after)).toEqual({ at: original.split('\n').indexOf(fill), removed: [fill], added: ['  accent.fill = #2070f0'] });
    expect(pin[1].after).toBe(original);
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

/** An app with the fixture design in a git repository of its own: `app`,
 * and git in it. */
function repo(name) {
  const app = mkdtempSync(join(tmpdir(), `exact-design-tool-${name}-`));
  writeFileSync(join(app, 'app.tamber'), readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8'));
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a"\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app, encoding: 'utf8' });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  // Who the server's commit is by, here, whatever this machine's git says.
  git('config', 'user.email', 't@example.test'); git('config', 'user.name', 't');
  return { app, git };
}

test('commit takes the session\'s writes, leaves other staging and never pushes; history reads it as edits, and revert holds a past commit\'s inverse', async () => {
  const { app, git } = repo('commit');
  // A past design commit to revert, other staging to leave alone, and a
  // remote that must not move.
  writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('surface.inset = light-dark(#f8f9fa, #101214)', 'surface.inset = light-dark(#f8f9fb, #101214)'));
  git('commit', '-q', '-am', 'a warmer inset');
  const past = git('rev-parse', 'HEAD').stdout.trim();
  writeFileSync(join(app, 'notes.txt'), 'one\n'); git('add', 'notes.txt'); git('commit', '-q', '-m', 'notes');
  writeFileSync(join(app, 'notes.txt'), 'two\n'); git('add', 'notes.txt');
  const staged = git('ls-files', '-s', 'notes.txt').stdout;
  const remote = mkdtempSync(join(tmpdir(), 'exact-design-tool-remote-'));
  spawnSync('git', ['init', '-q', '--bare', remote]);
  git('remote', 'add', 'origin', remote); git('push', '-q', 'origin', 'HEAD:main');
  const pushed = spawnSync('git', ['ls-remote', remote], { encoding: 'utf8' }).stdout;
  // A hand edit, just before history is read: its row says what moved.
  const tool = await proxied(app, request => {
    if (request.method === 'history') writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)'));
  });
  try {
    const replies = await drive(tool.url, `design-tool-commit-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      'type dial-hue-stepper 210', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      'tap commit', 'clock data', 'state',
      'tap toggle-history', 'clock data', 'state',
      `tap revert-${past.slice(0, 12)}`, 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const entry = states[0].resources.changeset.entries[0];
    expect(entry).toMatchObject({ who: 'design-tool:Ada', mine: true, committed: false });
    expect(entry.text).toMatch(/^app\.tamber: tone\.base oklch\(.+\) → oklch\(.+\) · design-tool:Ada$/);
    expect(entry.diff).toContain('+  hue: number = 210');
    const made = git('log', '-1', '--format=%H%n%B').stdout;
    expect(made).toContain('design: 1 change by design-tool:Ada');
    expect(made).toContain(entry.text);
    expect(git('show', '--name-only', '--format=', 'HEAD').stdout.trim()).toBe('app.tamber');
    expect(git('ls-files', '-s', 'notes.txt').stdout).toBe(staged);
    expect(spawnSync('git', ['ls-remote', remote], { encoding: 'utf8' }).stdout).toBe(pushed);
    expect(states[1].slots.committed).toEqual({ ok: true, error: '', commit: made.slice(0, 12), files: ['app.tamber'], refused: [], edits: [{ n: 0, text: entry.text }] });
    expect(states[1].resources.changeset.entries[0].committed).toBe(true);
    // Nothing left to commit: Commit is off.
    expect(states[1].resources.scope).toEqual({ open: 0, refused: false, unavailable: false });
    const history = states[2].resources.history;
    expect(history.commits[0]).toMatchObject({ subject: 'design: 1 change by design-tool:Ada', files: [{ file: 'app.tamber', refused: '', edits: ['hue 200 → 210'] }] });
    expect(history.commits.find(c => c.commit === past).files[0].edits).toEqual(['surface.inset light-dark(#f8f9fa, #101214) → light-dark(#f8f9fb, #101214)']);
    expect(states[3].slots.reverted).toMatchObject({ ok: true, commit: past });
    // Held, not written: in the inbox, the file as it was.
    expect(states[3].resources.changes.held.map(h => h.proposal)).toContain(states[3].slots.reverted.proposal);
    expect(readFileSync(join(app, 'app.tamber'), 'utf8')).toContain('surface.inset = light-dark(#f8f9fb, #101214)');
    const hand = states[3].resources.changeset.entries.find(e => e.who === 'changed on disk');
    expect(hand.text).toBe('app.tamber: tone.warm oklch(0.7 0.1 20) → oklch(0.71 0.1 20) · changed on disk');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
    rmSync(remote, { recursive: true, force: true });
  }
}, 900_000);

test('commit refuses a file dirty or staged at its baseline, or edited by hand between two writes, saying why', async () => {
  const cases = [
    ['dirty', ({ app }) => writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)')), 'it differed from HEAD when the session first wrote it'],
    ['staged', ({ app, git }) => { writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)')); git('add', 'app.tamber'); }, 'it was staged when the session first wrote it'],
    ['hand', () => {}, 'it was changed by someone other than this server'],
  ];
  for (const [name, before, why] of cases) {
    const r = repo(name);
    before(r);
    // Between the two writes, a hand edit: just before the tool proposes
    // the second.
    let proposals = 0;
    const tool = await proxied(r.app, request => {
      if (name === 'hand' && request.method === 'propose' && request.ops?.[0]?.op === 'set-token' && ++proposals === 1) writeFileSync(join(r.app, 'app.tamber'), readFileSync(join(r.app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)'));
    });
    try {
      const head = r.git('rev-parse', 'HEAD').stdout;
      const replies = await drive(tool.url, `design-tool-refuse-${name}-${process.pid}`, [
        'type name Ada', 'tap save-name', 'clock data',
        'type dial-hue-stepper 210', 'clock data', 'tap apply', 'clock data', 'clock +1', 'clock data',
        'tap cell-surface.page-2-dark', 'clock data', 'type branch-value light-dark(#ffffff, #101820)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data', 'clock +1', 'clock data',
        'tap commit', 'clock data', 'tree commit-refused', 'state',
      ]);
      expect(tool.writes.map(w => [w.method, w.ok]), name).toEqual([['transaction', true], ['transaction', true]]);
      expect(replies.at(-1).slots.committed, name).toMatchObject({ ok: false, refused: [{ file: 'app.tamber', why }] });
      expect(JSON.stringify(replies.at(-2)), name).toContain(`app.tamber: ${why}`);
      expect(r.git('rev-parse', 'HEAD').stdout, name).toBe(head);
    } finally {
      await tool.close();
      rmSync(r.app, { recursive: true, force: true });
    }
  }
}, 900_000);

test('an agent\'s write arrives attributed within one poll, and held proposals are accepted whole, in part and recomputed when stale', async () => {
  const { app } = repo('agents');
  const read = () => readFileSync(join(app, 'app.tamber'), 'utf8');
  let tool, accepts = 0;
  const agentWrite = async () => {
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const op = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', tool.origin, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, stdio: 'ignore' });
    expect(await new Promise(ok => op.once('close', ok))).toBe(0);
  };
  let explains = 0;
  tool = await proxied(app, async request => {
    // The agent writes just before the tool's first explain; a hand edit
    // lands just before the third accept, so that proposal is stale.
    if (request.method === 'explain' && ++explains === 1) await agentWrite();
    if (request.method === 'accept' && ++accepts === 3) writeFileSync(join(app, 'app.tamber'), read().replace('accent.fill = accent', 'accent.fill = #3070f0'));
  });
  try {
    const { token } = await (await fetch(`${tool.origin}/__tamber/token`)).json();
    const ask = async request => (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) })).json();
    const p1 = (await ask({ method: 'propose', hold: true, client: 'agent:codex', ops: [{ op: 'set-token', name: 'text.primary', value: 'light-dark(#101010, #e7e9ea)' }, { op: 'set-token', name: 'ink.on-brand', value: '#fefefe' }] })).ok.proposal;
    const p2 = (await ask({ method: 'propose', hold: true, client: 'agent:claude', ops: [{ op: 'set-token', name: 'accent.fill', value: '#2070f0' }] })).ok.proposal;
    const id = n => p1.replace(/-1$/, `-${n}`);
    expect(p2).toBe(id(2));
    const replies = await drive(tool.url, `design-tool-agents-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      'tap cell-surface.page-0-light', 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      // Part of p1: the server recomputes it as a new proposal, reviewed afresh.
      `tap pick-${p1}-0`, `tap accept-picked-${p1}`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap accept-${id(3)}`, 'clock data', 'clock +1', 'clock data', 'state',
      // p2, with the file moved since: recomputed, then accepted.
      `tap accept-${p2}`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap accept-${id(4)}`, 'clock data', 'clock +1', 'clock data', 'state',
      // An accepted proposal is one step: undone, and redone.
      'tap undo', 'clock data', 'clock +1', 'clock data', 'tap redo', 'clock data', 'clock +1', 'clock data', 'state',
      // And highlights fade, oldest first, one each 3 s.
      ...Array.from({ length: 8 }, () => ['clock +3000', 'clock data']).flat(), 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const [first, part, whole, stale, fresh, redone, faded] = states;
    expect(first.resources.changeset.entries[0]).toMatchObject({ who: 'agent:codex', mine: false });
    expect(first.resources.changeset.entries[0].text).toBe('app.tamber: surface.card light-dark(#f1f3f5ff, #16181cff) → light-dark(#f1f3f5ff, #17191dff) · agent:codex');
    expect(first.resources.lit).toMatchObject({ names: ['surface.card'], whos: ['agent:codex'] });
    // Only the cells that moved: the dark side, where the default applies.
    expect(first.resources.lit.cells).toEqual(['surface.card|0-dark', 'surface.card|1-dark']);
    // Each with the tokens it changes (a role that follows one among them).
    expect(first.resources.changes.held.map(h => [h.proposal, h.client, [...h.tokens].sort(), h.items.map(i => i.text)])).toEqual([
      [p1, 'agent:codex', ['-exact-label', 'ink.on-brand', 'text.primary'], ['text.primary = light-dark(#101010, #e7e9ea) (app.tamber)', 'ink.on-brand = #fefefe (app.tamber)']],
      [p2, 'agent:claude', ['accent.fill'], ['accent.fill = #2070f0 (app.tamber)']],
    ]);
    expect(part.slots.written.note).toContain(`as ${id(3)}`);
    const recomputed = part.resources.changes.held.find(h => h.proposal === id(3));
    expect(recomputed).toMatchObject({ replaces: p1 });
    expect([...recomputed.tokens].sort()).toEqual(['-exact-label', 'text.primary']);
    expect(recomputed.diff).toContain('#101010');
    expect(recomputed.diff).not.toContain('#fefefe');
    expect(whole.slots.written.note).toBe(`Accepted ${id(3)}.`);
    expect(stale.slots.written.note).toContain(`as ${id(4)}`);
    const restaled = stale.resources.changes.held.find(h => h.proposal === id(4));
    expect(restaled).toMatchObject({ replaces: p2, client: 'agent:claude' });
    // Its diff is the new one, from the hand edit's value.
    expect(restaled.diff).toContain('-  accent.fill = #3070f0');
    expect(fresh.slots.written.note).toBe(`Accepted ${id(4)}.`);
    expect(fresh.resources.changes.held).toEqual([]);
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['accept', true], ['accept', true], ['accept', true], ['accept', true], ['undo', true], ['transaction', true]]);
    expect(tool.writes[4].after).toBe(tool.writes[2].after);
    expect(tool.writes[5].after).toBe(tool.writes[3].after);
    expect(redone.slots.written).toMatchObject({ ok: true, last: 'redo' });
    // Redone as its proposer's.
    expect(redone.resources.changeset.entries[0].who).toBe('agent:claude');
    expect(redone.resources.changeset.entries[0].text).toContain('redone by design-tool:Ada');
    expect(faded.resources.lit.names).toEqual([]);
    const now = read();
    expect(now).toContain('text.primary = light-dark(#101010, #e7e9ea)');
    expect(now).not.toContain('#fefefe');
    expect(now).toContain('accent.fill = #2070f0');
    expect(now).not.toContain('accent.fill = #3070f0');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('T1a: an import reviewed through the file picker, overrides off until ticked, every unmapped value listed, accepted as one transaction and undone in one step', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-import-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const fixtures = resolve(ROOT, 'tamber/tests/import');
  const figma = readdirSync(join(fixtures, 'figma')).map(n => join(fixtures, 'figma', n));
  const tool = await proxied(app);
  try {
    const replies = await drive(tool.url, `design-tool-import-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // shadcn's default theme, picked as a person would.
      'type import-kind shadcn', 'tap import-choose', `type @import-file ${join(fixtures, 'shadcn-neutral.css')}`, 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const review = states[0].resources.changes.held.find(h => h.importer === 'shadcn');
    expect(review.counted).toBe('47 source values = 46 mapped + 1 reported');
    expect(review.countOk).toBe(true);
    expect(review.mapped).toHaveLength(23);
    expect(review.mapped[0]).toMatchObject({ name: 'shadcn.background', modes: [{ label: 'default', light: 'oklch(1 0 0)', dark: 'oklch(0.145 0 0)' }] });
    // Every override off until ticked, every unmapped value with its reason.
    expect(review.offers.map(o => [o.role, o.on])).toEqual([['-exact-background', false], ['-exact-label', false], ['-exact-secondary-background', false], ['-exact-separator', false], ['AccentColor', false]]);
    expect(states[0].slots.picked).toEqual([]);
    expect(review.unmapped.map(u => u.name)).toEqual(['--radius', '--radius-sm', '--radius-md', '--radius-lg', '--radius-xl']);
    expect(review.unmapped[0].why).toBe('kind pending (stage 6): length');
    const id = review.proposal, nth = k => id.replace(/-(\d+)$/, (_, n) => `-${Number(n) + k}`);
    const [edited, kept] = [nth(1), nth(2)];
    // Tick AccentColor, leave shadcn.foreground out, and edit one value: the
    // edit holds again what the card had chosen, with the edit, its
    // AccentColor on. Untick it there and Accept: the server recomputes
    // without it (nothing written), and that is accepted as one
    // transaction; one Undo gives the bytes back.
    const more = await drive(tool.url, `design-tool-import-${process.pid}`, [
      'clock data', 'clock +1', 'clock data',
      `tap override-${id}-AccentColor`, `tap take-${id}-1`,
      `type mapped-value-${id}-0-0 light-dark(oklch(0.99 0 0), oklch(0.145 0 0))`, `type mapped-value-${id}-0-0 key Enter`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap override-${edited}-AccentColor`, `tap accept-${edited}`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap accept-${kept}`, 'clock data', 'clock +1', 'clock data', 'state',
      'tap undo', 'clock data', 'clock +1', 'clock data', 'state',
      // Tailwind's default theme, and Figma's export (a file a mode).
      'type import-kind tailwind', 'tap import-choose', `type @import-file ${join(fixtures, 'tailwind-theme.css')}`, 'clock data', 'clock +1', 'clock data', 'state',
      'type import-kind dtcg', 'tap import-choose', `type @import-file ${figma.join('\n')}`, 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    const later = more.filter(r => r.resources);
    const after = later[0].resources.changes.held.find(h => h.proposal === edited);
    expect(after).toMatchObject({ from: id, kept: true });
    expect(after.mapped[0].modes[0].value).toBe('light-dark(oklch(0.99 0 0), oklch(0.145 0 0))');
    expect(after.mapped.map(m => m.name)).not.toContain('shadcn.foreground');
    expect(after.offers.filter(o => o.items !== '').map(o => [o.role, o.on])).toEqual([['AccentColor', true]]);
    expect(after.diff).toContain('AccentColor = shadcn.primary');
    expect(later[1].slots.written.note).toContain(`as ${kept}`);
    expect(later[1].resources.changes.held.find(h => h.proposal === kept)).toMatchObject({ replaces: edited });
    expect(later[2].slots.written.note).toBe(`Accepted ${kept}.`);
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['accept', true], ['accept', true], ['undo', true]]);
    // The partial accept wrote nothing; the accept wrote what was chosen.
    expect(tool.writes[0].after).toBe(original);
    const written = tool.writes[1].after;
    expect(written).not.toContain('AccentColor = shadcn.primary');
    expect(written).toContain('shadcn.background = light-dark(oklch(0.99 0 0), oklch(0.145 0 0))');
    expect(written).not.toContain('shadcn.foreground =');
    expect(written).not.toContain('-exact-background = shadcn.background');
    // One step back: the bytes as they were.
    expect(tool.writes[2].after).toBe(original);
    const tailwind = later[4].resources.changes.held.find(h => h.importer === 'tailwind');
    expect(tailwind.countOk).toBe(true);
    expect(tailwind.mapped.length).toBeGreaterThan(200);
    const dtcg = later[5].resources.changes.held.find(h => h.importer === 'dtcg');
    expect(dtcg.counted).toBe('104 source values = 40 mapped + 64 reported');
    expect(dtcg.unmapped).toHaveLength(64);
    // Its axis is said as an axis; a reference is painted as the token it
    // names, in the same mode.
    expect(dtcg.mapped[0]).toMatchObject({ kind: 'axis', modes: [] });
    const byName = new Map(dtcg.mapped.map(m => [m.name, m]));
    const ref = dtcg.mapped.find(m => m.kind === 'token' && byName.has(m.modes[0].value));
    expect(ref.modes[0].light).toBe(byName.get(ref.modes[0].value).modes[0].light);
    // A token's other mode, edited and accepted, is written.
    const moded = dtcg.mapped.find(m => m.modes.length > 1);
    const figmaId = dtcg.proposal, figmaEdited = figmaId.replace(/-(\d+)$/, (_, n) => `-${Number(n) + 1}`);
    const last = await drive(tool.url, `design-tool-import-${process.pid}`, [
      'clock data', 'clock +1', 'clock data',
      `type mapped-value-${figmaId}-${moded.index}-1 #123457`, `type mapped-value-${figmaId}-${moded.index}-1 key Enter`, 'clock data', 'clock +1', 'clock data',
      `tap accept-${figmaEdited}`, 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    expect(last.filter(r => r.resources).at(-1).slots.written.note).toBe(`Accepted ${figmaEdited}.`);
    expect(tool.writes.at(-1).after).toContain(`when ${moded.modes[1].label}: #123457`);
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
  // A literal is judged by its kind: oklch() and rgb() with decimals are a
  // colour's, a name folds to its hex, `20px` is no number's.
  expect(dial('color', { control: 'select', options: 'oklch(0.6 0.2 30)|rgb(255 0 0 / 0.5)' })).toMatchObject({ control: 'select', options: ['#1083feff', 'oklch(0.6 0.2 30)', 'rgb(255 0 0 / 0.5)'] });
  expect(dial('color', { control: 'select', options: 'red|blue' }, '#ff0000ff')).toMatchObject({ value: 'red', options: ['red', 'blue'] });
  expect(dial('number', { control: 'select', options: '20px|40' }, '20', ['0', '60'])).toMatchObject({ options: ['20', '40'], help: '20px is not a value a dial can take (a dial is a literal), so it is not offered.' });
  // A length's step in px, and with none one fine enough for the value.
  expect(controlOf({ name: 'gap', kind: 'length', value: '12.5px', range: ['0px', '24px'], meta: {} }).step).toBe(0.1);
  expect(controlOf({ name: 'gap', kind: 'length', value: '12px', range: ['0px', '24px'], meta: { step: '2px' } }).step).toBe(2);
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
  expect(inputsOf('LABEL', ['-exact-label'], [])).toEqual({ tokens: ['-exact-label'], dials: [], roles: [] });
  expect(inputsOf('oklch(from accent calc(l + lift) c h)', ['accent.soft'], ['accent', 'lift'])).toEqual({ tokens: [], dials: ['accent', 'lift'], roles: [] });
  expect(inputsOf('#1abcde', ['abcde'], [])).toEqual({ tokens: [], dials: [], roles: [] });
  // A platform role or system colour no token overrides is an input too.
  expect(inputsOf('-exact-secondary-label', [], [])).toEqual({ tokens: [], dials: [], roles: ['-exact-secondary-label'] });
  expect(inputsOf('color-mix(in srgb, Canvas 50%, accent)', [], ['accent'])).toEqual({ tokens: [], dials: ['accent'], roles: ['Canvas'] });
  // Two axes with like-named values make two labels.
  const cols = columnsOf([{ name: 'a', values: ['normal', 'compact'], under: null }, { name: 'b', values: ['normal', 'compact'], under: null }]);
  expect(new Set(cols.map(c => c.label)).size).toBe(cols.length);
  // Text over a background, composited; a translucent background has no one ratio.
  expect(contrast('#00000000', '#ffffffff')).toBe(1);
  expect(contrast('#000000', '#ffffff')).toBe(21);
  expect(contrast('#000000', '#ffffff80')).toBe('translucent background');
  expect(contrast('oklch(0.5 0 0)', '#ffffff')).toBe('not a colour');
});
