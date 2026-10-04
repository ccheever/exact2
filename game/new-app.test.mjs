import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { outsideWorkspaceProblems, pathFrom, readManifest } from '../scripts/app.mjs';
import { createApp } from './new.mjs';

test('a new outside app passes the checks every run makes, and a drifted one is told what to paste', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.ok(existsSync(resolve(dir, 'app.test.contract')));
    // The diary instructions travel in full inside the generated block, and .exact/ stays local.
    const agents = () => readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(agents(), /<!-- exact:begin[^]*## The authoring diary[^]*### Needed[^]*<!-- exact:end -->/);
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.exact\/$/m);
    // Execute the generated dispatcher against fake SDK entry points: cwd may
    // be anywhere, but the source and test file must still name this app.
    const sdk = resolve(parent, 'sdk');
    for (const file of ['host/web/build.mjs', 'scripts/agent.mjs', 'scripts/exact.mjs']) {
      mkdirSync(resolve(sdk, file, '..'), { recursive: true });
      writeFileSync(resolve(sdk, file), 'console.log(JSON.stringify({args:process.argv.slice(2),app:process.env.EXACT_APP_DIR}));');
    }
    const testArgs = host => [host, '--app', 'field-log', '--test', resolve(realpathSync(dir), 'app.test.contract')];
    for (const [command, args] of [
      [['web-build'], ['field-log']],
      [['test'], testArgs('web')],
      [['test', 'web'], testArgs('web')],
      [['test', 'ios'], testArgs('ios')],
      [['test', 'macos', '--size', '800x600'], [...testArgs('macos'), '--size', '800x600']],
      [['agent', 'web', 'tree', 'type title a title with spaces'], ['web', '--app', 'field-log', 'tree', 'type title a title with spaces']],
      [['agent', 'ios', 'state'], ['ios', '--app', 'field-log', 'state']],
      [['contract', 'build', 'app.contract', '--json'], ['contract', 'build', 'app.contract', '--json']],
    ]) {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), { args, app: realpathSync(dir) });
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
    const logged = readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.deepEqual(logged.map(c => [c.verb, c.exit]).slice(-2), [['test ios', 2], ['test ios', 7]], 'each command is logged, without its arguments, a glob that matched nothing too');
    assert.ok(logged.some(c => c.verb === 'contract' && c.exit === 0), 'the contract command is logged too');
    const manifest = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
    writeFileSync(resolve(dir, 'Cargo.toml'), manifest.replace(/^taffy = .*\n/m, ''));
    writeFileSync(resolve(dir, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.0.0"\n');
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
    assert.match(agents(), /^# Mine\n\nAfter\.\n\n<!-- exact:begin[^]*## The authoring diary[^]*<!-- exact:end -->\n$/);
    assert.ok(!agents().includes('stale'));
    assert.equal(readFileSync(resolve(dir, '.gitignore'), 'utf8'), '/target/\n/.exact/\n');
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.ok(!readFileSync(web, 'utf8').includes('/nowhere'));
    const appTest = resolve(dir, 'app.test.contract');
    assert.match(readFileSync(appTest, 'utf8'), /the greeting loads/, 'update preserves existing tests');
    rmSync(appTest);
    assert.doesNotMatch(createApp(dir, { update: true }), /AGENTS|gitignore/, 'current instructions are left as they are');
    assert.match(readFileSync(appTest, 'utf8'), /test "the app opens"\n  clock settle/);
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
    assert.equal(packages.length, 2);
    const paths = { 'exact-logic': 'logic', 'exact-apple': 'host/apple', 'exact-web': 'host/web', 'exact-web-capabilities': 'host/web-capabilities', 'exact-js': 'js', 'exact-js-web': 'js/web', 'exact-js-bake': 'js/bake' };
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
    for (const guide of ['contract-for-agents.md', 'agent-pitfalls.md', 'contract-for-humans.md', 'contract-grammar.md']) {
      assert.ok(notes.includes(resolve(root, 'docs', guide)), guide);
      assert.ok(existsSync(resolve(root, 'docs', guide)), guide);
    }
    assert.match(notes, /bun exact\.mjs contract types app\.contract -o app\.contract\.d\.ts/);
    assert.ok(lstatSync(resolve(dir, 'CLAUDE.md')).isSymbolicLink());
    assert.equal(readFileSync(resolve(dir, 'CLAUDE.md'), 'utf8'), notes);
    const manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
    assert.equal(realpathSync(resolve(dir, manifest.$schema)), resolve(root, 'scripts/app.schema.json'));
    readManifest(dir, 'field-log');
    // An author's notes around the block survive; the block is rewritten.
    writeFileSync(resolve(dir, 'AGENTS.md'), `# Mine\n\n${notes.replace('Read before writing code', 'STALE')}\nKeep this.\n`);
    createApp(dir, { update: true });
    const updated = readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(updated, /^# Mine\n/);
    assert.match(updated, /Keep this\.\n$/);
    assert.ok(updated.includes('Read before writing code') && !updated.includes('STALE'));
    // An app made before the notes existed gets both files.
    rmSync(resolve(dir, 'AGENTS.md')); rmSync(resolve(dir, 'CLAUDE.md'));
    createApp(dir, { update: true });
    assert.ok(existsSync(resolve(dir, 'AGENTS.md')) && existsSync(resolve(dir, 'CLAUDE.md')));
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 60_000);
