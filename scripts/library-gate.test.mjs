// apps/library-gate (LLP 1139 §5 E) places every exported library component
// once per value of each variant prop, one varied at a time (the others at
// their first value; a bool's first is false), and a component with no
// variant props once. The exports and their variants are the compiler's
// (`contract tamber ir`'s `library`: D0's exports, D5's variants), not a read
// of the sources. Cells whose id starts `sample-` show other states and are
// held only to naming real props. Every cell renders (no `if`, `for` or
// `match` in the view), and every PostCard's slot holds its marker.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { cellsFor, conditional, markedSlot, option, placements, uncommented } from './library-gate.mjs';
import { SLOTTED } from './stage12-gate.mjs';
import { COMPONENTS, DERIVED, EMPTY, EXPORTED_TOKENS, PARAM_TOKENS, READS, cases as restyleCases, newTokens, packageParams, readsOf } from './s13c-restyle.mjs';

const ROOT = new URL('..', import.meta.url).pathname;
const APP = join(ROOT, 'apps/library-gate/app.contract');

/** The components of these packages that declare a slot, by their sources. */
function slottedIn(packages) {
  const out = [];
  for (const pkg of packages) {
    const dir = join(ROOT, 'packages', pkg);
    for (const f of readdirSync(dir).filter(f => f.endsWith('.contract'))) {
      for (const block of readFileSync(join(dir, f), 'utf8').split(/^(?=component )/m)) {
        const name = /^component (\w+)/.exec(block)?.[1];
        if (name && /^  slot\s*$/m.test(block.split(/^  view\s*$/m)[0])) out.push(name);
      }
    }
  }
  return out;
}

function ir() {
  const out = join(mkdtempSync(join(tmpdir(), 'library-gate-ir-')), 'ir.json');
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'ir', join(ROOT, 'apps/library-gate'), '-o', out], { cwd: ROOT, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`contract tamber ir: ${r.stderr}`);
  return JSON.parse(readFileSync(out, 'utf8')).library.filter(c => c.library);
}

test('every exported component, once per variant value, every cell rendered, every slot marked', () => {
  const library = ir();
  const names = library.map(c => c.name);
  expect(names.sort()).toEqual(['ActionRow', 'AtCard', 'Avatar', 'BarItem', 'ComposeFrame', 'FollowButton', 'HeaderButton', 'ListRow', 'PersonRow', 'PostButton', 'PostCard', 'ProfileHeader', 'SwitchRow', 'VerifiedBadge']);
  // The gate's slot-rendered line reads every component with a slot (one at
  // its view's root, as ComposeFrame's, is in no part).
  const slotted = slottedIn(['actions', 'cards', 'lists']).filter(n => names.includes(n));
  expect(slotted.sort()).toEqual([...SLOTTED].sort());
  const app = readFileSync(APP, 'utf8');
  // One component, the root, so every placement is in the rendered view.
  expect([...app.matchAll(/^component (\w+)/gm)].map(m => m[1])).toEqual(['App']);
  expect(conditional(app)).toEqual([]);
  const placed = placements(app, names);
  // Every placement has an id: its own prop, or the box that holds only it.
  expect(placed.filter(p => !p.id).map(p => p.name)).toEqual([]);
  for (const c of library) {
    const mine = placed.filter(p => p.name === c.name);
    const cells = mine.filter(p => !p.id.startsWith('sample-'));
    const want = cellsFor(c.variants).map(v => JSON.stringify(v)).sort();
    const got = cells.map(p => JSON.stringify(Object.fromEntries(c.variants.map(v => [v.prop, option(p.values[v.prop])])))).sort();
    expect([c.name, got]).toEqual([c.name, want]);
    // A slot's marker in every placement of a component with one: the
    // slot's first child, the glyph directly under it.
    if (slotted.includes(c.name)) {
      for (const p of mine) expect([p.id, markedSlot(p)]).toEqual([p.id, true]);
    }
  }
  // Ids are unique, and only the library's exports are used.
  const ids = placed.map(p => p.id);
  expect(new Set(ids).size).toBe(ids.length);
  const used = [...app.matchAll(/^use (.+) from "@exact\/(\w+)"/gm)].flatMap(([, n]) => n.split(',').map(x => x.trim()));
  expect(used.filter(n => names.includes(n)).sort()).toEqual(names.sort());
  // Besides the components, only the shapes their props take.
  expect(used.filter(n => !names.includes(n))).toEqual(['ProfileAlert']);
}, 600_000);

