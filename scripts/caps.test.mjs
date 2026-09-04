#!/usr/bin/env node
/**
 * Proves every caps rule fires. A budget check that cannot fail is worse than
 * no check: it reports green while inspecting nothing, and everyone believes it.
 *
 * Each case builds a throwaway repository, breaks exactly one thing, and asserts
 * the matching code appears. The last case asserts a clean repository passes, so
 * the suite cannot be satisfied by a check that simply always fails.
 */

import { spawn, spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign as cryptoSign } from 'node:crypto';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readlinkSync, realpathSync, renameSync, writeFileSync, rmSync, symlinkSync, utimesSync } from 'node:fs';
import { createServer } from 'node:http';
import { hostname, tmpdir } from 'node:os';
import { isAbsolute, join, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import { applyStaticChange, applyStaticTreeChange, appManifestDigest, builtAppMatches, copyStaticTree, copyStaticTreeIfPresent, installStaticCandidate, listPublicFiles, publicFileCards, shaderInterfaceDigests, staticFile, readStaticFile, serveStatic, syncStaticTree, watchStaticTrees, webEnvelope } from '../host/web/serve.mjs';
import { assertWebDistApp } from './agent.mjs';
import { copyAppleStaticTrees, deviceLaunchArgs } from '../host/apple/build.mjs';
import { canonicalBytes, classify, defaultRelease, deployRun, inspectHead, materializeSnapshot, publishStream, publishRoot, webRelease, renderTable, snapshotOf, streamHead } from './deploy.mjs';
import { blobPath, DirectoryOrigin, HttpsOrigin, OriginUnavailable, webRootPath, webReleasePath, sha256 } from './origin.mjs';

const CAPS = join(dirname(fileURLToPath(import.meta.url)), 'caps.mjs');
const BOOT = join(dirname(fileURLToPath(import.meta.url)), 'boot.mjs');
const DEPLOY = join(dirname(fileURLToPath(import.meta.url)), 'deploy.mjs');

const GOOD_RULES = `# Rules

- **5 blocking checks, 60s total.** **[check]**
- **15 documents in the working set.** **[check]**
- **10 documents in the foundation.** **[check]**
- **This file: 700 words.** **[check]**
- **1,500 lines per source file.** **[check]**

## Time budgets

| | |
|---|---|
| Cold start to interactive | 100ms |

## The five checks

\`build\` · \`test\` · \`lint\` · \`caps\` · \`boot\`
`;

function repo(files) {
  const dir = mkdtempSync(join(tmpdir(), 'caps-'));
  spawnSync('git', ['init', '-q', '-b', 'main'], { cwd: dir });
  spawnSync('git', ['config', 'user.email', 't@t.io'], { cwd: dir });
  spawnSync('git', ['config', 'user.name', 't'], { cwd: dir });
  for (const [path, body] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(path)), { recursive: true });
    writeFileSync(join(dir, path), body);
  }
  spawnSync('git', ['add', '-A'], { cwd: dir });
  spawnSync('git', ['commit', '-qm', 'x'], { cwd: dir });
  return dir;
}

function run(dir) {
  const r = spawnSync('node', [CAPS], { cwd: dir, encoding: 'utf8' });
  return { out: r.stdout + r.stderr, code: r.status };
}

const cases = [
  ['missing rules file fails', {}, 'rules:missing'],
  ['unparseable budget fails closed', { 'rules/RULES.md': '# Rules\n\nno budgets here at all\n' }, 'budget:words'],
  ['over the word cap fails',
    { 'rules/RULES.md': GOOD_RULES.replace('This file: 700 words', 'This file: 10 words') }, 'cap:words'],
  ['too many blocking checks fails',
    { 'rules/RULES.md': GOOD_RULES.replace('**5 blocking checks', '**3 blocking checks') }, 'cap:checks'],
  ['unfilled product-latency row fails',
    { 'rules/RULES.md': GOOD_RULES.replace('| Cold start to interactive | 100ms |', '| **<the product latency that actually matters>** | |') },
    'cap:product-latency'],
  ['oversize source file fails',
    { 'rules/RULES.md': GOOD_RULES, 'src/big.js': 'x\n'.repeat(2000) }, 'cap:lines'],
  ['generated file is exempt',
    { 'rules/RULES.md': GOOD_RULES, 'src/ok.js': 'const a = 1;\n',
      'src/gen.js': '// @generated do not edit\n' + 'x\n'.repeat(2000) }, null],
  ['a marker matching every file is caught',
    Object.fromEntries([['rules/RULES.md', GOOD_RULES],
      ...Array.from({ length: 6 }, (_, i) => [`src/g${i}.js`, '// @generated\nconst a = 1;\n'])]),
    'sources:all-skipped'],
  ['an over-full working set fails',
    Object.fromEntries([['rules/RULES.md', GOOD_RULES.replace('**15 documents in the working set.**', '**2 documents in the working set.**')],
      ...Array.from({ length: 4 }, (_, i) => [`llp/000${i}-d.md`, `# D${i}\n\n**Status:** Accepted\n`]),
      ...Array.from({ length: 4 }, (_, i) => [`llp/current/000${i}-d.md`, `../000${i}-d.md`])]),
    'cap:current'],
  ['an over-full foundation fails',
    Object.fromEntries([['rules/RULES.md', GOOD_RULES.replace('**10 documents in the foundation.**', '**2 documents in the foundation.**')],
      ...Array.from({ length: 4 }, (_, i) => [`llp/000${i}-d.md`, `# D${i}\n\n**Status:** Active\n`]),
      ...Array.from({ length: 4 }, (_, i) => [`llp/foundation/000${i}-d.md`, `../000${i}-d.md`])]),
    'cap:foundation'],
  ['a large corpus with a small working set passes',
    Object.fromEntries([['rules/RULES.md', GOOD_RULES],
      ...Array.from({ length: 40 }, (_, i) => [`llp/${String(i).padStart(4, '0')}-d.md`, `# D${i}\n\n**Status:** Accepted\n`]),
      ...Array.from({ length: 3 }, (_, i) => [`llp/current/${String(i).padStart(4, '0')}-d.md`, `../${String(i).padStart(4, '0')}-d.md`])]),
    null],
  ['a baseline tolerates pre-existing oversize files',
    { 'rules/RULES.md': GOOD_RULES.replace('**1,500 lines per source file.**', '**1,500 lines per source file**, baseline 2.'),
      'src/a.js': 'x\n'.repeat(2000), 'src/b.js': 'x\n'.repeat(2000) }, null],
  ['a baseline still catches the file that exceeds it',
    { 'rules/RULES.md': GOOD_RULES.replace('**1,500 lines per source file.**', '**1,500 lines per source file**, baseline 2.'),
      'src/a.js': 'x\n'.repeat(2000), 'src/b.js': 'x\n'.repeat(2000), 'src/c.js': 'x\n'.repeat(9000) }, 'cap:lines'],
  ['llp/reviews are not counted as design docs',
    Object.fromEntries([['rules/RULES.md', GOOD_RULES.replace('**20 active design docs', '**2 active design docs')],
      ...Array.from({ length: 6 }, (_, i) => [`llp/reviews/r${i}.md`, `# R${i}\n\n**Status:** Accepted\n`])]),
    null],
  ['vendored source is not held to the line cap',
    { 'rules/RULES.md': GOOD_RULES, 'vendor/brotli/enc.c': 'x\n'.repeat(9000), 'src/ok.js': 'const a = 1;\n' }, null],
  ['a clean repository passes', { 'rules/RULES.md': GOOD_RULES, 'src/ok.js': 'const a = 1;\n' }, null],
];

let failed = 0;
let total = cases.length;
for (const [name, files, expect] of cases) {
  const dir = repo(files);
  const { out, code } = run(dir);
  rmSync(dir, { recursive: true, force: true });
  const ok = expect ? (code === 1 && out.includes(expect)) : code === 0;
  if (!ok) {
    failed += 1;
    console.log(`FAIL  ${name}`);
    console.log(`      expected ${expect ? `code 1 with "${expect}"` : 'code 0'}, got code ${code}`);
    console.log(out.split('\n').map((l) => '      | ' + l).join('\n'));
  } else {
    console.log(`ok    ${name}`);
  }
}

function result(name, ok, detail = '') {
  total += 1;
  if (ok) console.log(`ok    ${name}`);
  else { failed += 1; console.log(`FAIL  ${name}`); if (detail) console.log(detail); }
}

// The boot check's parser must see every valid spelling that can execute.
const BOOT_RULES = GOOD_RULES.replace('| Cold start to interactive | 100ms |', '| Cold start to interactive | 100ms |\n| App JS executed before first pixel | none |');
function boot(html, files = {}) {
  const dir = repo({ 'rules/RULES.md': BOOT_RULES, 'host/web/index.html': html, 'host/web/glue.js': '', ...files });
  mkdirSync(join(dir, 'scripts'), { recursive: true });
  const fixtureBoot = join(dir, 'scripts/boot.mjs');
  copyFileSync(BOOT, fixtureBoot);
  const r = spawnSync('node', [fixtureBoot], { cwd: dir, encoding: 'utf8' });
  rmSync(dir, { recursive: true, force: true });
  return { code: r.status, out: r.stdout + r.stderr };
}
for (const [name, html, files, expectCode, expect] of [
  ['boot sees a single-quoted src', "<script type='module' src='./glue.js'></script>", {}, 0, 'modules reachable before first pixel: 1'],
  ['boot sees an unquoted src', '<script type=module src=./glue.js></script>', {}, 0, 'modules reachable before first pixel: 1'],
  ['boot follows re-exports', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': "export * from '../../apps/app.js';\n", 'apps/app.js': '' }, 1, 'app JS before first pixel'],
  ['boot ignores comments', '<!-- <script src=../../apps/bypass.js></script> --><script src=./glue.js></script>', { 'host/web/glue.js': "// export * from '../../apps/bypass.js';\n/* import '../../apps/also.js'; */\n" }, 0, 'modules reachable before first pixel: 1'],
  ['boot fails closed on malformed tags', '<script type=module src="./glue.js></script>', {}, 1, 'malformed HTML'],
]) {
  const r = boot(html, files);
  result(name, r.code === expectCode && r.out.includes(expect), r.out);
}

// The exact resolver used by serve/dev/agent/metrics: only current public
// build outputs, no dot paths or symlink traversal.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-static-'));
  const dist = join(dir, 'dist');
  const outside = join(dir, 'outside');
  mkdirSync(join(dist, 'assets'), { recursive: true });
  mkdirSync(join(dist, '.chrome-profile', 'Default'), { recursive: true });
  mkdirSync(outside);
  writeFileSync(join(dist, 'index.html'), 'ok');
  writeFileSync(join(dist, 'assets', 'logo.png'), 'ok');
  writeFileSync(join(dist, 'stale.txt'), 'private');
  writeFileSync(join(dist, '.chrome-profile', 'Default', 'Cookies'), 'private');
  writeFileSync(join(outside, 'secret'), 'private');
  symlinkSync(join(outside, 'secret'), join(dist, 'assets', 'linked-file'));
  symlinkSync(outside, join(dist, 'assets', 'linked-dir'));
  const ok = staticFile(dist, '/index.html')?.route === '/index.html'
    && staticFile(dist, '/assets/logo.png')?.route === '/assets/logo.png'
    && staticFile(dist, '/stale.txt') === null
    && staticFile(dist, '/.chrome-profile/Default/Cookies') === null
    && staticFile(dist, '/assets/linked-file') === null
    && staticFile(dist, '/assets/linked-dir/secret') === null;
  rmSync(dir, { recursive: true, force: true });
  result('web serving rejects stale, dot, and symlink paths', ok);
}

