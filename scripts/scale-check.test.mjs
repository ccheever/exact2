// scale-check.mjs's reading of a corner and a padding: a rounded box drawn
// here with supersampled coverage, then fitted back to its radius.
import { expect, test } from 'bun:test';
import { check, edges, fitRadius, parse, reduced, rgb } from './scale-check.mjs';

/** A `width`×`height` RGBA image, white, with a box of `fill` and corner
 * radius `r` (CSS-reduced) at x, y, w, h (device pixels, fractional), each
 * pixel's coverage taken from 8×8 samples. */
function drawn(width, height, { x, y, w, h }, r, fill) {
  const data = new Uint8Array(width * height * 4).fill(255);
  r = reduced(r, w, h);
  const inside = (px, py) => {
    if (px < x || py < y || px > x + w || py > y + h) return false;
    const cx = Math.min(Math.max(px, x + r), x + w - r), cy = Math.min(Math.max(py, y + r), y + h - r);
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r;
  };
  for (let py = 0; py < height; py++) for (let px = 0; px < width; px++) {
    let n = 0;
    for (let s = 0; s < 64; s++) n += inside(px + ((s % 8) + 0.5) / 8, py + (Math.floor(s / 8) + 0.5) / 8);
    const c = n / 64, i = (py * width + px) * 4;
    for (let k = 0; k < 3; k++) data[i + k] = Math.round(255 + (fill[k] - 255) * c);
  }
  return { width, height, data };
}

const red = rgb('#ff0000'), white = rgb('#ffffff');
const fit = (image, box) => fitRadius(edges(image, box, red, white), Math.max(box.w, box.h) / 2 + 1);

test('a corner is fitted to its radius within a third of a pixel, square or round, on whole or fractional edges', () => {
  for (const origin of [10, 10.25, 10.5, 10.75]) {
    for (const r of [0, 4, 8, 12, 16, 26]) {
      const box = { x: origin, y: origin + 0.3, w: 120, h: 80 };
      expect(Math.abs(fit(drawn(160, 110, box, r, red), box) - r)).toBeLessThan(0.3);
    }
  }
});

test('a pill on a large box at 3x is fitted past any fixed ceiling', () => {
  const box = { x: 3, y: 3, w: 900, h: 900 };
  expect(Math.abs(fit(drawn(906, 906, box, 9999, red), box) - 450)).toBeLessThan(1);
});

const layout = { viewport: { w: 100 }, nodes: [
  { id: 1, x: 0, y: 0, w: 100, h: 60, testId: 'padded' }, { id: 2, x: 16, y: 16, w: 20, h: 20, testId: 'inner' },
  { id: 3, x: 10, y: 20, w: 60, h: 24, testId: 'pill' }, { id: 4, x: 0, y: 70, w: 30, h: 0, testId: 'empty' },
  { id: 5, x: 40, y: 70, w: 10, h: 10, testId: 'offside' }, { id: 6, x: 56, y: 72, w: 4, h: 4, testId: 'tall' },
] };
// At 2x, as a phone's screenshot is.
const image = drawn(200, 200, { x: 20, y: 40, w: 120, h: 48 }, 9999, rgb('#0000ff'));

test('a radius over half the box is a pill, and a padding is its named child\'s offset on both axes', () => {
  const rows = check('linux', layout, image, { boxes: [
    { testId: 'padded', child: 'inner', padding: { linux: 16, default: 10 } },
    { testId: 'pill', radius: 9999, fill: '#0000ff' },
  ] });
  expect(rows.map(r => r.ok)).toEqual([true, true]);
  expect(rows[1].expected).toBe(12);
});

test('what cannot be read or is not expected fails, never passes', () => {
  const rows = check('linux', layout, image, { boxes: [
    // Wrong on one axis.
    { testId: 'offside', child: 'tall', padding: 16 },
    // No child named.
    { testId: 'padded', padding: 16 },
    // Expected for another host only.
    { testId: 'padded', child: 'inner', padding: { ios: 16 } },
    // A zero-sized box: no corner to read.
    { testId: 'empty', radius: 9999, fill: '#0000ff' },
    // No fill there.
    { testId: 'offside', radius: 4, fill: '#0000ff' },
    // The wrong radius.
    { testId: 'pill', radius: 4, fill: '#0000ff' },
  ] });
  expect(rows.map(r => r.ok)).toEqual([false, false, false, false, false, false]);
  expect(rows[2].why).toContain('no padding expected for linux');
  expect(rows[3].why).toContain('could not be read');
});

test('the command line is refused when malformed', () => {
  expect(parse(['a.contract', 'e.json', '--host'])).toContain('needs a value');
  expect(parse(['a.contract', 'e.json', '--hots', 'ios'])).toContain('unknown flag');
  expect(parse(['a.contract'])).toContain('a plan and an expectation file');
  expect(parse(['a.contract', 'e.json', '--out', '/tmp/a b'])).toContain('no spaces');
  expect(parse(['a.contract', 'e.json', '--host', 'web', '--host', 'macos']).hosts).toEqual(['web', 'macos']);
});

test('a corner with another colour in it (a shadow, a border) is not read', () => {
  const box = { x: 10, y: 10, w: 120, h: 80 };
  const image = drawn(160, 110, box, 12, red);
  // A dark pixel where the corner's cut-out is.
  const i = (12 * 160 + 11) * 4; image.data[i] = image.data[i + 1] = image.data[i + 2] = 40;
  expect(edges(image, box, red, white)).toBeNull();
  // A fill too close to the background to read.
  expect(edges(image, box, rgb('#fefefe'), white)).toBeNull();
});

test('a small radius on a fractional edge, a covered left edge, an unnamed child and a value that is no number', () => {
  // 1.2 px at 1x, its top 0.01 px under a pixel's: no row read shows it, so
  // it reads as square, and square is within it.
  const small = { x: 10, y: 10.01, w: 60, h: 40 };
  const rows = check('linux', { viewport: { w: 100 }, nodes: [{ id: 1, ...small, testId: 'small' }, { id: 2, x: 26, y: 26.01, w: 4, h: 4 }] },
    drawn(100, 60, small, 1.2, red), { boxes: [
      { testId: 'small', radius: 1.2, fill: '#ff0000' },
      { testId: 'small', padding: 16 },
      { testId: 'small', radius: '12px', fill: '#ff0000' },
      { testId: 'small', child: '', padding: Number.NaN },
    ] });
  expect(rows.map(r => r.ok)).toEqual([true, false, false, false]);
  // The middle row's first pixels covered by the background: not read.
  const box = { x: 10, y: 10, w: 120, h: 80 };
  const covered = drawn(160, 110, box, 12, red);
  for (let x = 10; x < 22; x++) { const i = (50 * 160 + x) * 4; covered.data[i] = covered.data[i + 1] = covered.data[i + 2] = 255; }
  expect(edges(covered, box, red, white)).toBeNull();
});
