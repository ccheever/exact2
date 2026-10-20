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
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { cellsFor, conditional, markedSlot, option, placements, uncommented } from './library-gate.mjs';

const ROOT = new URL('..', import.meta.url).pathname;
const APP = join(ROOT, 'apps/library-gate/app.contract');

function ir() {
  const out = join(mkdtempSync(join(tmpdir(), 'library-gate-ir-')), 'ir.json');
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'ir', join(ROOT, 'apps/library-gate'), '-o', out], { cwd: ROOT, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`contract tamber ir: ${r.stderr}`);
  return JSON.parse(readFileSync(out, 'utf8')).library.filter(c => c.library);
}

test('every exported component, once per variant value, every cell rendered, every slot marked', () => {
  const library = ir();
  const names = library.map(c => c.name);
  expect(names.sort()).toEqual(['ActionRow', 'AtCard', 'PostButton', 'PostCard']);
  const app = readFileSync(APP, 'utf8');
  // One component, the root, so every placement is in the rendered view.
  expect([...app.matchAll(/^component (\w+)/gm)].map(m => m[1])).toEqual(['App']);
  expect(conditional(app)).toEqual([]);
  const placed = placements(app, names);
  for (const c of library) {
    const mine = placed.filter(p => p.name === c.name);
    const cells = mine.filter(p => !p.id.startsWith('sample-'));
    const want = cellsFor(c.variants).map(v => JSON.stringify(v)).sort();
    const got = cells.map(p => JSON.stringify(Object.fromEntries(c.variants.map(v => [v.prop, option(p.values[v.prop])])))).sort();
    expect([c.name, got]).toEqual([c.name, want]);
    // A slot's marker in every placement of a component with one: the
    // slot's first child, the glyph directly under it.
    if (c.parts.some(p => p.slot)) {
      for (const p of mine) expect([p.id, markedSlot(p)]).toEqual([p.id, true]);
    }
  }
  // Ids are unique, and only the library's exports are used.
  const ids = placed.map(p => p.id);
  expect(new Set(ids).size).toBe(ids.length);
  const used = [...app.matchAll(/^use (.+) from "@exact\/(\w+)"/gm)].flatMap(([, n]) => n.split(',').map(x => x.trim()));
  expect(used.sort()).toEqual(names.sort());
}, 600_000);

// `contract gallery` (the design tool's variant gallery, LLP 1139 §9 Q2): the same cells as this app's, by the compiler's
// own rule, each component's every cell placed (none refused), and the gallery's design the app's own.
test("contract gallery places the cells this app does, each placed, on the app's design", () => {
  const library = ir();
  mkdirSync(join(ROOT, 'target'), { recursive: true });
  const out = mkdtempSync(join(ROOT, 'target', 'library-gate-gallery-'));
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'gallery', join(ROOT, 'apps/library-gate'), '-o', out], { cwd: ROOT, encoding: 'utf8' });
  expect([r.status, r.stderr]).toEqual([0, '']);
  expect(r.stdout).toContain('gallery: 4 components, 11 cells (0 refused)');
  const gallery = readFileSync(join(out, 'app.contract'), 'utf8');
  expect(gallery).not.toContain("can't place");
  const placed = placements(gallery, library.map(c => c.name));
  for (const c of library) {
    const want = cellsFor(c.variants).map(v => JSON.stringify(v)).sort();
    const got = placed.filter(p => p.name === c.name).map(p => JSON.stringify(Object.fromEntries(c.variants.map(v => [v.prop, option(p.values[v.prop])])))).sort();
    expect([c.name, got]).toEqual([c.name, want]);
  }
  expect(JSON.parse(readFileSync(join(out, 'app.json'), 'utf8')).tamber).toBe(relative(out, join(ROOT, 'apps/library-gate/app.tamber')));
  rmSync(out, { recursive: true, force: true });
}, 600_000);

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
});
