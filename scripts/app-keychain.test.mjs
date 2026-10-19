// LLP 1126.001 WP16: the native Keychain migration with isolated Security calls.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

test('the first provisioned app launch migrates login secrets, retaining originals on failure; extensions never migrate', async () => {
  if (process.platform !== 'darwin') return;
  const { plistFile } = await import('../host/apple/assets.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-keychain-migration-'));
  const run = (command, args) => { const r = spawnSync(command, args, { encoding: 'utf8' }); assert.equal(r.status, 0, r.stderr); };
  try {
    const binary = resolve(dir, 'Fixture');
    run('xcrun', ['clang++', '-std=c++17', '-fobjc-arc', '-framework', 'Foundation', '-framework', 'Security', resolve(import.meta.dir, '../host/apple/tests/fixtures/keychain-migration.mm'), '-o', binary]);
    run(binary, ['ordinary']);
    for (const [suffix, mode] of [['app', 'app'], ['appex', 'extension']]) {
      const bundle = resolve(dir, 'Fixture.' + suffix);
      mkdirSync(resolve(bundle, 'Contents/MacOS'), { recursive: true });
      cpSync(binary, resolve(bundle, 'Contents/MacOS/Fixture'));
      writeFileSync(resolve(bundle, 'Contents/Info.plist'), plistFile({ CFBundleIdentifier: 'dev.exact.migration', CFBundleExecutable: 'Fixture',
        CFBundlePackageType: suffix === 'app' ? 'APPL' : 'XPC!', ExactAppIdentifier: 'dev.exact.migration', ExactKeychainAccessGroup: 'TEAM.dev.exact.migration' }));
      for (const scenario of mode === 'app' ? ['app', 'denied', 'canceled', 'locked', 'decode', 'missing-account', 'missing-data', 'locked-list'] : [mode]) {
        run(resolve(bundle, 'Contents/MacOS/Fixture'), [scenario]);
      }
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
