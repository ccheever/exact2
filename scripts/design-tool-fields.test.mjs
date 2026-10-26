// The design tool's import review and typed fields against a real design server (split from design-tool.test.mjs, so
// a lane runs them beside the others).
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ROOT, STUB_APP, drive, edit, proxied, toolTurns } from './design-tool-harness.mjs';

toolTurns();

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
    // edit holds the import again whole, with the edit, every override still
    // offered (third pass #6), the card's choice carried to it (AccentColor
    // ticked, shadcn.foreground left out). Untick AccentColor there and Accept:
    // the server recomputes without it (nothing written), and that is accepted
    // as one transaction; one Undo gives the bytes back.
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
    // (it begins as a fresh import does, overrides off until ticked: `kept`, the card's heading, says so)
    expect(after).toMatchObject({ from: id, kept: false });
    expect(after.mapped[0].modes[0].value).toBe('light-dark(oklch(0.99 0 0), oklch(0.145 0 0))');
    // Whole, the choice carried: shadcn.foreground still held but left out, every override offered, AccentColor alone
    // ticked (a recomputed import begins with all on; the carried ids flip the rest off).
    const flips = later[0].slots.picked;
    expect(after.mapped.map(m => m.name)).toContain('shadcn.foreground');
    expect(flips).toContain(`${edited}:1`);
    expect(after.offers.map(o => o.role)).toEqual(['-exact-background', '-exact-label', '-exact-secondary-background', '-exact-separator', 'AccentColor']);
    expect(after.offers.filter(o => o.on !== flips.includes(`${edited}:${o.items}`)).map(o => o.role)).toEqual(['AccentColor']);
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

test('third pass #6, end to end: ticking a role override then editing a value, and editing then ticking, hold the same import and the same choice, and the override is written', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-reheld-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const css = join(resolve(ROOT, 'tamber/tests/import'), 'shadcn-neutral.css');
  const value = 'light-dark(oklch(0.99 0 0), oklch(0.145 0 0))';
  const tool = await proxied(app);
  const session = `design-tool-reheld-${process.pid}`;
  const step = async (steps, first = false) => (await drive(tool.url, session, ['clock data', 'clock +1', 'clock data', ...steps, 'clock data', 'clock +1', 'clock data', 'state'], first ? {} : { keepStorage: true })).filter(r => r.resources).at(-1);
  const importOne = ['type import-kind shadcn', 'tap import-choose', `type @import-file ${css}`];
  const editOne = id => [`type mapped-value-${id}-0-0 ${value}`, `type mapped-value-${id}-0-0 key Enter`];
  // What a held import's card has chosen: each token in unless flipped, each override as it begins (`on`) unless
  // flipped.
  const choice = (st, h) => {
    const flips = st.slots.picked;
    return { tokens: h.mapped.filter(m => !flips.includes(`${h.proposal}:${m.index}`)).map(m => m.name),
      offers: h.offers.filter(o => o.items !== '' && o.on !== flips.includes(`${h.proposal}:${o.items}`)).map(o => o.role) };
  };
  try {
    let st = await step(['type name Ada', 'tap save-name', 'clock data', ...importOne], true);
    const a = st.resources.changes.held.find(h => h.importer === 'shadcn').proposal, nth = k => a.replace(/-(\d+)$/, (_, n) => `-${Number(n) + k}`);
    // Each order within one page's session (a card's choice is the page's): order one, tick AccentColor and leave
    // shadcn.foreground out, then edit a value; order two, a second import of the same file, edit the value first,
    // then tick and leave out on the edited one; then accept order two's, recomputed to its choice, and that.
    const [a2, b, b2, kept] = [nth(1), nth(2), nth(3), nth(4)];
    const settle = ['clock data', 'clock +1', 'clock data'];
    const run = async steps => (await drive(tool.url, session, [...settle, ...steps], { keepStorage: true })).filter(r => r.resources);
    const heldOf = (r, id) => r.resources.changes.held.find(h => h.proposal === id);
    const first = await run([`tap override-${a}-AccentColor`, `tap take-${a}-1`, ...editOne(a), ...settle, 'state', ...importOne, ...settle, 'state']);
    const one = { held: heldOf(first[0], a2), choice: choice(first[0], heldOf(first[0], a2)) };
    // (the second import's held list may land after the drive: waited for, as order one's choice is already read)
    for (let i = 0; i < 40 && !heldOf(st = await step([]), b); i++) await new Promise(ok => setTimeout(ok, 250));
    expect(heldOf(st, b)).toBeDefined();
    const replies = await run([...editOne(b), ...settle, `tap override-${b2}-AccentColor`, `tap take-${b2}-1`, ...settle, 'state',
      `tap accept-${b2}`, ...settle, `tap accept-${kept}`, ...settle, 'state']);
    const two = { held: heldOf(replies[0], b2), choice: choice(replies[0], heldOf(replies[0], b2)) };
    expect([one.held.from, two.held.from]).toEqual([a, b]);
    // The same held import (its diff, tokens and every override offered) and the same choice, either way.
    expect(two.held.diff).toBe(one.held.diff);
    expect(two.held.mapped.map(m => [m.name, m.modes.map(x => x.value)])).toEqual(one.held.mapped.map(m => [m.name, m.modes.map(x => x.value)]));
    expect(two.held.offers.map(o => o.role)).toEqual(['-exact-background', '-exact-label', '-exact-secondary-background', '-exact-separator', 'AccentColor']);
    expect(one.held.offers.map(o => o.role)).toEqual(two.held.offers.map(o => o.role));
    expect(two.choice).toEqual(one.choice);
    expect(two.choice.offers).toEqual(['AccentColor']);
    expect(two.choice.tokens).not.toContain('shadcn.foreground');
    // Accepted: written with the override and the edit, without what was left out.
    expect(replies[1].slots.written.note).toBe(`Accepted ${kept}.`);
    const written = tool.writes.at(-1).after;
    expect(written).toContain('AccentColor = shadcn.primary');
    expect(written).toContain(`shadcn.background = ${value}`);
    expect(written).not.toContain('shadcn.foreground =');
    expect(written).not.toContain('-exact-background = shadcn.background');
    // A page loaded again forgets the card's choice: an edited import begins as a fresh one, its overrides off, so
    // none is on that no one ticked there (order one's, still held).
    st = await step([]);
    const again = heldOf(st, a2);
    expect(again.kept).toBe(false);
    expect(again.offers.filter(o => o.on).map(o => o.role)).toEqual([]);
    expect(choice(st, again).offers).toEqual([]);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

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

