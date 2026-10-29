import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { decodePng, encodePng } from './png.mjs';
import { bindingShape, bindingsCheck, identityOf, namedCrops, namedOf, namedReport, pixelDiff, unionNamed } from './stage12-named.mjs';
import { BINDINGS, NAMED, RETIRED, SCREENS, STAGES, comparePixels, useStage } from './stage12-gate.mjs';

const img = (w, h, f) => { const data = new Uint8Array(w * h * 4); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) data.set(f(x, y), (y * w + x) * 4); return encodePng({ width: w, height: h, data }); };
const black = () => [0, 0, 0, 255], red = [255, 0, 0, 255];

test('a named box takes its differing pixels, reported with their box; outside it they still fail', () => {
  const a = img(8, 8, black);
  // Three pixels inside the box at x 2-5, y 2-5, one outside it.
  const b = img(8, 8, (x, y) => ((x === 2 && y === 3) || (x === 4 && y === 5) || (x === 5 && y === 2) || (x === 7 && y === 7) ? red : black()));
  const named = [{ target: 'rows', change: 'row padding 20->16, rows below shift', rect: [2, 2, 4, 4] }];
  const d = pixelDiff(a, b, [], null, named);
  expect(d.differ).toBe(1);
  expect(d.names).toEqual([{ target: 'rows', change: 'row padding 20->16, rows below shift', rect: [2, 2, 4, 4], differ: 3, box: [2, 2, 6, 6] }]);
  // The allowlist first: an allowed pixel is nobody's.
  expect(pixelDiff(a, b, [[2, 3, 1, 1]], null, named).names[0].differ).toBe(2);
  // Without names the result reads as it always did.
  expect(pixelDiff(a, b)).toEqual({ differ: 4, total: 64 });
  // A box no pixel differs in says so: none, no box.
  expect(pixelDiff(a, a, [], null, named).names[0]).toMatchObject({ differ: 0, box: null });
});

test("a name's box comes from the whole layout, clipped to the viewport, and is the union of both sides'", () => {
  const out = ['{"op":"tap"}', JSON.stringify({ viewport: { w: 100, h: 50 }, nodes: [{ testId: 'rows', x: 10.4, y: 20.6, w: 30.2, h: 40 }, { props: { testId: 'empty' }, x: 0, y: 0, w: 0, h: 0 }] })].join('\n');
  const named = [{ screen: 'people', target: 'rows', change: 'c' }, { screen: '*', target: 'empty', change: 'e' }, { screen: 'other', target: 'rows', change: 'x' }];
  expect(namedOf(named, 'people', out)).toEqual([{ target: 'rows', change: 'c', rect: [10, 20, 31, 30] }, { target: 'empty', change: 'e', rect: null }]);
  expect(namedOf(named, 'nowhere', out)).toEqual([{ target: 'empty', change: 'e', rect: null }]);
  expect(unionNamed([{ target: 'rows', change: 'c', rect: [10, 20, 30, 10] }], [{ target: 'rows', change: 'c', rect: [12, 18, 30, 20] }, { target: 'gone', change: 'g', rect: null }]))
    .toEqual([{ target: 'rows', change: 'c', rect: [10, 18, 32, 20] }]);
});

test('the report names each change NAMED, STALE where it differs nowhere, MISSING where no shot showed it', () => {
  const named = [{ screen: 'people', target: 'rows', change: 'c' }, { screen: 'menu', target: 'sheet', change: 's' }, { screen: 'lists', target: 'empty', change: 'e' }];
  const compared = [
    { shot: 'web light people', names: [{ target: 'rows', differ: 12, box: [0, 0, 4, 3] }] },
    { shot: 'web dark people', names: [{ target: 'rows', differ: 0, box: null }] },
    { shot: 'web light menu', names: [{ target: 'sheet', differ: 0, box: null }] },
  ];
  const r = namedReport(named, compared);
  expect(r.map(n => [n.target, n.status, n.differ, n.shots.length, n.seen])).toEqual([['rows', 'NAMED', 12, 1, 2], ['sheet', 'STALE', 0, 0, 1], ['empty', 'MISSING', 0, 0, 0]]);
});

