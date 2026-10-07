#!/usr/bin/env bun
// The downloadable build (20261005-portable-app-download item 1): `T3-Code-<version>-arm64.zip` and its
// `SHA256SUMS`, the `.app` another person's Apple Silicon Mac (macOS 14 or later) unzips and opens.
//
//   bun examples/t3-code/package-app.mjs [--ref <commit>] [--out <dir>] [--work <dir>]
//                                        [--deny <path>]… [--no-sandbox]
//
// 1. Export: `git archive <ref>` (default HEAD; uncommitted changes are not part of it) into
//    `<work>/exact2`, a fresh folder outside every checkout (default `/tmp/t3-code-package`).
// 2. Build there, under `sandbox-exec` with a profile that denies reading and writing this checkout,
//    `~/.t3` and every --deny (the reference checkout, another tree): `bun install`, the pinned stage
//    step (stage-runtime.mjs, its cache `<work>/runtime-cache` — the only network use, once), the
//    terminal page, and `host/apple/build.mjs t3-code-macos --bundle --distribution` ad hoc signed
//    (`EXACT_IDENTITY=-`; decision U11), with Rust's source paths remapped (the checkout to ``, Cargo's
//    home to `cargo`, std to `/rustc/<commit>`) so no build-machine path is compiled in.
// 3. Package: a copy of the bundle with every Mach-O file's local symbols stripped (`strip -x`), the
//    toolchain's absolute LC_RPATHs and the build-path install names of its own dylibs removed,
//    `LICENSE-T3`, and `Contents/Resources/distribution.json` `{"flavor":"packaged"}` (the marker that
//    makes it use `~/.t3` and the port scan from 3773: T3LocalBackend.swift), then signed ad hoc inside out, checked
//    with `codesign --verify --deep --strict`, zipped with `ditto -c -k --keepParent` (modes and links
//    kept) and hashed into `SHA256SUMS`.
// 4. Audit: audit-bundle.mjs over the packaged `.app`, with every --deny as this machine's paths and
//    the export folder as a build root; a finding fails the package.
//
// Outputs go to `<out>` (default `target/t3-package` in this checkout): the zip, `SHA256SUMS`,
// `build.log`, `audit.txt`, `sandbox.sb`. No private key is read: an ad hoc signature names none.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const APP_NAME = 'T3 Code (Exact)';

/** The clone's version (the client version it speaks, version-skew.ts), for the archive's name. */
export function clientVersion(dir = here) {
  const match = /export const CLIENT_VERSION = '([^']+)'/.exec(readFileSync(join(dir, 'version-skew.ts'), 'utf8'));
  if (!match) throw new Error('version-skew.ts has no CLIENT_VERSION');
  return match[1];
}
export const archiveName = (version) => `T3-Code-${version}-arm64.zip`;

/** The SBPL profile the build runs under: everything allowed but these trees (read or written). */
export function sandboxProfile(denied) {
  const paths = [...new Set(denied.map(path => resolve(path)))];
  const subpaths = paths.map(path => `(subpath ${JSON.stringify(path)})`).join(' ');
  return `(version 1)\n(allow default)\n(deny file-read* file-write* ${subpaths})\n`;
}

/** rustc's remaps for the app's own target: no checkout, Cargo home or toolchain path is compiled in. */
export function remapFlags(exportRoot, env = process.env) {
  const rustc = (...args) => spawnSync('rustc', args, { cwd: exportRoot, env, encoding: 'utf8' }).stdout ?? '';
  const commit = /^commit-hash: (\S+)$/m.exec(rustc('-vV'))?.[1], sysroot = rustc('--print', 'sysroot').trim();
  const pairs = [[resolve(env.CARGO_HOME ?? join(homedir(), '.cargo')), 'cargo']];
  if (sysroot && commit) pairs.push([join(sysroot, 'lib/rustlib/src/rust'), `/rustc/${commit}`]);
  pairs.push([exportRoot, '']);
  return pairs.map(([from, to]) => `--remap-path-prefix=${from}=${to}`).join(' ');
}

const say = (log, line) => { console.log(line); if (log) writeFileSync(log, `${line}\n`, { flag: 'a' }); };
function step(log, cmd, args, opts = {}) {
  say(log, `$ ${[cmd, ...args].join(' ')}`);
  const result = spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 256 << 20, ...opts });
  if (log) writeFileSync(log, `${result.stdout ?? ''}${result.stderr ?? ''}`, { flag: 'a' });
  if (result.status !== 0) {
    process.stderr.write(`${result.stdout ?? ''}${result.stderr ?? ''}`.slice(-6000));
    throw new Error(`${cmd} ${args[0] ?? ''} failed (exit ${result.status ?? result.error?.message})`);
  }
  return result;
}

