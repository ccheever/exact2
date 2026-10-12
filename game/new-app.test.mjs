import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { outsideWorkspaceProblems, pathFrom, readManifest } from '../scripts/app.mjs';
import { appVerbs } from '../scripts/app-cli.mjs';
import { applyPattern, createApp, createGame, patternFiles, patterns } from './new.mjs';

/** A fake exact2 at `sdk` runs the real app-cli.mjs that an app's exact.mjs hands its verb to (LLP 1116.006 D7), and the
 * verify.mjs whose server `test` runs for an app that has one (LLP 1116.007 D2). */
const withCli = (sdk) => { mkdirSync(resolve(sdk, 'scripts'), { recursive: true }); for (const file of ['app-cli', 'verify', 'help']) copyFileSync(resolve(import.meta.dir, `../scripts/${file}.mjs`), resolve(sdk, `scripts/${file}.mjs`)); };

test('a new outside app passes the checks every run makes, and a drifted one is told what to paste', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.ok(existsSync(resolve(dir, 'app.test.contract')));
    assert.deepEqual(readFileSync(resolve(dir, '.cargo/config.toml')), readFileSync(resolve(import.meta.dir, '../.cargo/config.toml')));
    // The diary is one line that sends the agent to `feedback status`, which prints its
    // instructions (LLP 1116 D3); the notes stay small, and .exact/ stays local.
    const agents = () => readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(agents(), /<!-- exact:begin[^]*Run `bun exact\.mjs feedback status` first; it prints what to keep\.[^]*<!-- exact:end -->/);
    assert.doesNotMatch(agents(), /## The authoring diary|### Needed/);
    // 1.5 KB, read at step one and carried to the end (LLP 1116.007 D2), and the three paths of a long temp dir
    assert.ok(Buffer.byteLength(agents()) <= 1536 + 3 * resolve(import.meta.dir, '..').length, `AGENTS.md is ${Buffer.byteLength(agents())} bytes`);
    assert.match(agents(), /exact\.mjs setup --check/);
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.exact\/$/m);
    // The runner is a few lines a builder can read (LLP 1116.006 D7): where the verbs are, and the checkout's app-cli.mjs.
    const launcher = readFileSync(resolve(dir, 'exact.mjs'), 'utf8');
    assert.ok(launcher.trimEnd().split('\n').length <= 25, `exact.mjs is ${launcher.split('\n').length} lines`);
    assert.match(launcher.split('\n')[1], /`bun exact\.mjs --help` lists the verbs/);
    // Execute the generated dispatcher against fake SDK entry points: cwd may
    // be anywhere, but the source and test file must still name this app.
    const sdk = resolve(parent, 'sdk');
    for (const file of ['host/web/build.mjs', 'scripts/agent.mjs', 'scripts/agent-build.mjs', 'scripts/exact.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), 'console.log(JSON.stringify({args:process.argv.slice(2),app:process.env.EXACT_APP_DIR,offline:process.env.HERMES_LEAN_SYS_OFFLINE}));');
    }
    withCli(sdk);
    const missing = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'web'], { cwd: parent, env: { ...process.env, EXACT2: resolve(parent, 'nowhere') }, encoding: 'utf8' });
    assert.deepEqual([missing.status, /^no exact2 at .*nowhere \(no scripts\/app-cli\.mjs\): EXACT2 names the checkout$/m.test(missing.stderr)], [1, true], missing.stderr);
    const testArgs = host => [host, '--app', 'field-log', '--test', resolve(realpathSync(dir), 'app.test.contract')];
    // A drive builds first (LLP 1116.002 D4): web-build, or agent-build for ios and macos, its output on
    // stderr so stdout stays the drive's; a phone's build stays explicit.
    const webBuild = ['field-log'], ios = ['ios', '--app', 'field-log'];
    for (const [command, args, built = null] of [
      [['web-build'], ['field-log']],
      [['test'], testArgs('web'), webBuild],
      [['test', 'web'], testArgs('web'), webBuild],
      [['test', 'ios'], testArgs('ios'), ios],
      [['test', 'macos', '--size', '800x600'], [...testArgs('macos'), '--size', '800x600'], ['macos', '--app', 'field-log']],
      [['agent', 'web', 'tree', 'type title a title with spaces'], ['web', '--app', 'field-log', 'tree', 'type title a title with spaces'], webBuild],
      [['agent', 'ios', 'state'], ['ios', '--app', 'field-log', 'state'], ios],
      [['agent', 'ios', '--sim', 'lane', 'tree'], ['ios', '--app', 'field-log', '--sim', 'lane', 'tree'], [...ios, '--sim', 'lane']],
      [['agent', 'ios', '--device', 'tree'], ['ios', '--app', 'field-log', '--device', 'tree']],
      [['contract', 'build', 'app.contract', '--json'], ['contract', 'build', 'app.contract', '--json']],
    ]) {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), { args, app: realpathSync(dir), offline: '1' });
      assert.deepEqual(result.stderr.trim() ? JSON.parse(result.stderr).args : null, built, `${command.join(' ')} builds first`);
    }
    // chat F19, calendar F13: the files named, from the current directory (a glob too), each run in turn.
    mkdirSync(resolve(dir, 'tests'));
    for (const name of ['b.test.contract', 'a.test.contract']) writeFileSync(resolve(dir, 'tests', name), '');
    const files = (command, cwd = dir) => {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      return result.stdout.trim().split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l).args);
    };
    const tested = (host, file, ...more) => [host, '--app', 'field-log', '--test', resolve(realpathSync(dir), file), ...more];
    assert.deepEqual(files(['test', 'ios', 'tests/b.test.contract', '--size', '800x600']), [tested('ios', 'tests/b.test.contract', '--size', '800x600')]);
    assert.deepEqual(files(['test', 'macos', 'tests/*.test.contract']), [tested('macos', 'tests/a.test.contract'), tested('macos', 'tests/b.test.contract')]);
    assert.deepEqual(files(['test', 'ios', '--test', 'field-log/tests/a.test.contract'], parent), [tested('ios', 'tests/a.test.contract')]);
    assert.equal(spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'test', 'ios', 'none/*.test.contract'], { cwd: dir, env: { ...process.env, EXACT2: sdk } }).status, 2);
    writeFileSync(resolve(sdk, 'scripts/agent.mjs'), 'process.exit(7);');
    const refused = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'test', 'ios'], { cwd: parent, env: { ...process.env, EXACT2: sdk } });
    assert.equal(refused.status, 7, 'a failed app test fails the generated command');
    writeFileSync(resolve(sdk, 'scripts/agent-build.mjs'), 'process.exit(5);');
    const unbuilt = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'agent', 'macos', 'tree'], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    assert.deepEqual([unbuilt.status, unbuilt.stdout], [5, ''], 'a failed build fails the drive, which never starts');
    const logged = readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.deepEqual(logged.map(c => [c.verb, c.exit]).slice(-3), [['test ios', 2], ['test ios', 7], ['agent macos', 5]], 'each command is logged, without its arguments, a glob that matched nothing and a failed build too');
    assert.ok(logged.some(c => c.verb === 'contract' && c.exit === 0), 'the contract command is logged too');
    const manifest = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
    writeFileSync(resolve(dir, 'Cargo.toml'), manifest.replace(/^taffy = .*\n/m, ''));
    writeFileSync(resolve(dir, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.0.0"\n');
    writeFileSync(resolve(dir, '.cargo/config.toml'), '[env]\nHERMES_LEAN_SYS_OFFLINE = "0"\n');
    const [patches, toolchain] = outsideWorkspaceProblems(dir);
    assert.match(patches, /must name exact2's vendored taffy\. Use .*:\n\[patch\.crates-io\]\ntaffy = /);
    assert.match(toolchain, /pins 1\.0\.0; exact2 builds with /);
    // A checkout that moved: every exact2 path in the app is wrong.
    const web = resolve(dir, 'web/Cargo.toml');
    writeFileSync(web, readFileSync(web, 'utf8').replaceAll(/path = "[^"]*"/g, 'path = "/nowhere/exact2/x"'));
    // An app from before the diary joined the generated block loses its old diary block, keeps its own text, and ignores .exact/.
    writeFileSync(resolve(dir, 'AGENTS.md'), '# Mine\n\n<!-- exact diary: old -->\nstale\n<!-- /exact diary -->\n\nAfter.\n');
    writeFileSync(resolve(dir, '.gitignore'), '/target/');
    assert.match(createApp(dir, { update: true }), /\.gitignore, AGENTS\.md, exact2 paths in web\/Cargo\.toml/);
    assert.match(agents(), /^# Mine\n\nAfter\.\n\n<!-- exact:begin[^]*feedback status[^]*<!-- exact:end -->\n$/);
    assert.ok(!agents().includes('stale'));
    assert.equal(readFileSync(resolve(dir, '.gitignore'), 'utf8'), '/target/\n/.exact/\n');
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.deepEqual(readFileSync(resolve(dir, '.cargo/config.toml')), readFileSync(resolve(import.meta.dir, '../.cargo/config.toml')));
    assert.ok(!readFileSync(web, 'utf8').includes('/nowhere'));
    const appTest = resolve(dir, 'app.test.contract');
    assert.match(readFileSync(appTest, 'utf8'), /the greeting loads/, 'update preserves existing tests');
    rmSync(appTest);
    assert.doesNotMatch(createApp(dir, { update: true }), /AGENTS|gitignore/, 'current instructions are left as they are');
    assert.match(readFileSync(appTest, 'utf8'), /test "the app opens"\n$/);
    assert.ok(!readFileSync(appTest, 'utf8').includes('greeting'), 'an older app need not have the scaffold IDs');
    assert.equal(readFileSync(resolve(dir, 'Cargo.toml'), 'utf8'), manifest, 'the patch table is rewritten in place');
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000); // Three offline Cargo resolutions; a cold metadata cache takes seconds.

// chat and onboarding F20: killing `bun exact.mjs web` left the dev server listening. A signal is passed on; a
// SIGKILL passes nothing, so the server watches its launcher (`EXACT_LAUNCHER_PID`, serve.mjs `watchLauncher`).
test('the generated exact.mjs ends its child with it: a signal is passed on, and a killed launcher leaves no server', async () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };
  const until = async (f) => { for (let i = 0; i < 100; i++) { const v = f(); if (v) return v; await new Promise((r) => setTimeout(r, 50)); } throw new Error('timed out'); };
  try {
    const dir = resolve(parent, 'field-log'), sdk = resolve(parent, 'sdk'), pidFile = resolve(parent, 'dev.pid');
    createApp(dir);
    withCli(sdk);
    mkdirSync(resolve(sdk, 'host/web'), { recursive: true });
    writeFileSync(resolve(sdk, 'host/web/dev.mjs'), `import { watchLauncher } from ${JSON.stringify(resolve(import.meta.dir, '../host/web/serve.mjs'))};
import { writeFileSync } from 'node:fs';
process.on('SIGTERM', () => process.exit(0));
watchLauncher(() => process.exit(0));
writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));
setInterval(() => {}, 1000);`);
    for (const signal of ['SIGTERM', 'SIGKILL']) {
      rmSync(pidFile, { force: true });
      const launcher = spawn(process.execPath, [resolve(dir, 'exact.mjs'), 'web'], { cwd: parent, env: { ...process.env, EXACT2: sdk }, stdio: 'ignore' });
      const dev = await until(() => existsSync(pidFile) && Number(readFileSync(pidFile, 'utf8')));
      launcher.kill(signal);
      await until(() => !alive(dev));
    }
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 30_000);

// paint, minesweeper, ledger: `update` regenerated exact.mjs and dropped the verbs the app had added to it.
// The app's own verbs are app.json's `commands`, which the generated file reads and update leaves alone.
test('the app\'s own verbs live in app.json, so update keeps them', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log'), sdk = resolve(parent, 'sdk');
    createApp(dir);
    withCli(sdk);
    writeFileSync(resolve(sdk, 'scripts/agent.mjs'), '');
    const manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest, commands: { seed: ['bun', 'seed.mjs', '--quick'] } }, null, 2));
    writeFileSync(resolve(dir, 'seed.mjs'), 'console.log(JSON.stringify({cwd:process.cwd(),args:process.argv.slice(2),app:process.env.EXACT_APP_DIR,exact2:process.env.EXACT2})); process.exit(3);');
    readManifest(dir, 'field-log');
    const exact = (...args) => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...args], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    createApp(dir, { update: true });
    const seeded = exact('seed', 'web');
    assert.equal(seeded.status, 3, seeded.stderr);
    assert.deepEqual(JSON.parse(seeded.stdout), { cwd: realpathSync(dir), args: ['--quick', 'web'], app: realpathSync(dir), exact2: sdk });
    assert.match(exact('nope').stderr, /Usage: bun exact\.mjs <web\|[^>]*\|seed>/);
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n')[0]).verb, 'seed');
    // A built-in verb's name is the generated file's, and an empty command is no command.
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest, commands: { test: ['bun', 'seed.mjs'] } }, null, 2));
    assert.match(exact('test').stderr, /commands\.test: test is one of exact\.mjs's own verbs/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest, commands: { seed: [] } }, null, 2));
    assert.throws(() => readManifest(dir, 'field-log'), /commands\.seed: fewer than 1 items/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000); // Two offline Cargo resolutions.

test('a new app and a new game get VS Code tasks over their own verbs, and update writes only a missing file', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const verbsOf = (dir) => {
      const usage = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'nope'], { cwd: parent, encoding: 'utf8' }).stderr;
      return new Set(/Usage: bun exact\.mjs <([^>]*)>/.exec(usage)[1].split('|'));
    };
    const app = resolve(parent, 'field-log'), game = resolve(parent, 'block-hop');
    createApp(app);
    createGame(game);
    for (const [dir, extra] of [[app, []], [game, ['test-rust']]]) {
      const tasks = JSON.parse(readFileSync(resolve(dir, '.vscode/tasks.json'), 'utf8')).tasks, verbs = verbsOf(dir);
      const used = tasks.map(t => /^bun exact\.mjs (\S+)/.exec(t.command)[1]);
      for (const verb of used) assert.ok(verbs.has(verb), `${dir}: task verb ${verb} is not one of exact.mjs's (${[...verbs].join(', ')})`);
      for (const verb of ['contract', 'web', 'test', 'mac', 'ios', ...extra]) assert.ok(used.includes(verb), `${dir}: no task runs ${verb}`);
      assert.equal(tasks.filter(t => t.group?.isDefault).length, 1);
    }
    const tasksPath = resolve(app, '.vscode/tasks.json');
    writeFileSync(tasksPath, '{"version": "2.0.0", "tasks": []}\n');
    assert.doesNotMatch(createApp(app, { update: true }), /tasks\.json/);
    assert.equal(readFileSync(tasksPath, 'utf8'), '{"version": "2.0.0", "tasks": []}\n');
    rmSync(resolve(app, '.vscode'), { recursive: true });
    assert.match(createApp(app, { update: true }), /, \.vscode\/tasks\.json/);
    assert.ok(JSON.parse(readFileSync(tasksPath, 'utf8')).tasks.length > 0);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000); // Three offline Cargo resolutions.