test("every named box's before and after crops are written, differing or not, none for a box no side showed", () => {
  const dir = mkdtempSync(join(tmpdir(), 'named-crops-'));
  try {
    const before = join(dir, 'b.png'), after = join(dir, 'a.png');
    Bun.write(before, img(6, 6, black)); Bun.write(after, img(6, 6, (x, y) => (x === 2 && y === 2 ? red : black())));
    const names = [{ target: 'rows', rect: [1, 1, 3, 2], differ: 1 }, { target: 'quiet', rect: [0, 0, 1, 1], differ: 0 }, { target: 'gone', rect: null, differ: 0 }];
    const files = namedCrops(before, after, names, join(dir, 'crops'), 'web light people');
    expect(files.map(f => f.split('/').pop())).toEqual(['web-light-people-rows-before.png', 'web-light-people-rows-after.png', 'web-light-people-quiet-before.png', 'web-light-people-quiet-after.png']);
    const crop = decodePng(require('node:fs').readFileSync(files[1]));
    expect([crop.width, crop.height, [...crop.data.slice((1 * 3 + 1) * 4, (1 * 3 + 1) * 4 + 4)]]).toEqual([3, 2, red]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('comparePixels gives each shot its named boxes from both sides and fails only what lies outside them', () => {
  const dir = mkdtempSync(join(tmpdir(), 'named-compare-'));
  try {
    const at = n => join(dir, n);
    Bun.write(at('b.png'), img(8, 8, black)); Bun.write(at('a.png'), img(8, 8, (x, y) => (x === 3 && y === 3 ? red : black())));
    const shot = (png, rect) => ({ host: 'web', scheme: 'light', screen: 'people', ok: true, png, allow: [], named: [{ target: 'rows', change: 'c', rect }] });
    const [inside] = comparePixels({ shots: [shot(at('b.png'), [2, 2, 2, 2])] }, { shots: [shot(at('a.png'), [2, 2, 1, 1])] });
    expect([inside.ok, inside.differ, inside.names[0].differ]).toEqual([true, 0, 1]);
    const [outside] = comparePixels({ shots: [shot(at('b.png'), [0, 0, 2, 2])] }, { shots: [shot(at('a.png'), [0, 0, 2, 2])] });
    expect([outside.ok, outside.differ, outside.names[0].differ]).toEqual([false, 1, 0]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('a binding draws nothing of its own and places only what it binds to', () => {
  const PR = '@exact/cards/person-row.contract#PersonRow', FB = '@exact/actions/follow-button.contract#FollowButton';
  const allowed = [PR, FB];
  const call = component => [{ file: '/copy/explore.contract', line: 170, component }];
  const node = (component, library, chainOf) => ({ file: library ? '/pkg/x.contract' : '/copy/explore.contract', line: 9, component, ...(library ? { library } : {}), chain: call(chainOf) });
  // PersonRow's own Avatar, drawn inside the package: its innermost call site is PersonRow's file, then the binding's.
  const avatar = { file: '/r/node_modules/@exact/cards/common.contract', line: 4, component: 'Avatar', library: '@exact/cards/common.contract#Avatar',
    chain: [{ file: '/r/node_modules/@exact/cards/person-row.contract', line: 60, component: 'PersonRow' }, ...call('PersonResultRow')] };
  const good = { nodes: [node('PersonRow', PR, 'PersonResultRow'), node('FollowButton', FB, 'PersonResultRow'), avatar, node('Avatar', '@exact/cards/common.contract#Avatar', 'Elsewhere')] };
  expect(bindingsCheck(good, [{ component: 'PersonResultRow', allowed }])).toEqual([{ binding: 'PersonResultRow', ok: true, said: 'binds to PersonRow and FollowButton only' }]);
  // A node of its own (a text beside the row), another package component in it, or no instance at all: each fails.
  const own = { nodes: [...good.nodes, { file: '/copy/explore.contract', line: 180, component: 'PersonResultRow', chain: call('Screen') }] };
  expect(bindingsCheck(own, [{ component: 'PersonResultRow', allowed }])[0]).toMatchObject({ ok: false, said: 'draws 1 node(s) of its own (explore.contract:180)' });
  const extra = { nodes: [...good.nodes, node('EmptyState', '@exact/cards/empty-state.contract#EmptyState', 'PersonResultRow')] };
  expect(bindingsCheck(extra, [{ component: 'PersonResultRow', allowed }])[0]).toMatchObject({ ok: false, said: 'instantiates @exact/cards/empty-state.contract#EmptyState besides PersonRow and FollowButton' });
  expect(bindingsCheck(good, [{ component: 'FindFollowRow', allowed }])[0]).toMatchObject({ ok: false, said: 'not instantiated' });
  // An app helper between the binding and a node of the app's own (Astra round 1): its text is drawn under the binding.
  const helper = { nodes: [...good.nodes, { file: '/copy/explore.contract', line: 300, component: 'OwnText', chain: [{ file: '/copy/explore.contract', line: 290, component: 'Helper' }, ...call('PersonResultRow')] }] };
  expect(bindingsCheck(helper, [{ component: 'PersonResultRow', allowed }])[0]).toMatchObject({ ok: false, said: 'draws through the app\'s own Helper (explore.contract:290)' });
  expect([identityOf('/a/packages/cards/person-row.contract', 'PersonRow'), identityOf('/c/node_modules/@exact/lists/menu.contract', 'MenuItem'), identityOf('/c/explore.contract', 'X')])
    .toEqual(['@exact/cards/person-row.contract#PersonRow', '@exact/lists/menu.contract#MenuItem', null]);
});

test("option B's body shape: one PersonRow, and in its slot nothing or the if around one FollowButton", () => {
  const head = 'use PersonRow as Row from "@exact/cards"\nuse FollowButton from "@exact/actions"\n\ncomponent B\n  props\n    p: P\n  view\n';
  const row = '    Row(\n      id="a",\n      name=p.name\n    )\n';
  const follow = '      if p.on\n        FollowButton(\n          id="f",\n          press=x\n        )\n';
  expect(bindingShape(head + row, 'B')).toEqual({ ok: true, said: 'one Row' });
  expect(bindingShape(head + row + follow + '// the next\ncomponent C\n  view\n    text "c"\n', 'B')).toEqual({ ok: true, said: 'one Row, and in its slot the if around one FollowButton' });
  for (const [bad, why] of [
    [head + row + row, 'under its Row'],
    [head + row + '      FollowButton(\n        id="f"\n      )\n', 'not the if around'],
    [head + row + follow + '      else\n        text "x"\n', 'more after the if'],
    [head + row + '      if p.on\n        text "x"\n', "its if's only child isn't"],
    [head + '    column\n' + row, "doesn't open with one Row( call"],
    [head.replace('use PersonRow as Row from "@exact/cards"\n', '') + row, 'imports no PersonRow'],
  ]) expect(bindingShape(bad, 'B')).toMatchObject({ ok: false, said: expect.stringContaining(why) });
});

test("s15b2 is batch 1's profile plus batch 2's five, its retired copies, bindings and screens", () => {
  const p = STAGES.s15b2;
  expect(p.tag).toBe('s15b2-baseline');
  expect(p.adopted.slice(0, STAGES.s15b.adopted.length)).toEqual(STAGES.s15b.adopted);
  expect(p.adopted.slice(STAGES.s15b.adopted.length).map(id => id.split('#')[1])).toEqual(['FollowButton', 'PersonRow', 'EmptyState', 'MenuItem', 'MenuGroup']);
  expect(p.before).toEqual(STAGES.s15b.adopted);
  expect(p.screens.map(([n]) => n).slice(0, STAGES.s15b.screens.length)).toEqual(STAGES.s15b.screens.map(([n]) => n));
  expect(new Set(p.screens.map(([n]) => n)).size).toBe(p.screens.length);
  // No step names a testId with a space in it (an op's words are split on spaces).
  for (const [, steps] of p.screens) for (const s of steps) if (s.startsWith('tap ')) expect(s.split(' ').length === 2 || / wheel | drag /.test(s)).toBe(true);
  // Each named change on one of its screens, with its expected change in words, never a page-wide container
  // (theme2 23:16 (a): the smallest container holding what changes).
  for (const n of p.named) {
    expect(p.screens.some(([name]) => name === n.screen)).toBe(true);
    expect(n.change.length > 20 && !['bluesky', 'panels'].includes(n.target) && !n.target.startsWith('panel-')).toBe(true);
  }
  useStage('s15b2');
  try {
    expect([SCREENS.length, RETIRED.length, BINDINGS.map(b => b.component)]).toEqual([p.screens.length, 7, ['PersonResultRow', 'FindFollowRow']]);
    expect(NAMED).toEqual(p.named);
  } finally { useStage('s12'); }
});