// Static bakes and live edits share one source policy. A rejected link or
// validator leaves the served file alone; a source removed after its bytes
// were captured cannot turn the committed candidate into partial output.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-static-copy-'));
  const source = join(dir, 'source');
  const target = join(dir, 'target');
  const outside = join(dir, 'outside');
  mkdirSync(source); mkdirSync(target);
  writeFileSync(outside, 'private');
  writeFileSync(join(source, 'ok.txt'), 'good');
  copyStaticTree(source, target);
  symlinkSync(outside, join(source, 'linked.txt'));
  let treeRefused = false;
  try { copyStaticTree(source, join(dir, 'other')); }
  catch (error) { treeRefused = error.message.includes('cannot be symlinks'); }
  rmSync(join(source, 'linked.txt'));

  writeFileSync(join(target, 'live.txt'), 'last good');
  writeFileSync(join(source, 'live.txt'), 'invalid');
  let invalidRefused = false;
  try { installStaticCandidate(source, 'live.txt', join(target, 'live.txt'), () => { throw new Error('invalid shader'); }); }
  catch (error) { invalidRefused = error.message === 'invalid shader'; }
  rmSync(join(source, 'live.txt'));
  symlinkSync(outside, join(source, 'live.txt'));
  let linkRefused = false;
  try { installStaticCandidate(source, 'live.txt', join(target, 'live.txt')); }
  catch (error) { linkRefused = error.message.includes('cannot be symlinks'); }
  rmSync(join(source, 'live.txt'));
  let missingRefused = false;
  try { installStaticCandidate(source, 'live.txt', join(target, 'live.txt')); }
  catch (error) { missingRefused = error.code === 'ENOENT'; }
  writeFileSync(join(source, 'live.txt'), 'next good');
  const installed = installStaticCandidate(source, 'live.txt', join(target, 'live.txt'), () => rmSync(join(source, 'live.txt')));

  mkdirSync(join(source, 'gone', 'inside'), { recursive: true });
  writeFileSync(join(source, 'gone', 'inside', 'one.txt'), 'one');
  mkdirSync(join(target, 'gone', 'inside'), { recursive: true });
  writeFileSync(join(target, 'gone', 'inside', 'one.txt'), 'old one');
  rmSync(join(source, 'gone'), { recursive: true });
  const removedDirectory = applyStaticChange(source, 'gone', join(target, 'gone'));

  const startupSource = join(dir, 'startup-source');
  const startupTarget = join(dir, 'startup-target');
  mkdirSync(startupSource); writeFileSync(join(startupSource, 'fresh.txt'), 'fresh');
  const startupPresent = syncStaticTree(startupSource, startupTarget)
    && readFileSync(join(startupTarget, 'fresh.txt'), 'utf8') === 'fresh';
  rmSync(startupSource, { recursive: true });
  const startupRemoved = !syncStaticTree(startupSource, startupTarget) && !existsSync(startupTarget);
  mkdirSync(startupTarget); writeFileSync(join(startupTarget, 'last-good.txt'), 'last good');
  symlinkSync(join(dir, 'does-not-exist'), startupSource);
  let brokenRootRefused = false;
  try { syncStaticTree(startupSource, startupTarget); }
  catch (error) { brokenRootRefused = error.message.includes('must be a real directory'); }
  const lastGoodRoot = readFileSync(join(startupTarget, 'last-good.txt'), 'utf8') === 'last good';
  rmSync(startupSource);
  const absentCopySkipped = copyStaticTreeIfPresent(startupSource, join(dir, 'absent-copy')) === false;
  symlinkSync(join(dir, 'still-does-not-exist'), startupSource);
  let brokenCopyRefused = false;
  try { copyStaticTreeIfPresent(startupSource, join(dir, 'broken-copy')); }
  catch (error) { brokenCopyRefused = error.message.includes('must be a real directory'); }

  const shaderSource = join(dir, 'shader-source');
  const shaderTarget = join(dir, 'shader-target');
  const reflector = resolve(dirname(fileURLToPath(import.meta.url)), '../target/debug/exact-gpu-reflect');
  if (!existsSync(reflector)) spawnSync('cargo', ['build', '-q', '-p', 'exact-gpu-reflect'], { cwd: resolve(dirname(fileURLToPath(import.meta.url)), '..') });
  mkdirSync(shaderSource);
  writeFileSync(join(shaderSource, 'surface.wgsl'), '@compute @workgroup_size(1) fn main() {}\n');
  const validateShaders = (candidate) => shaderInterfaceDigests(candidate, reflector);
  syncStaticTree(shaderSource, shaderTarget, validateShaders);
  writeFileSync(join(shaderSource, 'surface.wgsl'), 'not wgsl');
  let startupShaderRefused = false;
  try { syncStaticTree(shaderSource, shaderTarget, validateShaders); }
  catch (error) { startupShaderRefused = error.message.includes('expected global item'); }
  const startupShaderPreserved = readFileSync(join(shaderTarget, 'surface.wgsl'), 'utf8').startsWith('@compute');

  result('static candidates reject links and preserve last-good bytes', treeRefused && invalidRefused
    && linkRefused && missingRefused && readFileSync(join(target, 'ok.txt'), 'utf8') === 'good'
    && installed.toString() === 'next good' && readFileSync(join(target, 'live.txt'), 'utf8') === 'next good'
    && removedDirectory.removed && removedDirectory.removedFiles.join(',') === 'inside/one.txt'
    && !existsSync(join(target, 'gone')) && startupPresent && startupRemoved
    && brokenRootRefused && lastGoodRoot && absentCopySkipped && brokenCopyRefused
    && startupShaderRefused && startupShaderPreserved);
  rmSync(dir, { recursive: true, force: true });
}

// The watcher is rooted at the stable app directory, not at whichever asset
// trees happened to exist at startup. Drive an absent tree through creation,
// whole-root deletion, and recreation; every phase reaches the served tree.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-static-watch-'));
  const appDir = join(dir, 'app');
  const source = join(appDir, 'assets');
  const dist = join(dir, 'dist');
  mkdirSync(appDir);
  const errors = [];
  const watcher = watchStaticTrees(appDir, [[source, 'assets']], (change) => {
    try {
      if (change.tree) applyStaticTreeChange(change.root, join(dist, change.targetRoot));
      else applyStaticChange(change.root, change.relative, join(dist, change.name));
    } catch (error) { errors.push(error.message); }
  });
  const until = async (predicate) => {
    for (let attempt = 0; attempt < 100; attempt++) {
      if (predicate()) return true;
      await new Promise((done) => setTimeout(done, 20));
    }
    return false;
  };
  await new Promise((done) => setTimeout(done, 50));
  mkdirSync(source); writeFileSync(join(source, 'live.txt'), 'first');
  const created = await until(() => existsSync(join(dist, 'assets', 'live.txt')));
  rmSync(source, { recursive: true });
  const deleted = await until(() => !existsSync(join(dist, 'assets')));
  mkdirSync(source); writeFileSync(join(source, 'live.txt'), 'second');
  const recreated = await until(() => {
    try { return readFileSync(join(dist, 'assets', 'live.txt'), 'utf8') === 'second'; }
    catch { return false; }
  });
  watcher.close();
  result('static watcher follows absent root creation, deletion, and recreation', created && deleted && recreated && errors.length === 0,
    JSON.stringify({ created, deleted, recreated, errors }));
  rmSync(dir, { recursive: true, force: true });
}

