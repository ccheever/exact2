import { test } from 'bun:test';
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import { appleComposition, assertCapabilityFrameworks, loadedDylibs } from './link.mjs';

test('production links what the plan uses, the manifest names and the grants open; development all but the photo picker and sign-in sheet, which it links as production would (LLP 1047.001.000 E, F)', () => {
  const graph = { uses: ['markdown', 'picker'] };
  assert.deepEqual(appleComposition(graph, true), { link: 'markdown,picker', products: ['ExactMarkdown'] });
  assert.deepEqual(appleComposition({ uses: [] }, true), { link: 'none', products: [] });
  assert.deepEqual(appleComposition(graph, true, [], true).products, ['ExactMarkdown', 'ExactAuthSession']);
  assert.deepEqual(appleComposition(graph, false).products, ['ExactGroupedLists', 'ExactMarkdown', 'ExactSurfaces', 'ExactDrag']);
  assert.deepEqual(appleComposition({ uses: ['picker', 'media_picker'] }, false, [], true).products,
    ['ExactGroupedLists', 'ExactMarkdown', 'ExactSurfaces', 'ExactDrag', 'ExactMediaPicker', 'ExactAuthSession']);
  assert.deepEqual(appleComposition(graph, false, ['auth_session']).products.at(-1), 'ExactAuthSession');
  assert.deepEqual(appleComposition({ uses: ['numerals'] }, false).products.at(-1), 'ExactNumerals');
  assert.ok(!appleComposition(graph, false).products.includes('ExactNumerals'));
  assert.equal(appleComposition(undefined, false).link, 'all');
});

test('the load commands are read from the file, each slice of a universal one; a binary with no capability framework passes the check', () => {
  assert.ok(loadedDylibs('/bin/ls').includes('/usr/lib/libSystem.B.dylib'));
  assert.doesNotThrow(() => assertCapabilityFrameworks('/bin/ls', []));
});


test('ordinary app executables refuse WidgetKit and ActivityKit load commands', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-activity-links-'));
  try {
    for (const framework of ['WidgetKit', 'ActivityKit']) {
      const name = Buffer.from(`/System/Library/Frameworks/${framework}.framework/${framework}\0`);
      const size = (24 + name.length + 7) & ~7, bytes = Buffer.alloc(32 + size);
      bytes.writeUInt32LE(0xfeedfacf, 0); bytes.writeUInt32LE(1, 16); bytes.writeUInt32LE(size, 20);
      bytes.writeUInt32LE(0xc, 32); bytes.writeUInt32LE(size, 36); bytes.writeUInt32LE(24, 40); name.copy(bytes, 56);
      const executable = join(dir, 'App'); writeFileSync(executable, bytes);
      assert.throws(() => assertCapabilityFrameworks(executable, []), new RegExp(framework));
      assert.doesNotThrow(() => assertCapabilityFrameworks(executable, ['ExactSnapshot']));
    }
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('the assembled app and extension refuse push-to-start symbols, remote registration and its background mode (LLP 1126.001 WP9)', async () => {
  const { assertActivityRefusals } = await import('./activities.mjs');
  const { plistFile } = await import('./assets.mjs');
  const { copyFileSync, mkdirSync, readFileSync } = await import('node:fs');
  const { spawnSync } = await import('node:child_process');
  const { resolve } = await import('node:path');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-activity-refusals-'));
  const runApple = (cmd, args) => {
    const result = spawnSync(cmd, args, { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result;
  };
  try {
    const extension = resolve(dir, 'PlugIns/Widgets.appex'); mkdirSync(extension, { recursive: true });
    for (const directory of [dir, extension]) {
      copyFileSync('/bin/ls', resolve(directory, 'Executable'));
      writeFileSync(resolve(directory, 'Info.plist'), plistFile({ CFBundleExecutable: 'Executable' }));
    }
    assert.equal(assertActivityRefusals(dir, runApple), 2);
    for (const directory of [dir, extension]) {
      writeFileSync(resolve(directory, 'Info.plist'), plistFile({ CFBundleExecutable: 'Executable', UIBackgroundModes: ['remote-notification'] }));
      assert.throws(() => assertActivityRefusals(dir, runApple), /remote-notification/);
      writeFileSync(resolve(directory, 'Info.plist'), plistFile({ CFBundleExecutable: 'Executable' }));
      for (const symbol of ['pushToStartTokenUpdates', 'registerForRemoteNotifications']) {
        // The scan covers selectors and Swift mangled references, not only undefined C symbols.
        writeFileSync(resolve(directory, 'Executable'), Buffer.concat([readFileSync('/bin/ls'), Buffer.from(`\0${symbol}\0`)]));
        assert.throws(() => assertActivityRefusals(dir, runApple), new RegExp(symbol));
      }
      copyFileSync('/bin/ls', resolve(directory, 'Executable'));
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
