// A native drive's build first (agent-build.mjs, LLP 1116.002 D4): the decision an app's exact.mjs makes before
// `test ios|macos` and `agent ios|macos`, on the driver's own record, with no simulator and no build run.
import { afterEach, beforeEach, test } from 'bun:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, renameSync, rmSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, relative, resolve } from 'node:path';
import { appleArtifacts, appleRunBinary } from '../host/apple/build.mjs';
import { listAssets } from '../host/web/serve.mjs';
import { moduleGraph, notBuildInput } from './agent-launch.mjs';
import { buildArgs, buildFirst, buildReason, childFailure, failureSummary, linuxRebuild, nativeBuild, overrideDir, planOverride, planSource, ownVerb, prebuild, rebuildCommand, refuseBuild } from './agent-build.mjs';
import { openStdio } from './agent-stdio.mjs';
import { linuxBinary, resolveApp, verifyBakeFiles } from './app.mjs';

test('Mac drives use the assembled app bundle even without catalogs or widgets', () => {
  const { dir } = scratchApp('ats-drive');
  try {
    const app = resolveApp(dir);
    const paths = appleArtifacts(app);
    assert.equal(appleRunBinary(app), resolve(paths.bundle, 'Contents/MacOS', paths.executable));
    assert.equal(appleRunBinary(app, true), appleArtifacts(app, { host: true }).binary);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('a child failure keeps stderr when stdout ends with successful build progress', () => {
  assert.match(childFailure({ stderr: 'error: simulator installation failed\n', stdout: 'Building for production...\n[1/2] ExactIOS\nBuild complete!\n' }), /simulator installation failed/);
  assert.match(childFailure({ error: new Error('spawn ENOENT') }), /spawn ENOENT/);
  assert.equal(childFailure({ stdout: 'error: compiler failed\n' }), 'error: compiler failed');
});

test('a child failure retains errors from either stream past warnings and progress', () => {
  assert.equal(childFailure({ stderr: 'warning: deprecated flag\n', stdout: 'Building...\nerror: could not find sdk\n' }), 'error: could not find sdk');
  assert.equal(childFailure({ stderr: 'error: simulator installation failed\nwarning: one\nwarning: two\nwarning: three\n', stdout: 'Build complete!\n' }), 'error: simulator installation failed');
});

test('a metrics build summary does not claim that a drive was planned', () => {
  const summary = failureSummary('error: missing sdk\n', { host: 'ios', code: 1, log: 'size.log', beforeDrive: false });
  assert.match(summary, /the iOS build failed \(exit 1\); its whole log: size.log/);
  assert.doesNotMatch(summary, /driven/);
});

/** An outside app, as `exact new` lays one out, with only what the decision reads. */
// Each case starts with no target of the environment's: a command a case expects names one only when it is set
// (`ownVerb`), and the cases that set one say so.
const inheritedTarget = process.env.CARGO_TARGET_DIR;
beforeEach(() => { delete process.env.CARGO_TARGET_DIR; });
afterEach(() => { if (inheritedTarget === undefined) delete process.env.CARGO_TARGET_DIR; else process.env.CARGO_TARGET_DIR = inheritedTarget; });

function scratchApp(name) {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-agent-build-')));
  const write = (path, text = '') => { mkdirSync(dirname(resolve(dir, path)), { recursive: true }); writeFileSync(resolve(dir, path), text); };
  write('app.contract', 'component App\n  view\n    text "hello"\n');
  write('app.json', JSON.stringify({ name, app: { id: `com.exact.${name}`, name } }));
  for (const kind of ['apple', 'web']) write(`${kind}/Cargo.toml`, `[package]\nname="${name}-${kind}"\nversion="0.1.0"\n`);
  write('exact.mjs', "// …\nconst built = await run(['scripts/agent-build.mjs', host]);\n");
  return { dir, write };
}

test('macOS freshness checks the full product receipt when the bundle carries a reduced receipt', () => {
  const previous = process.env.EXACT_APP_DIR, { dir, write } = scratchApp('ats-drive');
  try {
    process.env.EXACT_APP_DIR = dir;
    const app = resolveApp('ats-drive'), source = resolve(dir, 'source.swift');
    write('source.swift', 'before');
    const paths = appleArtifacts(app), executable = resolve(paths.bundle, 'Contents/MacOS', paths.executable);
    mkdirSync(dirname(executable), { recursive: true }); writeFileSync(executable, '');
    const receipt = resolve(paths.products, 'receipt.json'), time = new Date(Date.now() + 60_000);
    mkdirSync(dirname(receipt), { recursive: true });
    writeFileSync(receipt, JSON.stringify({ target: paths.target, started: time.toISOString(), build: { trust: 'development', binary: { sha256: 'dev',
      inputs: [{ name: 'source.swift', path: source, sha256: createHash('sha256').update('before').digest('hex') }], missing: [], directories: [] } } }));
    utimesSync(receipt, time, time);
    const bundled = resolve(paths.bundle, 'Contents/Resources/receipt.json');
    mkdirSync(dirname(bundled), { recursive: true });
    const shipped = JSON.parse(readFileSync(receipt, 'utf8')); shipped.build.binary = { sha256: 'dev' };
    writeFileSync(bundled, JSON.stringify(shipped));
    assert.equal(nativeBuild('macos', app).receipt, receipt);
    assert.deepEqual(nativeBuild('macos', app).changed, []);
    write('source.swift', 'after');
    assert.deepEqual(nativeBuild('macos', app).changed, ['source.swift']);
    write('source.swift', 'before');
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a release published after a development build must be rebuilt before an agent drive', () => {
  const previous = process.env.EXACT_APP_DIR, { dir, write } = scratchApp('release-after-dev');
  try {
    process.env.EXACT_APP_DIR = dir;
    const app = resolveApp('release-after-dev'), paths = appleArtifacts(app), receipt = resolve(paths.products, 'receipt.json');
    const bundled = resolve(paths.bundle, 'Contents/Resources/receipt.json'), binary = appleRunBinary(app);
    const development = { built: new Date(Date.now() + 60_000).toISOString(), target: paths.target, composition: paths.composition,
      build: { trust: 'development', binary: { sha256: 'dev', inputs: [], missing: [], directories: [] } } };
    write(relative(dir, binary), 'development executable');
    write(relative(dir, receipt), JSON.stringify(development));
    write(relative(dir, bundled), JSON.stringify(development));
    assert.deepEqual(nativeBuild('macos', app).changed, []);
    const release = { ...development, build: { trust: 'production', binary: { sha256: 'release' } } };
    write(relative(dir, bundled), JSON.stringify(release));
    write(relative(dir, binary), 'production executable');
    const stale = nativeBuild('macos', app);
    assert.ok(stale.changed.includes(bundled), 'the development source receipt cannot approve a production bundle');
    assert.throws(() => refuseBuild('macos', stale, app), /build is stale/);
    write(relative(dir, bundled), JSON.stringify(development));
    write(relative(dir, binary), 'rebuilt development executable');
    assert.deepEqual(nativeBuild('macos', app).changed, []);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(dir, { recursive: true, force: true });
  }
});

// Both of a Linux drive's refusals (no build, a stale one) name this command (agent-stdio.mjs).
test('a Linux drive is told the build that writes where it reads', () => {
  const previous = { app: process.env.EXACT_APP_DIR, target: process.env.CARGO_TARGET_DIR };
  const { dir } = scratchApp('field-log');
  // An app directory inside this checkout, under the target/ a build makes and git ignores
  mkdirSync(resolve(import.meta.dir, '../target'), { recursive: true });
  const inside = mkdtempSync(resolve(import.meta.dir, '../target/agent-build-inside-'));
  const target = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-agent-target-')));
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.CARGO_TARGET_DIR;
    assert.equal(linuxRebuild(resolveApp('field-log')), `(cd '${dir}' && bun exact.mjs linux)`);
    // A target the environment names is resolved where the drive ran; the command runs in the app, so it names it whole.
    process.env.CARGO_TARGET_DIR = relative(process.cwd(), target);
    const app = resolveApp('field-log');
    assert.equal(app.target, target);
    assert.equal(linuxRebuild(app), `(cd '${dir}' && CARGO_TARGET_DIR='${target}' bun exact.mjs linux)`);
    // An Apple host's command names it too.
    assert.equal(rebuildCommand(app, 'ios'), `(cd '${dir}' && CARGO_TARGET_DIR='${target}' bun exact.mjs ios)`);
    // An app inside this checkout has no verb of its own, an exact.mjs or not.
    writeFileSync(resolve(inside, 'exact.mjs'), '');
    assert.equal(ownVerb({ dir: inside }, 'linux'), null);
  } finally {
    for (const [name, value] of [['EXACT_APP_DIR', previous.app], ['CARGO_TARGET_DIR', previous.target]]) {
      if (value === undefined) delete process.env[name]; else process.env[name] = value;
    }
    for (const made of [dir, inside, target]) rmSync(made, { recursive: true, force: true });
  }
});

// Each refusal reached through the driver, and its command run as written, by an exact.mjs that says where it ran and
// which target it was given: the one the drive reads.
test('a Linux drive refused for want of a current build names a command that builds where it reads', async () => {
  const previous = { app: process.env.EXACT_APP_DIR, target: process.env.CARGO_TARGET_DIR, bin: process.env.EXACT_LINUX_BIN };
  const { dir, write } = scratchApp('field-log');
  const target = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-agent-target-')));
  try {
    Object.assign(process.env, { EXACT_APP_DIR: dir, CARGO_TARGET_DIR: target });
    delete process.env.EXACT_LINUX_BIN;
    const app = resolveApp('field-log'), bin = linuxBinary(app), command = linuxRebuild(app);
    await assert.rejects(openStdio({ host: 'linux', app: 'field-log' }), { message: `run ${command} first` });
    // A build a minute older than a source its dep-info names
    mkdirSync(dirname(bin), { recursive: true });
    writeFileSync(bin, '');
    writeFileSync(`${bin}.d`, `${bin}: ${resolve(dir, 'app.contract')}\n`);
    const now = new Date(), past = new Date(now.getTime() - 60_000);
    utimesSync(resolve(dir, 'app.contract'), now, now);
    utimesSync(bin, past, past);
    await assert.rejects(openStdio({ host: 'linux', app: 'field-log' }), (e) => e.message.includes('linux build is stale') && e.message.includes(`; run ${command}`));
    const ran = resolve(dir, 'ran.json');
    write('exact.mjs', `import { writeFileSync } from 'node:fs';\nwriteFileSync(${JSON.stringify(ran)}, JSON.stringify({ cwd: process.cwd(), target: process.env.CARGO_TARGET_DIR, verb: process.argv[2] }));\n`);
    // From a shell whose target is relative, as the drive's was before it resolved it
    execFileSync('sh', ['-c', command], { env: { ...process.env, CARGO_TARGET_DIR: relative(process.cwd(), target) } });
    assert.deepEqual(JSON.parse(readFileSync(ran, 'utf8')), { cwd: dir, target, verb: 'linux' });
    assert.ok(bin.startsWith(`${target}/`), bin);
  } finally {
    for (const [name, value] of [['EXACT_APP_DIR', previous.app], ['CARGO_TARGET_DIR', previous.target], ['EXACT_LINUX_BIN', previous.bin]]) {
      if (value === undefined) delete process.env[name]; else process.env[name] = value;
    }
    rmSync(dir, { recursive: true, force: true });
    rmSync(target, { recursive: true, force: true });
  }
});

test('a missing or stale build is built first, said in one line; a fresh one runs nothing and says nothing', async () => {
  const previous = process.env.EXACT_APP_DIR, macBin = process.env.EXACT_MAC_BIN;
  const { dir, write } = scratchApp('field-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.EXACT_MAC_BIN;
    const app = resolveApp('field-log');
    const built = new Date(Date.now() + 10_000), edited = new Date(+built + 10_000), before = new Date(Date.now() - 60_000);
    for (const file of ['app.contract', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) utimesSync(resolve(dir, file), before, before);
    const said = [], builds = [];
    const drive = (host, { code = 0, output = '' } = {}) => buildFirst(host, app, {
      say: line => said.push(line), wait: () => {}, build: async (args, log) => (builds.push({ args, log }), { code, output }),
    });

    // No build yet: built first, for either host, by the command `bun exact.mjs ios|mac` runs.
    for (const host of ['ios', 'macos']) assert.equal(buildReason(nativeBuild(host, app), app), 'there is no build yet');
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), new RegExp(`^Error: run \\(cd '${dir}' && bun exact\\.mjs ios\\) first$`));
    assert.equal(await drive('ios'), 0);
    // The drive installs, keeping a Release build it replaces: the build installs nothing.
    assert.deepEqual(builds.map(b => b.args), [['--ios', 'field-log-apple', '--no-install']]);
    assert.equal(builds[0].log, resolve(dir, '.exact/ios-build.log'));
    assert.equal(said.length, 1);
    assert.match(said[0], /^building for iOS: there is no build yet; its log: \S*\.exact\/ios-build\.log$/);
    assert.deepEqual(buildArgs('macos', app), ['field-log-apple']);

    // A build its receipt shows fresh: nothing runs, nothing is said.
    const bundle = appleArtifacts(app, { destination: 'ios-simulator' }).bundle, receipt = resolve(bundle, 'receipt.json');
    mkdirSync(bundle, { recursive: true });
    writeFileSync(receipt, JSON.stringify({ target: 'aarch64-apple-ios-sim' }));
    utimesSync(receipt, built, built);
    said.length = builds.length = 0;
    assert.equal(buildReason(nativeBuild('ios', app), app), null);
    assert.doesNotThrow(() => refuseBuild('ios', nativeBuild('ios', app), app));
    assert.equal(await drive('ios'), 0);
    assert.deepEqual([said, builds], [[], []]);

    // A source edited since: the same refusal the driver makes, and the build first instead, naming what changed.
    utimesSync(resolve(dir, 'app.contract'), edited, edited);
    assert.deepEqual(nativeBuild('ios', app).changed.map(p => resolve(p)), [resolve(dir, 'app.contract')]);
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), /^Error: ios build is stale: .*app\.contract changed since .*; run \(cd .* && bun exact\.mjs ios\)$/);
    assert.equal(await drive('ios'), 0);
    // (A contract edit alone is a plan-only edit, which a build with no module receipt to check its bake against still rebuilds.)
    assert.deepEqual([said, builds.map(b => b.args)], [['building for iOS: the build is older than its sources (app.contract changed), and the build has no module receipt to check a new plan against; its log: ' + said[0].split('its log: ')[1]], [['--ios', 'field-log-apple', '--no-install']]]);

    // A scratch file in .exact/ (the build's log among them) is not a source.
    utimesSync(resolve(dir, 'app.contract'), before, before);
    write('.exact/ios-build.log', 'Compiling …');
    write('.exact/notes.json', '{}');
    assert.equal(buildReason(nativeBuild('ios', app), app), null);

    // A failed build fails the command with what it said: from its first error, then where its whole log is.
    utimesSync(resolve(dir, 'app.contract'), edited, edited);
    said.length = builds.length = 0;
    const output = '   Compiling exact-kernel v0.1.0\n   Compiling field-log-apple v0.1.0\nerror[E0425]: cannot find value `x` in this scope\n --> src/lib.rs:3:5\nerror: could not compile `field-log-apple`\ncargo rustc exited 101\n';
    assert.equal(await drive('ios', { code: 101, output }), 101);
    assert.equal(said.length, 2);
    assert.ok(said[1].startsWith('error[E0425]: cannot find value `x` in this scope\n --> src/lib.rs:3:5\n'), said[1]);
    assert.ok(!said[1].includes('Compiling'), 'the summary leaves out the progress');
    assert.match(said[1], /\ncargo rustc exited 101\nthe iOS build failed \(exit 101\), so nothing was driven; its whole log: \S*\.exact\/ios-build\.log$/);
    assert.match(failureSummary('a\nb\nc\n', { host: 'macos', code: 1, log: 'x.log' }), /^a\nb\nc\nthe macOS build failed \(exit 1\)/, 'no error line: the tail');

    // macOS: its binary is the record; a binary EXACT_MAC_BIN names is the caller's, never built.
    said.length = builds.length = 0;
    assert.equal(await drive('macos'), 0);
    assert.deepEqual([said.length, builds.map(b => b.args)], [1, [['field-log-apple']]]);
    assert.match(said[0], /^building for macOS: there is no build yet/);
    mkdirSync(dirname(appleRunBinary(app)), { recursive: true });
    writeFileSync(appleRunBinary(app), '');
    assert.equal(buildReason(nativeBuild('macos', app), app), null, 'a Mac binary without a receipt is not refused');
    process.env.EXACT_MAC_BIN = '/nowhere/bin';
    said.length = builds.length = 0;
    assert.equal(await buildFirst('mac', app, { say: line => said.push(line), wait: () => assert.fail('waited'), build: () => assert.fail('built') }), 0);
    assert.deepEqual(said, []);

    // An exact.mjs from before test and agent built first is told how to get that.
    assert.equal(rebuildCommand(app, 'ios'), `(cd '${dir}' && bun exact.mjs ios)`);
    write('exact.mjs', "const cli = resolve(EXACT2, 'scripts/app-cli.mjs');\n"); // the launcher (LLP 1116.006 D7)
    assert.equal(rebuildCommand(app, 'ios'), `(cd '${dir}' && bun exact.mjs ios)`);
    write('exact.mjs', '// an older one\n');
    assert.equal(rebuildCommand(app, 'mac'), `(cd '${dir}' && bun exact.mjs mac) (after bun exact.mjs update, test and agent build it themselves)`);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (macBin === undefined) delete process.env.EXACT_MAC_BIN; else process.env.EXACT_MAC_BIN = macBin;
    rmSync(dir, { recursive: true, force: true });
  }
});