// A removal stays in the protocol rather than being filtered out, and every
// reported kind remains paired with the row that produced it.
{
  const attrs = new Map([['src', '/assets/gone.png']]);
  const image = { dataset: {}, getAttribute: (name) => attrs.get(name) ?? null,
    setAttribute: (name, value) => attrs.set(name, value), removeAttribute: (name) => attrs.delete(name), decode: async () => {} };
  const context = { EventSource: undefined, location: { reload() {} }, fetch: async () => { throw new Error('unexpected fetch'); },
    document: { querySelectorAll: (selector) => selector === 'img' ? [image] : [] }, console };
  runInNewContext(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../host/web/dev.js'), 'utf8'), context);
  const removed = await context.applyDevAssetChanges([{ name: 'assets/gone.png', removed: true }]);
  const cleared = !attrs.has('src') && image.dataset.exactDevAsset === 'assets/gone.png';
  const restored = await context.applyDevAssetChanges([{ name: 'assets/gone.png', sha256: '1'.repeat(64) }]);
  const called = [];
  const assets = [{ name: 'assets/gone.png', removed: true }, { name: 'deck/live.html', sha256: '1'.repeat(64) }];
  const kinds = await context.applyDevAssetChanges(assets,
    async (asset) => { called.push(`change ${asset.name}`); return 'deck'; },
    async (asset) => { called.push(`remove ${asset.name}`); return 'image removed'; });
  result('web dev applies and labels asset removals in row order',
    removed[0] === 'image removed (1)' && cleared && restored[0] === 'image (1)'
    && attrs.get('src') === `/assets/gone.png?v=${'1'.repeat(12)}` && !image.dataset.exactDevAsset
    && called.join(',') === 'remove assets/gone.png,change deck/live.html'
    && assets.map((asset, i) => `${asset.name} → ${kinds[i]}`).join(',')
      === 'assets/gone.png → image removed,deck/live.html → deck');
}

{
  const dir = mkdtempSync(join(tmpdir(), 'exact-built-app-'));
  const id = 'com.exact.one';
  const app = { id, displayName: 'One', manifest: { name: 'Install One', app: { id, name: 'One' } } };
  const plan = Buffer.alloc(36 + Buffer.byteLength(id));
  plan.write('EXPL'); plan.writeUInt32LE(4, 4); plan.writeBigUInt64LE(1n, 16);
  plan.writeUInt32LE(Buffer.byteLength(id), 32); plan.write(id, 36);
  const planDigest = createHash('sha256').update(plan).digest('hex');
  writeFileSync(join(dir, 'app.wasm'), 'wasm');
  writeFileSync(join(dir, 'exact.json'), JSON.stringify({ exact: 1, app: { id, name: app.displayName }, plan: { url: './app.plan', sha256: planDigest, bytes: plan.length } }));
  result('dev startup rejects an incomplete dist', !builtAppMatches(dir, app));
  writeFileSync(join(dir, 'app.plan'), plan);
  writeFileSync(join(dir, 'glue.js'), '');
  writeFileSync(join(dir, 'index.html'), '');
  writeFileSync(join(dir, 'manifest.json'), '{}');
  writeFileSync(join(dir, '.exact-build.json'), JSON.stringify({ exactBuild: 1,
    app: { id, name: app.displayName }, manifestSha256: appManifestDigest(app),
    files: publicFileCards(dir) }));
  const complete = builtAppMatches(dir, app);
  const staleName = !builtAppMatches(dir, { ...app, displayName: 'Renamed' });
  const staleManifest = !builtAppMatches(dir, { ...app, manifest: { ...app.manifest, theme_color: '#000000' } });
  const wasm = readFileSync(join(dir, 'app.wasm'));
  writeFileSync(join(dir, 'app.wasm'), 'another app');
  const replacedWasm = !builtAppMatches(dir, app);
  writeFileSync(join(dir, 'app.wasm'), wasm);
  writeFileSync(join(dir, 'glue.js'), 'truncated');
  const changedRuntime = !builtAppMatches(dir, app);
  writeFileSync(join(dir, 'glue.js'), '');
  const named = JSON.parse(readFileSync(join(dir, 'exact.json'), 'utf8'));
  delete named.app.id;
  writeFileSync(join(dir, 'exact.json'), JSON.stringify(named));
  const missingEnvelopeId = !builtAppMatches(dir, app);
  let unnamedPlanRefused = false;
  try { webEnvelope(app, Buffer.alloc(36), []); } catch (error) { unnamedPlanRefused = error.message.includes('nonempty app id'); }
  writeFileSync(join(dir, 'app.plan'), 'corrupt');
  result('dev startup identifies only a complete coherent named app build', complete && staleName
    && staleManifest && replacedWasm && changedRuntime && missingEnvelopeId
    && unnamedPlanRefused && !builtAppMatches(dir, app));
  rmSync(dir, { recursive: true, force: true });
}

// The completion marker is build-private. The exact same public inventory
// drives deploy classification, so it cannot leak to an origin even though it
// lives beside public artifacts in the completed stage.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-public-web-'));
  writeFileSync(join(dir, 'index.html'), 'public');
  writeFileSync(join(dir, '.exact-build.json'), 'private');
  writeFileSync(join(dir, 'unexpected.txt'), 'private too');
  const origin = { kind: 'directory', describe: () => 'test-origin', get: async () => null };
  const table = await classify({ app: { id: 'com.exact.test', displayName: 'Test', dir, manifest: {} }, opts: { platform: [] },
    origin, channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'test', web: dir, bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] },
    compat: { web: { id: 'a'.repeat(32), inputs: {} } }, platforms: [], wantOrigin: true });
  const published = table.rows[0]?.files.new ?? [];
  result('deploy publishes only the shared public web inventory', listPublicFiles(dir).join(',') === 'index.html'
    && published.join(',') === 'index.html' && !published.includes('.exact-build.json'), JSON.stringify(published));
  rmSync(dir, { recursive: true, force: true });
}

// A complete prior graph survives a failure before/after every individual
// payload write and pointer write. Readers follow the exact index/envelope
// they received while another publish runs, including removal of AASA.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-web-release-'));
  const makeWeb = (generation) => {
    const root = join(dir, generation);
    mkdirSync(join(root, 'assets'), { recursive: true });
    mkdirSync(join(root, '.well-known'), { recursive: true });
    for (const name of ['app.plan', 'app.wasm', 'glue.js', 'assets/live.txt', 'assets/space #?.txt']) writeFileSync(join(root, name), generation);
    writeFileSync(join(root, 'index.html'), '<!doctype html><meta charset="utf-8"><link rel="manifest" href="./manifest.json"><script src="./glue.js"></script>');
    writeFileSync(join(root, 'manifest.json'), JSON.stringify({ name: 'Test', start_url: './', scope: './', ...(generation === 'new' ? { id: './identity' } : {}), icons: [{ src: './assets/live.txt' }], shortcuts: [{ name: 'Go', url: './go' }] }));
    if (generation === 'old') {
      writeFileSync(join(root, 'assets/removed.txt'), 'removed');
      writeFileSync(join(root, '.well-known/apple-app-site-association'), 'association');
    }
    const card = (name) => { const body = readFileSync(join(root, name)); return { name, url: './' + name, sha256: sha256(body), bytes: body.length }; };
    writeFileSync(join(root, 'exact.json'), JSON.stringify({ exact: 1, plan: card('app.plan'), assets: listPublicFiles(root).filter((n) => n.startsWith('assets/')).map(card) }));
    return root;
  };
  const oldWeb = makeWeb('old'), newWeb = makeWeb('new');
  const old = webRelease(oldWeb), next = webRelease(newWeb);
  const seed = (name) => {
    const root = join(dir, name);
    for (const file of old.files) {
      const path = join(root, webReleasePath(old.pointer.id), file.name);
      mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, file.body);
    }
    mkdirSync(dirname(join(root, webRootPath)), { recursive: true });
    writeFileSync(join(root, webRootPath), JSON.stringify(old.pointer));
    // Legacy fixed files must never resurrect an omitted canonical path.
    mkdirSync(join(root, '.well-known'), { recursive: true });
    mkdirSync(join(root, 'assets'), { recursive: true });
    writeFileSync(join(root, '.well-known/apple-app-site-association'), 'stale');
    writeFileSync(join(root, 'assets/removed.txt'), 'stale');
    return root;
  };
  let whole = true;
  for (const moment of ['before', 'after']) for (let stop = 1; stop <= next.files.length + 1; stop++) {
    const root = seed(`failure-${moment}-${stop}`), origin = new DirectoryOrigin(root);
    let writes = 0;
    const wrap = (operation) => async (...args) => {
      const at = ++writes;
      if (moment === 'before' && at === stop) throw new Error('injected before write');
      const value = await operation(...args);
      if (moment === 'after' && at === stop) throw new Error('injected after write');
      return value;
    };
    origin.put = wrap(origin.put.bind(origin)); origin.putHead = wrap(origin.putHead.bind(origin));
    try { await publishRoot({ origin, row: {}, web: newWeb }); } catch (error) { whole &&= error.message.startsWith('injected'); }
    const selected = JSON.parse(readFileSync(join(root, webRootPath)));
    const committed = moment === 'after' && stop === next.files.length + 1;
    whole &&= selected.id === (committed ? next.pointer.id : old.pointer.id);
    for (const card of selected.files) whole &&= sha256(readStaticFile(root, `/${webReleasePath(selected.id)}/${card.name}`).body) === card.sha256;
    whole &&= !!readStaticFile(root, '/.well-known/apple-app-site-association') === !committed;
    whole &&= !!readStaticFile(root, '/assets/removed.txt') === !committed;
    whole &&= readStaticFile(root, `/${webReleasePath(old.pointer.id)}/assets/removed.txt`).body.toString() === 'removed';
  }
  const root = seed('concurrent'), origin = new DirectoryOrigin(root);
  const native = `.exact/web/${'a'.repeat(64)}`;
  mkdirSync(join(root, native, 'releases'), { recursive: true });
  writeFileSync(join(root, native, 'exact.json'), 'mutable native head');
  writeFileSync(join(root, native, 'releases/r1.json'), 'immutable native receipt');
  const app = { id: 'test', displayName: 'Test', dir: newWeb, manifest: {} };
  const table = await classify({ app, opts: {}, origin, channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, release: 'test', web: newWeb,
    bundle: {}, compat: {}, platforms: [], wantOrigin: true });
  whole &&= table.rows[0].files.removed.join(',') === '.well-known/apple-app-site-association,assets/removed.txt';
  const server = createServer((req, res) => serveStatic(root, req, res));
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const url = `http://127.0.0.1:${server.address().port}`;
  let rounds = 0;
  const readGraph = async () => {
    const index = await fetch(url);
    const html = await index.text(), base = /<base href="([^"]+)"/.exec(html)?.[1];
    if (!base || index.headers.get('cache-control') !== 'no-store') return false;
    const id = base.split('/').at(-2), expected = id === old.pointer.id ? 'old' : id === next.pointer.id ? 'new' : null;
    const payload = await Promise.all(['glue.js', 'app.wasm', 'assets/live.txt'].map(async (name) => {
      const response = await fetch(url + base + name);
      return response.ok && response.headers.get('cache-control') === 'public, max-age=31536000, immutable' && await response.text() === expected;
    }));
    const envelopeResponse = await fetch(url, { headers: { accept: 'application/vnd.exact.envelope+json' } });
    const envelope = await envelopeResponse.json();
    for (const card of [envelope.plan, ...envelope.assets]) {
      const response = await fetch(url + card.url), bytes = Buffer.from(await response.arrayBuffer());
      payload.push(response.ok && card.url.startsWith('/.exact/root/web/releases/') && sha256(bytes) === card.sha256 && bytes.length === card.bytes);
    }
    rounds++;
    return expected && payload.every(Boolean) && envelopeResponse.headers.get('vary') === 'Accept';
  };
  try {
    whole &&= await readGraph();
    const nativeHead = await fetch(`${url}/${native}/exact.json`), nativeReceipt = await fetch(`${url}/${native}/releases/r1.json`);
    whole &&= nativeHead.ok && nativeHead.headers.get('cache-control') === 'no-store' && await nativeHead.text() === 'mutable native head';
    whole &&= nativeReceipt.ok && nativeReceipt.headers.get('cache-control') === 'public, max-age=31536000, immutable';
    const original = origin.put.bind(origin);
    origin.put = async (...args) => { const value = await original(...args); whole &&= await readGraph(); return value; };
    const reader = (async () => { for (let i = 0; i < 12; i++) whole &&= await readGraph(); })();
    await publishRoot({ origin, row: table.rows[0], web: newWeb });
    await reader;
    whole &&= await readGraph();
    for (const path of ['/.well-known/apple-app-site-association', '/assets/removed.txt']) {
      const response = await fetch(url + path);
      whole &&= response.status === 404 && response.headers.get('cache-control') === 'no-store';
    }
    const oldAssociation = await fetch(`${url}/${webReleasePath(old.pointer.id)}/.well-known/apple-app-site-association`);
    whole &&= oldAssociation.ok && await oldAssociation.text() === 'association';
    for (const release of [old, next]) {
      const manifest = await (await fetch(`${url}/${webReleasePath(release.pointer.id)}/manifest.json`)).json();
      whole &&= manifest.start_url === '/' && manifest.scope === '/' && (manifest.id === undefined || manifest.id === '/identity')
        && manifest.icons[0].src === `/${webReleasePath(release.pointer.id)}/assets/live.txt` && manifest.shortcuts[0].url === '/go';
    }
    const pointer = await fetch(`${url}/${webRootPath}`);
    whole &&= pointer.headers.get('cache-control') === 'no-store';
  } finally { await new Promise((done) => server.close(done)); rmSync(dir, { recursive: true, force: true }); }
  result('web root switches a complete immutable graph; every write failure, concurrent readers, removals and HTTP caches', whole && rounds >= 20, `reader rounds: ${rounds}`);
}

