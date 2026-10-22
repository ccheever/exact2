// The design tool (LLP 1122 T1) driven as a person would, against a real
// design server: `bun exact.mjs tamber tool` on a copy of the theme fixture
// serves it at its own URL, and the agent drives that page (`agent web
// --url`). The tool shows the server's hello, and an agent's write arrives
// on its long poll attributed. Its stub-server tests are
// apps/design-tool/app.test.contract.
// Its heaviest drives are files of their own, so a lane runs them side by side (design-tool-ops, -canvas and
// -scrub.test.mjs); each claims its own port and temp dirs (design-tool-harness.mjs).
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, ROOT, STUB_APP, TOOL_ORIGIN, drive, edit, lines, proxied, repo, stop, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('the tool opens on the theme fixture, shows its hello, and an agent\'s write arrives attributed; a second tool reuses it or is refused', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  let drive, other;
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
    drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'state'], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
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
    expect(hello).toMatchObject({ ok: true, protocol: 'tamber/1.16', app });
    expect(hello.files.map(f => f.path)).toEqual(['app.contract', 'app.tamber']);
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
    // Through a dev loop, it is not: it holds the design server, so --via
    // to any loop is refused rather than given its page (none).
    const through = spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', 'http://127.0.0.1:9', '--no-open'], { cwd: ROOT, encoding: 'utf8' });
    expect(through.status, through.stderr).toBe(1);
    expect(through.stderr).toContain(`already serves this design, not through http://127.0.0.1:9; stop it first`);
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
  const serve = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', mine], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
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
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', mine, '--via', at, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const { token } = await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json();
    const hello = await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    expect(hello.ok.app).toBe(mine);
    expect((await fetch(`${TOOL_ORIGIN}/__tamber/tool/`)).status).toBe(200);
    // The same --via again reuses it (it forwards to that server's run);
    // another is refused, and the tool's own port never taken.
    const again = run([mine, '--via', at]);
    expect(again.status, again.stderr).toBe(0);
    expect(again.stdout).toContain('already running');
    const elsewhere = run([mine, '--via', 'http://127.0.0.1:9']);
    expect(elsewhere.status).toBe(1);
    expect(elsewhere.stderr).toContain('already serves this design, not through http://127.0.0.1:9; stop it first');
    expect(run([mine, '--via', TOOL_ORIGIN]).status).toBe(2);
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
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
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
    const drive = spawnSync(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-dials-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', 'tap more-Colour', 'tap more-Tone', 'tree dials',
      // The sliders' own range and step, which the page keeps a value to:
      // a step of 0.001 to 0.005, a value past the end held at it.
      'type dial-fine-slider 0.004', 'state', 'type dial-fine-slider 1', 'state', 'type dial-lift-slider 0.374', 'state', 'type dial-chroma-slider -1', 'state',
      // A length dial's value, written in px, which the real server takes (stage 6).
      'type dial-gap-slider 10', 'clock data', 'tap apply', 'clock data'], { cwd: ROOT, env: DRIVEN.env, encoding: 'utf8' });
    expect(drive.status, drive.stderr).toBe(0);
    const out = drive.stdout.split('\n').filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const tree = out.find(r => r.nodes);
    expect(out.filter(r => r.slots).map(r => [r.slots.editTarget, r.slots.editValue])).toEqual([['fine', '0.004'], ['fine', '0.005'], ['lift', '0.37'], ['chroma', '0']]);
    expect(readFileSync(join(app, 'app.tamber'), 'utf8')).toContain('gap: length = 10px');
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
      { group: 'Space', dials: [
        // A length dial: its value, range and step in px (LLP 1121 D15).
        dial('gap', 'slider', 'Gap', '8px', '0px – 24px · step 2px'),
        dial('inset', 'stepper', 'inset', '4px', '0px – 16px · step 1px'),
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

test('a dial edit and a Dim cell edit each write one line, undo gives the bytes back, and a stale undo says who changed the file', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-edit-'));
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8');
  writeFileSync(join(app, 'app.tamber'), original);
  // A second design file that sorts before the app's: a write to the app's
  // design still carries the app's digest.
  writeFileSync(join(app, 'accent.tamber'), original.replace('tamber dials', 'tamber accent'));
  writeFileSync(join(app, 'app.contract'), STUB_APP);
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

test('T3: a re-scope reads the branch it moves: the alias there, the param that aliases a token there, a role by its spelling, none toward a value that names its destination, none from another revision', async () => {
  const { paramInfoOf, rescopeOf, tokenOfIn } = await import('../apps/design-tool/rescope.ts');
  const ir = { tokens: [{ name: 'radius.md' }, { name: 'radius.sm' }, { name: '-exact-label', role: 'label' }], components: [{ params: [
    { name: 'a#Card.radius', short: 'Card.radius', kind: 'length', default: 'radius.md', set: { value: 'radius.md', whens: 1 } },
    { name: 'a#Card.ink', short: 'Card.ink', kind: 'color', default: 'Label', set: null },
    { name: 'a#Chip.radius', short: 'Chip.radius', kind: 'length', default: '4px', set: { value: '4px', whens: 1 } },
  ] }] };
  const params = paramInfoOf(ir), tokenOf = tokenOfIn(ir);
  const compact = [{ axis: 'density', value: 'compact' }];
  const files = { 'app.tamber': 'd1' }, key = JSON.stringify([['app.tamber', 'd1']]);
  const branches = {
    'a#Card.radius': { value: 'radius.md', whens: [{ when: compact, value: 'radius.sm' }], files },
    'a#Card.ink': { value: 'Label', whens: [], files },
    // Chip's compact branch names radius.sm too.
    'a#Chip.radius': { value: '4px', whens: [{ when: compact, value: 'radius.sm' }], files },
    'radius.sm': { value: '2px', whens: [], files },
    'radius.md': { value: '12px', whens: [], files },
    '-exact-label': { value: '#000', whens: [], files },
  };
  const explain = async name => branches[name] ?? null;
  // Card.radius's compact branch aliases radius.sm, not radius.md: moved there, on compact.
  const m = await rescopeOf({ op: 'set-param', name: 'Card.radius', when: compact, value: '8px' }, params, tokenOf, explain, key);
  expect(m).toEqual({ text: 'Apply to the token radius.sm instead', ops: [
    { op: 'set-param', name: 'Card.radius', when: compact, value: 'radius.sm' },
    { op: 'set-token', name: 'radius.sm', when: compact, value: '8px' },
  ] });
  // A role override by its role's spelling.
  expect((await rescopeOf({ op: 'set-param', name: 'Card.ink', when: [], value: '#111' }, params, tokenOf, explain, key)).text).toBe('Apply to the token -exact-label instead');
  // Toward a value that names the destination: none (an alias loop).
  expect(await rescopeOf({ op: 'set-param', name: 'Card.radius', when: [], value: 'radius.md' }, params, tokenOf, explain, key)).toBe(null);
  // radius.sm on compact is aliased by two params there: none.
  expect(await rescopeOf({ op: 'set-token', name: 'radius.sm', when: compact, value: '3px' }, params, tokenOf, explain, key)).toBe(null);
  // radius.md, unconditionally, by Card.radius alone: moved to it, its own line kept.
  expect(await rescopeOf({ op: 'set-token', name: 'radius.md', when: [], value: '10px' }, params, tokenOf, explain, key)).toEqual({ text: 'Apply to Card.radius instead', ops: [
    { op: 'set-token', name: 'radius.md', when: [], value: '12px' },
    { op: 'set-param', name: 'Card.radius', when: [], value: '10px' },
  ] });
  // A copy a shorter `when` covers (Grok's case): on dim+compact, Fill shows paper (its dim line), so an
  // edit of ink there moves nowhere, and Fill's Only-this-mode edit there moves to paper.
  const two = { tokens: [{ name: 'ink' }, { name: 'paper' }], components: [{ params: [{ name: 'a#Card.fill', short: 'Card.fill', kind: 'color', default: 'ink', set: { value: 'ink', whens: 1 } }] }] };
  const dim = [{ axis: 'dark-style', value: 'dim' }], both = [...dim, ...compact];
  const twoBranches = {
    'a#Card.fill': { value: 'ink', whens: [{ when: dim, value: 'paper' }], files },
    ink: { value: '#010101', whens: [{ when: both, value: '#222222' }], files },
    paper: { value: '#ffffff', whens: [], files },
  };
  const ex2 = async name => twoBranches[name] ?? null;
  expect(await rescopeOf({ op: 'set-token', name: 'ink', when: both, value: '#abcdef' }, paramInfoOf(two), tokenOfIn(two), ex2, key)).toBe(null);
  expect((await rescopeOf({ op: 'set-param', name: 'Card.fill', when: both, value: '#abcdef' }, paramInfoOf(two), tokenOfIn(two), ex2, key)).text).toBe('Apply to the token paper instead');
  // A line of the param more specific than the edit's keeps its copies either way: on dim, Fill shows ink.
  const wider = { 'a#Card.fill': { value: 'ink', whens: [{ when: both, value: 'paper' }], files }, ink: { value: '#010101', whens: [], files } };
  expect((await rescopeOf({ op: 'set-token', name: 'ink', when: dim, value: '#abcdef' }, paramInfoOf(two), tokenOfIn(two), async n => wider[n] ?? null, key)).text).toBe('Apply to Card.fill instead');
  // An explain of another revision than the edit's: none.
  expect(await rescopeOf({ op: 'set-param', name: 'Card.ink', when: [], value: '#111' }, params, tokenOf, async n => ({ ...branches[n], files: { 'app.tamber': 'd2' } }), key)).toBe(null);
});

test('T3: Reset gives a param\'s line back byte for byte in one undo; a param edit re-scoped to the token it aliases is one transaction, undone a level at a time; a re-scope after another write is refused', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-params-'));
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8');
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  let transactions = 0;
  // Before the fifth transaction (the second re-scope), a hand edit of the component's default.
  const tool = await proxied(app, request => {
    // (The component's own source: no file the move edits, but one it was worked out on.)
    if (request.method === 'transaction' && ++transactions === 5) writeFileSync(join(app, 'app.contract'), readFileSync(join(app, 'app.contract'), 'utf8').replace('fill: color = "accent.fill"', 'fill: color = "surface.card"'));
  });
  try {
    const replies = await drive(tool.url, `design-tool-params-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // Reset: the line goes; undo gives it back.
      'tap param-Card.pad-reset', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tap undo', 'clock data',
      // A param edit, applied; moved to the token it aliases; undone twice.
      'clock +1', 'clock data', 'type dial-app.contract#Card.fill-field #ff0000', 'type dial-app.contract#Card.fill-field key Enter', 'clock data', 'state',
      'tap apply', 'clock data', 'clock +1', 'clock data', 'state', 'tap rescope', 'clock data', 'clock +1', 'clock data', 'state', 'tap undo', 'clock data', 'clock +1', 'clock data', 'tap undo', 'clock data',
      // Again, but the file moves between the edit and the move: refused.
      'clock +1', 'clock data', 'type dial-app.contract#Card.fill-field #00ff00', 'type dial-app.contract#Card.fill-field key Enter', 'clock data',
      'tap apply', 'clock data', 'clock +1', 'clock data', 'tap rescope', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.proposal).toMatchObject({ ok: true, active: true });
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['transaction', true], ['undo', true], ['transaction', true], ['transaction', true], ['undo', true], ['undo', true], ['transaction', true], ['transaction', false]]);
    const pad = '  Card.pad = 12px';
    expect(edit(original, tool.writes[0].after)).toEqual({ at: original.split('\n').indexOf(pad), removed: [pad], added: [] });
    expect(tool.writes[1].after).toBe(original);
    // The param edit: one new line in the params block.
    expect(edit(original, tool.writes[2].after).added).toEqual(['  Card.fill = #ff0000']);
    expect(states[2].derives.stack.rescope).toBe('Apply to the token accent.fill instead');
    // Re-scoped: the param's line gone and the token's value the edit's, in one write.
    const moved = tool.writes[3].after;
    expect(moved).not.toContain('Card.fill =');
    expect(moved).toContain('  accent.fill = #ff0000\n');
    expect(edit(original, moved).removed).toEqual(['  accent.fill = accent']);
    // One undo: the param edit as it was; another: the design as it was.
    expect(tool.writes[4].after).toBe(tool.writes[2].after);
    expect(tool.writes[5].after).toBe(original);
    expect(states[3].derives.stack.rescope).toBe('');
    // The move found the file changed since the edit: nothing written, said so.
    expect(tool.writes[7].error.message).toContain('changed since');
    expect(states.at(-1).derives.stack.error).toContain('Not moved:');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

/** An app with the fixture design in a git repository of its own: `app`,
 * and git in it. */
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
    expect(review.counted).toBe('47 source values = 47 mapped + 0 reported');
    expect(review.countOk).toBe(true);
    // 23 colours, `--radius` a length and its derived `lg` (stage 6 H).
    expect(review.mapped).toHaveLength(25);
    expect(review.mapped[0]).toMatchObject({ name: 'shadcn.background', modes: [{ label: 'default', light: 'oklch(1 0 0)', dark: 'oklch(0.145 0 0)' }] });
    // Every override off until ticked, every unmapped value with its reason.
    expect(review.offers.map(o => [o.role, o.on])).toEqual([['-exact-background', false], ['-exact-label', false], ['-exact-secondary-background', false], ['-exact-separator', false], ['AccentColor', false]]);
    expect(states[0].slots.picked).toEqual([]);
    expect(review.unmapped.map(u => u.name)).toEqual(['--radius-sm', '--radius-md', '--radius-xl']);
    expect(review.unmapped.every(u => u.derived && u.why.startsWith('derived from --radius, and not a length Tamber folds'))).toBe(true);
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
    ], { keepStorage: true });
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
    // Its numbers are tokens too (stage 6 H): only the strings and booleans
    // are reported.
    expect(dtcg.counted).toBe('104 source values = 101 mapped + 3 reported');
    expect(dtcg.unmapped).toHaveLength(3);
    // Its axis is said as an axis; a reference is painted as the token it
    // names, in the same mode.
    expect(dtcg.mapped[0]).toMatchObject({ kind: 'axis', modes: [] });
    const byName = new Map(dtcg.mapped.map(m => [m.name, m]));
    const ref = dtcg.mapped.find(m => m.kind === 'token' && byName.has(m.modes[0].value));
    expect(ref.modes[0].light).toBe(byName.get(ref.modes[0].value).modes[0].light);
    // A colour token's other mode, edited and accepted, is written.
    const moded = dtcg.mapped.find(m => m.modes.length > 1 && m.modes[1].light);
    const figmaId = dtcg.proposal, figmaEdited = figmaId.replace(/-(\d+)$/, (_, n) => `-${Number(n) + 1}`);
    const last = await drive(tool.url, `design-tool-import-${process.pid}`, [
      'clock data', 'clock +1', 'clock data',
      `type mapped-value-${figmaId}-${moded.index}-1 #123457`, `type mapped-value-${figmaId}-${moded.index}-1 key Enter`, 'clock data', 'clock +1', 'clock data',
      `tap accept-${figmaEdited}`, 'clock data', 'clock +1', 'clock data', 'state',
    ], { keepStorage: true });
    expect(last.filter(r => r.resources).at(-1).slots.written.note).toBe(`Accepted ${figmaEdited}.`);
    expect(tool.writes.at(-1).after).toContain(`when ${moded.modes[1].label}: #123457`);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('controls from metadata that does not fit, steps, select values, column labels, branches, inputs and contrast over a background', async () => {
  const { controlOf, columnsOf, contrast, branchOf, inputsOf, cellText } = await import('../apps/design-tool/app.ts');
  // A cell's text for each stage 6 kind, from `resolve`'s tuples as the server gives them (tamber/tests/corpus).
  expect(cellText('length', { amount: 12, kind: 'length', unit: 'px', value: '12px' })).toBe('12px');
  expect(cellText('length', { amount: 1.5, kind: 'length', unit: 'rem', value: '1.5rem' })).toBe('1.5rem');
  expect(cellText('number', { amount: 0.5, kind: 'number', value: '0.5' })).toBe('0.5');
  // The server's spelling, not JavaScript's (measures.tamber's `tiny`).
  expect(cellText('number', { amount: 0.0000004, kind: 'number', value: '0.0000004' })).toBe('0.0000004');
  expect(cellText('box-shadow', { kind: 'box-shadow', shadows: [], value: 'inset 0px 2px 4px -1px #000000ff' })).toBe('inset 0px 2px 4px -1px #000000ff');
  expect(cellText('type', { family: '"InterVariable"', kind: 'type', leading: '1.35', size: '-exact-body', style: null, value: '-exact-body/1.35 "InterVariable"', weight: null })).toBe('-exact-body/1.35 · weight unsaid · style unsaid · "InterVariable"');
  expect(cellText('type', { family: null, kind: 'type', leading: null, size: '-exact-subheadline', style: 'italic', value: '', weight: 600 })).toBe('-exact-subheadline · weight 600 · italic · family unsaid');
  expect(cellText('family', { family: 'ui-monospace', kind: 'family', value: 'ui-monospace' })).toBe('ui-monospace');
  expect(cellText('duration', { kind: 'duration', ms: 400, value: '400ms' })).toBe('400 ms');
  expect(cellText('easing', { kind: 'easing', value: 'cubic-bezier(0.2, 0, 0, 1)' })).toBe('cubic-bezier(0.2, 0, 0, 1)');
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
  expect(controlOf({ name: 'gap', kind: 'length', value: '12px', range: ['0px', '24px'], meta: {} })).toMatchObject({ control: 'slider', number: 12, min: 0, max: 24, step: 1, bounds: '0px – 24px · step 1px', unit: 'px' });
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
  // A length's own branch names no under-dark axis (it has no sides); a duration has none.
  expect(branchOf(axes, dark3, whens, 'length').own).toBe('density=compact');
  expect(branchOf(axes, light1, whens, 'length').own).toBe('density=compact');
  expect(branchOf(axes, dark3, whens, 'duration').own).toBe('');
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

test('typed fields: every value joins back byte for byte, and a field edits only its part', async () => {
  const { partsOf, joined, missing } = await import('../apps/design-tool/app.ts');
  const kinds = new Map([['space.blur', 'length'], ['border.subtle', 'color'], ['type.body', 'type'], ['family.body', 'family']]);
  const isLength = name => kinds.get(name);
  const values = [
    ['length', '12px'], ['length', ' 1.5rem '], ['length', '0'], ['number', '0.5'],
    ['box-shadow', '0 1px 2px rgb(0 0 0 / 0.2), inset 0 0 0 1px border.subtle'], ['box-shadow', '0  1px space.blur #000'],
    ['box-shadow', '0 1px 2px 3px red inset'], ['box-shadow', 'inset 0 1px light-dark(#000, #fff),0 0 1px  2px accent'],
    ['text-shadow', '1px 1px'], ['text-shadow', '0 1px 2px rgba(0,0,0,.5)'],
    ['type', 'italic 600 1.25rem/1.4 family.body'], ['type', 'bold -exact-title3 / 1.2 "Inter", sans-serif'], ['type', '1rem'], ['type', '600.0 16px'],
    ['text-shadow', '0 1px +2px'], ['box-shadow', '0 1e0px 2px INSET red'],
  ];
  for (const [kind, v] of values) {
    const p = partsOf(kind, v, isLength);
    expect(p).not.toBeNull();
    expect(joined(p.parts, p.tail)).toBe(v);
  }
  // No field where none fits: a function, a named length, an expression, a kind with none.
  for (const [kind, v] of [['length', 'calc(1px + 2px)'], ['length', '-exact-hairline'], ['number', 'lift'], ['box-shadow', '0 red'], ['box-shadow', '0 1px 2px red blue'], ['text-shadow', 'inset 0 1px red'], ['text-shadow', '1px 1px red, 0 0 blue'], ['type', 'type.body'], ['type', 'normal 16px'], ['type', 'oblique 16px'], ['type', 'italic 600 normal'], ['box-shadow', '0 1px 2px#000'], ['box-shadow', 'red.0 1px 2px'], ['duration', '200ms'], ['color', '#fff']]) expect(partsOf(kind, v, isLength)).toBeNull();
  // Each word read as the server reads it: a signed or exponent length is a
  // length, not the colour; a weight with a fraction is the weight; a weight
  // before `italic` leaves `italic` the size, as typeface.rs refuses it.
  const fields = (kind, v) => Object.fromEntries(partsOf(kind, v, isLength).parts.filter(x => x.control !== 'fixed').map(x => [x.key, x.text]));
  expect(fields('text-shadow', '0 1px +2px')).toEqual({ '0.x': '0', '0.y': '1px', '0.blur': '+2px', '0.color': '' });
  expect(fields('box-shadow', '0 1e0px 2px INSET red')).toMatchObject({ '0.y': '1e0px', '0.inset': 'INSET', '0.color': 'red' });
  expect(fields('type', '600.0 16px')).toMatchObject({ weight: '600.0', size: '16px', family: '' });
  expect(fields('type', '600 italic 16px')).toMatchObject({ weight: '600', size: 'italic', style: '' });
  // `normal` as the line height is the line height.
  expect(fields('type', '16px / normal')).toMatchObject({ size: '16px', leading: 'normal' });
  // A blur holds its place while a spread is written after it; a unit may
  // be none (the server judges a bare amount).
  const set = (kind, v, edits) => { const p = partsOf(kind, v, isLength); return p.parts.map(x => (x.key in edits ? { ...x, text: edits[x.key] } : x)); };
  expect(missing(set('box-shadow', '0 1px 2px red', { '0.spread': '1px', '0.blur': '' })).map(x => x.label)).toEqual(['blur']);
  expect(missing(set('box-shadow', '0 1px 2px red', { '0.blur': '' }))).toEqual([]);
  expect(missing(set('box-shadow', '0 1px 2px 3px red', { '0.spread': '', '0.blur': '' }))).toEqual([]);
  expect(missing(set('box-shadow', '0 1px 2px 3px red', { '0.y': '' })).map(x => x.label)).toEqual(['y']);
  expect(missing(set('length', '0', { unit: '' }))).toEqual([]);
  // A text style is a size; a type token's name is its whole value.
  expect(fields('type', '-exact-body')).toMatchObject({ size: '-exact-body' });
  const edit = (kind, v, key, text) => { const p = partsOf(kind, v, isLength); return joined(p.parts.map(x => (x.key === key ? { ...x, text } : x)), p.tail); };
  const ring = '0 1px 2px rgb(0 0 0 / 0.2), inset 0 0 0 1px border.subtle';
  expect(edit('box-shadow', ring, '1.inset', '')).toBe('0 1px 2px rgb(0 0 0 / 0.2), 0 0 0 1px border.subtle');
  expect(edit('box-shadow', ring, '0.inset', 'inset')).toBe('inset 0 1px 2px rgb(0 0 0 / 0.2), inset 0 0 0 1px border.subtle');
  expect(edit('box-shadow', ring, '0.spread', '4px')).toBe('0 1px 2px 4px rgb(0 0 0 / 0.2), inset 0 0 0 1px border.subtle');
  expect(edit('box-shadow', ring, '1.color', 'accent')).toBe('0 1px 2px rgb(0 0 0 / 0.2), inset 0 0 0 1px accent');
  expect(edit('box-shadow', ring, '1.spread', '')).toBe('0 1px 2px rgb(0 0 0 / 0.2), inset 0 0 0 border.subtle');
  expect(edit('box-shadow', '0 1px 2px 3px red inset', '0.inset', '')).toBe('0 1px 2px 3px red');
  expect(edit('text-shadow', '1px 1px', '0.color', 'red')).toBe('1px 1px red');
  expect(edit('text-shadow', '0 1px 2px rgba(0,0,0,.5)', '0.color', '')).toBe('0 1px 2px');
  expect(edit('type', 'italic 600 1.25rem/1.4 family.body', 'style', '')).toBe('600 1.25rem/1.4 family.body');
  expect(edit('type', 'italic 600 1.25rem/1.4 family.body', 'leading', '')).toBe('italic 600 1.25rem family.body');
  expect(edit('type', '1rem', 'weight', '700')).toBe('700 1rem');
  expect(edit('type', '1rem', 'style', 'italic')).toBe('italic 1rem');
  expect(edit('type', '1rem', 'leading', '1.5')).toBe('1rem/1.5');
  expect(edit('type', '1rem', 'family', 'family.mono')).toBe('1rem family.mono');
  expect(edit('length', '12px', 'unit', 'rem')).toBe('12rem');
  // Labels name the shadow in a list, for aria.
  expect(partsOf('box-shadow', ring, isLength).parts.filter(x => x.control !== 'fixed').map(x => x.label)).toEqual([
    'shadow 1 inset', 'shadow 1 x', 'shadow 1 y', 'shadow 1 blur', 'shadow 1 spread', 'shadow 1 colour',
    'shadow 2 inset', 'shadow 2 x', 'shadow 2 y', 'shadow 2 blur', 'shadow 2 spread', 'shadow 2 colour',
  ]);
});

test('typed fields write one line through set-token for each kind: a length, a number, a shadow list with inset and a token colour, a text shadow, a type longhand', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-typed-'));
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8');
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  // Each: the cell, its field edits, the line before and after.
  const edits = [
    ['space.edge', ['type part-amount-field 16'], '  space.edge = 12px', '  space.edge = 16px'],
    ['opacity.muted', ['type part-amount-field 0.25'], '  opacity.muted = 0.5', '  opacity.muted = 0.25'],
    ['shadow.ring', ['type part-0.spread-field 1px', 'type part-0.inset-field inset'], '  shadow.ring: box-shadow = 0px 1px 2px #00000033, inset 0px 0px 0px 1px surface.inset', '  shadow.ring: box-shadow = inset 0px 1px 2px 1px #00000033, inset 0px 0px 0px 1px surface.inset'],
    ['shadow.text', ['type part-0.blur-field 3px'], '  shadow.text: text-shadow = 0px 1px 2px #00000080', '  shadow.text: text-shadow = 0px 1px 3px #00000080'],
    ['type.title', ['type part-style-field italic'], '  type.title: type = 600 1.25rem/1.25', '  type.title: type = italic 600 1.25rem/1.25'],
  ];
  const tool = await proxied(app, () => {});
  try {
    const replies = await drive(tool.url, `design-tool-typed-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      ...edits.flatMap(([cell, ops]) => ['clock +1', 'clock data', `tap cell-${cell}-0-light`, 'clock data', ...ops.flatMap(o => [o, 'clock data']), 'state', 'tap apply', 'clock data']),
    ]);
    const states = replies.filter(r => r.resources);
    for (const st of states) expect(st.resources.proposal).toMatchObject({ ok: true, active: true });
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual(edits.map(() => ['transaction', true]));
    let before = original;
    for (const [k, [, , from, to]] of edits.entries()) {
      expect(edit(before, tool.writes[k].after)).toEqual({ at: original.split('\n').indexOf(from), removed: [from], added: [to] });
      before = tool.writes[k].after;
    }
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
