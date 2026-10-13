// LLP 1116.008 D8: a window capture of the app with the software keyboard put in from the simulator's display.
// A fake `xcrun` stands for `simctl io <udid> screenshot`: it copies a prepared display picture to the path it is given.
import { test, expect } from 'bun:test';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { decodePng, encodePng } from '../../scripts/png.mjs';
import { withKeyboard } from './devices.mjs';

const solid = (width, height, rgba) => encodePng({ width, height, data: Uint8Array.from({ length: width * height * 4 }, (_, i) => rgba[i % 4]) });

function withFakeSimctl(display, body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'devices-test-'));
  const [path, env] = [resolve(dir, 'display.png'), { PATH: process.env.PATH, DEVELOPER_DIR: process.env.DEVELOPER_DIR }];
  try {
    writeFileSync(path, display);
    writeFileSync(resolve(dir, 'xcrun'), `#!/bin/sh\nfor last; do :; done\ncp '${path}' "$last"\n`);
    chmodSync(resolve(dir, 'xcrun'), 0o755);
    process.env.PATH = `${dir}:${env.PATH}`;
    process.env.DEVELOPER_DIR ??= '/Applications/Xcode.app/Contents/Developer';
    return body(dir);
  } finally {
    process.env.PATH = env.PATH;
    if (env.DEVELOPER_DIR === undefined) delete process.env.DEVELOPER_DIR;
    rmSync(dir, { recursive: true, force: true });
  }
}

test('the rows from the keyboard\'s top down are the display\'s, the app above them its own', () => {
  withFakeSimctl(solid(4, 8, [0, 0, 255, 255]), (dir) => {
    const shot = resolve(dir, 'app.png');
    writeFileSync(shot, solid(4, 8, [255, 0, 0, 255]));
    const r = withKeyboard({ scale: 2, keyboard: { visible: true, top: 2, windowTop: 2 }, note: 'old' }, shot, 'UDID');
    const png = decodePng(readFileSync(shot)), at = (y) => [...png.data.subarray(y * 16, y * 16 + 4)];
    expect([at(0), at(3)]).toEqual([[255, 0, 0, 255], [255, 0, 0, 255]]);
    expect([at(4), at(7)]).toEqual([[0, 0, 255, 255], [0, 0, 255, 255]]);
    expect(r.keyboard.captured).toBe(true);
    expect(r.note).toContain('in the capture');
  });
});

test('a display of another size leaves the picture as it was, and says why', () => {
  withFakeSimctl(solid(6, 8, [0, 0, 255, 255]), (dir) => {
    const shot = resolve(dir, 'app.png'), app = solid(4, 8, [255, 0, 0, 255]);
    writeFileSync(shot, app);
    const r = withKeyboard({ scale: 2, keyboard: { visible: true, top: 2, windowTop: 2 } }, shot, 'UDID');
    expect(readFileSync(shot).equals(Buffer.from(app))).toBe(true);
    expect(r.keyboard.captured).toBeUndefined();
    expect(r.note).toContain('the display is 6x8, the capture 4x8');
  });
});

test('a capture that says no keyboard top is left alone', () => {
  const r = withKeyboard({ scale: 2, keyboard: { visible: true, top: 2 } }, '/nonexistent.png', 'UDID');
  expect(r.note).toContain('no keyboard top');
});
