// `exact release` signs every nested Mach-O file and bundle, innermost first (#119).
// `bun test ./scripts/exact.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, relative, resolve } from 'node:path';
import { signingOrder } from './exact.mjs';

const plist = (executable) => `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>${executable}</string></dict></plist>\n`;

function inDir(body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-release-'));
  try { return body(dir); } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('the signing order holds every Mach-O by its magic and each nested bundle after its contents', () => inDir((dir) => {
  const app = resolve(dir, 'Fixture.app');
  const put = (path, bytes) => { mkdirSync(dirname(resolve(app, path)), { recursive: true }); writeFileSync(resolve(app, path), bytes); };
  const word = (...words) => Buffer.concat(words.map((w) => { const b = Buffer.alloc(4); b.writeUInt32BE(w >>> 0); return b; }));
  put('Contents/Info.plist', plist('ExactMac'));
  put('Contents/MacOS/ExactMac', word(0xcffaedfe, 0x0c000001)); // the main executable: the bundle's own signature
  put('Contents/MacOS/libexact_web.dylib', word(0xcffaedfe, 0x0c000001));
  put('Contents/MacOS/spawn-helper', word(0xcafebabe, 2)); // universal, two architectures
  put('Contents/Resources/assets/helper', word(0xcffaedfe, 0x0c000001)); // no extension to go by
  put('Contents/Resources/assets/addon.node', word(0xcefaedfe, 7)); // 32-bit
  put('Contents/Resources/assets/Big.class', word(0xcafebabe, 0x00000034)); // Java: the fat magic, a class version
  put('Contents/Resources/assets/notes.dylib', 'not code\n'); // a name is not a format
  put('Contents/Resources/assets/short', Buffer.from([0xcf, 0xfa]));
  put('Contents/Helpers/Inner.app/Contents/Info.plist', plist('Inner'));
  put('Contents/Helpers/Inner.app/Contents/MacOS/Inner', word(0xfeedfacf, 0x0100000c));
  put('Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/Kit', word(0xcffaedfe, 1));
  symlinkSync('A', resolve(app, 'Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/Current'));
  symlinkSync('Versions/Current/Kit', resolve(app, 'Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Kit'));

  const order = signingOrder(app).map((path) => relative(dir, path));
  assert.deepEqual([...order].sort(), [
    'Fixture.app',
    'Fixture.app/Contents/Helpers/Inner.app',
    'Fixture.app/Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework',
    'Fixture.app/Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/Kit',
    'Fixture.app/Contents/MacOS/libexact_web.dylib',
    'Fixture.app/Contents/MacOS/spawn-helper',
    'Fixture.app/Contents/Resources/assets/addon.node',
    'Fixture.app/Contents/Resources/assets/helper',
  ]);
  // Innermost first: everything a bundle holds is signed before the bundle.
  for (const [i, path] of order.entries()) {
    for (const later of order.slice(i + 1)) assert.ok(!later.startsWith(`${path}/`), `${later} is signed after its container ${path}`);
  }
  assert.equal(order.at(-1), 'Fixture.app');
}));

const tools = process.platform === 'darwin' && ['clang', 'codesign'].every((tool) => Bun.which(tool));
test.skipIf(!tools)('signing in that order seals a bundle with an unsigned helper and an unsigned nested app', () => inDir((dir) => {
  const app = resolve(dir, 'Fixture.app');
  const source = resolve(dir, 'main.c');
  writeFileSync(source, 'int main(void){return 0;}\n');
  const cc = (out, ...flags) => {
    mkdirSync(dirname(resolve(app, out)), { recursive: true });
    const r = spawnSync('clang', [...flags, '-o', resolve(app, out), source], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  };
  cc('Contents/MacOS/ExactMac');
  writeFileSync(resolve(app, 'Contents/Info.plist'), plist('ExactMac'));
  cc('Contents/Resources/assets/helper', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/Inner.app/Contents/MacOS/Inner', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Inner.app/Contents/Info.plist'), plist('Inner'));

  for (const path of signingOrder(app)) {
    const r = spawnSync('codesign', ['--force', '--sign', '-', '--options', 'runtime', path], { encoding: 'utf8' });
    assert.equal(r.status, 0, `${path}: ${r.stderr}`);
  }
  const verify = spawnSync('codesign', ['--verify', '--deep', '--strict', app], { encoding: 'utf8' });
  assert.equal(verify.status, 0, verify.stderr);
  const helper = spawnSync('codesign', ['-dv', resolve(app, 'Contents/Resources/assets/helper')], { encoding: 'utf8' });
  assert.match(helper.stderr, /flags=0x10002\(adhoc,runtime\)/);
}), 60000);