{
  const id = 'com.exact.names';
  const bytes = Buffer.alloc(36 + Buffer.byteLength(id));
  bytes.write('EXPL'); bytes.writeUInt32LE(4, 4); bytes.writeBigUInt64LE(1n, 16);
  bytes.writeUInt32LE(Buffer.byteLength(id), 32); bytes.write(id, 36);
  const app = { name: 'internal-slug', id, displayName: 'Cross-platform Display', manifest: { name: 'Web Install Name' } };
  const web = webEnvelope(app, bytes, []);
  let mismatchedPlanRefused = false;
  try { webEnvelope({ ...app, id: 'com.exact.another' }, bytes, []); }
  catch (error) { mismatchedPlanRefused = error.message.includes(`plan is for ${id}`); }
  const stream = streamHead({ app, bundle: { plan: { bytes, sha256: web.plan.sha256, formatVersion: 4, kernelSchema: '0000000000000001' }, assets: [] },
    stream: { channel: 'prod', compatibilityId: 'a'.repeat(32) }, seq: 1, release: 'test' });
  result('web and stream envelopes use the cross-platform display name', app.name !== app.manifest.name
    && app.manifest.name !== app.displayName && web.app.name === app.displayName
    && stream.app.name === web.app.name && mismatchedPlanRefused);
}

