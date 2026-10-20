// scripts/stage12-gate.mjs: the command line, a screen's ops, the pixel
// diff and its allowlist, and that every line says WAITS-ON before its parts.
import { expect, test } from 'bun:test';
import { encodePng } from './png.mjs';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { allowOf, barTop, beforeDrift, BASELINE_TAG, comparePixels, expected, lines, releaseIdentity, parse, pixelDiff, RESTYLE, SCREENS, screenOps, WEB_NOISE } from './stage12-gate.mjs';

test('the command line', () => {
  expect(parse(['baseline', '--copy', '/c'])).toBe('baseline needs --copy and --out');
  expect(parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'tvos'])).toMatch(/^--host tvos/);
  expect(parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'terminal'])).toMatch(/^--host terminal/);
  expect(parse(['--host', 'terminal']).hosts).toEqual(['terminal']);
  expect(parse(['--only', 'nope'])).toMatch(/^--only nope/);
  expect(parse(['--bogus', 'x'])).toBe('unknown argument --bogus');
  const b = parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'web']);
  expect([b.mode, b.copy, b.out, b.hosts]).toEqual(['baseline', '/c', '/o', ['web']]);
});

test('a screen is reached by its taps, each settled, the scheme first', () => {
  for (const [name, taps] of SCREENS) {
    const ops = screenOps(name, 'dark', '/s.png');
    expect(ops.slice(0, 3)).toEqual(['clock +1000 real', 'clock settle', 'prefer prefers-color-scheme dark']);
    expect(ops.slice(ops.indexOf('screenshot /s.png'))).toEqual(['screenshot /s.png', 'logs', ...(name === 'composer' ? ['layout compose-text'] : [])]);
    for (const t of taps) expect(ops[ops.indexOf(t) + 1]).toBe('clock settle');
  }
});

test('the pixel diff counts what differs outside the allowlist', () => {
  const img = (w, h, f) => { const data = new Uint8Array(w * h * 4); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) data.set(f(x, y), (y * w + x) * 4); return encodePng({ width: w, height: h, data }); };
  const a = img(4, 4, () => [0, 0, 0, 255]);
  const b = img(4, 4, (x, y) => (x === 1 && y === 1) || (x === 3 && y === 0) ? [255, 0, 0, 255] : [0, 0, 0, 255]);
  expect(pixelDiff(a, a)).toEqual({ differ: 0, total: 16 });
  expect(pixelDiff(a, b)).toEqual({ differ: 2, total: 16 });
  expect(pixelDiff(a, b, [[1, 1, 1, 1]])).toEqual({ differ: 1, total: 16 });
  expect(pixelDiff(a, img(4, 5, () => [0, 0, 0, 255])).differ).toBe(16);
  // The web's noise: up to its floor on an anti-aliased edge pixel, never in
  // a flat area, where one level fails.
  const flat = img(4, 4, (x, y) => x === 1 && y === 1 ? [1, 0, 0, 255] : [0, 0, 0, 255]);
  expect([pixelDiff(a, flat).differ, pixelDiff(a, flat, [], WEB_NOISE).differ]).toEqual([1, 1]);
  const edgy = img(4, 4, (x) => x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  const edgy2 = img(4, 4, (x, y) => x === 1 && y === 1 ? [2, 2, 0, 255] : x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  const edgy3 = img(4, 4, (x, y) => x === 1 && y === 1 ? [3, 0, 0, 255] : x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  expect([pixelDiff(edgy, edgy2).differ, pixelDiff(edgy, edgy2, [], WEB_NOISE).differ, pixelDiff(edgy, edgy3, [], WEB_NOISE).differ]).toEqual([1, 0, 1]);
});

test('a line whose parts are missing waits on them', async () => {
  const waiting = ['consumer-pixels', 'slot-check-time'];
  for (const [id, , check] of lines({ baseline: null, hosts: [] }).filter(([id]) => waiting.includes(id))) expect([id, (await check()).status]).toEqual([id, expect.stringMatching(/^WAITS-ON /)]);
});

test('the composer\'s caret strip comes from its field\'s layout', () => {
  const out = ['screenshot x', JSON.stringify({ node: { testId: 'compose-text', space: { viewport: { x: 70, y: 75, w: 334, h: 128 }, capture: { scale: 1 } } } })].join('\n');
  expect(allowOf('composer', out)).toEqual([{ rect: [70, 75, 4, 40], reason: expect.stringContaining('caret') }]);
  expect(allowOf('composer', 'no layout')).toBe(null);
  // With the system keyboard up, its rectangle too, from the layout's inset.
  const kb = JSON.stringify({ viewport: { w: 402, h: 546 }, env: { 'keyboard-inset-height': 328 }, node: { testId: 'compose-text', space: { viewport: { x: 70, y: 124, w: 320, h: 120 } } } });
  expect(allowOf('composer', kb).map(r => r.rect)).toEqual([[70, 124, 4, 40], [0, 546, 402, 328]]);
  expect(allowOf('discover', '')).toEqual([]);
  expect(screenOps('composer', 'light', '/s.png').at(-1)).toBe('layout compose-text');
});

test('before and after compare shot for shot, strict, with both allowlists', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-'));
  const img = (name, f) => { const data = new Uint8Array(4 * 4 * 4); for (let i = 0; i < 16; i++) data.set(f(i % 4, Math.floor(i / 4)), i * 4); const p = join(dir, name); writeFileSync(p, encodePng({ width: 4, height: 4, data })); return p; };
  const black = () => [0, 0, 0, 255];
  const before = { shots: [{ host: 'web', scheme: 'light', screen: 'discover', ok: true, png: img('b1.png', black), allow: [] },
    { host: 'web', scheme: 'light', screen: 'composer', ok: true, png: img('b2.png', black), allow: [{ rect: [0, 0, 1, 4], reason: 'caret' }] }] };
  const after = { shots: [{ host: 'web', scheme: 'light', screen: 'discover', ok: true, png: img('a1.png', (x, y) => x === 2 && y === 2 ? [0, 0, 1, 255] : [0, 0, 0, 255]), allow: [] },
    { host: 'web', scheme: 'light', screen: 'composer', ok: true, png: img('a2.png', (x) => x === 0 ? [255, 0, 0, 255] : [0, 0, 0, 255]), allow: [] }] };
  const r = comparePixels(before, after);
  expect(r.map(x => [x.shot, x.ok, x.differ])).toEqual([['web light discover', false, 1], ['web light composer', true, 0]]);
  expect(comparePixels(before, { shots: [after.shots[1]] })[0]).toMatchObject({ ok: false, said: 'no after shot' });
});

test('the before differs from the baseline\'s tag only in its lock', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-git-'));
  const git = (...a) => spawnSync('git', a, { cwd: dir, encoding: 'utf8' });
  git('init', '-q'); git('config', 'user.email', 't@t'); git('config', 'user.name', 't');
  writeFileSync(join(dir, 'Cargo.lock'), 'a'); writeFileSync(join(dir, 'app.contract'), 'a');
  git('add', '-A'); git('commit', '-qm', 'base'); git('tag', BASELINE_TAG);
  writeFileSync(join(dir, 'Cargo.lock'), 'b'); git('commit', '-qam', 'lock');
  expect(beforeDrift(dir)).toEqual([]);
  writeFileSync(join(dir, 'app.contract'), 'b'); git('commit', '-qam', 'more');
  expect(beforeDrift(dir)).toEqual(['app.contract']);
});

