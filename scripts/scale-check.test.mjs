// scale-check.mjs's reading of a corner: a rounded box drawn here with
// supersampled coverage, then fitted back to its radius.
import { expect, test } from 'bun:test';
import { check, edges, fitRadius, reduced, rgb } from './scale-check.mjs';

/** A `width`×`height` RGBA image, white, with a box of `fill` and corner
 * radius `r` (CSS-reduced) at x, y, w, h, each pixel's coverage taken from 8×8 samples. */
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

test('a corner is fitted to its radius within a fifth of a pixel', () => {
  for (const r of [4, 8, 12, 16, 26]) {
    const box = { x: 10, y: 10, w: 120, h: 80 };
    const image = drawn(160, 110, box, r, rgb('#ff0000'));
    expect(Math.abs(fitRadius(edges(image, box, rgb('#ff0000'), rgb('#ffffff'))) - r)).toBeLessThan(0.2);
  }
});

test('a radius over half the box is a pill, and a padding is its first child\'s offset', () => {
  const layout = { viewport: { w: 100 }, nodes: [
    { id: 1, x: 0, y: 0, w: 100, h: 60, testId: 'padded' }, { id: 2, x: 16, y: 16, w: 20, h: 20 },
    { id: 3, x: 10, y: 20, w: 60, h: 24, testId: 'pill' },
  ] };
  // At 2x, as a phone's screenshot is.
  const image = drawn(200, 120, { x: 20, y: 40, w: 120, h: 48 }, 9999, rgb('#0000ff'));
  const rows = check('linux', layout, image, { boxes: [
    { testId: 'padded', padding: { linux: 16, default: 10 } },
    { testId: 'pill', radius: 9999, fill: '#0000ff' },
  ] });
  expect(rows.map(r => r.ok)).toEqual([true, true]);
  expect(rows[1].expected).toBe(12);
});
