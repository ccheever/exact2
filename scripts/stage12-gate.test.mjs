// scripts/stage12-gate.mjs: the command line, a screen's ops, the pixel
// diff and its allowlist, and that every line says WAITS-ON before its parts.
import { expect, test } from 'bun:test';
import { encodePng } from './png.mjs';
import { allowOf, lines, parse, pixelDiff, SCREENS, screenOps } from './stage12-gate.mjs';

test('the command line', () => {
  expect(parse(['baseline', '--copy', '/c'])).toBe('baseline needs --copy and --out');
  expect(parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'tvos'])).toMatch(/^--host tvos/);
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
  // A floor: a channel off by up to it is the same pixel.
  const near = img(4, 4, (x, y) => x === 0 && y === 0 ? [2, 1, 0, 255] : [0, 0, 0, 255]);
  expect([pixelDiff(a, near).differ, pixelDiff(a, near, [], 2).differ, pixelDiff(a, near, [], 1).differ]).toEqual([1, 0, 1]);
});

test('every line waits on its parts until they exist', async () => {
  for (const [, , check] of lines({ baseline: null })) expect((await check()).status).toMatch(/^WAITS-ON /);
});

test('the composer\'s caret strip comes from its field\'s layout', () => {
  const out = ['screenshot x', JSON.stringify({ node: { testId: 'compose-text', space: { viewport: { x: 70, y: 75, w: 334, h: 128 }, capture: { scale: 1 } } } })].join('\n');
  expect(allowOf('composer', out)).toEqual([{ rect: [70, 75, 4, 40], reason: expect.stringContaining('caret') }]);
  expect(allowOf('composer', 'no layout')).toBe(null);
  expect(allowOf('discover', '')).toEqual([]);
  expect(screenOps('composer', 'light', '/s.png').at(-1)).toBe('layout compose-text');
});