test('a game outside this checkout gets an app\'s runner, with its own verbs and no app.json, and update rewrites it (the platformer\'s diary, R1)', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'block-hop'), sdk = resolve(parent, 'sdk');
    createGame(dir);
    for (const file of ['AGENTS.md', 'exact.mjs', 'app.test.contract']) assert.ok(existsSync(resolve(dir, file)), file);
    assert.ok(lstatSync(resolve(dir, 'CLAUDE.md')).isSymbolicLink());
    assert.ok(!existsSync(resolve(dir, 'app.json')), 'a game\'s manifest stays optional');
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.exact\/$/m);
    for (const file of ['game/app/shells.mjs', 'game/prove.mjs', 'host/web/build.mjs', 'scripts/agent.mjs', 'scripts/feedback.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), 'console.log(JSON.stringify({args:process.argv.slice(2),app:process.env.EXACT_APP_DIR}));');
    }
    withCli(sdk);
    const real = realpathSync(dir);
    const drive = (command) => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    for (const [command, args] of [
      [['test-rust'], [real, '--test']],
      [['prove', '--repin'], [real, '--repin']],
      [['web-build'], ['block-hop']],
      [['agent', 'web', 'tree'], ['web', '--app', 'block-hop', 'tree']],
      [['feedback', 'status'], ['status']],
    ]) {
      const result = drive(command);
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout.trim().split('\n').at(-1)), { args, app: real }, command.join(' '));
    }
    writeFileSync(resolve(dir, 'exact.mjs'), '');
    writeFileSync(resolve(dir, 'AGENTS.md'), `# Mine\n\n${readFileSync(resolve(dir, 'AGENTS.md'), 'utf8').replace('an Exact game', 'stale')}`);
    assert.match(createApp(dir, { update: true }), /^Updated .*: exact\.mjs, AGENTS\.md$/);
    assert.match(readFileSync(resolve(dir, 'exact.mjs'), 'utf8'), /name: "block-hop", game: true,/);
    const rewritten = drive(['test-rust']);
    assert.deepEqual([rewritten.status, JSON.parse(rewritten.stdout.trim().split('\n').at(-1))], [0, { args: [real, '--test'], app: real }], rewritten.stderr);
    assert.match(readFileSync(resolve(dir, 'AGENTS.md'), 'utf8'), /^# Mine\n\n<!-- exact:begin[^]*an Exact game/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('a new app refuses a name no host crate can carry, and a directory that is not empty', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    assert.throws(() => createApp(resolve(parent, 'Field_Log')), /lowercase-hyphenated/);
    assert.throws(() => createApp(resolve(parent, 'field-web')), /no host suffix/);
    mkdirSync(resolve(parent, 'field-log'));
    writeFileSync(resolve(parent, 'field-log/stray'), '');
    assert.throws(() => createApp(resolve(parent, 'field-log')), /not empty/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('paths into the checkout are relative only when the two share a tree', () => {
  const root = resolve(import.meta.dir, '..');
  assert.equal(pathFrom(resolve(root, 'apps/caltrain'), resolve(root, 'vendor/taffy')), '../../vendor/taffy');
  // The temporary directory and the checkout share nothing below / here.
  assert.equal(pathFrom(tmpdir(), root), realpathSync(root));
});

test('a sibling app resolves every host dependency from its own manifest', () => {
  const root = resolve(import.meta.dir, '..');
  const dir = resolve(root, '..', `exact-new-${randomUUID()}`);
  try {
    // A cold Cargo cache defers the lock to the first build, which resolves it.
    if (/still exact2's/.test(createApp(dir))) return assert.deepEqual(readFileSync(resolve(dir, 'Cargo.lock')), readFileSync(resolve(root, 'Cargo.lock')));
    const result = spawnSync('cargo', ['metadata', '--offline', '--locked', '--no-deps', '--format-version', '1'], { cwd: dir, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const packages = JSON.parse(result.stdout).packages;
    assert.equal(packages.length, 3);
    const paths = { 'exact-logic': 'logic', 'exact-apple': 'host/apple', 'exact-linux': 'host/linux', 'exact-web': 'host/web', 'exact-web-capabilities': 'host/web-capabilities', 'exact-js': 'js', 'exact-js-web': 'js/web', 'exact-js-bake': 'js/bake' };
    for (const pkg of packages) for (const dep of pkg.dependencies) {
      assert.equal(realpathSync(dep.path), realpathSync(resolve(root, paths[dep.name])), `${pkg.name}: ${dep.name}`);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('offline, with none of exact2\'s crates in Cargo\'s cache, a new app keeps exact2\'s lock for its first build', () => {
  const root = resolve(import.meta.dir, '..'), parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  const saved = { CARGO_HOME: process.env.CARGO_HOME, CARGO_NET_OFFLINE: process.env.CARGO_NET_OFFLINE };
  Object.assign(process.env, { CARGO_HOME: resolve(parent, 'cargo-home'), CARGO_NET_OFFLINE: 'true' });
  try {
    assert.match(createApp(resolve(parent, 'field-log')), /Cargo\.lock is still exact2's/);
    assert.deepEqual(readFileSync(resolve(parent, 'field-log/Cargo.lock')), readFileSync(resolve(root, 'Cargo.lock')));
  } finally {
    for (const [key, value] of Object.entries(saved)) if (value === undefined) delete process.env[key]; else process.env[key] = value;
    rmSync(parent, { recursive: true, force: true });
  }
});

test('failed creation removes the half-written app', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  const dir = resolve(parent, 'field-log');
  try {
    const result = spawnSync(process.execPath, ['--eval', `import { createApp } from ${JSON.stringify(resolve(import.meta.dir, 'new.mjs'))}; createApp(${JSON.stringify(dir)});`], {
      env: { ...process.env, PATH: '', CARGO_HOME: resolve(parent, 'no-rustup') }, encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /cargo was not found/);
    assert.equal(existsSync(dir), false);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('a new app tells its agent where the guides are, and update keeps what the author added (LLP 1086 D1, D5)', () => {
  const root = realpathSync(resolve(import.meta.dir, '..')), parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const notes = readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    // The one required read; everything else is start-here's to name (LLP 1116 D3, LLP 1116.007 D2).
    assert.ok(notes.includes(resolve(root, 'docs/start-here.md')));
    assert.ok(existsSync(resolve(root, 'docs/start-here.md')));
    assert.match(notes, /the only\s+required reading/);
    // `help` is the pointer to the rest, and a verb's `--help` is for a flag in question: no tour of the verbs' help.
    assert.match(notes, /`bun exact\.mjs help` lists every verb/);
    assert.match(notes, /`--help` only when a flag is in question/);
    assert.doesNotMatch(notes, /Read\s+these|contract types|no-tells|\.exact\//);
    assert.ok(lstatSync(resolve(dir, 'CLAUDE.md')).isSymbolicLink());
    assert.equal(readFileSync(resolve(dir, 'CLAUDE.md'), 'utf8'), notes);
    const manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    assert.equal(realpathSync(resolve(dir, manifest.$schema)), resolve(root, 'scripts/app.schema.json'));
    readManifest(dir, 'field-log');
    // An author's notes around the block survive; the block is rewritten.
    writeFileSync(resolve(dir, 'AGENTS.md'), `# Mine\n\n${notes.replace('an Exact app', 'STALE')}\nKeep this.\n`);
    createApp(dir, { update: true });
    const updated = readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(updated, /^# Mine\n/);
    assert.match(updated, /Keep this\.\n$/);
    assert.ok(updated.includes('an Exact app') && !updated.includes('STALE'));
    // An app made before the notes existed gets both files.
    rmSync(resolve(dir, 'AGENTS.md')); rmSync(resolve(dir, 'CLAUDE.md'));
    createApp(dir, { update: true });
    assert.ok(existsSync(resolve(dir, 'AGENTS.md')) && existsSync(resolve(dir, 'CLAUDE.md')));
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

// LLP 1086 / LLP 1107: the scaffold writes a Linux host crate (the Android host is the same crate), with
// `linux` and `android` build verbs; an app made before it gains the crate on update.
test('a new app has a Linux host crate, and update adds one to an older app', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const linux = readFileSync(resolve(dir, 'linux/Cargo.toml'), 'utf8');
    assert.match(linux, /^name = "field-log-linux"$/m);
    assert.match(linux, /^exact-linux = \{ path = "[^"]*host\/linux" \}$/m);
    assert.match(readFileSync(resolve(dir, 'linux/src/main.rs'), 'utf8'), /exact_linux::run::<AppData>\(PLAN, COMPAT\)/);
    assert.match(readFileSync(resolve(dir, 'Cargo.toml'), 'utf8'), /^members = \["apple", "linux", "web"\]$/m);
    assert.match(readFileSync(resolve(dir, 'exact.mjs'), 'utf8'), /name: "field-log", exact2/);
    const verbs = appVerbs('field-log', dir);
    assert.deepEqual([verbs.linux, verbs.android], [['scripts/build-linux.mjs', 'field-log'], ['scripts/agent-android.mjs', 'build', 'field-log']]);
    // An older app: no linux/, and a workspace that does not list it.
    rmSync(resolve(dir, 'linux'), { recursive: true });
    const manifest = resolve(dir, 'Cargo.toml');
    writeFileSync(manifest, readFileSync(manifest, 'utf8').replace('members = ["apple", "linux", "web"]', 'members = ["apple", "web"]'));
    assert.match(createApp(dir, { update: true }), /a linux\/ host crate/);
    assert.ok(readFileSync(resolve(dir, 'linux/Cargo.toml'), 'utf8').includes('name = "field-log-linux"'));
    assert.match(readFileSync(manifest, 'utf8'), /^members = \["apple", "linux", "web"\]$/m);
    assert.doesNotMatch(createApp(dir, { update: true }), /linux\/ host crate/, 'a second update leaves it');
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

// Authoring bench (LLP 1087): an app copied into a folder of another name kept its `todo-list-*` crates,
// and `update` wrote an exact.mjs that asked Cargo for `todo-apple`.
test('update names the app by its crates, not its folder', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const made = resolve(parent, 'todo-list'), moved = resolve(parent, 'todo');
    createApp(made);
    spawnSync('mv', [made, moved]);
    // A hand-edited manifest: a comment after the package name.
    const web = resolve(moved, 'web/Cargo.toml');
    writeFileSync(web, readFileSync(web, 'utf8').replace(/^name = "todo-list-web"$/m, 'name = "todo-list-web" # formerly "old-web"'));
    createApp(moved, { update: true });
    const commands = readFileSync(resolve(moved, 'exact.mjs'), 'utf8');
    assert.match(commands, /name: "todo-list",/);
    assert.doesNotMatch(commands, /name: "todo",/);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});

// LLP 1116.004 D8: bake-off contacts, round 6, drove an earlier round's contacts.json: both rounds' apps were
// `com.example.contacts-exact2`, and a store is named by the app's id. A new app's id is its own; an app made
// before keeps the id it has, since its stores, bundle and update stream are named by it.
test('a new app\'s id is its own, and update keeps the id an older app has', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-id-'));
  try {
    const one = resolve(parent, 'r5/contacts'), two = resolve(parent, 'r6/contacts');
    createApp(one); createApp(two);
    const read = (dir, file) => readFileSync(resolve(dir, file), 'utf8');
    const ids = [one, two].map(dir => {
      const manifest = JSON.parse(read(dir, 'app.json'));
      assert.match(manifest.app.id, /^com\.example\.contacts-[0-9a-f]{6}$/);
      assert.equal(manifest.id, manifest.app.id, 'the web manifest names the same app');
      assert.ok(read(dir, 'app.ts').includes(`\nexport const appId = '${manifest.app.id}';\n`), 'app.ts names the id app.json does');
      assert.equal(readManifest(dir, 'contacts').app.id, manifest.app.id, 'the id is one the schema accepts');
      return manifest.app.id;
    });
    assert.notEqual(ids[0], ids[1]);
    // An app from before keeps `com.example.contacts` through update, in app.json and app.ts.
    for (const file of ['app.json', 'app.ts']) writeFileSync(resolve(one, file), read(one, file).replaceAll(ids[0], 'com.example.contacts'));
    const before = ['app.json', 'app.ts'].map(file => read(one, file));
    createApp(one, { update: true });
    assert.deepEqual(['app.json', 'app.ts'].map(file => read(one, file)), before);
    assert.equal(JSON.parse(before[0]).app.id, 'com.example.contacts');
    // And a pattern writes the id it has, taking the template's app.ts as untouched.
    assert.match(applyPattern(one, 'one-screen').said, /\(com\.example\.contacts\)$/);
    assert.ok(read(one, 'app.ts').includes("\nexport const appId = 'com.example.contacts';\n"));
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
}, 60_000); // Three offline Cargo resolutions.

// LLP 1116.001 D3: a pattern replaces the template's three files with the app's own id and name; a file the
// author edited is refused unless forced; rest brings its server, test runner and command, which go with it.
test('a pattern replaces the template, refuses an edited file unless forced, and takes its extras with it', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-pattern-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    assert.deepEqual(patterns().map(p => p.name), ['one-screen', 'list-detail', 'tabs', 'dashboard', 'rest']);
    for (const { line } of patterns()) assert.ok(line.length > 20 && line.length <= 92 && !line.startsWith('//'), line);
    const read = (file) => readFileSync(resolve(dir, file), 'utf8');
    // The app's own id (LLP 1116.004 D8), which the template's untouched app.ts carries.
    const id = JSON.parse(read('app.json')).app.id;
    assert.match(id, /^com\.example\.field-log-[0-9a-f]{6}$/);
    const { said } = applyPattern(dir, 'list-detail');
    assert.equal(said, `list-detail: wrote app.contract, app.ts, app.test.contract as Field Log (${id})`);
    assert.ok(read('app.ts').includes(`\nexport const appId = '${id}';\n`));
    assert.match(read('app.contract'), /^\/\/ Field Log: a list that pushes a detail/);
    assert.match(read('app.contract'), /text "Field Log" role="heading" aria-level=1/);
    assert.doesNotMatch(read('app.contract') + read('app.ts'), /Starter|com\.example\.starter/);
    // Untouched, it gives way to another pattern; edited, it is refused, named, and nothing is written.
    applyPattern(dir, 'tabs');
    writeFileSync(resolve(dir, 'app.ts'), `${read('app.ts')}// mine\n`);
    const before = read('app.contract');
    assert.throws(() => applyPattern(dir, 'dashboard'), /^Error: pattern: app\.ts was edited \(not what exact new or a pattern wrote\); --force replaces it$/);
    assert.equal(read('app.contract'), before);
    applyPattern(dir, 'dashboard', { force: true });
    assert.doesNotMatch(read('app.ts'), /\/\/ mine/);
    assert.throws(() => applyPattern(dir, 'nope'), /no pattern nope; there are one-screen, list-detail/);
    // rest: its server, which `test` and `verify` run (LLP 1116.007 D2), no command of its own; another pattern takes it away again.
    assert.match(applyPattern(dir, 'rest').next, /bun exact\.mjs test web$/);
    assert.ok(existsSync(resolve(dir, 'server.ts')) && !existsSync(resolve(dir, 'test.mjs')));
    assert.equal(JSON.parse(read('app.json')).commands, undefined);
    assert.equal(JSON.parse(read('app.json')).app.id, id);
    assert.match(applyPattern(dir, 'one-screen').said, /removed server\.ts$/);
    assert.ok(!existsSync(resolve(dir, 'server.ts')) && JSON.parse(read('app.json')).commands === undefined);
    // A name with a quote stays one Contract string.
    const quoted = patternFiles('one-screen', { id: 'com.example.q', title: 'Say "Hi" $5' }).files['app.contract'];
    assert.match(quoted, /text "Say \\"Hi\\" \\\$5" role="heading"/);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});
