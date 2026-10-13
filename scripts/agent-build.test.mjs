// A native drive's build first (agent-build.mjs, LLP 1116.002 D4): the decision an app's exact.mjs makes before
// `test ios|macos` and `agent ios|macos`, on the driver's own record, with no simulator and no build run.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { appleArtifacts, appleRunBinary } from '../host/apple/build.mjs';
import { moduleGraph } from './agent-launch.mjs';
import { buildArgs, buildFirst, buildReason, failureSummary, nativeBuild, overrideDir, planOverride, planSource, rebuildCommand, refuseBuild } from './agent-build.mjs';
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
    // (A contract edit alone is a plan-only edit, which a build with no module receipt to check its bake against still rebuilds.)
    assert.deepEqual([said, builds.map(b => b.args)], [['building for iOS: the build is older than its sources (app.contract changed), and the build has no module receipt to check a new plan against; its log: ' + said[0].split('its log: ')[1]], [['--ios', 'field-log-apple']]]);

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
    write('app.ts', "import type { Book } from './types';\nimport { shelf } from './lib/model.js';\nimport seed from './lib/seed.json';\nexport const grants = '';\n");
    write('types.ts', 'export type Book = { title: string };\n');
    write('lib/model.ts', "export const shelf = [] as const;\n");
    write('lib/seed.json', '[]\n');
    write('server/index.ts', "import { Database } from 'bun:sqlite';\nimport { routes } from './routes';\n");
    write('server/routes.ts', 'export const routes = {};\n');
    write('server/fixtures.json', '{}\n');
    for (const file of ['app.contract', 'app.ts', 'types.ts', 'lib/model.ts', 'lib/seed.json', 'server/index.ts', 'server/routes.ts', 'server/fixtures.json', 'app.json', 'exact.mjs', 'apple/Cargo.toml', 'web/Cargo.toml']) stamp(file, -60);
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
    assert.deepEqual([...moduleGraph(dir, app.manifest)].map(p => p.slice(dir.length + 1)).sort(), ['app.ts', 'lib/model.ts', 'lib/seed.json', 'types.ts']);
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
    stamp('server/index.ts', 15); stamp('app.contract.d.ts', 15);
    assert.deepEqual(nativeBuild('ios', app).changed, []);
    assert.equal(await drive(), 0);
    assert.deepEqual([said, builds, bakes, kept()], [[], [], [], null]);

    // A contract edit: baked and kept beside the build, in one line, and no build; the driver runs it. The
    // compiler's warnings the bake said follow it, one line each (LLP 1116.004 D7).
    write('app.contract', 'component App\n  view\n    text "hello again" color="#336699"\n');
    stamp('app.contract', 20);
    const warning = 'app.contract:3:31 warning [lower-colour-one-scheme] `#336699` on `color` stays the same in dark mode';
    bakeSays = `${warning}\n{"version":1}\n`;
    assert.throws(() => refuseBuild('ios', nativeBuild('ios', app), app), /ios build is stale/, 'before the bake, the driver refuses as before');
    assert.equal(await drive(), 0);
    assert.deepEqual([said, builds, bakes.length], [['iOS: app.contract changed; running the new plan on the existing build (no native rebuild)', warning], [], 1]);
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
    assert.match(said[0], /^iOS: (app\.contract, app\.ts|app\.ts, app\.contract) changed; running the new plan on the existing build \(no native rebuild\)$/);
    assert.ok(kept());

    // A newer real build retires it: fresh, the build's own plan runs; a later touch of the same sources does not bring it back.
    build({}, 40, 'two');
    assert.equal(kept(), null);
    stamp('app.contract', 50);
    assert.deepEqual(nativeBuild('ios', app).changed.map(p => resolve(p)), [resolve(dir, 'app.contract')]);
    assert.equal(kept(), null, 'kept for another build: never run as fresh');

    // Anything else edited is built, and what was kept goes with it; the warnings Cargo said for the bake's
    // build script follow, in the compiler's own form.
    reset();
    stamp('app.json', 60);
    buildSays = `   Compiling plan-log-apple v0.1.0\nwarning: plan-log-apple@0.1.0: ${warning}\nhost/apple: built\n`;
    assert.equal(await drive(), 0);
    buildSays = '';
    assert.deepEqual([bakes.length, builds.length, said.slice(1)], [0, 1, [warning]]);
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
