// The TypeScript recorder against the shared cases (LLP 1056 §4): the same
// getters and throws Chrome gave, as the Rust recorder is held to them.
import { test, expect } from 'bun:test';
import { runCases } from './cases.mjs';
import { Recorder, parseColor, serializeColor } from '../recorder.js';

test('the shared cases hold for the TypeScript recorder', () => {
  const failed = runCases(() => new Recorder()).filter((r) => r.fail.length);
  expect(failed).toEqual([]);
});

test('colours serialise as Chrome serialises them', () => {
  const c = (s) => { const v = parseColor(s); return v === 'current' ? 'current' : v ? serializeColor(v) : 'none'; };
  expect(c('rgba(0,0,0,0.123456)')).toBe('rgba(0, 0, 0, 0.12)');
  expect(c('rgb(1 2 3 / 0.999)')).toBe('#010203');
  expect(c('hsl(120 50 50)')).toBe('#40bf40');
  expect(c('hsl(120, 50, 50)')).toBe('none');
  expect(c('#abcd')).toBe('rgba(170, 187, 204, 0.867)');
});

// A list's records as [code, operands] (canvas/src/list.rs).
function records(bytes) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), out = [];
  for (let at = 8; at + 8 <= v.byteLength;) {
    const code = v.getUint32(at, true), count = v.getUint32(at + 4, true), k = [];
    at += 8;
    for (let i = 0; i < count; i++, at += 8) k.push(v.getFloat64(at, true));
    out.push([code, k]);
  }
  return out;
}

test('a display-p3 canvas records Display P3 bytes, as the Rust recorder does', () => {
  // canvas/tests/cases.rs `a_display_p3_canvas_records_display_p3_bytes`.
  const ctx = new Recorder();
  ctx._.env.p3 = true;
  ctx.fillStyle = 'red'; ctx.fillRect(0, 0, 1, 1);
  ctx.fillStyle = 'color(display-p3 1 0 0)'; ctx.fillRect(0, 0, 1, 1);
  ctx.putImageData(new ImageData(new Uint8ClampedArray([255, 0, 0, 255]), 1), 0, 0);
  expect(ctx.createImageData(1, 1).colorSpace).toBe('display-p3');
  const r = records(ctx._.take()[0]);
  expect(r.filter(([c]) => c === 10).map(([, k]) => k)).toEqual([[234, 51, 35, 1], [255, 0, 0, 1]]);
  const pixel = r.find(([c]) => c === 72)[1][4];
  expect([pixel >>> 24, (pixel >>> 16) & 255, (pixel >>> 8) & 255, pixel & 255]).toEqual([234, 51, 35, 255]);
});
