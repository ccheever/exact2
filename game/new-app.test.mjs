import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { copyFileSync, cpSync, statSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, relative, resolve } from 'node:path';
import { outsideWorkspaceProblems, pathFrom, readManifest, resolveApp } from '../scripts/app.mjs';
import { bringPlatform, ensureShells, namePlatform, platformOf, shellCrates, withoutPlatformKeys } from '../scripts/app-shells.mjs';
import { authoredWorkspace } from '../scripts/cargo-check.mjs';
import { appVerbs } from '../scripts/app-cli.mjs';
import { applyPattern, authoredApp, createApp, createGame, patternFiles, patterns, rosterSeeds, wireSnapback, withoutMember } from './new.mjs';

/** An app that authored its crates, as `exact new` wrote them before LLP 1153 (or one that needs Rust of its own): the
 * generated workspace copied up beside app.json, each crate's bake path one level up, then `update` to settle its paths. */
function authorCrates(dir) {
  const { crates } = ensureShells(dir);
  const shells = resolve(dir, '.exact/shells');
  for (const entry of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', ...crates]) cpSync(resolve(shells, entry), resolve(dir, entry), { recursive: true });
  for (const kind of crates) {
    const script = resolve(dir, kind, 'build.rs');
    writeFileSync(script, readFileSync(script, 'utf8').replace('Path::new("../../..")', 'Path::new("..")'));
  }
  rmSync(shells, { recursive: true, force: true });
  createApp(dir, { update: true });
  return crates;
}

/** A fake exact2 at `sdk` runs the real app-cli.mjs that an app's exact.mjs hands its verb to (LLP 1116.006 D7), and the
 * verify.mjs whose server `test` runs for an app that has one (LLP 1116.007 D2). What verify.mjs imports from outside
 * scripts/ is linked at its path, so each resolves its own siblings from the checkout. */
const withCli = (sdk) => {
  const root = resolve(import.meta.dir, '..');
  mkdirSync(resolve(sdk, 'scripts'), { recursive: true });
  const copied = ['app-cli', 'verify', 'help', 'bun', 'port-lock'].map(file => `scripts/${file}.mjs`);
  for (const file of copied) copyFileSync(resolve(root, file), resolve(sdk, file));
  // A copy resolves its static relative imports beside itself: each is linked at its path, and a link resolves its own
  // from the checkout, so one level is enough. A dynamic import (app-cli.mjs's native-verb hook, scripts/app-shells.mjs)
  // is not linked, so the fixture stays the verbs' and the hook's own tests are below.
  const link = (target) => { const at = resolve(sdk, target); if (!existsSync(at)) { mkdirSync(resolve(at, '..'), { recursive: true }); symlinkSync(resolve(root, target), at); } };
  for (const file of copied) {
    for (const m of readFileSync(resolve(root, file), 'utf8').matchAll(/^(?:import|export) [^;]*? from '(\.{1,2}\/[^']+)'/gm)) link(relative(root, resolve(root, file, '..', m[1])));
  }
  // A bare `test` lists the app's authored tests through scripts/app.mjs.
  link('scripts/app.mjs');
  // The settings module an app's build imports for `import.meta.env` (LLP 1153 D4).
  mkdirSync(resolve(sdk, 'js/bake/src'), { recursive: true });
  copyFileSync(resolve(root, 'js/bake/src/__exact_env.mjs'), resolve(sdk, 'js/bake/src/__exact_env.mjs'));
};

test('a new outside app passes the checks every run makes, and a drifted one is told what to paste', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    // Eight files and no Rust (LLP 1153 D1): the platforms are app.json's keys, the web's alone at first.
    assert.deepEqual(readdirSync(dir).sort(), ['.gitignore', '.vscode', 'AGENTS.md', 'CLAUDE.md', 'app.contract', 'app.json', 'app.test.contract', 'app.ts', 'exact.mjs', 'package.json']);
    assert.ok(!authoredApp(dir));
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).host, { web: {} });
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).deploy, { store: { web: '0' } });
    assert.ok(existsSync(resolve(dir, 'app.test.contract')));
    // The diary is one clause that names `feedback status`, which prints its instructions (LLP 1116 D3), and
    // orders nothing first: builders load this file as instructions, so each order is a step (LLP 1116.012 D1).
    const agents = () => readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(agents(), /<!-- exact:begin[^]*`bun exact\.mjs feedback status` says whether this project keeps one\.[^]*<!-- exact:end -->/);
    assert.doesNotMatch(agents(), /first;|before writing code|Read the card|Run `bun exact\.mjs feedback/, 'no ordered preface');
    assert.doesNotMatch(agents(), /## The authoring diary|### Needed/);
    // 1.5 KB, read at step one and carried to the end (LLP 1116.007 D2), besides the checkout's path in each path it names
    const checkout = realpathSync(resolve(import.meta.dir, '..')), text = Buffer.byteLength(agents()) - (agents().split(checkout).length - 1) * checkout.length;
    assert.ok(text <= 1536, `AGENTS.md is ${text} bytes besides the checkout's paths`);
    assert.match(agents(), /exact\.mjs setup --check/);
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.exact\/$/m);
    // A setting's local value (`import.meta.env`, LLP 1153 D4) stays out of the repository.
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.env\.local$/m);
    // The runner is a few lines a builder can read (LLP 1116.006 D7): where the verbs are, and the checkout's app-cli.mjs.
    const launcher = readFileSync(resolve(dir, 'exact.mjs'), 'utf8');
    assert.ok(launcher.trimEnd().split('\n').length <= 25, `exact.mjs is ${launcher.split('\n').length} lines`);
    assert.match(launcher.split('\n')[2], /`bun exact\.mjs --help` lists the verbs/);
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
      // An iOS drive shows UIKit's bars, and an Apple agent drive's screenshots are a pixel a point.
      [['test', 'ios'], [...testArgs('ios'), '--chrome', 'platform'], ios],
      [['test', 'macos', '--size', '800x600'], [...testArgs('macos'), '--size', '800x600'], ['macos', '--app', 'field-log']],
      [['agent', 'web', 'tree', 'type title a title with spaces'], ['web', '--app', 'field-log', 'tree', 'type title a title with spaces'], webBuild],
      [['agent', 'ios', 'state'], ['ios', '--app', 'field-log', 'state', '--chrome', 'platform', '--scale', '1'], ios],
      [['agent', 'ios', '--sim', 'lane', '--scale', '3', 'tree'], ['ios', '--app', 'field-log', '--sim', 'lane', '--scale', '3', 'tree', '--chrome', 'platform'], ios],
      [['agent', 'ios', '--device', 'tree'], ['ios', '--app', 'field-log', '--device', 'tree', '--chrome', 'platform', '--scale', '1']],
      [['contract', 'build', 'app.contract', '--json'], ['contract', 'build', resolve(realpathSync(dir), 'app.contract'), '--json']],
      [['contract', 'types', 'app.contract', '-o', 'app.contract.d.ts'], ['contract', 'types', resolve(realpathSync(dir), 'app.contract'), '-o', resolve(realpathSync(dir), 'app.contract.d.ts')]],
      [['contract', 'vocab', 'background-color'], ['contract', 'vocab', 'background-color']],
    ]) {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), { args, app: realpathSync(dir), offline: '1' });
      assert.deepEqual(result.stderr.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line).args)[0] ?? null, built, `${command.join(' ')} builds first`);
    }
    // From inside the app, a path from its parent finds the same file; one found nowhere is named.
    const inside = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'contract', 'build', 'field-log/app.contract'], { cwd: dir, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    assert.deepEqual(JSON.parse(inside.stdout).args, ['contract', 'build', resolve(realpathSync(dir), 'app.contract')]);
    assert.match(inside.stderr, /field-log\/app\.contract is .*field-log\/app\.contract \(the app is /);
    const nowhere = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'contract', 'build', 'nope.contract'], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    assert.match(nowhere.stderr, /no nope\.contract in .* or in the app, /);
    assert.deepEqual(JSON.parse(nowhere.stdout).args, ['contract', 'build', 'nope.contract']);
    // App farm wave 1: `test --help` ran the suite, `web --help` started a server. Help builds nothing; a verb whose
    // script has more to say is asked for its usage alone, and the rest print their line.
    const helped = (...command) => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    const testHelp = helped('test', '--help');
    assert.equal(testHelp.status, 0, testHelp.stderr);
    assert.match(testHelp.stdout, /^bun exact\.mjs test /);
    assert.ok(!testHelp.stdout.includes('{'), 'test --help starts no child');
    for (const command of [['ios', '-h'], ['update', '--help']]) {
      const said = helped(...command);
      assert.equal(said.status, 0, said.stderr);
      assert.match(said.stdout, new RegExp(`^bun exact\\.mjs ${command[0]}(?: |\\n)`));
      assert.ok(!said.stdout.includes('{'), `${command[0]} --help ran its script`);
    }
    assert.match(helped('--help').stdout, /^bun exact\.mjs <verb>[^]*  test +/ );
    // chat F19, calendar F13: the files named, from the current directory (a glob too), each run in turn.
    mkdirSync(resolve(dir, 'tests'));
    for (const name of ['b.test.contract', 'a.test.contract']) writeFileSync(resolve(dir, 'tests', name), '');
    const files = (command, cwd = dir) => {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      return result.stdout.trim().split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l).args);
    };
    const tested = (host, file, ...more) => [host, '--app', 'field-log', '--test', resolve(realpathSync(dir), file), ...more];
    assert.deepEqual(files(['test', 'ios', 'tests/b.test.contract', '--size', '800x600']), [tested('ios', 'tests/b.test.contract', '--size', '800x600', '--chrome', 'platform')]);
    assert.deepEqual(files(['test', 'macos', 'tests/*.test.contract']), [tested('macos', 'tests/a.test.contract'), tested('macos', 'tests/b.test.contract')]);
    assert.deepEqual(files(['test', 'ios', '--test', 'field-log/tests/a.test.contract'], parent), [tested('ios', 'tests/a.test.contract', '--chrome', 'platform')]);
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
    // An app that authored its crates (one `exact new` wrote before LLP 1153) keeps update's Cargo path: a drifted
    // workspace is told what to paste, and a checkout that moved is followed.
    namePlatform(dir, 'ios');
    authorCrates(dir);
    assert.ok(authoredApp(dir));
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.deepEqual(readFileSync(resolve(dir, '.cargo/config.toml')), readFileSync(resolve(import.meta.dir, '../.cargo/config.toml')));
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
    assert.equal(readFileSync(resolve(dir, '.gitignore'), 'utf8'), '/target/\n/.exact/\n/.env.local\n');
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
    assert.match(exact('nope').stderr, /Usage: bun exact\.mjs <status\|web\|[^>]*\|seed>/);
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
    for (const file of ['game/app/shells.mjs', 'game/prove.mjs', 'host/web/build.mjs', 'scripts/agent.mjs', 'scripts/feedback.mjs', 'scripts/exact.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), 'console.log(JSON.stringify({args:process.argv.slice(2),app:process.env.EXACT_APP_DIR}));');
    }
    mkdirSync(resolve(sdk, 'snapback4/web'), { recursive: true });
    writeFileSync(resolve(sdk, 'snapback4/web/device-build.mjs'), 'export function devicePreflight() { throw new Error("a manifest-free game has no Snapback device"); }');
    withCli(sdk);
    const real = realpathSync(dir);
    const drive = (command) => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    for (const [command, args] of [
      [['test-rust'], [real, '--test']],
      [['prove', '--repin'], [real, '--repin']],
      [['web-build'], ['block-hop']],
      [['agent', 'web', 'tree'], ['web', '--app', 'block-hop', 'tree']],
      [['feedback', 'status'], ['status']],
      [['contract', 'build', 'app.contract'], ['contract', 'build', resolve(real, 'app.contract')]],
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
    // A folder named after the SDK would write exact-apple, exact-linux and exact-web beside exact2's own.
    assert.throws(() => createApp(resolve(parent, 'exact')), /the folder's name exact would make the crate exact-apple, which is one of exact2's own/);
    assert.throws(() => createApp(resolve(parent, 'exact-js')), /would make the crate exact-js-web/);
    assert.ok(!existsSync(resolve(parent, 'exact')) && !existsSync(resolve(parent, 'exact-js')), 'a refused name writes nothing');
    mkdirSync(resolve(parent, 'field-log'));
    writeFileSync(resolve(parent, 'field-log/stray'), '');
    assert.throws(() => createApp(resolve(parent, 'field-log')), /not empty/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('exact new names the app and its bundle id before any build, and refuses an id no platform takes', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    assert.throws(() => createApp(dir, { id: 'com.yourcompany.field_log' }), /^Error: exact new --bundle-id com\.yourcompany\.field_log: not a reverse-DNS id;.* as com\.yourcompany\.fieldlog$/);
    assert.throws(() => createApp(dir, { id: 'fieldlog' }), /not a reverse-DNS id/);
    assert.throws(() => createApp(dir, { title: 'Field\nLog' }), /--name: give the name people see, on one line/);
    assert.ok(!existsSync(dir), 'a refused name writes nothing');
    createApp(dir, { title: 'Field "Log" $5', id: 'com.yourcompany.fieldlog' });
    const manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    assert.deepEqual([manifest.app, manifest.name, manifest.short_name, manifest.id], [{ id: 'com.yourcompany.fieldlog', name: 'Field "Log" $5' }, 'Field "Log" $5', 'Field "Log" $5', 'com.yourcompany.fieldlog']);
    assert.match(readFileSync(resolve(dir, 'app.ts'), 'utf8'), /^export const appId = 'com\.yourcompany\.fieldlog';$/m);
    const view = readFileSync(resolve(dir, 'app.contract'), 'utf8');
    assert.match(view, /^component FieldLog$/m, 'the component keeps the folder\'s name, which is always an identifier');
    assert.ok(view.includes('text "Field \\"Log\\" \\$5" role="heading"'), view);
    // A pattern still sees the named app's files as exact new's, untouched.
    assert.match(applyPattern(dir, 'one-screen').said, /as Field "Log" \$5 \(com\.yourcompany\.fieldlog\)$/);
    // The flags name a new app: the CLI refuses them on --update and --game before touching anything.
    const cli = (...args) => spawnSync(process.execPath, [resolve(import.meta.dir, '../scripts/exact.mjs'), 'new', ...args], { encoding: 'utf8' });
    const update = cli(dir, '--update', '--bundle-id', 'com.yourcompany.other');
    assert.deepEqual([update.status, /^update: unknown flag --bundle-id/.test(update.stderr)], [2, true], update.stderr);
    const game = cli(resolve(parent, 'field-game'), '--game', '--bundle-id', 'com.yourcompany.other');
    assert.deepEqual([game.status, /--name and --bundle-id name an app; a game/.test(game.stderr)], [1, true], game.stderr);
    const bare = cli(dir, '--name');
    assert.deepEqual([bare.status, /--name takes a value, as in --name "Field Log"/.test(bare.stderr)], [1, true], bare.stderr);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('paths into the checkout are relative only when the two share a tree', () => {
  const root = resolve(import.meta.dir, '..');
  assert.equal(pathFrom(resolve(root, 'apps/caltrain'), resolve(root, 'vendor/taffy')), '../../vendor/taffy');
  // The temporary directory and the checkout share nothing below / here.
  assert.equal(pathFrom(tmpdir(), root), realpathSync(root));
});

test('a sibling app\'s generated workspace resolves every host dependency from exact2', () => {
  const root = resolve(import.meta.dir, '..');
  const dir = resolve(tmpdir(), `exact-new-${randomUUID()}`);
  try {
    createApp(dir, { with: ['ios', 'linux'] });
    const { workspace, crates } = ensureShells(dir);
    assert.deepEqual(crates, ['web', 'apple', 'linux']);
    const result = spawnSync('cargo', ['metadata', '--offline', '--locked', '--no-deps', '--format-version', '1'], { cwd: workspace, encoding: 'utf8' });
    // A cold Cargo cache keeps exact2's lock for the first build, which resolves it.
    if (result.status !== 0) return assert.deepEqual(readFileSync(resolve(workspace, 'Cargo.lock')), readFileSync(resolve(root, 'Cargo.lock')));
    const packages = JSON.parse(result.stdout).packages;
    assert.deepEqual(packages.map(p => p.name).sort(), [`${basename(dir)}-apple`, `${basename(dir)}-linux`, `${basename(dir)}-web`]);
    const paths = { 'exact-logic': 'logic', 'exact-apple': 'host/apple', 'exact-apple-update': 'host/apple-update', 'exact-linux': 'host/linux', 'exact-web': 'host/web', 'exact-web-capabilities': 'host/web-capabilities', 'exact-js': 'js', 'exact-js-web': 'js/web', 'exact-js-bake': 'js/bake' };
    for (const pkg of packages) for (const dep of pkg.dependencies) {
      assert.equal(realpathSync(dep.path), realpathSync(resolve(root, paths[dep.name])), `${pkg.name}: ${dep.name}`);
    }
    // Each crate's bake names the app, three directories up from the crate.
    for (const kind of crates) assert.match(readFileSync(resolve(workspace, kind, 'build.rs'), 'utf8'), /Path::new\("\.\.\/\.\.\/\.\."\)/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('offline, with none of exact2\'s crates in Cargo\'s cache, the generated workspace keeps exact2\'s lock for its first build', () => {
  const root = resolve(import.meta.dir, '..'), parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  const saved = { CARGO_HOME: process.env.CARGO_HOME, CARGO_NET_OFFLINE: process.env.CARGO_NET_OFFLINE };
  Object.assign(process.env, { CARGO_HOME: resolve(parent, 'cargo-home'), CARGO_NET_OFFLINE: 'true' });
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir, { with: ['ios'] });
    const { workspace } = ensureShells(dir);
    assert.deepEqual(readFileSync(resolve(workspace, 'Cargo.lock')), readFileSync(resolve(root, 'Cargo.lock')));
  } finally {
    for (const [key, value] of Object.entries(saved)) if (value === undefined) delete process.env[key]; else process.env[key] = value;
    rmSync(parent, { recursive: true, force: true });
  }
});

test('failed creation removes the half-written app', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  const dir = resolve(parent, 'field-log');
  try {
    assert.throws(() => createApp(dir, { with: ['nope'] }), /--with nope: the platforms are ios, mac, linux, android, windows/);
    assert.equal(existsSync(dir), false);
    // An empty directory the author made stays theirs: emptied, not removed.
    mkdirSync(dir);
    assert.throws(() => createApp(dir, { with: ['nope'] }), /--with nope/);
    assert.deepEqual([existsSync(dir), readdirSync(dir)], [true, []]);
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
    // Where to start, not an order of steps (LLP 1116.012 D1): the nearest pattern, the guide, a name in question
    // answered by the card, `verify` when the features work and `verify ios` once at the end; `help` lists the verbs.
    assert.match(notes, /already in your instructions\. Start from the nearest pattern: `bun exact\.mjs\s+pattern` lists them and `pattern <name>` writes one/);
    assert.match(notes, /`bun exact\.mjs contract vocab <name>` answers a\s+name/);
    assert.match(notes, /`bun exact\.mjs docs <heading words>` a guide's\s+section/);
    assert.match(notes, /`bun exact\.mjs verify`\s+when the features work, and `verify ios\|android` at the end for a\s+phone app/);
    assert.match(notes, /`bun exact\.mjs help`\s+lists the verbs/);
    assert.doesNotMatch(notes, /after each edit|milestone/);
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
test('a native verb names its platform, and the generated workspace follows app.json (LLP 1153 D1)', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const manifest = () => JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    assert.deepEqual([platformOf('ios'), platformOf('mac'), platformOf('test', 'ios'), platformOf('agent', 'macos'), platformOf('web'), platformOf('test', 'web')], ['ios', 'macos', 'ios', 'macos', null, null]);
    assert.deepEqual(namePlatform(dir, 'ios'), ['host.ios', 'deploy.store.ios']);
    assert.deepEqual(namePlatform(dir, 'ios'), [], 'named once');
    assert.deepEqual(manifest().host.ios, { minimumOS: '17.0', deviceFamily: ['iphone', 'ipad'] });
    assert.deepEqual(namePlatform(dir, 'android'), ['host.android'], 'no store for Android');
    // Windows's host refuses any store level but "0", and an absent key reads as "A" (LLP 1115.004 D15).
    assert.deepEqual(namePlatform(dir, 'windows'), ['host.windows', 'deploy.store.windows']);
    assert.deepEqual([manifest().host.windows.painter, manifest().host.windows.window.width, manifest().deploy.store.windows], ['ink', 1024, '0']);
    const windows = ensureShells(dir).workspace;
    assert.match(readFileSync(resolve(windows, 'windows/src/main.rs'), 'utf8'), /exact_windows::run_composed::<AppData>\("Field Log", window\(\), PLAN, COMPAT, composed\)/);
    assert.match(createApp(dir, { update: true, without: 'windows' }), /without windows \(removed host\.windows, deploy\.store\.windows\)/);
    assert.deepEqual(shellCrates(manifest()), ['web', 'apple', 'linux', 'android']);
    readManifest(dir, 'field-log');
    const first = ensureShells(dir);
    assert.deepEqual([first.wrote, first.crates, ensureShells(dir).wrote], [true, ['web', 'apple', 'linux', 'android'], false]);
    assert.match(readFileSync(resolve(first.workspace, 'Cargo.toml'), 'utf8'), /^members = \["web", "apple", "linux", "android"\]$/m);
    assert.match(readFileSync(resolve(first.workspace, 'linux/Cargo.toml'), 'utf8'), /^name = "field-log-linux"$/m);
    // Android's library for the Canvas reader (LLP 1107): a cdylib, empty off Android, baked by `build_android`.
    const android = readFileSync(resolve(first.workspace, 'android/Cargo.toml'), 'utf8');
    assert.match(android, /^name = "field-log-android"$/m);
    assert.match(android, /^crate-type = \["cdylib"\]$/m);
    assert.match(android, /^\[target\.'cfg\(target_os = "android"\)'\.dependencies\]\nexact-logic = /m);
    assert.match(readFileSync(resolve(first.workspace, 'android/build.rs'), 'utf8'), /exact_js_bake::build_android\(std::path::Path::new\("\.\.\/\.\.\/\.\."\)\)/);
    assert.match(readFileSync(resolve(first.workspace, 'android/src/lib.rs'), 'utf8'), /exact_linux::canvas_jni!\(super::AppData, hatches = crate::android::exact_hatches\(\)\);/);
    assert.ok(!existsSync(resolve(dir, 'Cargo.toml')) && !existsSync(resolve(dir, 'linux')), 'nothing Rust in the author\'s tree');
    // resolveApp takes the generated workspace, with the target at the app, when nothing authored one.
    const was = process.env.EXACT_APP_DIR;
    process.env.EXACT_APP_DIR = dir;
    try {
      const app = resolveApp();
      assert.deepEqual([app.generated, app.workspace, app.target, app.crate('linux'), app.cargoPackage('linux')?.name], [true, realpathSync(first.workspace), resolve(realpathSync(dir), 'target'), 'field-log-linux', 'field-log-linux']);
    } finally { if (was === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = was; }
    // A platform dropped takes its crate with it; the last native one may go (deploy reads none as none, D1.5).
    assert.match(createApp(dir, { update: true, without: 'android' }), /without android \(removed host\.android\)/);
    assert.deepEqual(ensureShells(dir).crates, ['web', 'apple']);
    assert.ok(!existsSync(resolve(first.workspace, 'linux')) && !existsSync(resolve(first.workspace, 'android')));
    assert.match(createApp(dir, { update: true, without: 'ios' }), /without ios \(removed host\.ios, deploy\.store\.ios\)/);
    assert.deepEqual([manifest().host, manifest().deploy.store, ensureShells(dir).crates], [{ web: {} }, { web: '0' }, ['web']]);
    assert.match(createApp(dir, { update: true, without: 'ios' }), /without ios \(it shipped none already\)/);
    assert.throws(() => createApp(dir, { update: true, without: 'web' }), /the web host is the dev loop's/);
    assert.throws(() => withoutPlatformKeys(dir, 'tv'), /the platforms are/);
    assert.match(createApp(dir, { update: true, with: ['linux', 'macos'] }), /with linux \(added host\.linux, deploy\.store\.linux\), with macos \(added host\.macos, deploy\.store\.macos\)/);
    assert.deepEqual(ensureShells(dir).crates, ['web', 'linux', 'apple']);
    const verbs = appVerbs('field-log', dir);
    assert.deepEqual([verbs.linux, verbs.android, verbs.windows], [['scripts/build-linux.mjs', 'field-log'], ['scripts/agent-android.mjs', 'build', 'field-log', '--apk', '--install'], ['host/windows/build.mjs', 'field-log']]);
    // --with at creation names them before any verb runs; `mac` is the CLI's spelling for macos (scripts/exact.mjs).
    const named = resolve(parent, 'named');
    createApp(named, { with: ['ios', 'macos'] });
    assert.deepEqual(Object.keys(JSON.parse(readFileSync(resolve(named, 'app.json'), 'utf8')).host), ['web', 'ios', 'macos']);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

// LLP 1086 / LLP 1107: an app that authored its crates before the scaffold wrote a Linux host gains one on update,
// and `update --without linux` takes it away again with its workspace member and lock entries.
test('an authored app gains a Linux crate on update and loses it with --without linux', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir, { with: ['ios'] });
    assert.deepEqual(authorCrates(dir), ['web', 'apple']);
    assert.match(readFileSync(resolve(dir, 'Cargo.toml'), 'utf8'), /^members = \["web", "apple"\]$/m);
    const manifest = resolve(dir, 'Cargo.toml');
    assert.doesNotMatch(createApp(dir, { update: true }), /linux\/ host crate/, 'no store, no crate');
    namePlatform(dir, 'linux');
    assert.match(createApp(dir, { update: true }), /a linux\/ host crate/);
    const linux = readFileSync(resolve(dir, 'linux/Cargo.toml'), 'utf8');
    assert.match(linux, /^name = "field-log-linux"$/m);
    assert.match(linux, /^exact-linux = \{ path = "[^"]*host\/linux" \}$/m);
    assert.match(readFileSync(resolve(dir, 'linux/build.rs'), 'utf8'), /Path::new\("\.\."\)/);
    assert.match(readFileSync(manifest, 'utf8'), /^members = \["apple", "linux", "web"\]$/m);
    assert.doesNotMatch(createApp(dir, { update: true }), /linux\/ host crate/, 'a second update leaves it');
    assert.match(createApp(dir, { update: true, without: 'linux' }), /without linux \(removed its workspace member, app\.json host\.linux, app\.json deploy\.store\.linux, linux\/\)/);
    assert.ok(!existsSync(resolve(dir, 'linux')));
    assert.match(readFileSync(manifest, 'utf8'), /^members = \["apple", "web"\]$/m);
    assert.ok(!('linux' in JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).deploy.store));
    assert.doesNotMatch(readFileSync(resolve(dir, 'Cargo.lock'), 'utf8'), /name = "field-log-linux"/);
    const locked = spawnSync('cargo', ['metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'], { cwd: dir, encoding: 'utf8' });
    assert.equal(locked.status, 0, locked.stderr);
    assert.doesNotMatch(createApp(dir, { update: true }), /linux\/ host crate/);
    // Another platform's keys go; the Apple crate iOS shares with macOS stays.
    assert.match(createApp(dir, { update: true, without: 'ios' }), /without ios \(removed host\.ios, deploy\.store\.ios\)/);
    assert.ok(existsSync(resolve(dir, 'apple/Cargo.toml')));
    assert.throws(() => createApp(dir, { update: true, without: 'web' }), /the web host is the dev loop's/);
    // Android alone brings the crate it shares with Linux; removing Android last takes the crate with it.
    const withAndroid = createApp(dir, { update: true, with: ['android'] });
    assert.match(withAndroid, /a linux\/ host crate/);
    assert.match(withAndroid, /with android \(added host\.android\)/);
    assert.ok(existsSync(resolve(dir, 'linux/Cargo.toml')));
    assert.match(createApp(dir, { update: true, without: 'android' }), /without android \(removed its workspace member, app\.json host\.android, android\/\); without linux \(removed its workspace member, linux\/\)/);
    assert.ok(!existsSync(resolve(dir, 'linux')));
    assert.ok(!('android' in JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).host));
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

test('update --without edits any spelling of the workspace member, and a refusal changes nothing', () => {
  for (const [text, want] of [
    ['[workspace]\nmembers = [\'apple\', \'linux\', \'web\']\n', '[workspace]\nmembers = [\'apple\', \'web\']\n'],
    ['[workspace]\nmembers = ["apple", "./linux/", "web"]\nresolver = "2"\n', '[workspace]\nmembers = ["apple", "web"]\nresolver = "2"\n'],
    ['[workspace]\nmembers = [\n  "apple", # the Apple host\n  "linux", # Linux, and Android\n  "web",\n]\n', '[workspace]\nmembers = [\n  "apple", # the Apple host\n  "web",\n]\n'],
    ['[workspace]\nmembers = ["apple", "web", "linux"]\ndefault-members = ["linux", "web"]\n', '[workspace]\nmembers = ["apple", "web"]\ndefault-members = ["web"]\n'],
    ['[workspace]\nmembers = ["apple", "web"]\n', '[workspace]\nmembers = ["apple", "web"]\n'],
  ]) assert.equal(withoutMember(text, 'linux'), want, text);
  assert.equal(withoutMember('[workspace]\nmembers = ["linux-tools", "linux"]\n', 'linux'), '[workspace]\nmembers = ["linux-tools"]\n');
  // Only the [workspace] table's own keys: another table's `members` is left as written.
  const metadata = '[workspace.metadata.hosts]\nmembers = ["linux"]\n\n[workspace]\nmembers = ["apple", "linux"]\n\n[package.metadata]\nmembers = ["linux"]\n';
  assert.equal(withoutMember(metadata, 'linux'), metadata.replace('members = ["apple", "linux"]', 'members = ["apple"]'));
  // A form it does not read with certainty is refused, naming the edit by hand.
  for (const [text, why] of [
    ['[workspace]\n"members" = ["apple", "linux"]\n', /a quoted members key/],
    ['[workspace]\nmembers = ["apple", "li\\u006eux"]\n', /an escaped string/],
    ['workspace.members = ["apple", "linux"]\n', /no \[workspace\] header/],
    ['workspace = { members = ["apple", "linux"] }\n', /no \[workspace\] header/],
    ['[workspace]\nmembers = ["apple"]\ndefault-members = ["linux"]\n', /default-members would be empty/],
  ]) assert.throws(() => withoutMember(text, 'linux'), err => why.test(err.message) && /remove "linux" from \[workspace\] members and default-members by hand/.test(err.message), text);
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'web-log');
    createApp(dir, { with: ['ios', 'linux'] });
    authorCrates(dir);
    const manifest = resolve(dir, 'app.json'), workspace = resolve(dir, 'Cargo.toml'), lock = resolve(dir, 'Cargo.lock');
    const state = () => [manifest, workspace, lock].map(p => readFileSync(p, 'utf8'));
    // A resolve that fails once the crate is aside (a member depends on it): the files, the
    // lock and the crate all come back, and nothing is left beside them.
    const apple = resolve(dir, 'apple/Cargo.toml');
    writeFileSync(apple, readFileSync(apple, 'utf8').replace('[dependencies]\n', '[dependencies]\nweb-log-linux = { path = "../linux" }\n'));
    const again = state();
    assert.throws(() => createApp(dir, { update: true, without: 'linux' }), /Cargo\.toml, app\.json, Cargo\.lock and linux\/ are as they were/);
    assert.deepEqual(state(), again);
    assert.ok(existsSync(resolve(dir, 'linux/Cargo.toml')));
    assert.deepEqual(readdirSync(dir).filter(f => f.startsWith('.linux')), []);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

// Authoring bench (LLP 1087): an app copied into a folder of another name kept its `todo-list-*` crates,
// and `update` wrote an exact.mjs that asked Cargo for `todo-apple`.
test('update names an authored app by its crates, and a generated one by its folder', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const made = resolve(parent, 'todo-list'), moved = resolve(parent, 'todo');
    createApp(made, { with: ['ios'] });
    authorCrates(made);
    spawnSync('mv', [made, moved]);
    // A hand-edited manifest: a comment after the package name.
    const web = resolve(moved, 'web/Cargo.toml');
    writeFileSync(web, readFileSync(web, 'utf8').replace(/^name = "todo-list-web"$/m, 'name = "todo-list-web" # formerly "old-web"'));
    createApp(moved, { update: true });
    const commands = readFileSync(resolve(moved, 'exact.mjs'), 'utf8');
    assert.match(commands, /name: "todo-list",/);
    assert.doesNotMatch(commands, /name: "todo",/);
    // A generated workspace is named by the folder, and follows a rename.
    const plain = resolve(parent, 'note-list'), renamed = resolve(parent, 'notes');
    createApp(plain, { with: ['ios'] });
    assert.match(readFileSync(resolve(ensureShells(plain).workspace, 'apple/Cargo.toml'), 'utf8'), /^name = "note-list-apple"$/m);
    spawnSync('mv', [plain, renamed]);
    createApp(renamed, { update: true });
    assert.match(readFileSync(resolve(renamed, 'exact.mjs'), 'utf8'), /name: "notes",/);
    assert.match(readFileSync(resolve(ensureShells(renamed).workspace, 'apple/Cargo.toml'), 'utf8'), /^name = "notes-apple"$/m);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
}, 60_000);

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

// `exact new --snapback` (the author's ruling of 2026-10-12, via the sb4 app farm): the README's setup block, run. The
// backend is the directory above; the device build is `bun exact.mjs snapback`'s, skipped here (`build: false`).
test('a new Snapback app is wired to the backend above it, and update follows a moved checkout\'s mount', () => {
  const root = realpathSync(resolve(import.meta.dir, '..')), parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'app'), manifest = () => JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    assert.throws(() => createApp(dir, { snapback: { port: 'x' } }), /--port: x is not a port/);
    const said = createApp(dir, { snapback: { port: 4410, build: false } });
    // No backend server installed above: the schema is written, nothing generated, and it says what to do next.
    assert.match(readFileSync(resolve(parent, 'snapback/schema.q'), 'utf8'), /^use personas alice, bob[^]*mutation post\(/);
    assert.match(said, /not installed in .*npm i snapback4@\d+\.\d+\.\d+[^]*bunx snapback4 dev --port 4410/);
    assert.equal(realpathSync(resolve(dir, manifest().typescript.sources.snapback4)), resolve(root, 'snapback4/ts'));
    assert.ok(!manifest().typescript.sources.snapback4.startsWith('/'), 'the native bake refuses an absolute mount');
    assert.equal(manifest().typescript.sources.backend, undefined);
    readManifest(dir, 'app');
    // The data module's origin, its grant and the tests' fetch prefixes name the port; its appId is app.json's.
    const ts = readFileSync(resolve(dir, 'app.ts'), 'utf8'), tests = readFileSync(resolve(dir, 'app.test.contract'), 'utf8');
    assert.match(ts, /const origin = 'http:\/\/127\.0\.0\.1:4410';[^]*net\.fetch \$\{origin\}\\nsqlite\.open app:\/data/);
    assert.ok(ts.includes(`export const appId = '${manifest().app.id}'`));
    assert.match(tests, /^fresh\nepoch now$[^]*fail fetch "backend"/m);
    const view = readFileSync(resolve(dir, 'app.contract'), 'utf8');
    assert.match(view, /failure\(board\)[^]*f\.reason/);
    assert.match(view, /resource board = notes\(persona, time.epochAtZero \+ sampledAt\)/);
    assert.match(view, /action refreshBoard\n    sampledAt = performanceNow\(\)\n    refresh board/);
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/assets\/snapback4\.wasm$/m);
    assert.match(readFileSync(resolve(dir, 'AGENTS.md'), 'utf8'), /this app's data is Snapback 4's[^]*bun exact\.mjs snapback/);
    // With the server installed above, `snapback` generates the operations once and mounts them; a schema stays the author's.
    writeFileSync(resolve(parent, 'snapback/schema.q'), 'use personas carol\n');
    mkdirSync(resolve(parent, 'node_modules/.bin'), { recursive: true });
    writeFileSync(resolve(parent, 'node_modules/.bin/snapback4'), '#!/bin/sh\n[ "$1" = check ] && mkdir -p snapback/generated && echo "export const api = {};" > snapback/generated/api.ts\n');
    mkdirSync(resolve(parent, 'node_modules/snapback4/bin'), {recursive: true});
    writeFileSync(resolve(parent, 'node_modules/snapback4/bin/snapback4.js'), `import {mkdirSync, writeFileSync} from 'node:fs';
mkdirSync('snapback/generated', {recursive: true});
writeFileSync('snapback/generated/api.ts', 'export const api = {};');`);
    assert.match(wireSnapback(dir, { build: false }), /snapback4 check: wrote/);
    assert.equal(manifest().typescript.sources.backend, '../snapback/generated');
    assert.equal(readFileSync(resolve(parent, 'snapback/schema.q'), 'utf8'), 'use personas carol\n');
    writeFileSync(resolve(dir, 'app.ts'), readFileSync(resolve(dir, 'app.ts'), 'utf8') + '\n// Another service: net.fetch http://127.0.0.1:44100\n');
    mkdirSync(resolve(dir, 'tests/nested'), { recursive: true });
    writeFileSync(resolve(dir, 'tests/nested/offline.test.contract'), 'fail fetch "http://127.0.0.1:4410/sync"\n');
    const changedPort = wireSnapback(dir, { build: false, port: 4411 });
    assert.match(changedPort, /dev --port 4411/);
    assert.match(changedPort, /app.ts's origin constant is overridden at bake by .exact\/backend-origin/);
    assert.match(readFileSync(resolve(dir, 'app.ts'), 'utf8'), /const origin = 'http:\/\/127\.0\.0\.1:4410'/);
    assert.match(readFileSync(resolve(dir, 'app.test.contract'), 'utf8'), /fail fetch "backend"/);
    assert.match(readFileSync(resolve(dir, 'tests/nested/offline.test.contract'), 'utf8'), /4410\/sync/);
    assert.ok(readFileSync(resolve(dir, 'app.ts'), 'utf8').includes('net.fetch http://127.0.0.1:44100'));
    const portSource = resolve(dir, 'app.ts');
    writeFileSync(portSource, readFileSync(portSource, 'utf8').replace("const origin = 'http://127.0.0.1:4411'", 'const origin = "http://127.0.0.1:4411/"'));
    const unchanged = readFileSync(portSource, 'utf8'), mtime = statSync(portSource).mtimeMs;
    wireSnapback(dir, {build: false, port: 4411});
    assert.equal(readFileSync(portSource, 'utf8'), unchanged);
    assert.equal(statSync(portSource).mtimeMs, mtime);
    wireSnapback(dir, {build: false, port: 4412});
    assert.equal(readFileSync(portSource, 'utf8'), unchanged);
    assert.match(readFileSync(resolve(dir, 'app.test.contract'), 'utf8'), /fail fetch "backend"/);
    assert.match(readFileSync(resolve(dir, 'tests/nested/offline.test.contract'), 'utf8'), /4410\/sync/);
    assert.equal(readFileSync(resolve(dir,'.exact/backend-origin'),'utf8').trim(), 'http://127.0.0.1:4412');
    assert.throws(() => wireSnapback(dir, { build: false, port: 'x' }), /is not a port/);
    const custom = resolve(parent, 'custom');
    assert.match(createApp(custom, { snapback: { build: false, port: 4412 } }), /backend already has a schema: kept the greeting app[^]*dev --port 4412/);
    assert.match(readFileSync(resolve(custom, 'app.ts'), 'utf8'), /http:\/\/127\.0\.0\.1:4412/);
    writeFileSync(resolve(custom, 'app.ts'), readFileSync(resolve(custom, 'app.ts'), 'utf8').replace('http://127.0.0.1:4412', 'https://api.example.com'));
    assert.throws(() => wireSnapback(custom, { build: false, port: 4413 }), /not a loopback backend/);
    writeFileSync(resolve(custom, 'app.ts'), "export const grants = '';\n");
    assert.match(wireSnapback(custom, { build: false, port: 4413 }), /added http:\/\/127\.0\.0\.1:4413/);
    assert.match(readFileSync(resolve(custom, 'app.ts'), 'utf8'), /const origin = 'http:\/\/127\.0\.0\.1:4413';[^]*net.fetch \$\{origin\}/);
    writeFileSync(resolve(custom, 'app.ts'), "const origin = process.env.BACKEND;\nexport const grants = '';\n");
    assert.throws(() => wireSnapback(custom, { build: false, port: 4413 }), /origin is not a literal URL/);
    assert.ok(!readFileSync(resolve(custom, 'app.ts'), 'utf8').includes('api.post'));
    assert.ok(!readFileSync(resolve(custom, 'app.contract'), 'utf8').includes('Guestbook'));
    const online = resolve(parent, 'online');
    createApp(online, { snapback: { online: true, build: false, port: 4414 } });
    assert.match(wireSnapback(online, { build: false }), /Online-only adapter/);
    assert.ok(!readFileSync(resolve(online, 'app.ts'), 'utf8').includes('sqlite.open'));
    const plainOnline = resolve(parent, 'plain-online');
    createApp(plainOnline);
    wireSnapback(plainOnline, {build: false, online: true, port: 4414});
    assert.ok(!readFileSync(resolve(plainOnline, 'app.ts'), 'utf8').includes('sqlite.open'));
    const rebuilt = wireSnapback(plainOnline, {build:false,rebuild:true});
    assert.equal(rebuilt.split('\n').length, 1);
    assert.match(rebuilt, /^Snapback rebuilt: \.\/snapback4 → .*snapback4\/online; \.\/backend → \.\.\/snapback\/generated \(online\)\.$/);
    const onlineManifest = JSON.parse(readFileSync(resolve(online, 'app.json'), 'utf8'));
    onlineManifest.typescript.sources.snapback4 = '..\\moved\\snapback4\\online';
    writeFileSync(resolve(online, 'app.json'), JSON.stringify(onlineManifest));
    createApp(online, { update: true });
    assert.equal(realpathSync(resolve(online, JSON.parse(readFileSync(resolve(online, 'app.json'), 'utf8')).typescript.sources.snapback4)), resolve(root, 'snapback4/online'));
    assert.match(readFileSync(resolve(online, 'AGENTS.md'), 'utf8'), /server-only Snapback adapter/);
    assert.equal(existsSync(resolve(online, 'assets/snapback4.wasm')), false);
    assert.equal(readFileSync(resolve(parent, 'snapback/schema.q'), 'utf8'), 'use personas carol\n');
    // The app's exact.mjs runs it, with the app's directory.
    const sdk = resolve(parent, 'sdk');
    mkdirSync(resolve(sdk, 'scripts'), { recursive: true });
    writeFileSync(resolve(sdk, 'scripts/exact.mjs'), 'console.log(JSON.stringify(process.argv.slice(2)));');
    withCli(sdk);
    const verb = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'snapback'], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
    assert.deepEqual(JSON.parse(verb.stdout), ['snapback', realpathSync(dir)]);
    // An appId that disagrees is named. A checkout that moved: update rewrites the mount, as it does the Cargo paths.
    writeFileSync(resolve(dir, 'app.ts'), ts.replace(/export const appId = '[^']*'/, "export const appId = 'com.example.other'"));
    assert.ok(wireSnapback(dir, { build: false }).includes(`app.ts must say: export const appId = "${manifest().app.id}"; (it says "com.example.other")`));
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...manifest(), typescript: { sources: { ...manifest().typescript.sources, snapback4: '../../moved/exact2/snapback4/ts' } } }, null, 2));
    const beforeMountRefusal = readFileSync(resolve(dir, 'app.ts'), 'utf8');
    assert.throws(() => wireSnapback(dir, { build: false, port: 9999 }), /refusing to rewrite an edited mount[^]*- snapback4:[^]*\+ snapback4:/);
    assert.equal(readFileSync(resolve(dir, 'app.ts'), 'utf8'), beforeMountRefusal);
    const beforeArtifact = ['app.json', 'app.ts', 'app.test.contract', 'AGENTS.md', '.gitignore'].map(file => readFileSync(resolve(dir, file), 'utf8'));
    assert.match(wireSnapback(dir, {build: false, artifactOnly: true}), /mounts kept unchanged/);
    assert.deepEqual(['app.json', 'app.ts', 'app.test.contract', 'AGENTS.md', '.gitignore'].map(file => readFileSync(resolve(dir, file), 'utf8')), beforeArtifact);
    assert.throws(() => wireSnapback(dir, {build: false, artifactOnly: true, port: 4410}), /cannot also change/);
    createApp(dir, { update: true });
    assert.equal(realpathSync(resolve(dir, manifest().typescript.sources.snapback4)), resolve(root, 'snapback4/ts'));
    // A plain app's notes say how to wire one.
    const plain = resolve(parent, 'plain');
    createApp(plain);
    assert.match(readFileSync(resolve(plain, 'AGENTS.md'), 'utf8'), /exact new <path> --snapback[^]*bun exact\.mjs snapback/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 90_000); // Three offline Cargo resolutions.


test('the online scaffold calls existing generated query/mutation handles and preserves authored apps', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-online-'));
  try {
    mkdirSync(resolve(parent, 'snapback/generated'), {recursive: true});
    writeFileSync(resolve(parent, 'snapback/schema.q'), 'use personas carol\n');
    writeFileSync(resolve(parent, 'snapback/generated/api.ts'), `export const api: Ops = {
  search: { kind: "query", name: "search" },
  create: { kind: "mutation", name: "create" },
};`);
    const dir = resolve(parent, 'explorer');
    createApp(dir, {snapback: {online: true, build: false, port: 30126}});
    const source = readFileSync(resolve(dir, 'app.ts'), 'utf8');
    assert.match(source, /db.read\(api.search, args\)/);
    assert.match(source, /db.send\(api.create, args\)/);
    assert.match(source, /ArgsOf<typeof api.create>/);
    assert.match(source, /'x-snapback-persona': "carol"/);
    assert.ok(!source.includes('sqlite.open'));
    const changed = source + '\n// authored fields follow\n';
    writeFileSync(resolve(dir, 'app.ts'), changed);
    wireSnapback(dir, {online: true, build: false});
    assert.equal(readFileSync(resolve(dir, 'app.ts'), 'utf8'), changed);
    const edited = resolve(parent, 'edited'); createApp(edited);
    const custom = readFileSync(resolve(edited, 'app.ts'), 'utf8') + '\n// my app source\n';
    writeFileSync(resolve(edited, 'app.ts'), custom);
    wireSnapback(edited, {online: true, build: false});
    assert.equal(readFileSync(resolve(edited, 'app.ts'), 'utf8'), custom);
    const plain = resolve(parent, 'plain'); createApp(plain);
    wireSnapback(plain, {online: true, build: false, port: 30126});
    assert.match(readFileSync(resolve(plain, 'app.ts'), 'utf8'), /db.send\(api.create, args\)/);
  } finally {rmSync(parent, {recursive: true, force: true});}
});

test('roster scaffolds seed fresh from a test-relative file and preserve backend fixtures', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-roster-'));
  const schema = `use personas alice, bob
table owners:
  person: principal
  role: enum('owner', 'reader')
  label: text 2..80
  enabled: bool
  insert <- deny
  update <- deny
`;
  try {
    mkdirSync(resolve(parent, 'snapback'), {recursive: true});
    writeFileSync(resolve(parent, 'snapback/schema.q'), schema);
    const sample = rosterSeeds(schema);
    assert.match(sample[0].row.id, /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/);
    assert.deepEqual(sample, [{table:'owners', row:{id:sample[0].row.id,person:'dev:alice',role:'owner',label:'example',enabled:true}}]);
    assert.deepEqual(rosterSeeds(schema), sample);
    assert.deepEqual(rosterSeeds(schema.replace('  insert <- deny\n', '')), sample, 'a missing insert rule denies inserts too');
    assert.deepEqual(rosterSeeds(schema.replace('  insert <- deny', '  insert <- .person = viewer')), [], 'a writable table needs no roster seed');
    const indented = schema.replace(/^  /gm, '    ').replace(/    update <- deny\n$/, '').trimEnd();
    assert.ok(indented.endsWith('    insert <- deny'));
    assert.deepEqual(rosterSeeds(indented), sample, 'four-space fields and an EOF deny rule preserve all required fields');
    assert.deepEqual(rosterSeeds(indented.replaceAll('\n', '\r\n')), sample, 'CRLF does not change the seeded shape');
    assert.deepEqual(rosterSeeds(indented.replace('    person:', '-- roster membership\n    person:') + ' -- seeded only'), sample, 'comments do not terminate a table or hide its deny rule');
    assert.deepEqual(rosterSeeds(indented.replace('table owners:', 'table owners: -- roster')), sample);
    writeFileSync(resolve(parent, 'snapback/schema.q'), indented);
    const app = resolve(parent, 'roster');
    const said = createApp(app, {snapback:{online:true,build:false}});
    assert.match(said, /review the persona, role and field values/);
    assert.match(readFileSync(resolve(app,'app.test.contract'),'utf8'), /^fresh "seed.json"\n/);
    assert.deepEqual(JSON.parse(readFileSync(resolve(app,'seed.json'),'utf8')), rosterSeeds(schema));
    const fixture = [{table:'owners',row:{id:'00000000000000000000000001',person:'dev:bob',role:'reader',label:'Bob',enabled:false}}];
    writeFileSync(resolve(parent,'seed.json'),JSON.stringify(fixture));
    const copied = resolve(parent,'copied');
    createApp(copied,{snapback:{online:true,build:false}});
    assert.deepEqual(JSON.parse(readFileSync(resolve(copied,'seed.json'),'utf8')),fixture);
    assert.throws(() => rosterSeeds(schema.replace('person: principal','person: people')), /cannot infer owners.person/);
    rmSync(resolve(parent, 'seed.json'));
    writeFileSync(resolve(parent, 'snapback/schema.q'), schema.replace('person: principal', 'person: people').replace('  insert <- deny\n', ''));
    const broken = resolve(parent, 'broken');
    assert.throws(() => createApp(broken, {snapback:{online:true,build:false}}), /cannot infer owners.person/);
    assert.equal(existsSync(broken), false, 'a failed seed removes the scaffold');
    assert.equal(readFileSync(resolve(parent, 'snapback/schema.q'), 'utf8'), schema.replace('person: principal', 'person: people').replace('  insert <- deny\n', ''));
    writeFileSync(resolve(parent, 'snapback/schema.q'), schema.replace('  insert <- deny\n', ''));
    const implicit = resolve(parent, 'implicit');
    createApp(implicit, {snapback:{online:true,build:false}});
    assert.deepEqual(JSON.parse(readFileSync(resolve(implicit, 'seed.json'), 'utf8')), sample);
    assert.match(readFileSync(resolve(implicit, 'app.test.contract'), 'utf8'), /^fresh \"seed.json\"\n/);
    assert.throws(() => rosterSeeds(schema.replace('use personas alice, bob','use identity')), /needs an authenticated principal/);
  } finally { rmSync(parent,{recursive:true,force:true}); }
}, 90_000);

test('launchers and generated own commands find pinned Bun with an old PATH', () => {
  const old = resolve(process.env.HOME, '.bun/bin/bun');
  if (!existsSync(old)) return;
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-bun-'));
  try {
    const dir = resolve(parent, 'pinned'); createApp(dir);
    const manifestPath = resolve(dir, 'app.json'), manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    manifest.commands = {runtime: ['bun', 'runtime.mjs']};
    writeFileSync(manifestPath, JSON.stringify(manifest));
    writeFileSync(resolve(dir, 'runtime.mjs'), 'console.log(JSON.stringify({version:process.versions.bun,bun:process.env.BUN,path:Bun.which("bun")}));');
    for (const explicit of [false, true]) {
      const directEnv = {...process.env, PATH: '/usr/bin:/bin'};
      if (explicit) directEnv.BUN = process.execPath; else delete directEnv.BUN;
      const direct = spawnSync(resolve(dir, 'exact.mjs'), ['runtime'], {env: directEnv, encoding: 'utf8'});
      assert.equal(direct.status, 0, direct.stderr);
      assert.equal(JSON.parse(direct.stdout).version, process.versions.bun);
      const rootDirect = spawnSync(resolve(import.meta.dirname, '../scripts/exact.mjs'), ['list'], {env: directEnv, encoding: 'utf8'});
      assert.equal(rootDirect.status, 0, rootDirect.stderr);
      const env = {...process.env, PATH: `${resolve(process.env.HOME, '.bun/bin')}:/usr/bin:/bin`};
      if (explicit) env.BUN = process.execPath; else delete env.BUN;
      const run = spawnSync(old, [resolve(dir, 'exact.mjs'), 'runtime'], {env, encoding: 'utf8'});
      assert.equal(run.status, 0, run.stderr);
      const observed = JSON.parse(run.stdout);
      assert.equal(observed.version, process.versions.bun);
      assert.equal(realpathSync(observed.bun), realpathSync(process.execPath));
      assert.equal(realpathSync(observed.path), realpathSync(process.execPath));
      const root = spawnSync(old, [resolve(import.meta.dirname, '../scripts/exact.mjs'), 'list'], {env, encoding: 'utf8'});
      assert.equal(root.status, 0, root.stderr);
    }
  } finally {rmSync(parent, {recursive: true, force: true});}
});

// LLP 1153 D1.2: a native verb names its platform only after that platform's setup rows, and writes nothing when one
// is missing, the report could not be read, or the machine cannot build it at all.
test('a native verb refuses before writing when its platform\'s setup is missing, and names it otherwise', async () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const manifest = () => readFileSync(resolve(dir, 'app.json'), 'utf8'), before = manifest();
    const missing = await bringPlatform(dir, 'ios', { report: [{ name: 'Xcode', have: 'missing', want: 'Xcode.app', ok: false, fix: 'install Xcode' }], os: 'darwin' });
    assert.match(missing.refused, /^bun exact\.mjs ios: this machine lacks what a ios build needs; nothing was written:\n  Xcode: missing \(SDK Xcode\.app\)\n    install Xcode$/);
    assert.match((await bringPlatform(dir, 'ios', { report: { error: 'no report' }, os: 'darwin' })).refused, /the setup report could not be read, so nothing was written: no report$/);
    assert.match((await bringPlatform(dir, 'ios', { report: [], os: 'linux' })).refused, /^bun exact\.mjs ios: an Apple build needs a Mac/);
    assert.match((await bringPlatform(dir, 'windows', { report: [], os: 'darwin' })).refused, /^bun exact\.mjs windows: a Windows build needs Windows/);
    assert.equal(manifest(), before);
    assert.ok(!existsSync(resolve(dir, '.exact/shells')), 'a refusal generates nothing');
    const brought = await bringPlatform(dir, 'macos', { report: [{ name: 'Xcode', ok: true }], os: 'darwin' });
    assert.match(brought.lines[0], /^app\.json: host\.macos, deploy\.store\.macos added: the app now ships macos \(bun exact\.mjs update --without macos stops it\)$/);
    assert.match(brought.lines[1], /the native workspace \(web, apple\), generated from app\.json$/);
    assert.ok(existsSync(resolve(dir, '.exact/shells/apple/Cargo.toml')));
    // Named already: the report is not read, and a current workspace says nothing.
    assert.deepEqual(await bringPlatform(dir, 'macos', { report: { error: 'not read' }, os: 'darwin' }), { lines: [] });
    // An app without app.json had the defaults readManifest supplies; naming a platform writes them down first.
    rmSync(resolve(dir, 'app.json'));
    const written = await bringPlatform(dir, 'linux', { report: [], os: 'darwin' });
    assert.match(written.lines[0], /^app\.json: written with the defaults the app had without one$/);
    assert.deepEqual(JSON.parse(manifest()).app, { id: 'com.exact.field-log', name: 'Field Log' });
    assert.deepEqual(Object.keys(JSON.parse(manifest()).host), ['web', 'linux']);
    // The stamp: a changed input, or a crate's file gone, regenerates; the same inputs do not.
    const shells = resolve(dir, '.exact/shells');
    assert.equal(ensureShells(dir).wrote, false);
    rmSync(resolve(shells, 'linux/Cargo.toml'));
    assert.equal(ensureShells(dir).wrote, true);
    rmSync(resolve(shells, 'web/build.rs'));
    assert.equal(ensureShells(dir).wrote, true, 'every generated file is checked');
    writeFileSync(resolve(shells, '.stamp.json'), JSON.stringify({ ...JSON.parse(readFileSync(resolve(shells, '.stamp.json'), 'utf8')), inputs: 'stale' }));
    assert.equal(ensureShells(dir).wrote, true);
    assert.equal(ensureShells(dir).wrote, false);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);

// LLP 1153 D1.4, D2.1: the app's files say whether it authored a workspace; a workspace above it owns it only when its
// members list the app or one of its crates, as Cargo reads them.
test('an app authored its workspace when its files say so, not because a directory above it has one', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'site');
    createApp(dir);
    assert.equal(authoredWorkspace(dir), null);
    // A site made inside another Rust repository: that repository's workspace does not list it.
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool"]\n');
    mkdirSync(resolve(parent, 'tool'));
    writeFileSync(resolve(parent, 'tool/Cargo.toml'), '[package]\nname = "tool"\nversion = "0.1.0"\n');
    assert.equal(authoredWorkspace(dir), null);
    // A crate beside app.json that the workspace above lists is owned by it; excluded, it is not.
    mkdirSync(resolve(dir, 'apple'));
    writeFileSync(resolve(dir, 'apple/Cargo.toml'), '[package]\nname = "site-apple"\nversion = "0.1.0"\n');
    assert.equal(authoredWorkspace(dir), dir, 'a package of its own, which no workspace lists, is its own root');
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool", "site/*"]\n');
    assert.equal(authoredWorkspace(dir), parent);
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool", "site/*"]\nexclude = ["site/apple"]\n');
    assert.equal(authoredWorkspace(dir), dir);
    // A member that depends on the crate by path makes it a member too (Cargo's automatic membership), unless excluded;
    // a dependency inherited from the workspace's table, or declared for one target, counts the same.
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool"]\n');
    writeFileSync(resolve(parent, 'tool/Cargo.toml'), '[package]\nname = "tool"\nversion = "0.1.0"\n[dependencies]\nsite-apple = { path = "../site/apple" }\n');
    assert.equal(authoredWorkspace(dir), parent);
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool"]\n[workspace.dependencies]\nsite-apple = { path = "site/apple" }\n');
    writeFileSync(resolve(parent, 'tool/Cargo.toml'), '[package]\nname = "tool"\nversion = "0.1.0"\n[dependencies]\nsite-apple = { workspace = true }\n');
    assert.equal(authoredWorkspace(dir), parent, 'an inherited dependency');
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool"]\n');
    writeFileSync(resolve(parent, 'tool/Cargo.toml'), '[package]\nname = "tool"\nversion = "0.1.0"\n[target.\'cfg(target_os = "macos")\'.dependencies]\nsite-apple = { path = "../site/apple" }\n');
    assert.equal(authoredWorkspace(dir), parent, 'a target\'s dependency');
    writeFileSync(resolve(parent, 'tool/Cargo.toml'), '[package]\nname = "tool"\nversion = "0.1.0"\n[dependencies]\nsite-apple = { path = "../site/apple" }\n');
    writeFileSync(resolve(parent, 'Cargo.toml'), '[workspace]\nmembers = ["tool"]\nexclude = ["site"]\n');
    assert.equal(authoredWorkspace(dir), dir);
    // A package that names its workspace, and an app whose own manifest is the workspace.
    writeFileSync(resolve(dir, 'apple/Cargo.toml'), '[package]\nname = "site-apple"\nversion = "0.1.0"\nworkspace = "../../tool"\n');
    assert.equal(authoredWorkspace(dir), resolve(parent, 'tool'));
    writeFileSync(resolve(dir, 'Cargo.toml'), '[workspace]\nmembers = ["apple"]\n');
    assert.equal(authoredWorkspace(dir), dir);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});