// "Current" means the production client admits the complete authenticated
// head. An unusable head contributes no rollback floor: repair advances from
// the largest immutable record whose embedded envelope verifies, or refuses
// when no such history exists. Raw JSON spellings rejected by Rust are also
// rejected before JavaScript can normalize them. The locked publisher repeats
// the same admission in case the head changed after the table was printed.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-head-admission-'));
  const compatibilityId = 'a'.repeat(32);
  const stream = { channel: 'prod', compatibilityId };
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const publicRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64');
  const app = { id: 'com.exact.admission', displayName: 'Admission', dir,
    manifest: { deploy: { signing: { key: 'test', keys: { test: publicRaw } } } } };
  const planBytes = Buffer.from('same plan');
  const assetBytes = Buffer.from('same asset');
  const bundle = {
    plan: { bytes: planBytes, sha256: createHash('sha256').update(planBytes).digest('hex'), formatVersion: 4, kernelSchema: '0'.repeat(16) },
    assets: [{ name: 'assets/icon.png', bytes: assetBytes, sha256: createHash('sha256').update(assetBytes).digest('hex') }],
  };
  const signer = { keyId: 'test', sign: (head) => ({ keyId: 'test', ed25519: cryptoSign(null, canonicalBytes(head), privateKey).toString('base64') }) };
  const signed = (edit = () => {}) => {
    const head = streamHead({ app, bundle, stream, seq: 7, release: 'old' });
    edit(head);
    head.signature = signer.sign(head);
    return head;
  };
  const found = (head) => {
    const bytes = Buffer.from(JSON.stringify(head) + '\n');
    return { json: head, bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
  };
  const rawFound = (text) => {
    const bytes = Buffer.from(text + '\n');
    return { json: JSON.parse(text), bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
  };
  const valid = signed();
  const history = Buffer.from(JSON.stringify({ envelope: valid }) + '\n');
  const tableFor = (candidate, withHistory = true) => classify({ app, opts: { platform: [] },
    origin: {
      kind: 'directory', describe: () => dir,
      get: async (name) => withHistory && name.endsWith('/releases/old.json') ? history : null,
      head: async () => candidate === null ? null : candidate?.bytes ? candidate : found(candidate),
      list: async (name) => withHistory && name.endsWith('/releases') ? ['old.json'] : [],
    },
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const missingSignature = signed(); delete missingSignature.signature;
  const badSignature = signed(); badSignature.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const forgedLow = signed((head) => { head.stream.seq = 2; }); forgedLow.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const forgedHuge = signed((head) => { head.stream.seq = Number.MAX_SAFE_INTEGER; }); forgedHuge.signature.ed25519 = Buffer.alloc(64).toString('base64');
  const noSeq = signed((head) => { head.stream.seq = 'seven'; });
  const decimal = rawFound(JSON.stringify(valid).replace('"seq":7', '"seq":7.0'));
  const exponent = rawFound(JSON.stringify(valid).replace('"seq":7', '"seq":7e0'));
  const zeroHead = signed((head) => { head.unknownNumber = 0; });
  const negativeZero = rawFound(JSON.stringify(zeroHead).replace('"unknownNumber":0', '"unknownNumber":-0'));
  const scalarHead = { ...valid, unknownText: String.fromCharCode(0xd800) };
  const loneSurrogate = rawFound(JSON.stringify(scalarHead));
  const variants = [
    missingSignature,
    badSignature,
    forgedLow,
    forgedHuge,
    noSeq,
    decimal,
    exponent,
    negativeZero,
    loneSurrogate,
    signed((head) => { head.stream.channel = 'beta'; }),
    signed((head) => { head.exact = 2; }),
    signed((head) => { delete head.plan.bytes; }),
  ];
  const validTable = await tableFor(valid);
  const reordered = { signature: valid.signature, stream: valid.stream, release: valid.release,
    plan: valid.plan, exact: valid.exact, assets: valid.assets, app: valid.app };
  const prettyBytes = Buffer.from(JSON.stringify(reordered, null, 2) + '\n');
  const pretty = { json: reordered, bytes: prettyBytes, sha256: createHash('sha256').update(prettyBytes).digest('hex') };
  const numberedKeys = signed((head) => { head.unknownKeys = { 2: 'two', 10: 'ten' }; });
  const numberedCanonical = canonicalBytes(numberedKeys).toString('utf8');
  let surrogateValueRefused = false;
  try { canonicalBytes({ value: String.fromCharCode(0xd800) }); }
  catch (error) { surrogateValueRefused = error.message.includes('not a Unicode scalar value'); }
  let surrogateKeyRefused = false;
  try { canonicalBytes({ [String.fromCharCode(0xdc00)]: 'value' }); }
  catch (error) { surrogateKeyRefused = error.message.includes('not a Unicode scalar value'); }
  const repaired = await Promise.all(variants.map((candidate) => tableFor(candidate)));
  const missingHeadTable = await tableFor(null);
  const emptyHeadTable = await tableFor(null, false);
  const unlistableTable = await classify({ app, opts: { platform: [] },
    origin: {
      kind: 'https', describe: () => 'https://origin.example', get: async () => null,
      head: async () => null, list: async () => null,
    },
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  let noHistoryRefused = false;
  try { await tableFor(badSignature, false); }
  catch (error) { noHistoryRefused = error.message.includes('release history has no authenticated sequence floor'); }

  let unknownHistoryRefused = false;
  let unknownHistoryWrote = false;
  const unknownOrigin = {
    kind: 'object', describe: () => 'unknown-object-origin', get: async () => null,
    head: async () => null, list: async () => null,
    withLock: async (_stream, body) => body(),
    put: async () => { unknownHistoryWrote = true; },
    putHead: async () => { unknownHistoryWrote = true; },
  };
  try {
    await publishStream({ origin: unknownOrigin,
      row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
      bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'unknown-history',
      snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  } catch (error) { unknownHistoryRefused = error.message.includes('cannot enumerate its authenticated release history'); }

  const badHistoryOrigin = (bytes) => ({
    kind: 'directory', describe: () => 'bad-history-origin',
    get: async (name) => name.endsWith('/releases/broken.json') ? bytes : null,
    head: async () => null, list: async () => ['broken.json'],
  });
  const foreignHistory = Buffer.from(JSON.stringify({ envelope: signed((head) => { head.app.id = 'com.exact.foreign'; }) }) + '\n');
  const badHistories = [Buffer.from('{not json'), Buffer.from(JSON.stringify({ envelope: missingSignature }) + '\n'), foreignHistory];
  const badHistoryClassifyRefused = [];
  for (const bytes of badHistories) {
    try {
      await classify({ app, opts: { platform: [] }, origin: badHistoryOrigin(bytes),
        channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
        release: 'next', web: dir, bundle, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
        platforms: ['linux'], wantOrigin: false });
      badHistoryClassifyRefused.push(false);
    } catch (error) { badHistoryClassifyRefused.push(error.message.includes('nonempty release history has no authenticated sequence floor')); }
  }
  let corruptHistoryPublishRefused = false;
  let corruptHistoryWrote = false;
  try {
    await publishStream({ origin: {
      ...badHistoryOrigin(Buffer.from('{not json')), withLock: async (_stream, body) => body(),
      put: async () => { corruptHistoryWrote = true; }, putHead: async () => { corruptHistoryWrote = true; },
    }, row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'corrupt-history',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  } catch (error) { corruptHistoryPublishRefused = error.message.includes('nonempty release history has no authenticated sequence floor'); }
  const vanishedTable = await classify({ app, opts: { platform: [] }, origin: badHistoryOrigin(null),
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'next', web: dir, bundle, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const vanishedHistoryUnavailable = vanishedTable.rows[0].action === 'unavailable'
    && vanishedTable.rows[0].reason.includes('disappeared while establishing');

  const origin = new DirectoryOrigin(dir);
  mkdirSync(join(dir, '.exact', stream.channel, compatibilityId), { recursive: true });
  writeFileSync(join(dir, '.exact', stream.channel, compatibilityId, 'exact.json'), found(forgedHuge).bytes);
  await origin.put(`.exact/${stream.channel}/${compatibilityId}/releases/old.json`, history, { immutable: true });
  const locked = await publishStream({ origin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'current', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'locked-repair',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const repairedHead = await origin.head(stream);
  const missingDir = mkdtempSync(join(tmpdir(), 'exact-missing-head-'));
  const missingOrigin = new DirectoryOrigin(missingDir);
  await missingOrigin.put(`.exact/${stream.channel}/${compatibilityId}/releases/old.json`, history, { immutable: true });
  const missingPublished = await publishStream({ origin: missingOrigin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'after-missing',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const hiddenDir = mkdtempSync(join(tmpdir(), 'exact-hidden-history-'));
  const hiddenOrigin = new DirectoryOrigin(hiddenDir);
  await hiddenOrigin.put(`.exact/${stream.channel}/${compatibilityId}/releases/.old.json`, history, { immutable: true });
  const hiddenTable = await classify({ app, opts: { platform: [] }, origin: hiddenOrigin,
    channel: stream.channel, snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'after-hidden', web: dir, bundle, compat: { linux: { id: compatibilityId, inputs: { store: { L: 'A' }, executors: [] } } },
    platforms: ['linux'], wantOrigin: false });
  const hiddenPublished = await publishStream({ origin: hiddenOrigin,
    row: { kind: 'stream', platform: 'linux', channel: stream.channel, compatibilityId, action: 'bundle', changes: [] },
    bundle, compat: { id: compatibilityId, inputs: {} }, app, signer, release: 'after-hidden',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} });
  const hiddenReleaseCli = spawnSync(process.execPath,
    [DEPLOY, 'caltrain', '--release', '.hidden', '--json'],
    { cwd: join(dirname(DEPLOY), '..'), encoding: 'utf8' });
  result('deploy calls only an admitted authenticated head current', validTable.rows[0].action === 'current'
    && validTable.rows[0].seq === 7 && inspectHead(found(valid), app, stream).usable && inspectHead(pretty, app, stream).usable
    && inspectHead(found(numberedKeys), app, stream).usable
    && numberedCanonical.includes('"unknownKeys":{"10":"ten","2":"two"}')
    && surrogateValueRefused && surrogateKeyRefused
    && missingHeadTable.rows[0].action === 'bundle' && missingHeadTable.rows[0].seq === 8
    && emptyHeadTable.rows[0].action === 'bundle' && emptyHeadTable.rows[0].seq === 1
    && unlistableTable.rows[0].action === 'unavailable'
    && unlistableTable.rows[0].reason.includes('cannot enumerate its authenticated release history')
    && repaired.every((table) => table.rows[0].action === 'bundle' && table.rows[0].seq === 8
      && table.rows[0].changes[0].name === 'exact.json' && table.rows[0].changes[0].change === 'repair')
    && noHistoryRefused && unknownHistoryRefused && !unknownHistoryWrote
    && badHistoryClassifyRefused.every(Boolean) && corruptHistoryPublishRefused && !corruptHistoryWrote
    && vanishedHistoryUnavailable
    && locked.action === 'published' && locked.seq === 8
    && missingPublished.action === 'published' && missingPublished.seq === 8
    && hiddenTable.rows[0].seq === 8 && hiddenPublished.seq === 8
    && hiddenReleaseCli.status === 1 && hiddenReleaseCli.stderr.includes('start with a letter or digit')
    && locked.changes[0].change === 'repair' && inspectHead(repairedHead, app, stream).usable,
  JSON.stringify({ missingHead: missingHeadTable.rows[0], emptyHead: emptyHeadTable.rows[0], unlistable: unlistableTable.rows[0],
    noHistoryRefused, unknownHistoryRefused, unknownHistoryWrote, badHistoryClassifyRefused, corruptHistoryPublishRefused, corruptHistoryWrote,
    vanishedHistoryUnavailable, hiddenTable: hiddenTable.rows[0], hiddenPublished: { action: hiddenPublished.action, seq: hiddenPublished.seq },
    hiddenReleaseCli: { status: hiddenReleaseCli.status, stderr: hiddenReleaseCli.stderr },
    locked: { action: locked.action, seq: locked.seq }, missingPublished: { action: missingPublished.action, seq: missingPublished.seq } }));
  rmSync(hiddenDir, { recursive: true, force: true });
  rmSync(missingDir, { recursive: true, force: true });
  rmSync(dir, { recursive: true, force: true });
}

{
  const dir = mkdtempSync(join(tmpdir(), 'exact-list-outage-'));
  const cohort = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  const origin = {
    kind: 'directory', describe: () => dir, get: async () => null, head: async () => null,
    list: async (name) => { if (name.endsWith('/releases')) return null; throw new OriginUnavailable('EACCES'); },
  };
  const table = await classify({ app: { id: 'com.exact.test', displayName: 'Test', dir, manifest: {} }, opts: { platform: [] },
    origin, channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
    release: 'test', web: dir, bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] },
    compat: { linux: { id: cohort, inputs: { store: { L: 'A' }, executors: [] } } }, platforms: ['linux'], wantOrigin: false });
  result('stream discovery outage retains the classified cohort row', table.rows.length === 1
    && table.rows[0].action === 'bundle' && table.notes.some((note) => note.includes('stream discovery unavailable')),
  JSON.stringify(table));
  rmSync(dir, { recursive: true, force: true });
}

// A dry-run renders network availability per row, including declared retired
// streams. A corrupt response remains a hard refusal rather than masquerading
// as a transient network problem.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-origin-table-'));
  writeFileSync(join(dir, 'index.html'), 'web');
  const app = { id: 'com.exact.test', displayName: 'Test', dir,
    manifest: { deploy: { streams: [{ channel: 'prod', compatibilityId: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' }] } } };
  const compat = {
    web: { id: 'wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww', inputs: {} },
    linux: { id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', inputs: { store: { L: 'A' }, executors: [] } },
  };
  const table = await classify({ app, opts: { platform: [] },
    origin: new HttpsOrigin('https://127.0.0.1:1'), channel: 'prod',
    snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir }, release: 'test', web: dir,
    bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] }, compat, platforms: ['linux'], wantOrigin: true });
  const unavailable = table.rows.filter((row) => row.action === 'unavailable');
  result('deploy tables preserve every row when HTTPS is unavailable', unavailable.length === 3
    && unavailable.some((row) => row.kind === 'origin')
    && unavailable.some((row) => row.platform === 'linux')
    && unavailable.some((row) => row.compatibilityId.startsWith('bbbb')),
  JSON.stringify(table.rows));
  rmSync(dir, { recursive: true, force: true });
}

{
  const dir = mkdtempSync(join(tmpdir(), 'exact-bad-head-'));
  const cohort = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  mkdirSync(join(dir, '.exact', 'prod', cohort), { recursive: true });
  writeFileSync(join(dir, '.exact', 'prod', cohort, 'exact.json'), '{not json');
  let message = '';
  try {
    await classify({ app: { id: 'com.exact.test', displayName: 'Test', dir, manifest: {} }, opts: { platform: [] },
      origin: new DirectoryOrigin(dir), channel: 'prod', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [], repo: dir },
      release: 'test', web: dir, bundle: { plan: { sha256: '0'.repeat(64) }, assets: [] },
      compat: { linux: { id: cohort, inputs: { store: { L: 'A' }, executors: [] } } }, platforms: ['linux'], wantOrigin: false });
  } catch (error) { message = error.message; }
  result('deploy does not hide a malformed head as unavailable', message.includes('is not JSON'), message);
  rmSync(dir, { recursive: true, force: true });
}

// Signing-key creation is one exclusive filesystem operation. Two publishers
// racing for an id cannot both report a public half and silently replace the
// private half behind the first one's result.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-keygen-'));
  const run = () => new Promise((done) => {
    const child = spawn(process.execPath, [DEPLOY, 'keygen', 'race', '--keys', dir, '--json']);
    let stdout = '', stderr = '';
    child.stdout.on('data', (bytes) => { stdout += bytes; });
    child.stderr.on('data', (bytes) => { stderr += bytes; });
    child.on('close', (code) => done({ code, stdout, stderr }));
  });
  const attempts = await Promise.all([run(), run()]);
  const winners = attempts.filter((attempt) => attempt.code === 0);
  const losers = attempts.filter((attempt) => attempt.code !== 0);
  let matches = false;
  try {
    const reported = Buffer.from(JSON.parse(winners[0]?.stdout ?? '{}').publicKey ?? '', 'base64');
    const actual = createPublicKey(createPrivateKey(readFileSync(join(dir, 'race.pem'))))
      .export({ type: 'spki', format: 'der' }).subarray(-32);
    matches = reported.equals(actual);
  } catch { /* the result below names the failed invariant */ }
  result('concurrent keygen has one truthful winner', winners.length === 1 && losers.length === 1
    && losers[0].stderr.includes('a signing key is never overwritten') && matches,
  attempts.map((attempt) => `exit ${attempt.code}: ${attempt.stdout}${attempt.stderr}`).join('\n'));
  rmSync(dir, { recursive: true, force: true });
}

// A release remains recognizable to a person without being the bake's lock
// or directory identity. Even an explicitly reused correlation id gets a
// separate stage, and generated ids in the same clock tick do not collide.
{
  const fixture = mkdtempSync(join(tmpdir(), 'exact-source-snapshot-'));
  const init = (repo, files) => {
    mkdirSync(repo, { recursive: true });
    spawnSync('git', ['init', '-q'], { cwd: repo });
    for (const [name, bytes] of Object.entries(files)) {
      mkdirSync(dirname(join(repo, name)), { recursive: true });
      writeFileSync(join(repo, name), bytes);
    }
    spawnSync('git', ['add', '-A'], { cwd: repo });
    return spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid', 'commit', '-qm', 'fixture'], { cwd: repo }).status === 0;
  };
  const internal = join(fixture, 'internal');
  const external = join(fixture, 'external');
  const exact = join(fixture, 'exact2');
  const cargoDep = join(fixture, 'cargo-dep');
  const linked = join(fixture, 'linked');
  const filtered = join(fixture, 'filtered');
  const outsideLink = join(fixture, 'outside-link.rs');
  const outsideTarget = join(fixture, 'outside-main.rs');
  let initialized = init(internal, { '.gitignore': '/apps/test/assets/ignored.txt\n/generated/\n/target/\n',
    'apps/test/app.contract': 'app Test\n', 'apps/test/assets/dist/published.txt': 'nested old\n',
    'host/runtime.rs': 'old\n', 'removed.txt': 'remove me\n' })
    && init(cargoDep, { '.gitignore': '/crates/fixture-dep/ignored.rs\n/generated/\n/target/\n',
      'Cargo.toml': '[workspace]\nmembers=["crates/fixture-dep"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\n',
      'shared.txt': '1\n',
      'generated/value.txt': 'ignored repository input\n',
      'crates/fixture-dep/Cargo.toml': '[package]\nname="fixture-dep"\nversion.workspace=true\nedition.workspace=true\n',
      'crates/fixture-dep/src/lib.rs': 'pub fn value() -> &\'static str { include_str!("../../../shared.txt").trim() }\n' })
    && init(external, { 'app.contract': 'app External\n',
      'Cargo.toml': '[package]\nname="external"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nfixture-dep={path="../cargo-dep/crates/fixture-dep"}\n',
      'src/lib.rs': 'pub fn value() -> &\'static str { fixture_dep::value() }\n',
      'src/main.rs': 'fn main() { println!("{}:{}", fixture_dep::value(), option_env!("RACE_VALUE").unwrap_or("sealed")); }\n' })
    && init(exact, { 'host/runtime.rs': 'old\n' })
    && init(linked, { 'app.contract': 'app Linked\n', 'src/main.rs': 'local\n' })
    && init(filtered, { 'app.contract': 'app Filtered\n', 'source.txt': 'WORKING SOURCE BYTES\n',
      'target-one.txt': 'ONE\n', 'target-two.txt': 'TWO\n' });
  if (initialized) {
    writeFileSync(outsideLink, 'external mutable bytes\n');
    writeFileSync(outsideTarget, 'fn main() { println!("live"); }\n');
    rmSync(join(linked, 'src/main.rs'));
    symlinkSync(outsideLink, join(linked, 'src/main.rs'));
    spawnSync('git', ['add', 'src/main.rs'], { cwd: linked });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'link'], { cwd: linked }).status === 0;
  }
  if (initialized) {
    spawnSync('git', ['config', 'filter.worktree.clean', 'sed s/WORKING/STORED/g'], { cwd: filtered });
    spawnSync('git', ['config', 'filter.worktree.smudge', 'sed s/STORED/WORKING/g'], { cwd: filtered });
    spawnSync('git', ['config', 'filter.worktree.required', 'true'], { cwd: filtered });
    writeFileSync(join(filtered, '.gitattributes'), 'source.txt filter=worktree\n');
    spawnSync('git', ['add', '--renormalize', '.'], { cwd: filtered });
    spawnSync('git', ['add', '.gitattributes'], { cwd: filtered });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'filter'], { cwd: filtered }).status === 0;
  }
  if (initialized) {
    symlinkSync('target-one.txt', join(filtered, 'alias.txt'));
    symlinkSync(join(realpathSync(filtered), 'alias.txt'), join(filtered, 'inside.txt'));
    spawnSync('git', ['add', 'alias.txt', 'inside.txt'], { cwd: filtered });
    initialized = spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'internal absolute link'], { cwd: filtered }).status === 0;
  }
  if (initialized) {
    initialized = spawnSync('cargo', ['generate-lockfile'], { cwd: external, stdio: 'ignore' }).status === 0;
    spawnSync('git', ['add', 'Cargo.lock'], { cwd: external });
    initialized = initialized && spawnSync('git', ['-c', 'user.name=Exact Test', '-c', 'user.email=exact@example.invalid',
      'commit', '-qm', 'lock'], { cwd: external }).status === 0;
  }
  const fixtureApp = (name, dir, workspace) => ({ name, dir, workspace, target: join(workspace, 'target'),
    crate: (kind) => `${name}-${kind}`, manifest: { app: { id: `com.exact.${name}`, name }, host: {}, deploy: {} },
    id: `com.exact.${name}`, displayName: name, origin: null, declared: false });
  const internalApp = fixtureApp('test', join(internal, 'apps/test'), internal);
  const externalApp = fixtureApp('external', external, external);
  const linkedApp = fixtureApp('linked', linked, linked);
  const filteredApp = fixtureApp('filtered', filtered, filtered);
  let escapingSymlinkRefused = false;
  try { snapshotOf(linkedApp, {}, linked); }
  catch (error) { escapingSymlinkRefused = error.message.includes('symlink src/main.rs') && error.message.includes('absolute'); }
  const linkRaceBin = join(fixture, 'link-race-bin');
  const linkRaceDone = join(fixture, 'link-race-done');
  mkdirSync(linkRaceBin);
  const linkRaceGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  writeFileSync(join(linkRaceBin, 'git'), `#!/bin/sh\n"${linkRaceGit}" "$@"\ncode=$?\ncase " $* " in\n  *" write-tree "*)\n    if [ ! -e "$EXACT_LINK_RACE_DONE" ]; then\n      ln -shf target-two.txt "$EXACT_LINK_RACE_ALIAS"\n      : > "$EXACT_LINK_RACE_DONE"\n    fi\n    ;;\nesac\nexit "$code"\n`);
  chmodSync(join(linkRaceBin, 'git'), 0o755);
  const linkRacePath = process.env.PATH;
  process.env.PATH = `${linkRaceBin}:${linkRacePath}`;
  process.env.EXACT_LINK_RACE_DONE = linkRaceDone;
  process.env.EXACT_LINK_RACE_ALIAS = join(filtered, 'alias.txt');
  let filteredSnapshot;
  try { filteredSnapshot = snapshotOf(filteredApp, {}, filtered); }
  finally {
    process.env.PATH = linkRacePath;
    delete process.env.EXACT_LINK_RACE_DONE;
    delete process.env.EXACT_LINK_RACE_ALIAS;
    rmSync(join(filtered, 'alias.txt'));
    symlinkSync('target-one.txt', join(filtered, 'alias.txt'));
  }
  const filteredMaterialized = materializeSnapshot(filteredSnapshot, deployRun(filteredApp.target, 'filtered'), filteredApp);
  const checkoutFilterPreserved = filteredSnapshot.id === filteredSnapshot.commit
    && readFileSync(join(filteredMaterialized.app.dir, 'source.txt'), 'utf8') === 'WORKING SOURCE BYTES\n'
    && !isAbsolute(readlinkSync(join(filteredMaterialized.app.dir, 'inside.txt')))
    && readFileSync(join(filteredMaterialized.app.dir, 'inside.txt'), 'utf8') === 'ONE\n';
  const linkedTarget = join(fixture, 'shared-target');
  mkdirSync(linkedTarget);
  symlinkSync(linkedTarget, join(internal, 'target'));
  const projectTmp = join(internal, 'project-tmp');
  mkdirSync(projectTmp);
  const priorTmp = process.env.TMPDIR;
  process.env.TMPDIR = projectTmp;
  let internalSnapshot;
  try { internalSnapshot = snapshotOf(internalApp, {}, internal); }
  finally {
    if (priorTmp === undefined) delete process.env.TMPDIR;
    else process.env.TMPDIR = priorTmp;
  }
  rmSync(join(internal, 'target'));
  rmSync(projectTmp, { recursive: true });

  // This is the race a before/after fingerprint cannot see: a transient edit
  // exists while the bake is being prepared, then the checkout is restored.
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient during bake\n');
  const cleanRun = deployRun(internalApp.target, 'clean-race');
  const cleanMaterialized = materializeSnapshot(internalSnapshot, cleanRun, internalApp);
  const cleanRaceBytes = readFileSync(join(cleanMaterialized.exactRoot, 'host/runtime.rs'), 'utf8');
  const projectTmpRejected = relative(internal, cleanMaterialized.sourceRoot).startsWith('..');
  writeFileSync(join(internal, 'host/runtime.rs'), 'old\n');

  // Restore the live file immediately after the capture freezes its tree. A
  // later live status read must not relabel those already-captured bytes as a
  // clean HEAD snapshot.
  const raceBin = join(fixture, 'race-bin');
  const raceDone = join(fixture, 'race-done');
  mkdirSync(raceBin);
  const realGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  writeFileSync(join(raceBin, 'git'), `#!/bin/sh\n"${realGit}" "$@"\ncode=$?\ncase " $* " in\n  *" write-tree "*|*" --binary "*)\n    if [ ! -e "$EXACT_CAPTURE_RACE_DONE" ]; then\n      printf 'old\\n' > "$EXACT_CAPTURE_RACE_FILE"\n      : > "$EXACT_CAPTURE_RACE_DONE"\n    fi\n    ;;\nesac\nexit "$code"\n`);
  chmodSync(join(raceBin, 'git'), 0o755);
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient during capture\n');
  const oldPath = process.env.PATH;
  process.env.PATH = `${raceBin}:${oldPath}`;
  process.env.EXACT_CAPTURE_RACE_FILE = join(internal, 'host/runtime.rs');
  process.env.EXACT_CAPTURE_RACE_DONE = raceDone;
  let racedSnapshot;
  try { racedSnapshot = snapshotOf(internalApp, { dirty: true }, internal); }
  finally {
    process.env.PATH = oldPath;
    delete process.env.EXACT_CAPTURE_RACE_FILE;
    delete process.env.EXACT_CAPTURE_RACE_DONE;
  }
  const racedMaterialized = materializeSnapshot(racedSnapshot, deployRun(internalApp.target, 'atomic-race'), internalApp);
  const atomicRaceCaptured = racedSnapshot.dirty && racedSnapshot.id !== racedSnapshot.commit
    && racedSnapshot.changes.some((change) => change.endsWith('host/runtime.rs'))
    && readFileSync(join(racedMaterialized.exactRoot, 'host/runtime.rs'), 'utf8') === 'transient during capture\n'
    && readFileSync(join(internal, 'host/runtime.rs'), 'utf8') === 'old\n';

  writeFileSync(join(internal, 'host/runtime.rs'), 'edited\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'nested edited\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored captured\n');
  rmSync(join(internal, 'removed.txt'));
  mkdirSync(join(internal, 'target'), { recursive: true });
  writeFileSync(join(internal, 'target/generated.bin'), 'not source\n');
  mkdirSync(join(internal, 'generated'));
  writeFileSync(join(internal, 'generated/output.bin'), 'ignored input root\n');
  let siblingRefused = false;
  try { snapshotOf(internalApp, {}, internal); }
  catch (error) { siblingRefused = error.message.includes('exact2') && error.message.includes('host/runtime.rs')
    && error.message.includes('assets/ignored.txt') && !error.message.includes('target/generated.bin'); }
  const dirtySnapshot = snapshotOf(internalApp, { dirty: true }, internal);
  writeFileSync(join(internal, 'host/runtime.rs'), 'transient replacement\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'transient nested replacement\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'transient ignored replacement\n');
  const dirtyRun = deployRun(internalApp.target, 'dirty-race');
  const dirtyMaterialized = materializeSnapshot(dirtySnapshot, dirtyRun, internalApp);
  writeFileSync(join(internal, 'host/runtime.rs'), 'edited\n');
  writeFileSync(join(internal, 'apps/test/assets/dist/published.txt'), 'nested edited\n');
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored captured\n');
  const capturedDirtyBytes = readFileSync(join(dirtyMaterialized.exactRoot, 'host/runtime.rs'), 'utf8') === 'edited\n'
    && readFileSync(join(dirtyMaterialized.app.dir, 'assets/dist/published.txt'), 'utf8') === 'nested edited\n'
    && readFileSync(join(dirtyMaterialized.app.dir, 'assets/ignored.txt'), 'utf8') === 'ignored captured\n'
    && !existsSync(join(dirtyMaterialized.exactRoot, 'removed.txt'))
    && readFileSync(join(dirtyMaterialized.exactRoot, 'generated/output.bin'), 'utf8') === 'ignored input root\n';
  writeFileSync(join(internal, 'apps/test/assets/ignored.txt'), 'ignored changed\n');
  const dirtyAgain = snapshotOf(internalApp, { dirty: true }, internal);
  let changedSnapshotRefused = false;
  try { snapshotOf(internalApp, { dirty: true, snapshot: dirtySnapshot.id }, internal); }
  catch (error) { changedSnapshotRefused = error.message.includes('same complete source set'); }

  let ignoredRootRefused = false;
  try { snapshotOf(externalApp, {}, exact); }
  catch (error) { ignoredRootRefused = error.message.includes('cargo') && error.message.includes('generated/value.txt'); }
  const externalSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  let partialPinRefused = false;
  try { snapshotOf(externalApp, { dirty: true, snapshot: externalSnapshot.commit }, exact); }
  catch (error) { partialPinRefused = error.message.includes('complete source set'); }
  const pinned = snapshotOf(externalApp, { dirty: true, snapshot: externalSnapshot.id.slice(0, 12) }, exact);
  const externalRun = deployRun(externalApp.target, 'external');
  const externalMaterialized = materializeSnapshot(externalSnapshot, externalRun, externalApp);
  const stagedGitProbe = spawnSync('git', ['rev-parse', '--show-toplevel'], {
    cwd: externalMaterialized.exactRoot, encoding: 'utf8',
  });
  const stagedGitSealed = stagedGitProbe.status !== 0 && stagedGitProbe.stdout.trim() === '';
  const ignoredRootCaptured = readFileSync(join(dirname(externalMaterialized.app.dir), 'cargo-dep/generated/value.txt'), 'utf8')
    === 'ignored repository input\n';
  writeFileSync(join(cargoDep, 'shared.txt'), '2\n');
  mkdirSync(join(external, '.cargo'));
  writeFileSync(join(external, '.cargo/config.toml'), '[env]\nRACE_VALUE="live ancestor"\n');
  const stagedRun = spawnSync('cargo', ['run', '--quiet', '--locked'], { cwd: externalMaterialized.app.workspace,
    env: { ...process.env, CARGO_TARGET_DIR: join(fixture, 'cargo-target') }, encoding: 'utf8' });
  const stagedDependencyStayedCaptured = stagedRun.status === 0 && stagedRun.stdout.trim() === '1:sealed';
  rmSync(join(external, '.cargo'), { recursive: true });
  writeFileSync(join(cargoDep, 'shared.txt'), '1\n');

  // Cargo accepts absolute path dependencies, but an immutable deploy cannot:
  // the staged manifest would otherwise reach back into the mutable checkout.
  const relativeManifest = readFileSync(join(external, 'Cargo.toml'), 'utf8');
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest.replace('../cargo-dep/crates/fixture-dep', join(cargoDep, 'crates/fixture-dep')));
  const absoluteSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  writeFileSync(join(cargoDep, 'shared.txt'), '2\n');
  let absoluteDependencyRefused = false;
  try { materializeSnapshot(absoluteSnapshot, deployRun(externalApp.target, 'absolute-dependency'), externalApp); }
  catch (error) { absoluteDependencyRefused = error.message.includes('resolves outside the captured source root'); }
  writeFileSync(join(cargoDep, 'shared.txt'), '1\n');
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest);

  writeFileSync(join(external, 'Cargo.toml'), `${relativeManifest}\n[[bin]]\nname="outside"\npath=${JSON.stringify(outsideTarget)}\n`);
  const absoluteTargetSnapshot = snapshotOf(externalApp, { dirty: true }, exact);
  let absoluteTargetRefused = false;
  try { materializeSnapshot(absoluteTargetSnapshot, deployRun(externalApp.target, 'absolute-target'), externalApp); }
  catch (error) { absoluteTargetRefused = error.message.includes('target outside') && error.message.includes('resolves outside'); }
  writeFileSync(join(external, 'Cargo.toml'), relativeManifest);

  writeFileSync(join(cargoDep, 'crates/fixture-dep/ignored.rs'), 'pub const CAPTURED: bool = true;\n');
  let ignoredDependencyRefused = false;
  try { snapshotOf(externalApp, {}, exact); }
  catch (error) { ignoredDependencyRefused = error.message.includes('cargo') && error.message.includes('crates/fixture-dep/ignored.rs'); }
  const dependencyDirty = snapshotOf(externalApp, { dirty: true }, exact);
  const dependencyRun = deployRun(externalApp.target, 'external-dependency');
  const dependencyMaterialized = materializeSnapshot(dependencyDirty, dependencyRun, externalApp);
  const capturedDependency = readFileSync(join(dirname(dependencyMaterialized.app.dir), 'cargo-dep/crates/fixture-dep/ignored.rs'), 'utf8')
    === 'pub const CAPTURED: bool = true;\n';
  rmSync(join(cargoDep, 'crates/fixture-dep/ignored.rs'));
  const rendered = renderTable({ release: 'test', snapshot: externalSnapshot, channel: 'prod',
    origin: { kind: 'directory', location: '/origin' }, notes: [], rows: [] });
  writeFileSync(join(exact, 'host/runtime.rs'), 'edited\n');
  let dependencyRefused = false;
  try { snapshotOf(externalApp, {}, exact); }
  catch (error) { dependencyRefused = error.message.includes('exact2') && error.message.includes('host/runtime.rs'); }
  result('deploy snapshots every source repository the bake reads', initialized
    && internalSnapshot.id === internalSnapshot.commit && internalSnapshot.sources.length === 1
    && internalSnapshot.sources[0].roles.join(',') === 'app,exact2'
    && cleanRaceBytes === 'old\n' && projectTmpRejected && checkoutFilterPreserved && atomicRaceCaptured
    && siblingRefused && dirtySnapshot.dirty && dirtySnapshot.id !== dirtyAgain.id
    && capturedDirtyBytes && changedSnapshotRefused
    && dirtySnapshot.changes.some((change) => change.endsWith('host/runtime.rs'))
    && dirtySnapshot.changes.some((change) => change.endsWith('apps/test/assets/dist/published.txt'))
    && dirtySnapshot.changes.some((change) => change.endsWith('apps/test/assets/ignored.txt'))
    && dirtySnapshot.changes.some((change) => change.endsWith('removed.txt'))
    && !dirtySnapshot.changes.some((change) => change.includes('target/generated.bin'))
    && dirtySnapshot.changes.some((change) => change.includes('generated/output.bin'))
    && /^[0-9a-f]{40}$/.test(externalSnapshot.id) && externalSnapshot.id !== externalSnapshot.commit
    && externalSnapshot.sources.length === 3 && partialPinRefused && pinned.id === externalSnapshot.id
    && relative(externalMaterialized.app.dir, externalMaterialized.exactRoot) === '../exact2'
    && stagedDependencyStayedCaptured && absoluteDependencyRefused && absoluteTargetRefused
    && stagedGitSealed && escapingSymlinkRefused
    && ignoredRootRefused && ignoredRootCaptured
    && ignoredDependencyRefused && capturedDependency
    && rendered.includes(`snapshot ${externalSnapshot.id.slice(0, 12)}`)
    && dependencyRefused,
  JSON.stringify({ initialized, internalSnapshot, cleanRaceBytes, projectTmpRejected, checkoutFilterPreserved, atomicRaceCaptured,
    siblingRefused, dirtySnapshot, dirtyAgain: dirtyAgain.id,
    capturedDirtyBytes, changedSnapshotRefused, externalSnapshot, partialPinRefused, pinned: pinned.id,
    externalLayout: relative(externalMaterialized.app.dir, externalMaterialized.exactRoot),
    stagedDependencyStayedCaptured, absoluteDependencyRefused, absoluteTargetRefused,
    stagedGitSealed, escapingSymlinkRefused,
    ignoredRootRefused, ignoredRootCaptured,
    ignoredDependencyRefused, capturedDependency, dependencyRefused }));
  rmSync(fixture, { recursive: true, force: true });
}

{
  const target = mkdtempSync(join(tmpdir(), 'exact-deploy-run-'));
  const now = new Date('2026-09-04T12:34:56.789Z');
  const ids = new Set(Array.from({ length: 32 }, () => defaultRelease('a'.repeat(40), now)));
  const first = deployRun(target, 'same-release');
  writeFileSync(join(first, 'still-here'), 'first');
  const second = deployRun(target, 'same-release');
  result('deploy ids and private stages do not collide in one clock tick', ids.size === 32
    && first !== second && readFileSync(join(first, 'still-here'), 'utf8') === 'first');
  rmSync(target, { recursive: true, force: true });
}

// Stream heads point only at immutable blobs. Their immutable audit record is
// prepared before the conditional head swap; a failed record cannot expose a
// head, and a failed head leaves the preceding one and every named blob whole.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-stream-commit-'));
  const origin = new DirectoryOrigin(dir);
  const compatibilityId = 'c'.repeat(32);
  const row = { kind: 'stream', platform: 'linux', channel: 'prod', compatibilityId,
    action: 'bundle', changes: [{ name: 'app.plan', change: 'new' }, { name: 'assets/icon.png', change: 'new' }] };
  const plan = Buffer.from('plan bytes');
  const asset = Buffer.from('asset bytes');
  const bundle = { plan: { bytes: plan, sha256: createHash('sha256').update(plan).digest('hex'), formatVersion: 4, kernelSchema: '0'.repeat(16) },
    assets: [{ name: 'assets/icon.png', bytes: asset, sha256: createHash('sha256').update(asset).digest('hex') }] };
  const app = { id: 'com.exact.test', displayName: 'Test', manifest: { deploy: {} } };
  const signer = { keyId: 'test', sign: () => ({ keyId: 'test', ed25519: Buffer.alloc(64).toString('base64') }) };
  const racedPath = blobPath('d'.repeat(64));
  const raced = await Promise.allSettled([
    origin.put(racedPath, Buffer.from('first'), { immutable: true }),
    origin.put(racedPath, Buffer.from('second'), { immutable: true }),
  ]);
  const racedBytes = await origin.get(racedPath);
  const immutableRaceHeld = raced.filter((attempt) => attempt.status === 'fulfilled').length === 1
    && raced.filter((attempt) => attempt.status === 'rejected').length === 1
    && (racedBytes.equals(Buffer.from('first')) || racedBytes.equals(Buffer.from('second')));
  const order = [];
  const put = origin.put.bind(origin);
  origin.put = async (rel, bytes, options) => { const result = await put(rel, bytes, options); order.push(`put ${rel}`); return result; };
  const putHead = origin.putHead.bind(origin);
  origin.putHead = async (...args) => { order.push('put head'); return putHead(...args); };
  const args = { origin, row, bundle, compat: { id: compatibilityId, inputs: {} }, app, signer,
    release: 'one', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, opts: {}, log: () => {} };
  await publishStream(args);
  const stream = { channel: 'prod', compatibilityId };
  const head = await origin.head(stream);
  const base = `.exact/prod/${compatibilityId}`;
  const recordPath = `${base}/releases/one.json`;
  const record = JSON.parse((await origin.get(recordPath)).toString('utf8'));
  let reusedRefused = false;
  const beforeRetry = order.length;
  try { await publishStream(args); }
  catch (error) { reusedRefused = error.message.includes('already has an immutable record'); }
  const names = await origin.list(base);
  const blobCards = [head.json.plan, ...head.json.assets];
  const recordBeforeHead = order.indexOf(`put ${recordPath}`) < order.indexOf('put head');

  const interrupted = [];
  for (const moment of ['before', 'after']) for (let stop = 1; stop <= 4; stop++) {
    const failedDir = mkdtempSync(join(tmpdir(), `exact-stream-failure-${moment}-${stop}-`));
    const failed = new DirectoryOrigin(failedDir);
    const oldPlan = Buffer.from('old plan');
    const oldBundle = { plan: { ...bundle.plan, bytes: oldPlan, sha256: createHash('sha256').update(oldPlan).digest('hex') }, assets: [] };
    await failed.put(blobPath(oldBundle.plan.sha256), oldPlan, { immutable: true });
    const oldHead = streamHead({ app, bundle: oldBundle, stream, seq: 1, release: 'old' });
    oldHead.signature = signer.sign(oldHead);
    const oldBytes = Buffer.from(JSON.stringify(oldHead) + '\n');
    await failed.withLock(stream, () => failed.putHead(stream, oldBytes));
    const rawPut = failed.put.bind(failed);
    let writes = 0;
    failed.put = async (...putArgs) => {
      writes++;
      if (writes === stop && moment === 'before') throw new Error(`injected before write ${stop}`);
      const result = await rawPut(...putArgs);
      if (writes === stop && moment === 'after') throw new Error(`injected after write ${stop}`);
      return result;
    };
    const rawHead = failed.putHead.bind(failed);
    failed.putHead = async (...headArgs) => {
      writes++;
      if (writes === stop && moment === 'before') throw new Error(`injected before write ${stop}`);
      const result = await rawHead(...headArgs);
      if (writes === stop && moment === 'after') throw new Error(`injected after write ${stop}`);
      return result;
    };
    let stopped = false;
    let result = null;
    try { result = await publishStream({ ...args, origin: failed, release: `failed-${moment}-${stop}` }); }
    catch (error) { stopped = error.message.includes('injected'); }
    const after = await failed.head(stream);
    interrupted.push((stop < 4
      ? stopped && after.bytes.equals(oldBytes)
      : moment === 'before'
        ? stopped && after.bytes.equals(oldBytes)
        : result?.action === 'published' && after.json.release === `failed-${moment}-${stop}`)
      && (await failed.get(blobPath(oldBundle.plan.sha256))).equals(oldPlan));
    rmSync(failedDir, { recursive: true, force: true });
  }

  result('stream publication commits immutable blobs and receipt before its head', recordBeforeHead
    && names.join(',') === 'exact.json,releases' && blobCards.every((card) => card.url === `../../blobs/${card.sha256}`)
    && blobCards.every((card) => existsSync(join(dir, '.exact', 'blobs', card.sha256)))
    && record.head.entryDigest === createHash('sha256').update(canonicalBytes(head.json)).digest('hex')
    && record.envelope.signature.keyId === 'test' && immutableRaceHeld && reusedRefused && order.length === beforeRetry
    && interrupted.every(Boolean));
  rmSync(dir, { recursive: true, force: true });
}

// A matching hand-written exact.json is not build identity. The agent must
// consume the complete private marker verifier before it drives a dist.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-dist-app-'));
  writeFileSync(join(dir, 'app.wasm'), 'wasm');
  writeFileSync(join(dir, 'exact.json'), '{"app":{"id":"com.exact.castle"}}\n');
  const app = { id: 'com.exact.castle', crate: (kind) => `castle-${kind}` };
  let refused = false;
  try { assertWebDistApp(dir, app); }
  catch (e) { refused = /not a complete build for selected app com\.exact\.castle/.test(e.message); }
  rmSync(dir, { recursive: true, force: true });
  result('web agent refuses an unauthenticated matching envelope', refused);
}

// A phone gets the caller's LAN dev URL through devicectl, never a path on
// this Mac. With no locator it remains a normal baked launch.
{
  const remote = deviceLaunchArgs('PHONE', 'com.example.app', { EXACT_DEV_PLAN: 'http://192.168.1.20:8765/' });
  const baked = deviceLaunchArgs('PHONE', 'com.example.app', {});
  let refused = false;
  try { deviceLaunchArgs('PHONE', 'com.example.app', { EXACT_DEV_PLAN: '/tmp/app.plan' }); }
  catch (e) { refused = /http\(s\) URL/.test(e.message); }
  result('device launch carries only a reachable dev-plan URL',
    remote.at(-3) === '--environment-variables'
      && remote.at(-2) === '{"EXACT_DEV_PLAN":"http://192.168.1.20:8765/"}'
      && remote.at(-1) === 'com.example.app'
      && !baked.includes('--environment-variables')
      && refused);
}

// All origin verbs reject links at the root, intermediate, and leaf boundary.
// Locks are permanent OS ownership, independent of age or claimed host/PID.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-origin-handles-'));
  const originDir = join(dir, 'origin');
  const outside = join(dir, 'outside');
  mkdirSync(originDir); mkdirSync(outside);
  writeFileSync(join(outside, 'secret'), 'private');
  const origin = new DirectoryOrigin(originDir);
  const stream = { channel: 'prod', compatibilityId: 'test' };
  const refusals = [];
  const refuse = async (fn) => { try { await fn(); return false; } catch { return true; } };
  symlinkSync(outside, join(originDir, 'escape'));
  symlinkSync(join(outside, 'secret'), join(originDir, 'linked'));
  symlinkSync(join(outside, 'absent'), join(originDir, 'dangling'));
  refusals.push(await refuse(() => origin.get('escape/secret')),
    await refuse(() => origin.list('escape')), await refuse(() => origin.get('linked')),
    await refuse(() => origin.list('linked')), await refuse(() => origin.put('linked', Buffer.from('public'))),
    await refuse(() => origin.put('dangling', Buffer.from('public'))),
    await refuse(() => origin.put('escape/new/deep/file', Buffer.from('public'))));
  symlinkSync(outside, join(originDir, '.exact'));
  refusals.push(await refuse(() => origin.putHead(stream, Buffer.from('{}'))),
    await refuse(() => origin.withLock(stream, async () => false)));
  rmSync(join(originDir, '.exact'));
  await origin.withLock(stream, async () => true);
  const lockPath = join(originDir, '.exact/prod/test/.lock');
  rmSync(lockPath);
  symlinkSync(join(outside, 'secret'), lockPath);
  refusals.push(await refuse(() => origin.withLock(stream, async () => false)));
  rmSync(lockPath);
  const headPath = join(originDir, '.exact/prod/test/exact.json');
  symlinkSync(join(outside, 'secret'), headPath);
  refusals.push(await refuse(() => origin.putHead(stream, Buffer.from('{}'))));
  rmSync(headPath);
  refusals.push(await refuse(() => copyStaticTreeIfPresent(join(originDir, 'escape/missing'), join(dir, 'static-candidate'))));
  const rootLink = join(dir, 'root-link'); symlinkSync(originDir, rootLink);
  refusals.push(await refuse(() => new DirectoryOrigin(rootLink).get('linked')),
    await refuse(() => new DirectoryOrigin(rootLink).put('new', Buffer.from('public'))));
  let agedHeld = false, otherHostHeld = false, replacedRefused = false, successorHeld = false;
  const priorHost = process.env.HOSTNAME;
  try {
    process.env.HOSTNAME = 'publisher-a';
    await origin.withLock(stream, async () => {
      utimesSync(lockPath, new Date(0), new Date(0));
      agedHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
      process.env.HOSTNAME = 'publisher-b';
      otherHostHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
      renameSync(lockPath, join(dir, 'retired-lock'));
      let entered;
      const ready = new Promise((resolve) => { entered = resolve; });
      let release;
      const pending = new Promise((resolve) => { release = resolve; });
      const successor = new DirectoryOrigin(originDir);
      // Keep the successor held until the old owner's finally has run.
      const held = successor.withLock(stream, async () => { entered(); await pending; });
      await ready;
      replacedRefused = await refuse(() => origin.putHead(stream, Buffer.from('{}')));
      origin.successor = { held, release };
    });
    successorHeld = await refuse(() => new DirectoryOrigin(originDir).withLock(stream, async () => false));
    origin.successor.release(); await origin.successor.held;
  } finally {
    if (priorHost === undefined) delete process.env.HOSTNAME; else process.env.HOSTNAME = priorHost;
  }
  const reusable = await origin.withLock(stream, async () => true);
  await origin.putHead(stream, Buffer.from('{"seq":1}'));
  const conditional = await refuse(() => origin.putHead(stream, Buffer.from('{"seq":2}')));
  result('origin handles reject every linked boundary and retain exclusive lock ownership',
    refusals.every(Boolean) && agedHeld && otherHostHeld && replacedRefused && successorHeld && reusable && conditional
    && readFileSync(join(outside, 'secret'), 'utf8') === 'private' && !existsSync(join(outside, 'new')),
    JSON.stringify({ refusals, agedHeld, otherHostHeld, replacedRefused, successorHeld, reusable, conditional }));
  rmSync(dir, { recursive: true, force: true });
}

// Apple packages the standalone app and then the sample host through the
// same static-file gate as web. A link introduced at either source boundary
// is refused instead of being followed into a signed bundle.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-apple-static-'));
  const source = join(dir, 'app');
  const bundle = join(dir, 'ExactIOS.app');
  const host = join(dir, 'ExactHostIOS.app');
  const outside = join(dir, 'private.txt');
  mkdirSync(join(source, 'assets'), { recursive: true });
  mkdirSync(join(source, 'gpu', 'shaders'), { recursive: true });
  writeFileSync(join(source, 'assets', 'logo.png'), 'image');
  writeFileSync(join(source, 'gpu', 'shaders', 'surface.wgsl'), 'shader');
  writeFileSync(outside, 'private');
  const appTrees = [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']];
  copyAppleStaticTrees(source, bundle, appTrees);
  copyAppleStaticTrees(bundle, host);
  const copied = readFileSync(join(bundle, 'assets', 'logo.png'), 'utf8') === 'image'
    && readFileSync(join(bundle, 'shaders', 'surface.wgsl'), 'utf8') === 'shader'
    && readFileSync(join(host, 'assets', 'logo.png'), 'utf8') === 'image'
    && readFileSync(join(host, 'shaders', 'surface.wgsl'), 'utf8') === 'shader';
  symlinkSync(outside, join(source, 'assets', 'linked'));
  let appRefused = false;
  try { copyAppleStaticTrees(source, join(dir, 'refused-app'), appTrees); }
  catch (error) { appRefused = error.message.includes('cannot be symlinks'); }
  symlinkSync(outside, join(bundle, 'assets', 'linked'));
  let hostRefused = false;
  try { copyAppleStaticTrees(bundle, join(dir, 'refused-host')); }
  catch (error) { hostRefused = error.message.includes('cannot be symlinks'); }
  rmSync(join(source, 'assets', 'linked'));
  const missingRoot = join(dir, 'missing-deck');
  symlinkSync(missingRoot, join(source, 'deck'));
  let danglingRootRefused = false;
  try { copyAppleStaticTrees(source, join(dir, 'refused-root'), appTrees); }
  catch (error) { danglingRootRefused = error.message.includes('must be a real directory'); }
  rmSync(dir, { recursive: true, force: true });
  result('Apple packages reject linked static files and roots', copied && appRefused && hostRefused && danglingRootRefused);
}

console.log(`\n${total - failed}/${total} passed`);
process.exit(failed ? 1 : 0);
