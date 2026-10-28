// The design tool's half of LLP 1121 stage 11 D against a real `tamber
// serve` (the server's half, tamber/1.14, and A2's package designs): Exact's
// base scale listed read-only, each platform the app ships first with its
// value and source as `describe` gives them; the app's override of a base
// name as its own line; a package's tokens under their package; Set for
// this app writes the app's line (set-token), Back to the base removes it,
// and Back's undo gives the file's bytes back. Its stub-server tests are
// apps/design-tool/*.test.contract.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { drive, proxied, ROOT, toolTurns } from './design-tool-harness.mjs';

toolTurns();

// As the tool says a column's source (apps/design-tool/base.ts `fromOf`).
const from = s => s.kind === 'metric' ? s.ref : s.kind === 'measured' ? `measured: ${s.id}` : s.kind === 'guideline' ? s.cite : s.kind === 'stretch' ? `${s.ref}${s.why ? `, ${s.why}` : ''}` : s.kind === 'full' ? 'the pill' : s.kind === 'terminal' ? 'cells' : '';
const LABELS = { ios: 'iOS', mac: 'Mac', neutral: 'neutral', material: 'Android', terminal: 'terminal' };

test('stage 11 on a real server: the base scale read-only with each shipped platform and its source, an override, a package\'s tokens; Set for this app and Back to the base each one line, and Back\'s undo gives the bytes back', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-scale-'));
  // The theme fixture's targets (iOS, the Mac, the web), a design overriding radius.md and naming a package's token.
  cpSync(resolve(ROOT, 'apps/theme-fixture/app.json'), join(app, 'app.json'));
  // (Its web target is its web crate's: the manifest alone names iOS and the Mac.)
  mkdirSync(join(app, 'web'), { recursive: true });
  cpSync(resolve(ROOT, 'apps/theme-fixture/web/Cargo.toml'), join(app, 'web/Cargo.toml'));
  const design = 'tamber t\n\ntheme\n  brand = reading.card\n  radius.md = 12px\n  card.pad: length = space.md\n';
  writeFileSync(join(app, 'app.tamber'), design);
  const pkg = join(app, 'node_modules/@acme/reading');
  mkdirSync(pkg, { recursive: true });
  writeFileSync(join(pkg, 'package.json'), '{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}');
  writeFileSync(join(pkg, 'index.contract'), 'component ReadingCard\n  view\n    column background-color="reading.card" padding="space.md"\n      text "x"\n');
  const pkgDesign = 'tamber reading\n\ntheme\n  reading.card = #112233\n  reading.measure: length = 640px\n';
  writeFileSync(join(pkg, 'reading.tamber'), pkgDesign);
  writeFileSync(join(app, 'app.contract'), 'use ReadingCard from "@acme/reading"\ncomponent App\n  view\n    column background-color="brand" padding="space.md" border-radius="radius.md"\n      ReadingCard()\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', 'app.json', 'app.tamber', 'app.contract'); git('commit', '-q', '-m', 'design');
  const tool = await proxied(app);
  try {
    // What the server says of the scale, the targets and the package, as the tool reads it.
    const token = (await (await fetch(`${tool.origin}/__tamber/token`)).json()).token;
    const described = (await (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"describe"}' })).json()).ok;
    const row = name => described.base.find(r => r.name === name);
    const shipped = [...new Set(described.targets.map(t => t.column).filter(Boolean))];
    expect(shipped.length).toBeGreaterThan(0);
    const cell = (name, column) => { const c = row(name).columns[column]; return `${LABELS[column]} ${c.value}${c.source.kind === 'stretch' ? ' (a stretch)' : ''} · ${from(c.source)}`; };
    const measure = described.ir.tokens.find(t => t.name === 'reading.measure'), pkgName = measure?.package;
    expect(pkgName).toBeTruthy();
    const replies = await drive(tool.url, `design-tool-scale-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'tree', 'layout',
      'tap token-base-more-space.md', 'tap token-base-more-radius.md', 'tree',
      // Set for this app: space.md, from the first shipped column's value.
      'tap set-for-app-space.md', 'clock data', 'clock +1', 'clock data', 'state', 'tree', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      // Newly the app's, its cell edits as the app's line.
      'tap cell-space.md-0-light', 'clock data', 'tree',
      // Back to the base, then undo it.
      'tap back-space.md', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      'tap undo', 'clock data', 'clock +1', 'clock data', 'tree',
      // A package's value opens read only; Set for this app from there writes the app's line over it, and Back to the package removes it.
      'tap cell-reading.measure-0-light', 'clock data', 'tree',
      'tap set-for-app-reading.measure', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      'tap back-reading.measure', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data', 'tree',
      // radius.full, the pill, set for this app starts from a length an app may write.
      'tap set-for-app-radius.full', 'clock data', 'clock +1', 'clock data', 'state', 'tap apply', 'clock data', 'clock +1', 'clock data',
      // An alias over a base name: the base name is the value's input.
      'tap cell-card.pad-0-light', 'clock data', 'tree',
      // The app's override of a base name edits as the app's line (explain carries its branches).
      'tap cell-radius.md-0-light', 'clock data', 'tree',
    ]);
    const trees = replies.filter(r => r.nodes && !('x' in (r.nodes[0] ?? {})));
    // A base row reads across the grid, as it is laid out: its name, then each shipped column's cell side by side
    // (not stacked in the name column), then its controls (finding #3 of the clone's pass).
    const laid = replies.find(r => r.nodes && 'x' in (r.nodes[0] ?? {}));
    const box = id => laid.nodes.find(n => (n.props?.testId ?? n.testId) === id);
    const cells = shipped.map(c => box(`token-base-cell-space.md-${c}`));
    expect(cells.every(Boolean)).toBe(true);
    for (let i = 1; i < cells.length; i++) { expect(cells[i].x).toBeGreaterThanOrEqual(cells[i - 1].x + cells[i - 1].w); expect(cells[i].y).toBeLessThan(cells[0].y + cells[0].h); expect(cells[0].y).toBeLessThan(cells[i].y + cells[i].h); }
    expect(cells[0].x).toBeGreaterThan(box('token-source-space.md').x);
    expect(box('set-for-app-space.md').x).toBeGreaterThanOrEqual(cells.at(-1).x + cells.at(-1).w);
    const node = (t, id) => t.nodes.find(n => n.props.testId === id);
    const text = (t, id) => node(t, id)?.props.text;
    const [first, expanded, setAsked, setDone, setCell, backDone, undone, pkgCell, pkgSet, pkgBack, aliasCell, overCell] = trees;
    const value = (name, column = shipped[0]) => row(name).columns[column].value;
    // The base group, its rows per platform: the shipped columns first, in describe's targets' order, the rest one expand away.
    expect(text(first, 'token-group-Base scale (Exact)')).toBe('Base scale (Exact)');
    expect(text(first, 'token-source-space.md')).toBe('Exact\'s base, per platform');
    const order = (t, name) => t.nodes.map(n => n.props.testId).filter(id => id?.startsWith(`token-base-${name}-`) && !id.startsWith(`token-base-more-`)).map(id => id.slice(`token-base-${name}-`.length));
    const rest = Object.keys(LABELS).filter(c => !shipped.includes(c));
    expect(order(first, 'space.md')).toEqual(shipped);
    expect(order(expanded, 'space.md')).toEqual([...shipped, ...rest]);
    for (const column of [...shipped, ...rest]) expect(text(expanded, `token-base-space.md-${column}`)).toBe(cell('space.md', column));
    // Every source kind the server emits, as the tool words it: metric, measured, guideline, a stretch, the pill, cells.
    for (const column of [...shipped, ...rest]) expect(text(expanded, `token-base-radius.md-${column}`)).toBe(cell('radius.md', column));
    // The Mac's space.xl, the one stretch (stage 13 D5): marked, with its metric and why.
    expect(row('space.xl').columns.mac.source.kind).toBe('stretch');
    expect(text(expanded, 'token-base-space.xl-mac')).toBe(cell('space.xl', 'mac'));
    expect(text(expanded, 'token-base-space.xl-mac')).toContain('(a stretch) · grouped-label-inset, ');
    expect(text(first, `token-base-radius.full-${shipped[0]}`)).toBe(cell('radius.full', shipped[0]));
    expect(new Set(Object.values(row('radius.md').columns).map(c => c.source.kind)).size).toBeGreaterThan(1);
    // The app's override of radius.md, and the package's tokens under their package, as the server names it.
    expect(text(first, 'token-over-radius.md')).toBe(`Set for this app, over Exact's base (${LABELS[shipped[0]]} ${value('radius.md')}).`);
    expect(node(first, 'set-for-app-radius.md')).toBeUndefined();
    expect(text(first, `token-group-${pkgName} package`)).toBe(`${pkgName} package`);
    expect(text(first, 'token-source-reading.measure')).toBe(`From the ${pkgName} package`);
    // Set for this app: set-token, from the first shipped column's value, said beside the proposal; the file is the design and that one line.
    const states = replies.filter(r => r.slots);
    expect([states[0].slots.editKind, states[0].slots.editTarget, states[0].slots.editValue]).toEqual(['set-token', 'space.md', value('space.md')]);
    expect(text(setAsked, 'set-for-app-note')).toBe(`space.md for this app starts from ${LABELS[shipped[0]]}'s ${value('space.md')}, and the app's value is every platform's.`);
    expect(tool.writes.slice(0, 3).map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true], ['undo', true]]);
    const set = tool.writes[0].after;
    expect(set).toBe(`${design}  space.md = ${value('space.md')}\n`);
    expect(text(setDone, 'token-over-space.md')).toBe(`Set for this app, over Exact's base (${LABELS[shipped[0]]} ${value('space.md')}).`);
    expect(node(setDone, 'set-for-app-space.md')).toBeUndefined();
    expect(text(setCell, 'branch-readonly')).toBeUndefined();
    expect(node(setCell, 'branch-value')?.props.value ?? text(setCell, 'branch-value')).toBe(value('space.md'));
    // Back to the base: remove-token, the design as it was, Exact's row again with Set offered; its undo gives back Set's bytes.
    expect([states[1].slots.editKind, states[1].slots.editTarget]).toEqual(['remove-token', 'space.md']);
    expect(tool.writes[1].after).toBe(design);
    expect(text(backDone, 'token-over-space.md')).toBeUndefined();
    expect(node(backDone, 'set-for-app-space.md')).toBeTruthy();
    expect(tool.writes[2].after).toBe(set);
    expect(text(undone, 'token-over-space.md')).toBe(text(setDone, 'token-over-space.md'));
    // The package's token: the app's line over it (its kind kept, as set-token writes it), then gone again, Set offered again.
    expect([states[2].slots.editKind, states[2].slots.editTarget, states[2].slots.editValue]).toEqual(['set-token', 'reading.measure', '640px']);
    expect(tool.writes.slice(3).map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true], ['transaction', true]]);
    expect(tool.writes[3].after).toBe(`${set}  reading.measure: length = 640px\n`);
    expect(text(pkgSet, 'token-over-reading.measure')).toBe(`Set for this app, over the ${pkgName} package's value.`);
    expect(node(pkgSet, 'set-for-app-reading.measure')).toBeUndefined();
    expect(states[3].slots.editKind).toBe('remove-token');
    expect(tool.writes[4].after).toBe(set);
    // Back to the package removed the app's line alone: the package's own files, byte for byte as they were (LLP 1139 D5).
    expect(readFileSync(join(pkg, 'reading.tamber'), 'utf8')).toBe(pkgDesign);
    expect(text(pkgBack, 'token-over-reading.measure')).toBeUndefined();
    expect(node(pkgBack, 'set-for-app-reading.measure')).toBeTruthy();
    // Read only in the branch editor (the field disabled); the app's override of a base name edits as the app's line.
    expect(text(pkgCell, 'branch-readonly')).toBe(`The ${pkgName} package's, read only: Set for this app (in the grid) to change it here.`);
    const disabled = (t, id) => { const n = node(t, id); return !!(n?.props.disabled ?? n?.props.accessibilityDisabled); };
    expect(disabled(pkgCell, 'branch-value')).toBe(true);
    expect(text(overCell, 'branch-readonly')).toBeUndefined();
    expect(disabled(overCell, 'branch-value')).toBe(false);
    expect(node(overCell, 'branch-value')?.props.value ?? text(overCell, 'branch-value')).toBe('12px');
    // The pill as a length: the server's proposal for this draft, applied, the app's line written.
    expect([states[4].slots.editTarget, states[4].slots.editValue]).toEqual(['radius.full', '9999px']);
    expect(states[4].resources.proposal).toMatchObject({ ok: true, active: true });
    expect(states[4].resources.proposal.diff).toContain('+  radius.full = 9999px');
    expect(tool.writes[5].after).toBe(`${set}  radius.full = 9999px\n`);
    // The alias's input is the base name it names (explain's bases).
    expect(text(aliasCell, 'branch-inputs')).toContain('space.md');
    expect(node(aliasCell, 'input-space.md')).toBeTruthy();
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('a package token set for this app: its value edited before Apply is still the app\'s line (set-token), and edited again after (D4 batch 2 #1)', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-override-'));
  const design = 'tamber t\n\ntheme\n  brand = reading.card\n';
  writeFileSync(join(app, 'app.tamber'), design);
  // Another design file the app holds, open in the tool beside the app's.
  writeFileSync(join(app, 'brand.tamber'), 'tamber brand\n\ntheme\n  brand.ink = #202020\n');
  const pkg = join(app, 'node_modules/@acme/reading');
  mkdirSync(pkg, { recursive: true });
  writeFileSync(join(pkg, 'package.json'), '{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}');
  writeFileSync(join(pkg, 'index.contract'), 'component ReadingCard\n  view\n    column background-color="reading.card" width="reading.measure"\n      text "x"\n');
  writeFileSync(join(pkg, 'reading.tamber'), 'tamber reading\n\ntheme\n  reading.card = #112233\n  reading.measure: length = 640px\n');
  writeFileSync(join(app, 'app.contract'), 'use ReadingCard from "@acme/reading"\ncomponent App\n  view\n    column background-color="brand"\n      ReadingCard()\n');
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', 'app.tamber', 'brand.tamber', 'app.contract'); git('commit', '-q', '-m', 'design');
  const tool = await proxied(app);
  try {
    const replies = await drive(tool.url, `design-tool-override-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data',
      // Set for this app, its value changed in the proposal's field before Apply (Enter submits it).
      'tap cell-reading.measure-0-light', 'clock data',
      'tap set-for-app-reading.measure', 'clock data',
      'type new-value 700px', 'type new-value key Enter', 'clock data', 'clock +1', 'clock data', 'state',
      'tap apply', 'clock data', 'clock +1', 'clock data',
      // Now the app's line: its cell edits it again.
      'tap cell-reading.measure-0-light', 'clock data', 'clock +1', 'clock data',
      'type branch-value 720px', 'type branch-value key Enter', 'clock data', 'clock +1', 'clock data', 'state',
      'tap apply', 'clock data', 'clock +1', 'clock data',
      // The form again after a cell edit (round 1, Astra): its own draft, not the cell's, so Apply goes.
      'type new-name reading.measure', 'type new-value 740px', 'type new-value key Enter', 'clock data', 'clock +1', 'clock data', 'state',
      'tap apply', 'clock data', 'clock +1', 'clock data',
      // An app token's name typed in the form stays add-token (round 1, Grok and Astra: decided by the rows, not the last Set).
      'type new-name brand.two', 'type new-value #445566', 'type new-value key Enter', 'clock data', 'state',
      'tap cancel-edit', 'clock data',
      // Their read-only cell open, then the form names the same token (round 2, Astra and Grok): the cell is no guard on it.
      'tap back-reading.measure', 'clock data', 'clock +1', 'clock data', 'tap apply', 'clock data', 'clock +1', 'clock data',
      'tap cell-reading.measure-0-light', 'clock data',
      'type new-name reading.measure', 'type new-value 760px', 'type new-value key Enter', 'clock data', 'clock +1', 'clock data', 'state',
      'tap apply', 'clock data', 'clock +1', 'clock data',
      // Another file open: their rows show beside its own, but a name added there is that file's line (round 2).
      'type file brand.tamber', 'clock data', 'clock +1', 'clock data',
      'type new-name reading.measure', 'type new-value 780px', 'type new-value key Enter', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.slots);
    expect([states[0].slots.editKind, states[0].slots.editTarget, states[0].slots.editValue]).toEqual(['set-token', 'reading.measure', '700px']);
    expect(states[0].resources.proposal).toMatchObject({ ok: true, active: true });
    expect(tool.writes.slice(0, 2).map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true]]);
    expect(tool.writes[0].after).toBe(`${design}  reading.measure: length = 700px\n`);
    expect([states[1].slots.editKind, states[1].slots.editTarget, states[1].slots.editValue]).toEqual(['set-token', 'reading.measure', '720px']);
    expect(tool.writes[1].after).toBe(`${design}  reading.measure: length = 720px\n`);
    expect([states[2].slots.editKind, states[2].slots.editValue]).toEqual(['set-token', '740px']);
    expect(tool.writes[2]).toMatchObject({ method: 'transaction', ok: true });
    expect(tool.writes[2].after).toBe(`${design}  reading.measure: length = 740px\n`);
    expect([states[3].slots.editKind, states[3].slots.editTarget]).toEqual(['add-token', 'brand.two']);
    expect([states[4].slots.editKind, states[4].slots.editTarget, states[4].slots.selToken]).toEqual(['set-token', 'reading.measure', '']);
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['transaction', true], ['transaction', true], ['transaction', true], ['transaction', true], ['transaction', true]]);
    expect(tool.writes[3].after).toBe(design);
    expect(tool.writes[4].after).toBe(`${design}  reading.measure: length = 760px\n`);
    expect([states[5].slots.file, states[5].slots.editKind, states[5].slots.editTarget]).toEqual(['brand.tamber', 'add-token', 'reading.measure']);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
