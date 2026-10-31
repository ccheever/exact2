// @fixture/vault's prebuilt library and the package that declares it (LLP 1154 §3 item 4), built as a package with
// native code is published: the library (`library/`, a few lines of Rust over rusqlite's bundled SQLite) is compiled as
// a dynamic library for this machine's targets, each target's file is put at a file: URL under `<out>/libraries/`,
// and a copy of the package at `<out>/package/` declares each by that URL and its SHA-256 in exact.json
// `native.library`. The checkout's exact.json names no target, so an app built from it is refused by name.
//
// What each target's file is (scripts/package-libraries.mjs says what the build reads of it):
// - a Mac: one zipped xcframework holding a macOS framework (versioned) and an iOS simulator's (flat), each named
//   `@rpath/vault.framework/…`, declared for aarch64-apple-darwin, which the Linux host on a Mac builds for too, and
//   aarch64-apple-ios-sim;
// - Linux: `libvault.so`, whose SONAME is the name it is carried by.
//
// usage: bun packages/fixtures/vault/test/library.mjs <out>   (prints the package's directory)
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const pkg = resolve(import.meta.dir, '..'), ANSWER = 4217;
export const TARGET = /^host: (.+)$/m.exec(spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout)[1];
const mac = process.platform === 'darwin';

const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} (exit ${r.status}):\n${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The library compiled for `target` as a dynamic library, linked with `linkArg` (its install name or SONAME), at
 * `out`. Its Cargo target directory is the one Cargo chooses for the crate (its own `target/`, or
 * `CARGO_TARGET_DIR`), kept between builds, named to the build so that the file copied is the one it wrote. */
function cdylib(target, out, linkArg) {
  const crate = join(pkg, 'library'), manifest = join(crate, 'Cargo.toml');
  const targetDir = JSON.parse(run('cargo', ['metadata', '--format-version', '1', '--no-deps', '--locked', '--manifest-path', manifest], crate)).target_directory;
  const env = { ...process.env, MACOSX_DEPLOYMENT_TARGET: '14.0', IPHONEOS_DEPLOYMENT_TARGET: '17.0' };
  run('cargo', ['rustc', '--locked', '--release', '--lib', '--crate-type', 'cdylib', '--target', target, '--target-dir', targetDir, '--manifest-path', manifest, '--', '-C', `link-arg=${linkArg}`], crate, env);
  cpSync(join(targetDir, target, 'release', target.includes('-apple-') ? 'libvault.dylib' : 'libvault.so'), out);
}

const plist = (keys = '') => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>vault</string><key>CFBundleIdentifier</key><string>dev.exact.fixture.vault</string><key>CFBundlePackageType</key><string>FMWK</string><key>CFBundleVersion</key><string>1</string>${keys}</dict></plist>
`;

/** This machine's targets' libraries under `<out>/libraries/`: the declaration's `native.library`. */
export function buildLibrary(out) {
  const dir = join(out, 'libraries');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  let file, targets;
  if (mac) {
    const fw = join(dir, 'mac/vault.framework');
    mkdirSync(join(fw, 'Versions/A/Resources'), { recursive: true });
    cdylib('aarch64-apple-darwin', join(fw, 'Versions/A/vault'), '-Wl,-install_name,@rpath/vault.framework/Versions/A/vault');
    symlinkSync('A', join(fw, 'Versions/Current'));
    symlinkSync('Versions/Current/vault', join(fw, 'vault'));
    symlinkSync('Versions/Current/Resources', join(fw, 'Resources'));
    writeFileSync(join(fw, 'Versions/A/Resources/Info.plist'), plist());
    // An iOS framework is flat (Apple's "Bundle Programming Guide", "Anatomy of Framework Bundles").
    const sim = join(dir, 'sim/vault.framework');
    mkdirSync(sim, { recursive: true });
    cdylib('aarch64-apple-ios-sim', join(sim, 'vault'), '-Wl,-install_name,@rpath/vault.framework/vault');
    writeFileSync(join(sim, 'Info.plist'), plist('<key>MinimumOSVersion</key><string>17.0</string>'));
    for (const f of [fw, sim]) run('codesign', ['--force', '--sign', '-', f], dir);
    run('xcodebuild', ['-create-xcframework', '-framework', fw, '-framework', sim, '-output', join(dir, 'vault.xcframework')], dir);
    file = join(dir, 'vault.xcframework.zip');
    run('ditto', ['-c', '-k', '--keepParent', 'vault.xcframework', file], dir);
    targets = ['aarch64-apple-darwin', 'aarch64-apple-ios-sim'];
  } else {
    file = join(dir, 'libvault.so');
    cdylib(TARGET, file, '-Wl,-soname,libvault.so');
    targets = [TARGET];
  }
  const entry = { url: pathToFileURL(file).href, sha256: createHash('sha256').update(readFileSync(file)).digest('hex') };
  return Object.fromEntries(targets.map(target => [target, entry]));
}

/** The package as published, at `<out>/package/`, declaring the libraries `buildLibrary` built (or `library`). */
export function buildPackage(out, library = buildLibrary(out)) {
  const to = join(out, 'package');
  rmSync(to, { recursive: true, force: true });
  for (const f of ['package.json', 'driver.js', 'driver.d.ts', 'native', 'web']) cpSync(join(pkg, f), join(to, f), { recursive: true });
  const declaration = JSON.parse(readFileSync(join(pkg, 'exact.json'), 'utf8'));
  declaration.native.library = library;
  writeFileSync(join(to, 'exact.json'), JSON.stringify(declaration, null, 2) + '\n');
  return { pkg: to, library };
}

if (import.meta.main) console.log(buildPackage(resolve(process.argv[2])).pkg);
