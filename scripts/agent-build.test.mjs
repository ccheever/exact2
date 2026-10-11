// A native drive's build first (agent-build.mjs, LLP 1116.002 D4): the decision an app's exact.mjs makes before
// `test ios|macos` and `agent ios|macos`, on the driver's own record, with no simulator and no build run.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { appleArtifacts, appleRunBinary } from '../host/apple/build.mjs';
import { buildArgs, buildFirst, buildReason, failureSummary, nativeBuild, rebuildCommand, refuseBuild } from './agent-build.mjs';
import { resolveApp } from './app.mjs';

/** An outside app, as `exact new` lays one out, with only what the decision reads. */
function scratchApp(name) {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-agent-build-')));
  const write = (path, text = '') => { mkdirSync(dirname(resolve(dir, path)), { recursive: true }); writeFileSync(resolve(dir, path), text); };
  write('app.contract', 'component App\n  view\n    text "hello"\n');
  write('app.json', JSON.stringify({ name, app: { id: `com.exact.${name}`, name } }));
  for (const kind of ['apple', 'web']) write(`${kind}/Cargo.toml`, `[package]\nname="${name}-${kind}"\nversion="0.1.0"\n`);
  write('exact.mjs', "// …\nconst built = await run(['scripts/agent-build.mjs', host]);\n");
  return { dir, write };
}

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
    const drive = (host, { sim, code = 0, output = '' } = {}) => buildFirst(host, app, {
      sim, say: line => said.push(line), wait: () => {}, build: async (args, log) => (builds.push({ args, log }), { code, output }),
    });

    // No build yet: built first, for either host, by the command `bun exact.mjs ios|mac` runs.
    for (const host of ['ios', 'macos']) assert.equal(buildReason(nativeBuild(host, app), app), 'there is no build yet');
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), new RegExp(`^Error: run \\(cd '${dir}' && bun exact\\.mjs ios\\) first$`));
    assert.equal(await drive('ios', { sim: 'lane-r5ab' }), 0);
    assert.deepEqual(builds.map(b => b.args), [['--ios', 'field-log-apple', '--sim', 'lane-r5ab']]);
    assert.equal(builds[0].log, resolve(dir, '.exact/ios-build.log'));
    assert.equal(said.length, 1);
    assert.match(said[0], /^building for iOS: there is no build yet; its log: \S*\.exact\/ios-build\.log$/);
    assert.deepEqual(buildArgs('macos', app, 'lane-r5ab'), ['field-log-apple'], 'a Mac build takes no simulator');

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
    assert.deepEqual([said, builds.map(b => b.args)], [['building for iOS: the build is older than its sources (app.contract changed); its log: ' + said[0].split('its log: ')[1]], [['--ios', 'field-log-apple']]]);

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
    write('exact.mjs', '// an older one\n');
    assert.equal(rebuildCommand(app, 'mac'), `(cd '${dir}' && bun exact.mjs mac) (after bun exact.mjs update, test and agent build it themselves)`);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    if (macBin === undefined) delete process.env.EXACT_MAC_BIN; else process.env.EXACT_MAC_BIN = macBin;
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