test('a shot with bars above its viewport: the offset that lines up every marker', () => {
  // A 10x14 shot of a 10x10 viewport, 3 rows of bars on top: two markers
  // a row tall whose pixels differ between the shots, 3 rows below their
  // layout rects (only that offset lines up both).
  const img = (on) => { const data = new Uint8Array(10 * 14 * 4).fill(255); for (const [x, y] of on) data.set([0, 0, 0, 255], (y * 10 + x) * 4); return { width: 10, height: 14, data }; };
  const A = img([]), B = img([[1, 4], [6, 9]]);
  const layout = { viewport: { w: 10, h: 10 }, nodes: [{ testId: 'marker-a', x: 0, y: 1, w: 2, h: 1 }, { testId: 'marker-b', x: 5, y: 6, w: 2, h: 1 }] };
  expect(barTop(A, B, layout, ['a', 'b'])).toBe(3);
  expect(barTop(A, B, { ...layout, viewport: { w: 10, h: 14 } }, ['a', 'b'])).toBe(0);
});

test('the restyle line expects the value, or its change from the base drive, in whole cells on the terminal', () => {
  const is = { is: true, axis: 'w' }, twice = { twice: true, axis: 'w' }, once = { once: true, axis: 'h' };
  expect(expected(is, 24, 12, 12, null)).toBe(24);
  expect(expected(twice, 24, 12, 80, null)).toBe(104);
  expect(expected(once, 24, 12, 50, null)).toBe(62);
  // A cell 8 wide and 16 tall: 24 is 3 columns, 12 one (a half rounds
  // down), and 20 and 12 are both one row.
  expect(expected(is, 24, 12, 16, { w: 8, h: 16 })).toBe(24);
  expect(expected(twice, 24, 12, 80, { w: 8, h: 16 })).toBe(112);
  expect(expected(once, 20, 12, 32, { w: 8, h: 16 })).toBe(32);
  // Each case names what it sets and how it is removed.
  for (const c of RESTYLE) {
    expect(c.set).toMatch(/^\d+px$/);
    expect(['remove-token', 'unset-param']).toContain(c.unset.op);
    expect(c.reads.length).toBeGreaterThan(0);
  }
  expect(new Set(RESTYLE.flatMap(c => c.reads.map(r => r.who)))).toEqual(new Set(['PostCard', 'ActionRow', 'AtCard', 'PostButton']));
});

test('release identity: every plan identical but library-gate\'s own', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-rel-'));
  const row = (source, status) => ({ run: 'a..b', commits: ['Stage 12 E: x'], source, status });
  const file = rows => { const f = join(dir, `${Math.random()}.json`); writeFileSync(f, JSON.stringify(rows)); return f; };
  expect(releaseIdentity(null).status).toBe('WAITS');
  expect(releaseIdentity(file([row('apps/a/app.contract', 'identical'), row('apps/library-gate/app.contract', 'build failed'), row('apps/x/app.contract', 'both failed')])).status).toBe('PASS');
  const moved = releaseIdentity(file([row('apps/a/app.contract', 'DIFFERS')]));
  expect(moved.status).toBe('FAIL');
  expect(moved.evidence[1]).toContain('MOVED apps/a/app.contract');
  expect(releaseIdentity(file([row('apps/a/app.contract', 'build failed')])).status).toBe('FAIL');
});