/** Inside the export (and the sandbox): stage, build, package. Writes `<work>/out`. */
async function inside(work) {
  const exportRoot = root, out = join(work, 'out'), log = join(out, 'build.log');
  const env = { ...process.env, EXACT_APP_DIR: here, EXACT_IDENTITY: '-', T3_RUNTIME_CACHE: join(work, 'runtime-cache'),
    CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS: remapFlags(exportRoot) };
  const run = (cmd, args) => step(log, cmd, args, { cwd: exportRoot, env });
  const bun = process.execPath;
  run(bun, ['install', '--frozen-lockfile']);
  // The server's own spawns fail with EPERM under sandbox-exec (embedded-server-runtime), so the stage
  // step's start-and-probe runs only outside one; the archive is the pin's bytes either way.
  run(bun, ['examples/t3-code/stage-runtime.mjs', ...(process.env.T3_PACKAGE_SANDBOXED ? ['--no-smoke'] : [])]);
  run(bun, ['examples/t3-code/terminal-host/build.mjs']);
  const built = run(bun, ['host/apple/build.mjs', 't3-code-macos', '--bundle', '--distribution']);
  const bundle = /^local client: (.+\.app)$/m.exec(`${built.stdout}`)?.[1];
  if (!bundle || !existsSync(bundle)) throw new Error('host/apple/build.mjs named no bundle');

  const packaged = join(work, 'package', `${APP_NAME}.app`);
  rmSync(dirname(packaged), { recursive: true, force: true });
  mkdirSync(dirname(packaged), { recursive: true });
  run('/usr/bin/ditto', [bundle, packaged]);
  const contents = join(packaged, 'Contents'), executables = join(contents, 'MacOS');
  for (const name of readdirSync(executables)) {
    const file = join(executables, name);
    // Local symbols and the debug map's object and rlib paths stay on the build machine.
    run('strip', ['-x', file]);
    // otool reads `name(member)` as an archive member: "T3 Code (Exact)" is read through a plain link.
    const plain = join(work, 'otool-subject');
    rmSync(plain, { force: true });
    symlinkSync(file, plain);
    const loads = spawnSync('otool', ['-l', plain], { encoding: 'utf8' }).stdout ?? '';
    rmSync(plain, { force: true });
    // The toolchain's back-deployment rpaths (…/usr/lib/swift-6.2/macosx in Xcode and the Metal
    // toolchain: nothing here loads from them) and a dylib's build-path install name.
    for (const [, rpath] of loads.matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (.+?) \(offset/g)) {
      if (rpath.startsWith('/') && !rpath.startsWith('/usr/lib/') && !rpath.startsWith('/System/')) run('install_name_tool', ['-delete_rpath', rpath, file]);
    }
    const id = /cmd LC_ID_DYLIB\n\s+cmdsize \d+\n\s+name (.+?) \(offset/.exec(loads)?.[1];
    if (id && id.startsWith('/')) run('install_name_tool', ['-id', `@rpath/${name}`, file]);
  }
  writeFileSync(join(contents, 'Resources/distribution.json'), `${JSON.stringify({ flavor: 'packaged' })}\n`);
  // T3 Code's MIT notice travels with the code ported from it.
  copyFileSync(join(here, 'LICENSE-T3'), join(contents, 'Resources/LICENSE-T3'));
  run('xattr', ['-cr', packaged]);
  const { signingOrder } = await import('../../host/apple/assets.mjs');
  for (const path of signingOrder(packaged)) {
    run('codesign', ['--force', '--sign', '-', '--timestamp=none', ...(path === packaged ? ['--identifier', 'com.exact.t3code.macos'] : []), path]);
  }
  run('codesign', ['--verify', '--deep', '--strict', '--verbose=1', packaged]);
  const zip = join(out, archiveName(clientVersion()));
  rmSync(zip, { force: true });
  run('/usr/bin/ditto', ['-c', '-k', '--keepParent', packaged, zip]);
  const digest = createHash('sha256').update(readFileSync(zip)).digest('hex');
  writeFileSync(join(out, 'SHA256SUMS'), `${digest}  ${basename(zip)}\n`);
  say(log, `packaged ${basename(zip)} sha256 ${digest}`);
}

async function outside(args) {
  const take = (flag) => { const values = []; for (let i = 0; i < args.length; i++) if (args[i] === flag) values.push(args[++i]); return values; };
  const ref = take('--ref')[0] ?? 'HEAD';
  const out = resolve(take('--out')[0] ?? join(root, 'target/t3-package'));
  // A fixed folder that names no user or checkout: two framework build scripts compile their
  // JavaScript with absolute source names, which stay in the binary (see bundle-allowlist.json).
  const work = resolve(take('--work')[0] ?? '/tmp/t3-code-package');
  const sandboxed = !args.includes('--no-sandbox');
  const git = (...rest) => spawnSync('git', ['-C', root, ...rest], { encoding: 'utf8' });
  const sha = git('rev-parse', '--verify', `${ref}^{commit}`).stdout.trim();
  if (!sha) throw new Error(`no commit ${ref}`);
  if (git('status', '--porcelain').stdout.trim()) console.warn('package-app: this checkout has uncommitted changes; the package is built from the commit alone');
  const exportRoot = join(work, 'exact2');
  if (exportRoot === root || exportRoot.startsWith(root + '/') || root.startsWith(exportRoot + '/')) throw new Error('--work must be outside this checkout');
  rmSync(exportRoot, { recursive: true, force: true });
  rmSync(join(work, 'out'), { recursive: true, force: true });
  mkdirSync(exportRoot, { recursive: true });
  mkdirSync(join(work, 'out'), { recursive: true });
  mkdirSync(join(work, 'runtime-cache'), { recursive: true });
  const archive = spawnSync('/bin/sh', ['-c', 'git -C "$1" archive --format=tar "$2" | tar -x -C "$3"', 'sh', root, sha, exportRoot], { stdio: 'inherit' });
  if (archive.status !== 0) throw new Error('git archive failed');
  console.log(`package-app: exported ${sha} to ${exportRoot}`);

  // Denied to the build: this checkout, the real T3 home, and whatever else the caller names.
  const denied = [root, join(homedir(), '.t3'), ...take('--deny')];
  const profile = join(work, 'sandbox.sb');
  writeFileSync(profile, sandboxProfile(denied));
  const started = Date.now();
  const command = [process.execPath, join(exportRoot, 'examples/t3-code/package-app.mjs'), '--inside', work];
  const result = spawnSync(sandboxed ? '/usr/bin/sandbox-exec' : command[0], sandboxed ? ['-f', profile, ...command] : command.slice(1), {
    cwd: exportRoot, stdio: 'inherit', env: { ...process.env, T3_PACKAGE_SANDBOXED: sandboxed ? '1' : '' } });
  if (result.status !== 0) throw new Error(`the packaged build failed (exit ${result.status}); see ${join(work, 'out/build.log')}`);
  const seconds = ((Date.now() - started) / 1000).toFixed(0);

  mkdirSync(out, { recursive: true });
  const zipName = archiveName(clientVersion());
  for (const name of [zipName, 'SHA256SUMS', 'build.log']) copyFileSync(join(work, 'out', name), join(out, name));
  copyFileSync(profile, join(out, 'sandbox.sb'));
  writeFileSync(join(out, 'build.log'), `\nexported ${sha}; built in ${seconds} s${sandboxed ? ' under sandbox-exec (sandbox.sb)' : ' without a sandbox (--no-sandbox)'}\n`, { flag: 'a' });

  const { audit, formatReport } = await import('./audit-bundle.mjs');
  const report = audit(join(work, 'package', `${APP_NAME}.app`), { forbid: denied, buildRoots: [exportRoot] });
  const text = formatReport(report);
  writeFileSync(join(out, 'audit.txt'), `${text}\n`);
  console.log(text);
  console.log(`\npackage-app: ${join(out, zipName)}\n  ${readFileSync(join(out, 'SHA256SUMS'), 'utf8').trim()}\n  built from ${sha.slice(0, 12)} in ${seconds} s`);
  if (report.findings.length) throw new Error(`audit-bundle: ${report.findings.length} findings (see ${join(out, 'audit.txt')})`);
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const run = args[0] === '--inside' ? inside(resolve(args[1])) : outside(args);
  run.catch((error) => { console.error(`package-app: ${error.message}`); process.exit(1); });
}