// LLP 1116.008 D2: never a stale build. The receipt is written at the build's end, so a file edited while the build ran
// is older than it, though the build read the file before the edit (polls: a prebuild tested the old UI); the receipt's
// `started` bounds it. And a build another process claims between the check and this one is waited for, not failed on.
test('a source edited while the build ran is stale; a build claimed meanwhile is waited for, then the check is made again', async () => {
  const previous = process.env.EXACT_APP_DIR, macBin = process.env.EXACT_MAC_BIN;
  const { dir, write } = scratchApp('race-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.EXACT_MAC_BIN;
    const app = resolveApp('race-log');
    const at = seconds => new Date(Date.now() + seconds * 1000), stamp = (file, seconds) => utimesSync(resolve(dir, file), at(seconds), at(seconds));
    for (const file of ['app.contract', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) stamp(file, -60);
    const bundle = appleArtifacts(app, { destination: 'ios-simulator' }).bundle, receipt = resolve(bundle, 'receipt.json');
    mkdirSync(bundle, { recursive: true });
    // Built from t-20 s to t+10 s; the contract edited at t-5 s, while it ran.
    const built = (started) => { writeFileSync(receipt, JSON.stringify({ target: 'aarch64-apple-ios-sim', ...(started ? { started: at(-20).toISOString() } : {}) })); utimesSync(receipt, at(10), at(10)); };
    stamp('app.contract', -5);
    built(false);
    assert.deepEqual(nativeBuild('ios', app).changed, [], 'a receipt without `started` is trusted by its time, as it was');
    built(true);
    assert.deepEqual(nativeBuild('ios', app).changed.map(p => resolve(p)), [resolve(dir, 'app.contract')]);
    assert.match(buildReason(nativeBuild('ios', app), app), /^the build is older than its sources \(app\.contract changed\)$/);
    stamp('app.contract', -60); // read before the build began: fresh
    assert.deepEqual(nativeBuild('ios', app).changed, []);

    // Two drives started together: the second's build finds the claim taken, waits for the first's, and finds it fresh.
    stamp('app.contract', 60);
    const said = [], builds = [];
    let waits = 0;
    const code = await buildFirst('ios', app, {
      say: line => said.push(line), wait: () => { waits++; },
      build: async (args) => { builds.push(args); stamp('app.contract', -60); built(true); return { code: 1, output: `Error: Apple build busy for com.exact.race-log: {"pid":1} (${dir}/.apple-build.lock); remove this lock explicitly\n` }; },
    });
    assert.deepEqual([code, builds.length, waits], [0, 1, 2]);
    assert.equal(said.length, 1);
    assert.match(said[0], /^building for iOS: the build is older than its sources/);
    // Another failure is still the build's: said, with its code.
    stamp('app.contract', 120);
    said.length = 0;
    assert.equal(await buildFirst('ios', app, { say: line => said.push(line), wait: () => {}, build: async () => ({ code: 101, output: 'error: could not compile\n' }) }), 101);
    assert.match(said.at(-1), /the iOS build failed \(exit 101\)/);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (macBin === undefined) delete process.env.EXACT_MAC_BIN; else process.env.EXACT_MAC_BIN = macBin;
    rmSync(dir, { recursive: true, force: true });
  }
});

// The Apple build signs a copy of the GPU dylib and the app's bake reads it, so the archive's dep-info names a file the
// build itself wrote after it started. Cargo leaves that dep-info as it was when a relink names the same paths, so its
// time says nothing: the receipt's digest of the product decides, and every other input keeps the build's start.
test('a product the build wrote and then read is judged by its recorded bytes; any other input by the build start', () => {
  const previous = process.env.EXACT_APP_DIR, macBin = process.env.EXACT_MAC_BIN, targetDir = process.env.CARGO_TARGET_DIR, cwd = process.cwd();
  const { dir, write } = scratchApp('gpu-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.EXACT_MAC_BIN; delete process.env.CARGO_TARGET_DIR;
    const app = resolveApp('gpu-log');
    const at = seconds => new Date(Date.now() + seconds * 1000), stamp = (file, seconds) => utimesSync(file, at(seconds), at(seconds));
    for (const file of ['app.contract', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) stamp(resolve(dir, file), -120);
    const bundle = appleArtifacts(app, { destination: 'ios-simulator' }).bundle, receipt = resolve(bundle, 'receipt.json');
    mkdirSync(bundle, { recursive: true });
    const profile = resolve(app.target, 'aarch64-apple-ios-sim', 'host-dev'), dylib = resolve(profile, 'signed/libgpu_log_gpu.dylib');
    const info = resolve(profile, 'libgpu_log_apple.d'), generated = resolve(profile, 'generated.contract');
    const sha256 = text => createHash('sha256').update(text).digest('hex');
    const changed = () => nativeBuild('ios', app).changed.map(p => resolve(p)).sort();
    // The first build, t-100 s to t-70 s, wrote the dep-info at t-80 s.
    write(dylib.slice(dir.length + 1), 'signed v1');
    write(generated.slice(dir.length + 1), 'component Generated');
    writeFileSync(info, `${profile}/libgpu_log_apple.a: ${resolve(dir, 'app.contract')} ${generated} ${dylib}\n`);
    stamp(info, -80); stamp(generated, -110);
    // A GPU source edited, and built again from t-20 s to t+10 s: the copy signed again at t-10 s, the archive relinked,
    // and the dep-info, naming the same paths, left as it was.
    writeFileSync(dylib, 'signed v2'); stamp(dylib, -10);
    writeFileSync(receipt, JSON.stringify({ target: 'aarch64-apple-ios-sim', started: at(-20).toISOString(), build: { products: [{ path: dylib, bytes: 9, sha256: sha256('signed v2') }] } }));
    stamp(receipt, 10);
    assert.deepEqual(changed(), []);
    // Other bytes, though put back with the time it had: not what the bundle holds.
    writeFileSync(dylib, 'signed v3'); stamp(dylib, -10);
    assert.deepEqual(changed(), [dylib]);
    rmSync(dylib);
    assert.deepEqual(changed(), [dylib]);
    write(dylib.slice(dir.length + 1), 'signed v2');
    assert.deepEqual(changed(), []);
    // A source under the profile directory that the receipt does not name as a product keeps the build's start.
    stamp(generated, -5);
    assert.deepEqual(changed(), [generated]);
    stamp(generated, -110); stamp(resolve(dir, 'app.contract'), -5);
    assert.deepEqual(changed(), [resolve(dir, 'app.contract')]);
    stamp(resolve(dir, 'app.contract'), -120);
    // `build.dep-info-basedir` writes the paths relative to the workspace Cargo ran in, and the drive runs from
    // elsewhere (here neither this checkout nor the workspace): both rules read them against that workspace.
    const rel = path => relative(app.workspace, path);
    writeFileSync(info, `${rel(profile)}/libgpu_log_apple.a: ${rel(resolve(dir, 'app.contract'))} ${rel(generated)} ${rel(dylib)}\n`);
    stamp(info, -80);
    process.chdir(dirname(dir));
    assert.notEqual(realpathSync(process.cwd()), app.workspace);
    assert.deepEqual(changed(), []);
    writeFileSync(dylib, 'signed v3'); stamp(dylib, -10);
    assert.deepEqual(changed(), [dylib]);
    writeFileSync(dylib, 'signed v2'); stamp(dylib, -10); stamp(generated, -5);
    assert.deepEqual(changed(), [generated]);
  } finally {
    process.chdir(cwd);
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (macBin === undefined) delete process.env.EXACT_MAC_BIN; else process.env.EXACT_MAC_BIN = macBin;
    if (targetDir !== undefined) process.env.CARGO_TARGET_DIR = targetDir;
    rmSync(dir, { recursive: true, force: true });
  }
});

// Admission of a complete bake receipt, without running Cargo or the packaging copier: a custom build script may
// generate these files after the build starts, before the bake records the exact bytes packaging must match.
test.each([
  ...['assets', 'deck'].flatMap(tree => ['unchanged after build start', 'changed bytes with an older mtime', 'missing captured file',
    'new file with an older mtime', 'empty inventory', 'empty inventory then new file', 'malformed bare tree card'].map(scenario => [tree, scenario])),
  ...['ordinary source edited during build', 'legacy receipt without captured assets', 'malformed inventory array', 'malformed asset path',
    'malformed asset hash', 'malformed asset size', 'malformed duplicate asset', 'malformed null card', 'symlinked captured file',
    'dangling captured file', 'symlinked tree root', 'dangling tree root', 'non-directory tree root',
    'shader source after start', 'entry source after start', 'unknown tree source after start',
    'malformed shader digest array', 'malformed entry digest array'].map(scenario => ['assets', scenario]),
])('captured native %s: %s', (tree, scenario) => {
  const previous = process.env.EXACT_APP_DIR, targetDir = process.env.CARGO_TARGET_DIR;
  const { dir, write } = scratchApp('generated-assets');
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.CARGO_TARGET_DIR;
    const app = resolveApp('generated-assets'), now = Date.now();
    const at = seconds => new Date(now + seconds * 1000);
    const stamp = (file, seconds) => utimesSync(resolve(dir, file), at(seconds), at(seconds));
    for (const file of ['app.contract', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) stamp(file, -60);
    const files = ['preview.html', 'preview.js', 'device.html', 'device.js'].map(name => `${tree}/generated/${name}`);
    for (const file of files) write(file, `generated ${file}\n`);
    const emptyInventory = scenario.startsWith('empty inventory') || scenario === 'non-directory tree root' || scenario === 'malformed bare tree card';
    if (emptyInventory) rmSync(resolve(dir, tree), { recursive: true });
    if (scenario === 'malformed bare tree card') mkdirSync(resolve(dir, tree));
    const other = { 'shader source after start': 'shaders/source.wgsl', 'entry source after start': 'plans/source.ts', 'unknown tree source after start': 'custom/source.rs',
      'malformed shader digest array': 'shaders/source.wgsl', 'malformed entry digest array': 'plans/source.ts' }[scenario];
    if (other) { write(other, 'source'); stamp(other, scenario.endsWith('digest array') ? -60 : -10); }
    const plan = Buffer.from('baked plan');
    const compat = { embedded: { plan: { bytes: plan.length, sha256: createHash('sha256').update(plan).digest('hex') }, assets: listAssets(dir) } };
    verifyBakeFiles(compat, plan, listAssets(dir));
    const target = 'aarch64-apple-ios-sim', profile = resolve(app.target, target, 'host-dev');
    mkdirSync(profile, { recursive: true });
    writeFileSync(resolve(profile, 'libgenerated_assets_apple.d'), `${profile}/libgenerated_assets_apple.a: ${[tree, ...(emptyInventory ? [] : files), ...(other ? [other] : [])].map(file => resolve(dir, file)).join(' ')}\n`);
    const bundle = appleArtifacts(app, { destination: 'ios-simulator' }).bundle, receipt = resolve(bundle, 'receipt.json');
    mkdirSync(bundle, { recursive: true });
    const build = { target, started: at(-20).toISOString(), build: { compat } };
    let expected = [];
    if (scenario === 'changed bytes with an older mtime') { write(files[0], 'different bytes'); expected = [files[0]]; }
    if (scenario === 'missing captured file') { rmSync(resolve(dir, files[1])); expected = [files[1]]; }
    if (scenario === 'new file with an older mtime' || scenario === 'empty inventory then new file') { write(`${tree}/generated/extra.js`, 'new bytes'); expected = [`${tree}/generated/extra.js`]; }
    if (scenario === 'ordinary source edited during build') { stamp('app.contract', -5); expected = ['app.contract']; }
    if (scenario === 'legacy receipt without captured assets') delete build.build.compat;
    if (scenario === 'malformed inventory array') compat.embedded.assets = {};
    if (scenario === 'malformed asset path') compat.embedded.assets[0].name = `${tree}/../app.contract`;
    if (scenario === 'malformed asset hash') compat.embedded.assets[0].sha256 = 'not a digest';
    if (scenario === 'malformed asset size') compat.embedded.assets[0].bytes = -1;
    if (scenario === 'malformed duplicate asset') compat.embedded.assets.push({ ...compat.embedded.assets[0] });
    if (scenario === 'malformed null card') compat.embedded.assets.push(null);
    if (scenario === 'malformed bare tree card') compat.embedded.assets.push({ name: tree, bytes: 0, sha256: createHash('sha256').update('').digest('hex') });
    if (scenario.endsWith('digest array')) { const card = compat.embedded.assets.find(card => card.name === other); card.sha256 = [card.sha256]; }
    if (scenario === 'symlinked captured file') {
      renameSync(resolve(dir, files[0]), resolve(dir, '.held-file'));
      symlinkSync(resolve(dir, '.held-file'), resolve(dir, files[0])); expected = [files[0]];
    }
    if (scenario === 'dangling captured file') { rmSync(resolve(dir, files[0])); symlinkSync(resolve(dir, '.missing-file'), resolve(dir, files[0])); expected = [files[0]]; }
    if (scenario === 'symlinked tree root') { renameSync(resolve(dir, tree), resolve(dir, '.held-tree')); symlinkSync(resolve(dir, '.held-tree'), resolve(dir, tree), 'dir'); }
    if (scenario === 'dangling tree root') { rmSync(resolve(dir, tree), { recursive: true }); symlinkSync(resolve(dir, '.missing-tree'), resolve(dir, tree), 'dir'); }
    if (scenario === 'non-directory tree root') { write(tree, 'not a directory'); stamp(tree, -60); }
    if (other) expected = [other];
    // Backdate the mutation cases, including directory entry changes, so their bytes/inventory decide.
    const assetTime = scenario === 'unchanged after build start' || scenario === 'legacy receipt without captured assets' || scenario.startsWith('malformed') ? -10 : -60;
    for (const file of [...files, `${tree}/generated/extra.js`]) if (existsSync(resolve(dir, file))) stamp(file, assetTime);
    for (const directory of [`${tree}/generated`, tree]) if (existsSync(resolve(dir, directory)) && lstatSync(resolve(dir, directory)).isDirectory()) stamp(directory, assetTime);
    writeFileSync(receipt, JSON.stringify(build));
    utimesSync(receipt, at(10), at(10));
    const state = nativeBuild('ios', app), changed = state.changed.map(path => resolve(path)).sort();
    if (scenario === 'malformed bare tree card') {
      assert.deepEqual(changed, [resolve(dir, tree)], scenario);
    } else if (scenario === 'legacy receipt without captured assets' || scenario.startsWith('malformed')) {
      for (const file of files) assert.ok(changed.includes(resolve(dir, file)), file);
    } else if (scenario.endsWith('tree root')) {
      assert.ok(changed.includes(resolve(dir, tree)), scenario);
    } else assert.deepEqual(changed, expected.map(file => resolve(dir, file)).sort(), scenario);
    if (scenario === 'unchanged after build start' || scenario === 'empty inventory') assert.doesNotThrow(() => refuseBuild('ios', state, app));
    else assert.throws(() => refuseBuild('ios', state, app), /ios build is stale/);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (targetDir === undefined) delete process.env.CARGO_TARGET_DIR; else process.env.CARGO_TARGET_DIR = targetDir;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a build already running ends before the check: the drive neither races it nor is refused as busy', async () => {
  const previous = process.env.EXACT_APP_DIR;
  const { dir } = scratchApp('busy-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    const app = resolveApp('busy-log'), lock = appleArtifacts(app).lock;
    mkdirSync(dirname(lock), { recursive: true });
    // This process holds the claim, as a background `bun exact.mjs ios` would, and lets it go in a moment.
    writeFileSync(lock, JSON.stringify({ pid: process.pid, app: app.id, source: dir, started: new Date().toISOString() }) + '\n');
    setTimeout(() => rmSync(lock), 400);
    const said = [], started = Date.now();
    const code = await buildFirst('ios', app, { say: line => said.push(line), build: async () => ({ code: 0, output: '' }) });
    assert.equal(code, 0);
    assert.ok(Date.now() - started >= 350, 'it waited for the claim');
    assert.deepEqual(said.map(l => l.split(':')[0]), ['waiting for the Apple build of busy-log already running', 'building for iOS']);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a plan-only edit runs its bake on the existing build; any other edit, or a newer real build, retires it', async () => {
  const previous = process.env.EXACT_APP_DIR, trust = process.env.EXACT_UPDATE_TRUST;
  const { dir, write } = scratchApp('plan-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    delete process.env.EXACT_UPDATE_TRUST;
    const app = resolveApp('plan-log');
    const at = offset => new Date(Date.now() + offset * 1000), stamp = (file, offset) => utimesSync(resolve(dir, file), at(offset), at(offset));
    // app.ts and what it imports, type-only imports included; a backend beside it that the app reaches over HTTP.
    write('app.ts', "import type { Book } from './types';\nimport { shelf } from './lib/model.js';\nimport seed from './lib/seed.json';\nimport { money } from './lib/format.mjs';\nexport const grants = '';\n");
    write('types.ts', 'export type Book = { title: string };\n');
    write('lib/model.ts', "export const shelf = [] as const;\n");
    write('lib/seed.json', '[]\n');
    write('lib/format.mjs', "import { digits } from './digits.mjs';\nexport const money = n => digits(n);\n");
    write('lib/digits.mjs', 'export const digits = n => String(n);\n');
    // A helper script beside the app that app.ts never imports (an export, a seeding script).
    write('scripts/export.mjs', 'console.log(1);\n');
    write('server/index.ts', "import { Database } from 'bun:sqlite';\nimport { routes } from './routes';\n");
    write('server/routes.ts', 'export const routes = {};\n');
    write('server/fixtures.json', '{}\n');
    for (const file of ['app.contract', 'app.ts', 'types.ts', 'lib/model.ts', 'lib/seed.json', 'lib/format.mjs', 'lib/digits.mjs', 'scripts/export.mjs', 'server/index.ts', 'server/routes.ts', 'server/fixtures.json', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) stamp(file, -60);
    // The build: its receipt names the plan its build script baked, beside which is the module receipt it paired.
    const target = 'aarch64-apple-ios-sim', out = resolve(app.target, target, 'host-dev/build/plan-log-apple-0123/out');
    const module = { version: 1, appId: 'com.exact.plan-log', grants: '', abi: 1, surfaces: {}, bytecodeVersion: 96 };
    mkdirSync(out, { recursive: true });
    writeFileSync(resolve(out, 'app.plan'), 'plan one');
    writeFileSync(resolve(out, 'app.module.json'), JSON.stringify(module));
    // The archive's dep-info also names every source the bake captured (its rerun-if-changed), the backend's among them.
    writeFileSync(resolve(app.target, target, `host-dev/lib${app.crate('ios').replace(/-/g, '_')}.d`), `${resolve(app.target, target, 'host-dev/libplan_log_apple.a')}: ${out}/app.plan ${out}/app.hbc ${['app.contract', 'app.ts', 'server/index.ts', 'server/fixtures.json'].map(f => resolve(dir, f)).join(' ')}\n`);
    const bundle = appleArtifacts(app, { destination: 'ios-simulator' }).bundle, receipt = resolve(bundle, 'receipt.json');
    const build = (inputs = {}, offset = 10, built = 'one') => {
      mkdirSync(bundle, { recursive: true });
      writeFileSync(receipt, JSON.stringify({ built, target, build: { compat: { inputs: { linked: ['markdown', 'router', 'io'], rustMode: 'off', dataCrate: null, ...inputs },
        embedded: { plan: { sha256: createHash('sha256').update('plan one').digest('hex') } } }, graph: { uses: ['router'] } } }));
      utimesSync(receipt, at(offset), at(offset));
    };
    build();
    const said = [], builds = [], bakes = [];
    let baked = { ...module }, bakeFails = null, bakeSays = '', buildSays = '';
    const bake = async (a, into) => {
      bakes.push(into);
      if (bakeFails) return { code: 1, output: bakeFails };
      mkdirSync(into);
      for (const [file, text] of [['app.plan', 'plan two'], ['app.hbc', 'bytecode'], ['app.module.json', JSON.stringify(baked)]]) writeFileSync(resolve(into, file), text);
      return { code: 0, output: bakeSays };
    };
    const drive = (code = 0) => buildFirst('ios', app, { say: line => said.push(line), wait: () => {}, bake,
      build: async args => (builds.push(args), { code, output: buildSays }) });
    const reset = () => { said.length = builds.length = bakes.length = 0; };
    const kept = () => planOverride('ios', app, nativeBuild('ios', app));

    // What app.ts imports is the plan's; what it does not import is no input at all (LLP 1116.003 D1.1).
    assert.deepEqual([...moduleGraph(dir, app.manifest)].map(p => p.slice(dir.length + 1)).sort(), ['app.ts', 'lib/digits.mjs', 'lib/format.mjs', 'lib/model.ts', 'lib/seed.json', 'types.ts']);
    // A script app.ts never imports is no input, a `.mjs` as a `.ts`; one it imports, through another, is.
    const input = notBuildInput(dir);
    assert.deepEqual(['scripts/export.mjs', 'lib/format.mjs', 'lib/digits.mjs', 'server/index.ts'].map(f => input(resolve(dir, f))), [true, false, false, true]);
    // Only the Contract and what app.ts imports, and nothing a native build reads, is a plan-only edit.
    for (const [path, only] of [['app.contract', true], ['app.ts', true], ['types.ts', true], ['lib/model.ts', true], ['lib/seed.json', true], ['screens/list.contract', true],
      ['server/index.ts', false], ['server/fixtures.json', false], ['app.json', false], ['app.contract.d.ts', false],
      ['assets/data.ts', false], ['apple/src/lib.rs', false], ['web/x.ts', false], ['fonts/a.ttf', false], ['../outside.ts', false]]) assert.equal(planSource(resolve(dir, path), app), only, path);
    write('data/Cargo.toml', '[package]\nname = "plan-log-data"\n');
    assert.equal(planSource(resolve(dir, 'data/answers.ts'), app), false, "a Rust crate's own files are its build's");
    rmSync(resolve(dir, 'data'), { recursive: true });

    // A fresh build: nothing runs, nothing is kept, the driver runs the build's own plan. An edit to the backend,
    // or to the declarations a web build writes beside app.ts, leaves it fresh (round 5's book-club rebuilt for server/index.ts).
    write('server/index.ts', "import { Database } from 'bun:sqlite';\nimport { routes } from './routes';\nexport const port = 9610;\n");
    write('app.contract.d.ts', 'export type Answer = unknown;\n');
    write('seed.json', '[{"table":"owners","row":{"person":"dev:alice"}}]\n');
    write('.exact/seed-log.json', '{}\n');
    stamp('server/index.ts', 15); stamp('app.contract.d.ts', 15);
    stamp('seed.json', 15); stamp('.exact/seed-log.json', 15);
    assert.deepEqual(nativeBuild('ios', app).changed, []);
    assert.equal(await drive(), 0);
    assert.deepEqual([said, builds, bakes, kept()], [[], [], [], null]);

    // A contract edit: baked and kept beside the build, in one line, and no build; the driver runs it. The
    // compiler's warnings the bake said follow it, one line each the first time, then a count naming their file.
    write('app.contract', 'component App\n  view\n    text "hello again" color="#336699"\n');
    stamp('app.contract', 20);
    const warning = 'app.contract:3:31 warning [lower-colour-one-scheme] `#336699` on `color` stays the same in dark mode';
    bakeSays = `${warning}\n{"version":1}\n`;
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), /ios build is stale/, 'before the bake, the driver refuses as before');
    assert.equal(await drive(), 0);
    assert.deepEqual([said.slice(0, 2), builds, bakes.length], [['iOS: app.contract changed; running the new plan on the existing build (no native rebuild). Only the agent\'s drives run it: a launch outside them (`simctl launch`, the home screen) runs the installed plan, so `bun exact.mjs ios` before timing one', warning], [], 1]);
    assert.equal(said.length, 3);
    assert.match(said[2], /^compiler: 1 warning; \S+\/\.exact\/warnings-ios\.txt lists them all$/);
    bakeSays = '';
    assert.deepEqual(kept(), { plan: resolve(overrideDir('ios', app), 'app.plan'), dir: overrideDir('ios', app) });
    assert.equal(readFileSync(kept().plan, 'utf8'), 'plan two');
    assert.ok(!existsSync(bakes[0]), 'the bake is staged, then moved beside the build');

    // Driven again with nothing edited: it runs as it is, saying nothing.
    reset();
    assert.equal(await drive(), 0);
    assert.deepEqual([said, builds, bakes], [[], [], []]);

    // The TypeScript edited since: what is kept is no longer the bake of these sources; the driver refuses it, test and agent bake again.
    write('app.ts', "export const grants = '';\nexport const more = 1;\n");
    stamp('app.ts', 30);
    assert.equal(kept(), null);
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), /ios build is stale/);
    reset();
    assert.equal(await drive(), 0);
    assert.deepEqual([said.length, builds.length, bakes.length], [1, 0, 1]);
    assert.match(said[0], /^iOS: (app\.contract, app\.ts|app\.ts, app\.contract) changed; running the new plan on the existing build \(no native rebuild\)\. Only the agent's drives run it: .*`bun exact\.mjs ios` before timing one$/);
    assert.ok(kept());

    // A newer real build retires it: fresh, the build's own plan runs; a later touch of the same sources does not bring it back.
    build({}, 40, 'two');
    assert.equal(kept(), null);
    stamp('app.contract', 50);
    assert.deepEqual(nativeBuild('ios', app).changed.map(p => resolve(p)), [resolve(dir, 'app.contract')]);
    assert.equal(kept(), null, 'kept for another build: never run as fresh');

    // Anything else edited is built, and what was kept goes with it; the warnings Cargo said for the bake's
    // build script follow, in the compiler's own form: in full, since the bake before said none.
    reset();
    stamp('app.json', 60);
    buildSays = `   Compiling plan-log-apple v0.1.0\nwarning: plan-log-apple@0.1.0: ${warning}\nhost/apple: built\n`;
    assert.equal(await drive(), 0);
    assert.deepEqual([bakes.length, builds.length, said[1], said.length], [0, 1, warning, 3]);
    // Built again over the same warning: its count alone.
    reset();
    stamp('app.json', 70);
    assert.equal(await drive(), 0);
    buildSays = '';
    assert.equal(said.length, 2);
    assert.match(said[1], /^compiler: 1 warning, as before \(\S+\/warnings-ios\.txt lists them; /);
    assert.match(said[0], /^building for iOS: the build is older than its sources \((app\.contract, app\.json|app\.json, app\.contract) changed\); its log: /);
    assert.ok(!existsSync(overrideDir('ios', app)));

    // What a build must still make: new grants, a Rust data crate, a build that links only what its plan used.
    for (const [setup, why] of [
      [() => { baked = { ...module, grants: 'net.fetch https://api.example' }; }, 'app.ts changed its grants, which only a build admits'],
      [() => build({ dataCrate: 'plan-log-data' }, 70), 'the app has a Rust data crate, whose plan only a build bakes'],
      [() => build({ linked: ['router', 'io'] }, 70), 'the build links only what its plan used (--link plan)'],
    ]) {
      build({}, 70); baked = { ...module }; setup(); reset();
      stamp('app.json', -60); stamp('app.contract', 80);
      assert.equal(await drive(), 0);
      assert.deepEqual([builds.length, kept()], [1, null], why);
      assert.match(said[0], new RegExp(`^building for iOS: the build is older than its sources \\(app\\.contract changed\\), and ${why.replace(/[().]/g, '\\$&')}; its log: `));
    }

    // A bake the compiler refuses fails the command with what it said, as the build would have; nothing is built or driven.
    build({}, 70); baked = { ...module }; reset();
    bakeFails = 'TypeScript bake: app.contract:3:5: unknown tag `txet`';
    assert.equal(await drive(), 1);
    assert.deepEqual([builds.length, kept()], [0, null]);
    assert.match(said[0], /^TypeScript bake: app\.contract:3:5: unknown tag `txet`\nthe iOS plan did not bake \(app\.contract changed\), so nothing was driven; its whole log: \S*\.exact\/ios-plan\.log$/);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (trust === undefined) delete process.env.EXACT_UPDATE_TRUST; else process.env.EXACT_UPDATE_TRUST = trust;
    rmSync(dir, { recursive: true, force: true });
  }
});

// A drive's compiler warnings keep the compiler's order, the symbols only the web draws blank one line at the end;
// the file beside the app lists each (the euxy port's header warning was one of 43, behind 21 of them).
// A background build ends apart from every shell, so the next build or drive says once how it failed (the euxy port's
// `--prebuild --release`, failed by an edit, said so only in its log).
test('a failed background build is said once by the next drive, from its part of the log', async () => {
  const previous = process.env.EXACT_APP_DIR;
  const { dir, write } = scratchApp('prebuilt-log');
  try {
    process.env.EXACT_APP_DIR = dir;
    const app = resolveApp('prebuilt-log'), said = [];
    // The drive rebuilds, writing the build log anew.
    const drive = () => buildFirst('ios', app, { say: line => said.push(line), wait: () => {}, build: async (args, log) => (writeFileSync(log, 'rebuilt'), { code: 0, output: '' }) });
    const build = 'app: the bake failed:\napp.contract:14:10 [syntax-expected] expected `)`, found `like`\n';
    write('.exact/ios-build.log', `${build}   Compiling the bake tool\n`);
    write('.exact/ios-prebuild.json', JSON.stringify({ args: ['--ios', 'prebuilt-log-apple', '--release'], started: '08:02:54', pid: 1, exit: 1, bytes: build.length }));
    await drive();
    assert.match(said[0], /^app: the bake failed:\napp\.contract:14:10 \[syntax-expected\] expected `\)`, found `like`\nthe background iOS build \(--prebuild --ios --release, started 08:02:54\) failed \(exit 1\); its whole log: \S*\.exact\/ios-prebuild-failed\.log$/);
    assert.equal(readFileSync(resolve(dir, '.exact/ios-prebuild-failed.log'), 'utf8'), build, 'its part, kept from the rebuild');
    assert.ok(!existsSync(resolve(dir, '.exact/ios-prebuild.json')), 'said once');
    said.length = 0;
    await drive();
    assert.ok(!said.some(line => line.includes('background')), said.join('\n'));
    // One that ended before it could say (its shell gone) is dropped, unsaid.
    write('.exact/ios-prebuild.json', JSON.stringify({ args: ['--ios'], started: '08:02:54', pid: 2 ** 22 + 1 }));
    await drive();
    assert.ok(!said.some(line => line.includes('background')) && !existsSync(resolve(dir, '.exact/ios-prebuild.json')));
    // A second started while a build holds the claim starts nothing, and leaves the first's log and record.
    const lock = appleArtifacts(app).lock;
    mkdirSync(dirname(lock), { recursive: true });
    writeFileSync(lock, JSON.stringify({ pid: process.pid, app: app.id, source: dir, started: new Date().toISOString() }) + '\n');
    write('.exact/ios-prebuild.json', '{"id":"first"}');
    assert.equal(await prebuild(app, ['--ios', 'prebuilt-log-apple', '--prebuild'], { say: line => said.push(line) }), 1);
    assert.match(said.at(-1), /already running, so none was started/);
    assert.equal(readFileSync(resolve(dir, '.exact/ios-prebuild.json'), 'utf8'), '{"id":"first"}');
    assert.equal(readFileSync(resolve(dir, '.exact/ios-build.log'), 'utf8'), 'rebuilt');
    rmSync(lock);
    // So does one started before the first's build has claimed it: that record names a live shell.
    write('.exact/ios-prebuild.json', JSON.stringify({ id: 'first', pid: process.pid }));
    assert.equal(await prebuild(app, ['--ios', 'prebuilt-log-apple', '--prebuild'], { say: line => said.push(line) }), 1);
    assert.equal(JSON.parse(readFileSync(resolve(dir, '.exact/ios-prebuild.json'), 'utf8')).id, 'first');
    assert.equal(readFileSync(resolve(dir, '.exact/ios-build.log'), 'utf8'), 'rebuilt');
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a drive says the web-only symbol warnings in one line and lists each in its file', async () => {
  const { warningLines } = await import('./app-cli.mjs');
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-warning-lines-')));
  try {
    const header = 'app.contract:9:3 warning [lower-header-unplaced] this `button` in this route\'s `header` is not shown on iOS';
    const first = 'app.contract:4:5 warning [lower-symbol-platform] symbol `sf/a` draws only on Apple, and the app ships web (app.json `host.web`), where it is blank. Declare an icon token';
    const later = 'app.contract:5:5 warning [lower-symbol-platform] symbol `sf/b` is blank on web too: write `symbol:add`, the role, which draws on every host';
    const native = 'app.contract:6:5 warning [lower-symbol-platform] symbol `sf/c` is blank on linux and web too: write `symbol:add`, the role, which draws on every host';
    const said = warningLines(dir, 'ios', [header, native, first, later]);
    assert.deepEqual(said.slice(0, 2), [header, native]);
    assert.match(said[2], /^app\.contract:4:5 warning \[lower-symbol-platform\] 2 SF Symbol uses draw blank on the web, here first: .*\.exact\/warnings-ios\.txt lists each, and how\)$/);
    assert.match(said[3], /^compiler: 4 warnings; /);
    assert.equal(readFileSync(resolve(dir, '.exact', 'warnings-ios.txt'), 'utf8'), [header, native, first, later].map((w) => `${w}\n`).join(''));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// An Apple build's bake says its warnings on every build: the SF Symbols only the web draws blank one line where the
// first was (euxy's 18, every Release build), the rest each.
test('an Apple bake says the web-only symbol warnings in one line', async () => {
  const { bakeWarnings } = await import('./linked-plan.mjs');
  const alias = 'app.contract:2:5 warning [lower-symbol-alias] symbol `sf/arrow.up.arrow.down` is the role `sort`: write `symbol:sort`';
  const first = 'app.contract:4:5 warning [lower-symbol-platform] symbol `sf/a` draws only on Apple, and the app ships web (app.json `host.web`), where it is blank. Declare an icon token';
  const later = 'app.contract:5:5 warning [lower-symbol-platform] symbol `sf/b` is blank on web too: write `symbol:add`, the role, which draws on every host';
  assert.deepEqual(bakeWarnings('euxy', [alias, first, later]), [
    `warning: euxy: ${alias}`,
    'warning: euxy: app.contract:4:5 warning [lower-symbol-platform] 2 SF Symbol uses draw blank on the web, here first: an icon token or a role draws on every host (`bun exact.mjs contract build --json` says each, and how)',
  ]);
  assert.deepEqual(bakeWarnings('euxy', [alias]), [`warning: euxy: ${alias}`]);
});
