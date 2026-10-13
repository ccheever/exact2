import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { evictArms } from './arms.mjs';

test('an arm keeps its three last used copies, another arm whose name ends in it keeps its own, and a day-old half-written copy goes', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-arms-'));
  try {
    const name = 'libexact_modules.dylib';
    const at = (file, secondsAgo) => {
      writeFileSync(join(dir, file), '');
      const t = new Date(Date.now() - secondsAgo * 1000);
      utimesSync(join(dir, file), t, t);
    };
    const keys = ['0000000000000001', '0000000000000002', '0000000000000003', '0000000000000004'];
    keys.forEach((key, i) => at(`${key}-${name}`, i * 10));
    at(`0000000000000009-probe-${name}`, 1000);
    at(`0000000000000001-${name}.42.tmp`, 2 * 86_400);
    at(`0000000000000002-${name}.43.tmp`, 60);
    evictArms(dir, name);
    assert.deepEqual(readdirSync(dir).sort(), [
      `0000000000000001-${name}`, `0000000000000002-${name}`, `0000000000000002-${name}.43.tmp`,
      `0000000000000003-${name}`, `0000000000000009-probe-${name}`,
    ]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