// `contract gallery` (the design tool's variant gallery, LLP 1139 §9 Q2): the same cells as this app's, by the compiler's
// own rule, each component's every cell placed (none refused), and the gallery's design the app's own.
test("contract gallery places the cells this app does, each placed, on the app's design", () => {
  const library = ir();
  mkdirSync(join(ROOT, 'target'), { recursive: true });
  const out = mkdtempSync(join(ROOT, 'target', 'library-gate-gallery-'));
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'gallery', join(ROOT, 'apps/library-gate'), '-o', out], { cwd: ROOT, encoding: 'utf8' });
  expect([r.status, r.stderr]).toEqual([0, '']);
  expect(r.stdout).toContain('gallery: 14 components, 54 cells (0 refused)');
  const gallery = readFileSync(join(out, 'app.contract'), 'utf8');
  expect(gallery).not.toContain("can't place");
  const placed = placements(gallery, library.map(c => c.name)), own = placements(readFileSync(APP, 'utf8'), library.map(c => c.name));
  for (const c of library) {
    const cells = placed.filter(p => p.name === c.name);
    // Beside the compiler's variants, a prop the view compares with literals in an `if` is the gallery's variant too:
    // a string's one cell a literal and the empty one (stage 15 D4 batch 1 #2), an enum's (by == or !=) one cell a
    // declared value (VerifiedBadge's, PostCard's and PersonRow's badge, stage 15 B batch 2): the values this app places.
    const extra = Object.keys(cells[0]?.values ?? {}).filter(k => !c.variants.some(v => v.prop === k) && new Set(cells.map(p => p.values[k])).size > 1)
      .map(prop => ({ prop, options: [...new Set(cells.map(p => option(p.values[prop])))] }));
    for (const { prop, options } of extra) expect([c.name, prop, [...options].sort()]).toEqual([c.name, prop, [...new Set(own.filter(p => p.name === c.name).map(p => option(p.values[prop])))].sort()]);
    const variants = [...c.variants, ...extra];
    const want = cellsFor(variants).map(v => JSON.stringify(v)).sort();
    const got = cells.map(p => JSON.stringify(Object.fromEntries(variants.map(v => [v.prop, option(p.values[v.prop])])))).sort();
    expect([c.name, got]).toEqual([c.name, want]);
  }
  expect(JSON.parse(readFileSync(join(out, 'app.json'), 'utf8')).tamber).toBe(relative(out, join(ROOT, 'apps/library-gate/app.tamber')));
  rmSync(out, { recursive: true, force: true });
}, 600_000);

// FollowButton says following by its label alone (theme2's 14:15 ruling, no
// aria-pressed; contract/cli/tests/it/actions_exports.rs asserts the label in
// both states, round too). A round one shows no text, so its label must not
// be "", which the compiler can't see (one prop required by another's
// value): every round cell names one, and in both states, the app's swap.
/** The placement of `id` in `app` (its line, comments set aside) writes
 * `prop` as a whole string literal with words in it (or, `empty`, exactly
 * `""`): the closing quote ends the argument (a `,` or the call's `)` next),
 * so `"Follow Ada".slice(99)`, `"" + "Following Ada"`, a missing prop and
 * `""` (unless `empty`) all fail. */
