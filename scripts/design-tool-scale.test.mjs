// The design tool's half of LLP 1121 stage 11 D against a real `tamber
// serve` (the server's half, tamber/1.14, and A2's package designs): Exact's
// base scale listed read-only, each platform the app ships first with its
// value and source as `describe` gives them; the app's override of a base
// name as its own line; a package's tokens under their package; Set for
// this app writes the app's line (set-token), Back to the base removes it,
// and Back's undo gives the file's bytes back. Its stub-server tests are in
// apps/design-tool/app.test.contract.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { drive, proxied, ROOT, toolTurns } from './design-tool-harness.mjs';

toolTurns();

// As the tool says a column's source (apps/design-tool/base.ts `fromOf`).
const from = s => s.kind === 'metric' ? s.ref : s.kind === 'measured' ? `measured: ${s.id}` : s.kind === 'guideline' ? s.cite : s.kind === 'full' ? 'the pill' : s.kind === 'terminal' ? 'cells' : '';
const LABELS = { ios: 'iOS', mac: 'Mac', neutral: 'neutral', material: 'Android', terminal: 'terminal' };

test('stage 11 on a real server: the base scale read-only with each shipped platform and its source, an override, a package\'s tokens; Set for this app and Back to the base each one line, and Back\'s undo gives the bytes back', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-scale-'));
  // The theme fixture's targets (iOS, the Mac, the web), a design overriding radius.md and naming a package's token.
  cpSync(resolve(ROOT, 'apps/theme-fixture/app.json'), join(app, 'app.json'));
  const design = 'tamber t\n\ntheme\n  brand = reading.card\n  radius.md = 12px\n';
  writeFileSync(join(app, 'app.tamber'), design);
  const pkg = join(app, 'node_modules/@acme/reading');
  mkdirSync(pkg, { recursive: true });
  writeFileSync(join(pkg, 'package.json'), '{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}');
  writeFileSync(join(pkg, 'index.contract'), 'component ReadingCard\n  view\n    column background-color="reading.card" padding="space.md"\n      text "x"\n');
  writeFileSync(join(pkg, 'reading.tamber'), 'tamber reading\n\ntheme\n  reading.card = #112233\n  reading.measure: length = 640px\n');
  writeFileSync(join(app, 'app.contract'), 'use ReadingCard from "@acme/reading"\ncomponent App\n  view\n    column background-color="brand" padding="space.md" border-radius="radius.md"\n      ReadingCard()\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', 'app.json', 'app.tamber', 'app.contract'); git('commit', '-q', '-m', 'design');
  const tool = await proxied(app);
  try {
    // What the server says of the scale, the targets and the package, as the tool reads it.
    const token = (await (await fetch(`${tool.origin}/__tamber/token`)).json()).token;
    const described = (await (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"describe"}' })).json()).ok;
    const row = name => described.base.find(r => r.name === name);
    const shipped = [...new Set(described.targets.map(t => t.column))];
    expect(shipped.length).toBeGreaterThan(0);
    const cell = (name, column) => { const c = row(name).columns[column]; return `${LABELS[column]} ${c.value}${c.source.kind === 'stretch' ? ' (a stretch)' : ''} · ${from(c.source)}`; };
    const pkgName = described.tokens?.find?.(t => t.name === 'reading.measure')?.package ?? '@acme/reading';
    const replies = await drive(tool.url, `design-tool-scale-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'tree',
      'tap token-base-more-space.md', 'tree',
      // Set for this app: space.md, from the first shipped column's value.
      'tap set-for-app-space.md', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      // Back to the base, then undo it.
      'tap back-space.md', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data',
      'tap undo', 'clock data', 'clock +1', 'clock data', 'tree',
      // A package's value opens read only; Set for this app writes the app's line over it, and Back to the package removes it.
      'tap set-for-app-reading.measure', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      'tap back-reading.measure', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      'tap cell-reading.measure-0-light', 'clock data', 'tree',
      // The app's override of a base name edits as the app's line (explain carries its branches).
      'tap cell-radius.md-0-light', 'clock data', 'tree',
    ]);
    const trees = replies.filter(r => r.nodes && !('x' in (r.nodes[0] ?? {})));
    const text = (t, id) => t.nodes.find(n => n.props.testId === id)?.props.text;
    // The base group, its rows per platform: the shipped columns first, the rest one expand away.
    expect(text(trees[0], 'token-group-Base scale (Exact)')).toBe('Base scale (Exact)');
    expect(text(trees[0], 'token-source-space.md')).toBe('Exact\'s base, per platform');
    for (const column of shipped) expect(text(trees[0], `token-base-space.md-${column}`)).toBe(cell('space.md', column));
    const rest = Object.keys(LABELS).filter(c => !shipped.includes(c));
    for (const column of rest) expect(text(trees[0], `token-base-space.md-${column}`)).toBeUndefined();
    for (const column of rest) expect(text(trees[1], `token-base-space.md-${column}`)).toBe(cell('space.md', column));
    expect(text(trees[0], 'token-base-radius.full-ios') ?? text(trees[0], `token-base-radius.full-${shipped[0]}`)).toContain('the pill');
    // The app's override of radius.md, and the package's tokens under their package.
    expect(text(trees[0], 'token-over-radius.md')).toBe(`Set for this app, over Exact's base (${LABELS[shipped[0]]} ${row('radius.md').columns[shipped[0]].value}).`);
    expect(text(trees[0], `token-group-${pkgName} package`)).toBe(`${pkgName} package`);
    expect(text(trees[0], 'token-source-reading.measure')).toBe(`From the ${pkgName} package`);
    // Set for this app: set-token, from the first shipped column's value; one line added.
    const states = replies.filter(r => r.slots);
    const s0 = states[0].slots;
    expect([s0.editKind, s0.editTarget, s0.editValue]).toEqual(['set-token', 'space.md', row('space.md').columns[shipped[0]].value]);
    expect(tool.writes.slice(0, 3).map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true], ['undo', true]]);
    const set = tool.writes[0].after;
    expect(set.split('\n').filter(l => !design.split('\n').includes(l))).toEqual([`  space.md = ${row('space.md').columns[shipped[0]].value}`]);
    expect(text(trees[2], 'token-over-space.md')).toBe(`Set for this app, over Exact's base (${LABELS[shipped[0]]} ${row('space.md').columns[shipped[0]].value}).`);
    // Back to the base: remove-token, the design as it was; its undo gives back the bytes Set for this app wrote.
    const s1 = states[1].slots;
    expect([s1.editKind, s1.editTarget]).toEqual(['remove-token', 'space.md']);
    expect(tool.writes[1].after).toBe(design);
    expect(tool.writes[2].after).toBe(set);
    expect(text(trees[3], 'token-over-space.md')).toBe(text(trees[2], 'token-over-space.md'));
    // The package's token: read only in the branch editor; the app's line over it, then gone again.
    expect(text(trees[6], 'branch-readonly')).toBe(`The ${pkgName} package's, read only: Set for this app (in the grid) to change it here.`);
    expect([states[2].slots.editKind, states[2].slots.editTarget, states[2].slots.editValue]).toEqual(['set-token', 'reading.measure', '640px']);
    expect(tool.writes.slice(3).map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true]]);
    // (The app's line keeps the package token's kind, as set-token writes it.)
    expect(tool.writes[3].after).toBe(`${set}  reading.measure: length = 640px\n`);
    expect(text(trees[4], 'token-over-reading.measure')).toBe(`Set for this app, over the ${pkgName} package's value.`);
    expect(states[3].slots.editKind).toBe('remove-token');
    expect(tool.writes[4].after).toBe(set);
    expect(text(trees[5], 'token-over-reading.measure')).toBeUndefined();
    expect(text(trees[7], 'branch-readonly')).toBeUndefined();
    const field = trees[7].nodes.find(n => n.props.testId === 'branch-value');
    expect(field?.props.value ?? field?.props.text).toBe('12px');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
