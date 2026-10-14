// AppKit integration: compile the actual standalone adapter against already-built
// Swift products, replacing only its final app.run() with the focus assertions.
// No facsimile window delegate and no per-window test menu installation.
// Build ExactMac and --test first; EXACT_MAC_TEST_PRODUCTS names the Debug products.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const products = process.env.EXACT_MAC_TEST_PRODUCTS;
const check = process.platform === 'darwin' && products ? test : test.skip;
check(process.env.EXACT_MENU_HELP_ONLY === '1' ? 'AppKit Help search stays first after menu syncs'
  : 'DocumentWindows switch menu actions, validation and removal policy with key focus', () => {
  const dir = mkdtempSync(join(root, 'target/menu-focus-'));
  try {
    for (const [name, removal] of [['removed', ' -exact-menu-remove="enter-full-screen"'], ['plain', '']]) {
      const source = `component App\n  state count = 0\n  action check\n    count = count + 1\n  view\n    column${removal}\n      button "Check for Updates…" -exact-menu="app" press=check testId="updates"\n      button "Documentation" -exact-menu="help" press=check\n      text "Ready"\n`;
      writeFileSync(join(dir, name + '.contract'), source);
      const result = spawnSync(process.env.EXACT_CONTRACT ?? join(root, 'target/debug/contract'), ['build', join(dir, name + '.contract'), '-o', join(dir, name + '.plan')], { encoding: 'utf8' });
      expect(result.status, result.stderr).toBe(0);
    }
    const source = readFileSync(process.env.EXACT_MAC_TEST_SOURCE ?? join(root, 'host/apple/Sources/ExactMac/main.swift'), 'utf8');
    expect(source.endsWith('app.run()\n')).toBe(true);
    const assertions = readFileSync(join(import.meta.dir, 'DocumentWindowMenuChecks.swift'), 'utf8');
    writeFileSync(join(dir, 'main.swift'), source.replace('import ExactKit', '@testable import ExactKit').replace(/app.run\(\)\n$/, assertions));
    const bundle = join(dir, 'MenuFocus.app/Contents');
    mkdirSync(join(bundle, 'MacOS'), { recursive: true });
    const binary = join(bundle, 'MacOS/ExactMac');
    writeFileSync(join(bundle, 'Info.plist'), `<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleExecutable</key><string>ExactMac</string><key>CFBundleIdentifier</key><string>com.exact.menu-focus-test</string><key>CFBundleName</key><string>Menu Focus</string><key>CFBundlePackageType</key><string>APPL</string><key>ExactLaunchMode</key><string>navigate-new</string></dict></plist>`);
    const objects = ['ExactKit', 'ExactComposition', 'ExactGroupedLists', 'ExactMarkdown', 'ExactSurfaces', 'ExactDrag']
      .map(name => join(products, name + '.o')).filter(existsSync);
    const build = spawnSync('xcrun', ['swiftc', '-I', products, '-I', join(root, 'host/apple/Sources/CExact'),
      '-L', process.env.EXACT_LIB_DIR ?? join(root, 'target/host-dev'), '-l' + (process.env.EXACT_LIB ?? 'caltrain_apple'), '-lc++',
      join(dir, 'main.swift'), ...objects, '-o', binary], { encoding: 'utf8' });
    expect(build.status, build.stderr).toBe(0);
    const output = join(dir, 'result.log'), errors = join(dir, 'errors.log');
    const run = spawnSync('/usr/bin/open', ['-W', '-n', '-a', resolve(bundle, '..'), '--stdout', output, '--stderr', errors,
      '--env', '__CFBundleIdentifier=com.exact.menu-focus-test', '--env', `EXACT_MENU_HELP_ONLY=${process.env.EXACT_MENU_HELP_ONLY ?? ''}`, '--env', `EXACT_PLAN=${join(dir, 'removed.plan')}`, '--env', `EXACT_MENU_OTHER_PLAN=${join(dir, 'plain.plan')}`],
      { encoding: 'utf8', timeout: 90000 });
    expect(run.status, run.stderr).toBe(0);
    const result = readFileSync(output, 'utf8') + readFileSync(errors, 'utf8');
    expect(result, result).toContain('DocumentWindow menu focus: 0 failures');
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 180000);