function wordsWritten(app, id, prop, empty = false) {
  const line = app.split('\n').map(uncommented).find(l => l.includes(`(id="${id}",`));
  const value = empty ? '""' : '"\\s*\\S[^"]*"';
  return !!line && new RegExp(`[(,]\\s*${prop}=${value}\\s*(?:,|\\)\\s*$)`).test(line);
}
test("every round FollowButton cell names its label, the label swapped with the state", () => {
  const app = readFileSync(APP, 'utf8');
  const placed = placements(app, ['FollowButton']);
  const round = placed.filter(p => option(p.values.shape) === 'round');
  expect(round.length).toBeGreaterThanOrEqual(3);
  for (const p of round) expect([p.id, wordsWritten(app, p.id, 'label')]).toEqual([p.id, true]);
  expect(new Set(round.map(p => option(p.values.following))).size).toBe(2);
  // The guard refuses what would leave a round button speechless.
  const cell = l => `      FollowButton(id="r", following=false, text="", ${l}tone="primary", size="small", shape="round", icon="both", press=press)\n`;
  expect(['label="Follow Ada", ', 'label="Follow Ada".slice(99), ', 'label="", ', 'label=" ", ', '', 'label=name, '].map(l => wordsWritten(cell(l), 'r', 'label'))).toEqual([true, false, false, false, false, false]);
  // A pill's "" is the empty literal alone, not the head of an expression.
  expect(['label="", ', 'label="" + "Following Ada", ', 'label="Follow", '].map(l => wordsWritten(cell(l), 'r', 'label', true))).toEqual([true, false, false]);
  // Each cell's words are its state's: the spoken label (the text where it's
  // "") starts "Following" following, "Follow " or is "Follow" not; every
  // label and text a whole literal (or "" for a label).
  for (const p of placed) {
    const said = option(p.values.label) || option(p.values.text);
    const on = option(p.values.following) === 'true';
    expect([p.id, on ? said.startsWith('Following') : !said.startsWith('Following') && said.startsWith('Follow')]).toEqual([p.id, true]);
    expect([p.id, wordsWritten(app, p.id, 'text'), wordsWritten(app, p.id, 'label') || wordsWritten(app, p.id, 'label', true)]).toEqual([p.id, true, true]);
  }
});

// A pill's label is as tall as its icon's box (Bluesky's leading_snug on
// text_sm and text_xs, 17 and 15 px), so it keeps its height when the icon
// comes or goes with the state (icon "follow" or "following").
test("a FollowButton label's line is its icon box's height, each size", () => {
  const design = readFileSync(join(ROOT, 'packages/actions/actions.tamber'), 'utf8');
  const px = name => {
    const v = new RegExp(`^\\s+${name.replace('.', '\\.')}: length = (\\S+)`, 'm').exec(design)?.[1];
    return v?.endsWith('rem') ? Number.parseFloat(v) * 16 : Number.parseFloat(v);
  };
  expect([px('actions.follow-line'), px('actions.follow-tiny-line')]).toEqual([px('actions.follow-icon-box'), px('actions.follow-tiny-icon-box')]);
  expect([px('actions.follow-line'), px('actions.follow-tiny-line')]).toEqual([17, 15]);
});

test('the cells rule, and the parse refuses what could hide a cell', () => {
  expect(cellsFor([{ prop: 'a', options: ['false', 'true'] }, { prop: 'k', options: ['x', 'y', 'z'] }])).toEqual([
    { a: 'false', k: 'x' }, { a: 'true', k: 'x' }, { a: 'false', k: 'y' }, { a: 'false', k: 'z' }]);
  expect(cellsFor([])).toEqual([{}]);
  // A trailing comment still counts as a placement; an `if` is reported.
  const app = '  view\n    column\n      Card(id="a", on=true)  // extra\n      if false\n        Card(id="b", on=false)\n';
  expect(placements(app, ['Card']).map(p => p.id)).toEqual(['a', 'b']);
  expect(conditional(app)).toEqual(['      if false']);
  expect(() => placements('    Card(\n      id="a"\n    )\n', ['Card'])).toThrow(/not on one line/);
  // A comment can't stand in for an argument or a marker.
  expect(placements('      Card(id="a", on=false)  // on=true\n', ['Card'])[0].values.on).toBe('false');
  expect(uncommented('      text "a // b"  // c')).toBe('      text "a // b"');
  const marked = '      Card(id="a")\n        box testId="marker-a" press=toggle width=24 height=24\n          text glyph\n';
  const unmarked = '      Card(id="a")\n        // box testId="marker-a" press=toggle width=24 height=24\n        text glyph\n';
  expect([markedSlot(placements(marked, ['Card'])[0]), markedSlot(placements(unmarked, ['Card'])[0])]).toEqual([true, false]);
  // A component with no id prop takes the id of the box holding only it:
  // the box two spaces out, named and nothing else, with no sibling after.
  const held = '      box testId="av-a"\n        Pic(src="")\n      box testId="av-b" width=4\n        Pic(src="")\n      box testId="av-c"\n        Pic(src="")\n        Pic(src="x")\n';
  expect(placements(held, ['Pic']).map(p => p.id)).toEqual(['av-a', undefined, undefined, undefined]);
  expect(placements('      Card(id="a")\n', ['Card'])[0].id).toBe('a');
});

