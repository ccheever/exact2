import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

// Missing builds stop before browser launch but still report every selected app.
test('conformance option values are not selected as apps', () => {
  const root = resolve(import.meta.dir, '../..');
  const dir = mkdtempSync(resolve(root, 'target/conform-args-'));
  try {
    const out = resolve(dir, 'report');
    const result = spawnSync(process.execPath, [
      resolve(import.meta.dir, 'conform.mjs'), '--jobs', '1', 'caltrain',
      '--steps', '0', '--label', 'probe', '--wasm-root', resolve(dir, 'wasm'),
      '--out', out, '--strict',
    ], { cwd: root, encoding: 'utf8' });
    expect(result.status).toBe(1);
    const report = JSON.parse(readFileSync(resolve(out, 'report.json'), 'utf8'));
    expect(report.failures.map(f => f.target)).toEqual(['caltrain']);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 60_000);
