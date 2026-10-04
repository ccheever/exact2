import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { outsideWorkspaceProblems, pathFrom } from '../scripts/app.mjs';
import { createApp } from './new.mjs';

test('a new outside app passes the checks every run makes, and a drifted one is told what to paste', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-new-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.ok(existsSync(resolve(dir, 'app.test.contract')));
    // The diary instructions travel in full, for every agent, and .exact/ stays local.
    const agents = () => readFileSync(resolve(dir, 'AGENTS.md'), 'utf8');
    assert.match(agents(), /<!-- exact diary[^]*## The authoring diary[^]*<!-- \/exact diary -->/);
    assert.equal(readFileSync(resolve(dir, 'CLAUDE.md'), 'utf8'), '@AGENTS.md\n');
    assert.match(readFileSync(resolve(dir, '.gitignore'), 'utf8'), /^\/\.exact\/$/m);
    // Execute the generated dispatcher against fake SDK entry points: cwd may
    // be anywhere, but the source and test file must still name this app.
    const sdk = resolve(parent, 'sdk');
    for (const file of ['host/web/build.mjs', 'scripts/agent.mjs']) {
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
    ]) {
      const result = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), ...command], { cwd: parent, env: { ...process.env, EXACT2: sdk }, encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), { args, app: realpathSync(dir) });
    }
    writeFileSync(resolve(sdk, 'scripts/agent.mjs'), 'process.exit(7);');
    const refused = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'test', 'ios'], { cwd: parent, env: { ...process.env, EXACT2: sdk } });
    assert.equal(refused.status, 7, 'a failed app test fails the generated command');
    const logged = readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.deepEqual(logged.map(c => [c.verb, c.exit]).slice(-2), [['agent ios', 0], ['test ios', 7]], 'each command is logged, without its arguments');
    const manifest = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
    writeFileSync(resolve(dir, 'Cargo.toml'), manifest.replace(/^taffy = .*\n/m, ''));
    writeFileSync(resolve(dir, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.0.0"\n');
    const [patches, toolchain] = outsideWorkspaceProblems(dir);
    assert.match(patches, /must name exact2's vendored taffy\. Use .*:\n\[patch\.crates-io\]\ntaffy = /);
    assert.match(toolchain, /pins 1\.0\.0; exact2 builds with /);
    // A checkout that moved: every exact2 path in the app is wrong.
    const web = resolve(dir, 'web/Cargo.toml');
    writeFileSync(web, readFileSync(web, 'utf8').replaceAll(/path = "[^"]*"/g, 'path = "/nowhere/exact2/x"'));
    // An older app gains the diary block; an author's own text is kept and a stale block replaced.
    writeFileSync(resolve(dir, 'AGENTS.md'), '# Mine\n\n<!-- exact diary: old -->\nstale\n<!-- /exact diary -->\n\nAfter.\n');
    writeFileSync(resolve(dir, 'CLAUDE.md'), '# Claude notes\n');
    writeFileSync(resolve(dir, '.gitignore'), '/target/');
    assert.match(createApp(dir, { update: true }), /the diary instructions, exact2 paths in web\/Cargo\.toml/);
    assert.match(agents(), /^# Mine\n\n<!-- exact diary[^]*## The authoring diary[^]*<!-- \/exact diary -->\n\nAfter\.\n$/);
    assert.ok(!agents().includes('stale'));
    assert.match(readFileSync(resolve(dir, 'CLAUDE.md'), 'utf8'), /^# Claude notes\n\n<!-- exact diary/);
    assert.equal(readFileSync(resolve(dir, '.gitignore'), 'utf8'), '/target/\n/.exact/\n');
    assert.deepEqual(outsideWorkspaceProblems(dir), []);
    assert.ok(!readFileSync(web, 'utf8').includes('/nowhere'));
    const appTest = resolve(dir, 'app.test.contract');
    assert.match(readFileSync(appTest, 'utf8'), /the greeting loads/, 'update preserves existing tests');
    rmSync(appTest);
    assert.doesNotMatch(createApp(dir, { update: true }), /the diary instructions/, 'current instructions are left as they are');
    assert.match(readFileSync(appTest, 'utf8'), /test "the app opens"\n  clock settle/);
    assert.ok(!readFileSync(appTest, 'utf8').includes('greeting'), 'an older app need not have the scaffold IDs');
    assert.equal(readFileSync(resolve(dir, 'Cargo.toml'), 'utf8'), manifest, 'the patch table is rewritten in place');
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
      env: { ...process.env, PATH: '' }, encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /cargo could not resolve/);
    assert.equal(existsSync(dir), false);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});