// The gate's restyle cases (theme2 09:49 (c), 10:28 (5)): every token and
// param stage 13 C added, by the packages' sources, is in a case and read
// somewhere, so a new one with no case or no read fails here; and every
// token stage 15 B's exported Avatar and VerifiedBadge draw with.
test("the restyle cases name every stage 13 C token and param and every exported use's token, each one read", () => {
  const tokens = newTokens(), params = packageParams(), all = restyleCases();
  const app = readFileSync(APP, 'utf8');
  const named = new Set(all.flatMap(c => c.set.map(s => (c.param ? s.key : s.name))));
  const derived = new Set(all.flatMap(c => c.derived));
  expect(tokens.map(t => t.name).filter(n => !named.has(n) && !derived.has(n))).toEqual([]);
  expect(params.map(p => p.key).filter(k => !named.has(k))).toEqual([]);
  // Each param's default is the token the cases pin it to (a default moved
  // to another token fails here, not in a case that would follow it).
  expect(Object.fromEntries(params.map(p => [p.key, p.token]))).toEqual(PARAM_TOKENS);
  // Read on a node: itself, through a param it is the default of, or as an
  // input of a derived token that is.
  const read = n => readsOf(n).length > 0 || params.some(p => PARAM_TOKENS[p.key] === n && readsOf(p.key).length > 0)
    || Object.entries(DERIVED).some(([d, x]) => x.from.includes(n) && readsOf(d).length > 0);
  expect([...named, ...derived].filter(n => !read(n))).toEqual([]);
  // No read of a name that isn't stage 13 C's or the exports'.
  expect(Object.keys(READS).filter(k => !tokens.some(t => t.name === k) && !params.some(p => p.key === k) && !EXPORTED_TOKENS.includes(k))).toEqual([]);
  // The exports' tokens are every token their sources name (a token one of
  // them comes to draw with fails here until the case sets it and a read
  // reads it), each set in the exports' case and read on their cells.
  const common = readFileSync(join(ROOT, 'packages/cards/common.contract'), 'utf8');
  const exportsBody = common.slice(common.indexOf('component Avatar'));
  const own = [...new Set([...exportsBody.matchAll(/"(cards\.[\w-]+)"/g)].map(m => m[1]))].filter(n => !/^cards\.(verified|verifier)(-check)?$/.test(n));
  const passed = [...new Set([...app.matchAll(/^\s+(?:Avatar|VerifiedBadge)\((.*)\)\s*$/gm)].flatMap(([, a]) => [...a.matchAll(/(?:size|ink|mark)="(cards\.[\w-]+)"/g)].map(m => m[1])))];
  expect([...new Set([...own, ...passed])].sort()).toEqual([...EXPORTED_TOKENS].sort());
  const exportsCase = all.find(c => c.name === 'Avatar and VerifiedBadge tokens');
  expect(exportsCase.set.map(s => s.name).sort()).toEqual([...EXPORTED_TOKENS].sort());
  // Each is read on the exports' own cells, only there, and by every one of
  // the two that draws with it (cards.badge-mark: Avatar's figure and
  // VerifiedBadge's check), so a read dropped from one fails here.
  const drawnBy = n => ['Avatar', 'VerifiedBadge'].filter(c => {
    const body = exportsBody.slice(exportsBody.indexOf(`component ${c}`)).split(/\n(?=component )/)[0];
    return body.includes(`"${n}"`) || [...app.matchAll(new RegExp(`^\\s+${c}\\((.*)\\)\\s*$`, 'gm'))].some(([, a]) => a.includes(`"${n}"`));
  });
  for (const n of EXPORTED_TOKENS) {
    const reads = readsOf(n);
    expect([n, reads.length > 0 && reads.every(r => ['Avatar', 'VerifiedBadge'].includes(r.component))]).toEqual([n, true]);
    expect([n, [...new Set(reads.map(r => r.component))].sort()]).toEqual([n, drawnBy(n).sort()]);
  }
  // The "none" badge's cell is library-gate's.
  expect(EMPTY.every(id => placements(app, ['VerifiedBadge']).some(p => p.id === id && option(p.values.badge) === 'none'))).toBe(true);
  // The cases' components and instances are library-gate's.
  // (The empty cell has nothing to read, so it is held apart, in EMPTY.)
  for (const [name, c] of Object.entries(COMPONENTS)) expect([name, placements(app, [name]).map(p => p.id).filter(id => !EMPTY.includes(id)).sort()]).toEqual([name, [...c.ids].sort()]);
});
