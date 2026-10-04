// Proves caps rules fire in throwaway repositories and a clean repo passes;
// also exercises the shared build and launch helpers.
// `bun test ./scripts/caps.test.mjs`.
import { test } from 'bun:test';
// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = `${process.env.CARGO_HOME ?? `${process.env.HOME}/.cargo`}/bin`;
if (!(process.env.PATH ?? '').split(':').includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}:${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import { applyStaticChange, applyStaticTreeChange, appManifestDigest, builtAppMatches, copyStaticTree, copyStaticTreeIfPresent, installStaticCandidate, listAssets, listPublicFiles, publicFileCards, readDevGeneration, retainDevGeneration, shaderInterfaceDigests, staticFile, readStaticFile, serveStatic, syncStaticTree, watchStaticTrees, webEnvelope } from '../host/web/serve.mjs';
import { assertWebDistApp, jsonLines } from './agent.mjs';
import { verifyBakeFiles, pendingBuildInputs } from './app.mjs';
import { newerThan, notBuildInput } from './agent-launch.mjs';
import { copyAppleStaticTrees } from '../host/apple/build.mjs';
import { developmentLaunchEnvironment, deviceLaunchArgs } from '../host/apple/devices.mjs';
import { classify, publishRoot, webRelease } from './deploy.mjs';
import { DirectoryOrigin, webRootPath, webReleasePath, sha256 } from './origin.mjs';
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
  const r = spawnSync(process.execPath, [CAPS], { cwd: dir, encoding: 'utf8' });
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

// Each case is checked while the file loads and reported as a bun:test test.
function result(name, ok, detail = '') {
  test(name, () => { if (!ok) throw new Error(detail || `${name}: failed`); });
}
for (const [name, files, expect] of cases) {
  const dir = repo(files);
  const { out, code } = run(dir);
  rmSync(dir, { recursive: true, force: true });
  const ok = expect ? (code === 1 && out.includes(expect)) : code === 0;
  result(name, ok, `expected ${expect ? `code 1 with "${expect}"` : 'code 0'}, got code ${code}\n${out}`);
}
async function rejects(action, matches) {
  try { await action(); return false; } catch (error) { return matches(error); }
}
// The boot check's parser must see every valid spelling that can execute.
const BOOT_RULES = GOOD_RULES.replace('| Cold start to interactive | 100ms |', '| Cold start to interactive | 100ms |\n| App JS executed before first pixel | none |');
function boot(html, files = {}) {
  // The pinned capture script is the one boot.mjs checks every page against.
  const capture = readFileSync(join(dirname(BOOT), '../host/web/capture.js'), 'utf8');
  const dir = repo({ 'rules/RULES.md': BOOT_RULES, 'host/web/index.html': html, 'host/web/glue.js': '', 'host/web/document-glue.js': '', 'host/web/capture.js': capture, ...files });
  mkdirSync(join(dir, 'scripts'), { recursive: true });
  const fixtureBoot = join(dir, 'scripts/boot.mjs');
  copyFileSync(BOOT, fixtureBoot);
  const r = spawnSync(process.execPath, [fixtureBoot], { cwd: dir, encoding: 'utf8' });
  rmSync(dir, { recursive: true, force: true });
  return { code: r.status, out: r.stdout + r.stderr };
}
for (const [name, html, files, expectCode, expect] of [
  ['boot sees a single-quoted src', "<script type='module' src='./glue.js'></script>", {}, 0, 'modules reachable before first pixel: 1'],
  ['boot sees an unquoted src', '<script type=module src=./glue.js></script>', {}, 0, 'modules reachable before first pixel: 1'],
  ['boot follows re-exports', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': "export * from '../../apps/app.js';\n", 'apps/app.js': '' }, 1, 'app JS before first pixel'],
  ['boot ignores comments', '<!-- <script src=../../apps/bypass.js></script> --><script src=./glue.js></script>', { 'host/web/glue.js': "// export * from '../../apps/bypass.js';\n/* import '../../apps/also.js'; */\n" }, 0, 'modules reachable before first pixel: 1'],
  ['boot rejects malformed JavaScript', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': 'export const =' }, 1, 'invalid module syntax'],
  ['boot retains unused static imports', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': "import unused from '../../apps/app.js';", 'apps/app.js': 'export default 1;' }, 1, 'app JS before first pixel'],
  ['boot rejects computed dynamic imports', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': 'import(globalThis.modulePath);' }, 1, 'dynamic import before first pixel'],
  ['document entry rejects an imported runtime', '<script type=module src=./glue.js></script>', { 'host/web/document-glue.js': "import './glue.js';" }, 1, 'document-glue.js must have no imports'],
  ['document entry rejects a dynamic app import', '<script type=module src=./glue.js></script>', { 'host/web/document-glue.js': 'import(globalThis.modulePath);' }, 1, 'document-glue.js must have no imports'],
  ['boot rejects top-level return', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': 'return;' }, 1, 'invalid module syntax'],
  ['boot enforces module strict mode', '<script type=module src=./glue.js></script>', { 'host/web/glue.js': 'with ({}) {}' }, 1, 'invalid module syntax'],
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
// Retained namespaces outlive the current process's discovery head. Quota
// refusal, malformed routes and interrupted writes never damage a live deck.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-dev-retained-')), cache = join(dir, 'cache');
  const epoch = 'a'.repeat(32), prefix = `/__dev/generation/${epoch}/1/`;
  const files = (seq) => {
    const plan = Buffer.from('plan'), page = Buffer.from(`guest ${seq}`);
    const card = (body) => ({ bytes: body.length, sha256: sha256(body) });
    return new Map([['app.plan', plan], ['deck/nested/page.html', page], ['exact.json', Buffer.from(JSON.stringify({
      exact: 1, dev: { epoch, seq }, plan: card(plan), assets: [{ name: 'deck/nested/page.html', ...card(page) }],
    }))]]);
  };
  const first = files(1); retainDevGeneration(cache, epoch, 1, first);
  const full = await rejects(() => retainDevGeneration(cache, epoch, 2, files(2), 48 + [...first.values()].reduce((n, b) => n + b.length, 0)), error => error.message.includes(cache) && error.message.includes('close all pages'));
  const refusedWhole = readDevGeneration(cache, `/__dev/generation/${epoch}/2/exact.json`) === null;
  for (let seq = 2; seq <= 7; seq++) retainDevGeneration(cache, epoch, seq, files(seq));
  retainDevGeneration(cache, epoch, 1, first);
  result('retained dev URLs survive later generations and quota refusal', full && refusedWhole
    && readDevGeneration(cache, prefix + 'deck/nested/page.html')?.body.toString() === 'guest 1');
  mkdirSync(join(cache, epoch, '8', 'deck'), { recursive: true });
  writeFileSync(join(cache, epoch, '8', 'deck', 'partial.html'), 'partial');
  const paths = ['../exact.json', '%2e%2e/exact.json', 'deck/%2e%2e/exact.json', 'deck%5csecret', 'deck/%00', '.retained/.lock', 'deck/unknown.html'];
  const refused = paths.every((path) => readDevGeneration(cache, prefix + path) === null)
    && readDevGeneration(cache, `/__dev/generation/${epoch}/8/deck/partial.html`) === null;
  // Model cache damage after an OS crash: an envelope may outlive its files.
  rmSync(join(cache, epoch, '7', 'app.plan'));
  writeFileSync(join(cache, epoch, '6', 'app.plan'), 'broken');
  writeFileSync(join(cache, epoch, '5', 'exact.json'), '{');
  result('damaged dev cache payloads and envelopes fail closed after restart',
    readDevGeneration(cache, `/__dev/generation/${epoch}/7/app.plan`) === null
      && readDevGeneration(cache, `/__dev/generation/${epoch}/6/app.plan`) === null
      && readDevGeneration(cache, `/__dev/generation/${epoch}/5/exact.json`) === null
      && readDevGeneration(cache, prefix + 'app.plan')?.body.toString() === 'plan');
  const secret = join(dir, 'secret'); writeFileSync(secret, 'outside');
  const page = join(cache, epoch, '1', 'deck', 'nested', 'page.html'); rmSync(page); symlinkSync(secret, page);
  result('retained dev routes reject links, traversal, undeclared and partial files', refused
    && readDevGeneration(cache, prefix + 'deck/nested/page.html') === null && readFileSync(secret, 'utf8') === 'outside');
  rmSync(dir, { recursive: true, force: true });
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
  const treeRefused = await rejects(() => copyStaticTree(source, join(dir, 'other')), error => error.message.includes('cannot be symlinks'));
  rmSync(join(source, 'linked.txt'));
  writeFileSync(join(target, 'live.txt'), 'last good');
  writeFileSync(join(source, 'live.txt'), 'invalid');
  const invalidRefused = await rejects(() => installStaticCandidate(source, 'live.txt', join(target, 'live.txt'), () => { throw new Error('invalid shader'); }), error => error.message === 'invalid shader');
  rmSync(join(source, 'live.txt'));
  symlinkSync(outside, join(source, 'live.txt'));
  const linkRefused = await rejects(() => installStaticCandidate(source, 'live.txt', join(target, 'live.txt')), error => error.message.includes('cannot be symlinks'));
  rmSync(join(source, 'live.txt'));
  const missingRefused = await rejects(() => installStaticCandidate(source, 'live.txt', join(target, 'live.txt')), error => error.code === 'ENOENT');
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
  const brokenRootRefused = await rejects(() => syncStaticTree(startupSource, startupTarget), error => error.message.includes('must be a real directory'));
  const lastGoodRoot = readFileSync(join(startupTarget, 'last-good.txt'), 'utf8') === 'last good';
  rmSync(startupSource);
  const absentCopySkipped = copyStaticTreeIfPresent(startupSource, join(dir, 'absent-copy')) === false;
  symlinkSync(join(dir, 'still-does-not-exist'), startupSource);
  const brokenCopyRefused = await rejects(() => copyStaticTreeIfPresent(startupSource, join(dir, 'broken-copy')), error => error.message.includes('must be a real directory'));
  const shaderSource = join(dir, 'shader-source');
  const shaderTarget = join(dir, 'shader-target');
  const reflector = resolve(dirname(fileURLToPath(import.meta.url)), '..', process.env.CARGO_TARGET_DIR ?? 'target', 'debug/exact-gpu-reflect');
  if (!existsSync(reflector)) spawnSync('cargo', ['build', '-q', '-p', 'exact-gpu-reflect'], { cwd: resolve(dirname(fileURLToPath(import.meta.url)), '..') });
  mkdirSync(shaderSource);
  writeFileSync(join(shaderSource, 'surface.wgsl'), '@compute @workgroup_size(1) fn main() {}\n');
  const validateShaders = (candidate) => shaderInterfaceDigests(candidate, reflector);
  syncStaticTree(shaderSource, shaderTarget, validateShaders);
  writeFileSync(join(shaderSource, 'surface.wgsl'), 'not wgsl');
  const startupShaderRefused = await rejects(() => syncStaticTree(shaderSource, shaderTarget, validateShaders), error => error.message.includes('expected global item'));
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
// Independent dev payloads share a bounded queue; integrity still gates return.
{
  const origin = 'http://exact.test/', body = Buffer.from('payload');
  const card = name => ({ url: origin + name, bytes: body.length, sha256: sha256(body) });
  const assets = Array.from({ length: 5 }, (_, i) => ({ name: `assets/${i}.txt`, ...card(`assets/${i}.txt`) }));
  const module = Object.fromEntries(['native', 'receipt', 'web'].map(key => [key, card(key)]));
  const canonical = { assets: assets.map(({ name, bytes, sha256 }) => ({ bytes, name, sha256 })),
    module: Object.fromEntries(['native', 'receipt', 'web'].map(key => [key, { bytes: body.length, sha256: sha256(body) }])),
    plan: { bytes: body.length, sha256: sha256(body) } };
  const identity = { epoch: 'a'.repeat(32), seq: 1, generation: sha256(Buffer.from(JSON.stringify(canonical))), program: 'b'.repeat(64) };
  const envelope = { exact: 1, app: { id: 'com.exact.test' }, dev: identity, plan: card('app.plan'), assets, module };
  let pending = [], peak = 0, count = 0, corrupt = false;
  const context = { TextEncoder, TextDecoder, AbortController, URL, setTimeout, clearTimeout,
    performance, location: { href: origin, origin: new URL(origin).origin }, exact: { ready: Promise.resolve(), compat: { inputs: { app: 'com.exact.test' } } },
    fetch: async url => {
      if (url === origin + 'exact.json') return new Response(JSON.stringify(envelope));
      count++;
      return await new Promise(resolve => {
        pending.push(() => resolve(new Response(corrupt && url === origin + 'web' ? 'damaged' : body)));
        peak = Math.max(peak, pending.length);
      });
    } };
  runInNewContext(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../host/web/dev.js'), 'utf8'), context);
  const run = async () => {
    let done = false;
    const promise = context.fetchGeneration({ ...identity, envelope: origin + 'exact.json' }, new AbortController().signal)
      .then(value => ({ value }), error => ({ error })).finally(() => { done = true; });
    await new Promise(setImmediate);
    const first = pending.length;
    for (let i = 0; !done && i < 20; i++) {
      const batch = pending; pending = []; batch.forEach(resolve => resolve());
      await new Promise(setImmediate);
    }
    if (!done) throw new Error('dev fetch queue did not settle');
    return { first, ...await promise };
  };
  const good = await run();
  corrupt = true;
  const bad = await run();
  result('dev payloads fetch concurrently within four slots and refuse bad integrity',
    good.first === 4 && peak === 4 && count === 16 && good.value?.assets.size === 5 && good.value?.module
      && bad.error?.message.includes('payload integrity failed: module web'),
    JSON.stringify({first:good.first,peak,count,goodError:good.error?.message,badError:bad.error?.message,assets:good.value?.assets.size}));
}
// The protocol reconciles complete namespaces and rejects out-of-order
// completions. Its LAN hash path must match SHA-256 at block boundaries.
{
  const context = { TextEncoder, TextDecoder, AbortController, setTimeout, clearTimeout };
  runInNewContext(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../host/web/dev.js'), 'utf8'), context);
  const { digest, generationClient } = context.exactDevProtocol;
  let hashes = true;
  for (const size of [0, 1, 55, 56, 63, 64, 65, 127, 128, 10000]) {
    const bytes = Uint8Array.from({ length: size }, (_, i) => i * 31 % 256);
    hashes &&= await digest(bytes) === createHash('sha256').update(bytes).digest('hex');
  }
  const waiting = new Map(), applied = [], failures = [];
  let live = new Map();
  const client = generationClient({
    fetchGeneration: (message, signal) => new Promise((resolve, reject) => waiting.set(`${message.epoch}/${message.seq}`, { resolve, reject, signal, message })),
    apply: async (candidate, current) => { if (!current()) return false; live = candidate.assets; applied.push(candidate.generation); return true; },
    failed: (error) => failures.push(error.message),
  });
  const message = (epoch, seq, value) => ({ epoch: epoch.repeat(32), seq, generation: value.repeat(64) });
  const finish = (m, assets = []) => waiting.get(`${m.epoch}/${m.seq}`).resolve({ ...m, assets: new Map(assets) });
  const a1 = message('a', 1, '1'), a9 = message('a', 9, '9'), a10 = message('a', 10, 'a'), b1 = message('b', 1, 'b');
  let p = client.receive(a1); finish(a1, [['assets/image.png', 'old'], ['shaders/live.wgsl', 'old']]); await p;
  const slow = client.receive(a9), fast = client.receive(a10);
  finish(a10, [['assets/image.png', 'new']]); await fast; finish(a9, [['assets/image.png', 'stale']]); await slow;
  const replaced = live.size === 1 && live.get('assets/image.png') === 'new' && waiting.get(`${a9.epoch}/9`).signal.aborted;
  p = client.receive(b1); finish(b1, []); await p;
  const before = applied.length;
  await client.receive({ ...b1, seq: 2 }); await client.receive(a10);
  const c1 = message('c', 1, 'c'); p = client.receive(c1);
  waiting.get(`${c1.epoch}/1`).reject(new Error('candidate refused')); await p;
  const preserved = live.size === 0;
  p = client.receive(c1); finish(c1, [['assets/image.png', 'restored']]); await p;
  const c4 = message('c', 4, 'd'); p = client.receive(c4);
  waiting.get(`${c4.epoch}/4`).reject(new Error('network failed')); await p;
  const delayed = await client.receive(message('c', 3, 'e'));
  const keptBarrier = delayed === false && !waiting.has(`${c4.epoch}/3`);
  const refused = [];
  const refusing = generationClient({ fetchGeneration: async (m) => m, apply: async () => false, failed: (e) => { if (!e.hostRefused) throw new Error('host refusals must not retry'); refused.push(e.message); } });
  await refusing.receive(c1);
  let programReloads = 0, finishApply, beganApply;
  const applyingStarted = new Promise((resolve) => { beganApply = resolve; });
  const programApplies = [];
  const programClient = generationClient({
    fetchGeneration: async (m) => m,
    apply: async (m, current) => {
      await new Promise((resolve) => { finishApply = resolve; beganApply(); });
      if (!current()) return false;
      programApplies.push(m.generation); return true;
    },
    programChanged: () => { programReloads++; },
  });
  const applying = programClient.receive({ ...a1, program: 'a'.repeat(64) });
  await applyingStarted;
  // Mutable discovery and SSE use this same gate. A rebuilt program cancels
  // even a candidate already awaiting host-side font or shader acceptance.
  await programClient.receive({ ...b1, program: 'b'.repeat(64) });
  finishApply(); await applying;
  await programClient.receive({ ...b1, seq: 2, program: 'b'.repeat(64) });
  result('every dev revision gates program identity and cancels pending acceptance', programReloads === 1 && programApplies.length === 0);
  result('complete dev generations heal reconnects, restart epochs, removals and reversed fetches',
    hashes && replaced && preserved && before === 3 && applied.length === 4 && failures.length === 2 && keptBarrier && refused.length === 1 && live.get('assets/image.png') === 'restored');
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
  result('dev startup rejects an incomplete dist', !await builtAppMatches(dir, app));
  writeFileSync(join(dir, 'app.plan'), plan);
  for (const name of ['glue.js', 'navigation.js']) writeFileSync(join(dir, name), '');
  writeFileSync(join(dir, 'index.html'), '');
  writeFileSync(join(dir, 'manifest.json'), '{}');
  writeFileSync(join(dir, '.exact-build.json'), JSON.stringify({ exactBuild: 1,
    app: { id, name: app.displayName }, manifestSha256: appManifestDigest(app),
    files: await publicFileCards(dir) }));
  const complete = await builtAppMatches(dir, app);
  const staleName = !await builtAppMatches(dir, { ...app, displayName: 'Renamed' });
  const staleManifest = !await builtAppMatches(dir, { ...app, manifest: { ...app.manifest, theme_color: '#000000' } });
  const wasm = readFileSync(join(dir, 'app.wasm'));
  writeFileSync(join(dir, 'app.wasm'), 'another app');
  const replacedWasm = !await builtAppMatches(dir, app);
  writeFileSync(join(dir, 'app.wasm'), wasm);
  writeFileSync(join(dir, 'glue.js'), 'truncated');
  const changedRuntime = !await builtAppMatches(dir, app);
  writeFileSync(join(dir, 'glue.js'), '');
  const named = JSON.parse(readFileSync(join(dir, 'exact.json'), 'utf8'));
  delete named.app.id;
  writeFileSync(join(dir, 'exact.json'), JSON.stringify(named));
  const missingEnvelopeId = !await builtAppMatches(dir, app);
  const unnamedPlanRefused = await rejects(() => webEnvelope(app, Buffer.alloc(36), []), error => error.message.includes('nonempty app id'));
  writeFileSync(join(dir, 'app.plan'), 'corrupt');
  result('dev startup identifies only a complete coherent named app build', complete && staleName
    && staleManifest && replacedWasm && changedRuntime && missingEnvelopeId
    && unnamedPlanRefused && !await builtAppMatches(dir, app));
  rmSync(dir, { recursive: true, force: true });
}
// The completion marker is build-private. The exact same public inventory
// drives deploy classification, so it cannot leak to an origin even though it
// lives beside public artifacts in the completed stage.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-public-web-'));
  writeFileSync(join(dir, 'index.html'), 'public');
  writeFileSync(join(dir, '.exact-build.json'), 'private');
  writeFileSync(join(dir, 'app.plan.map.json'), '{"file":"private-source.contract"}');
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
  const directHeadRefused = await rejects(async () => await origin.put(`.exact/web/${'b'.repeat(64)}/exact.json`, Buffer.from('unguarded'), { immutable: true }), error => error.message.includes('stream heads require putHead'));
  whole &&= directHeadRefused;
  const rootChannel = await classify({ app, opts: {}, origin, channel: 'root', snapshot: { commit: '0'.repeat(40), dirty: false, changes: [] }, release: 'test', web: newWeb,
    bundle: { plan: { sha256: '0'.repeat(64), bytes: 3 }, assets: [] }, compat: { linux: { id: 'c'.repeat(32), inputs: { store: { L: 'A' }, executors: [] } } }, platforms: ['linux'], wantOrigin: false });
  whole &&= rootChannel.rows.length === 1 && rootChannel.rows[0].compatibilityId === 'c'.repeat(32);
  const server = createServer((req, res) => serveStatic(root, req, res));
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const url = `http://127.0.0.1:${server.address().port}`;
  let rounds = 0;
  const readGraph = async () => {
    const index = await fetch(url);
    const html = await index.text(), base = /<base href="([^"]+)"/.exec(html)?.[1];
    if (!base || index.headers.get('cache-control') !== 'no-cache') return false;
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
  const dir=mkdtempSync(join(tmpdir(),'exact-input-staleness-')),file=join(dir,'declared');
  writeFileSync(file,'replaced a watched directory');
  const pending=pendingBuildInputs({binary:{inputs:[{name:'source',path:dir,sha256:'0'.repeat(64)}],missing:[],directories:[{path:file,names:['old.h']}]}});
  result('dev reports replaced compiler input types as pending without terminating',pending.includes('source')&&pending.includes(file),JSON.stringify(pending));
  rmSync(dir,{recursive:true,force:true});
}
{
  // A screenshot or log saved into an app is not an input, ignored by Git or not;
  // what the bake captures, a declared shader root and a Rust crate's files are.
  const dir=realpathSync(mkdtempSync(join(tmpdir(),'exact-build-inputs-')));
  for(const sub of ['shots','gen','shader-gen','data']) mkdirSync(join(dir,sub));
  for(const [name,text] of [['.gitignore','/local.ts\n/gen/\n/shader-gen/\n'],['Cargo.toml','[workspace]'],['app.contract','view'],['local.ts','key'],['run.log','log'],['shots/one.png','png'],['shots/notes.txt','notes'],['gen/made.rs','fn f() {}'],['shader-gen/paint.wgsl','fn main() {}'],['shader-gen/table.bin','bytes'],['data/Cargo.toml','[package]'],['data/table.bin','bytes']]) writeFileSync(join(dir,name),text);
  const walk=()=>newerThan(0,[dir],notBuildInput(dir,[join(dir,'shader-gen')])).map(p=>relative(dir,p)).sort();
  const outside=walk();
  spawnSync('git',['init','-q'],{cwd:dir});
  const inside=walk(),want='["Cargo.toml","app.contract","data/Cargo.toml","data/table.bin","gen/made.rs","local.ts","shader-gen/paint.wgsl","shader-gen/table.bin"]';
  result('the staleness walk skips files no build reads, ignored or not',
    JSON.stringify(inside)===want&&JSON.stringify(outside)===want,JSON.stringify({inside,outside}));
  rmSync(dir,{recursive:true,force:true});
}
// A matching hand-written exact.json is not build identity. The agent must
// consume the complete private marker verifier before it drives a dist.
{
  const dir = mkdtempSync(join(tmpdir(), 'exact-dist-app-'));
  writeFileSync(join(dir, 'app.wasm'), 'wasm');
  writeFileSync(join(dir, 'exact.json'), '{"app":{"id":"com.exact.castle"}}\n');
  const app = { id: 'com.exact.castle', crate: (kind) => `castle-${kind}` };
  const refused = await rejects(() => assertWebDistApp(dir, app), e => /not a complete build for selected app com\.exact\.castle/.test(e.message));
  rmSync(dir, { recursive: true, force: true });
  result('web agent refuses an unauthenticated matching envelope', refused);
}
// Phone URL launch and closed-agent transport regressions.
{
  const stream = new (await import('node:stream')).PassThrough(), writes = [], lines = jsonLines(stream, {write: x => writes.push(x)}, []), pending = lines.ask({op:'state'}).catch(e => e.message);
  lines.fail('phone crashed'); lines.fail('socket closed'); const later = await lines.ask({op:'logs'}).catch(e => e.message);
  result('a dead agent rejects pending and future requests without writing again', await pending === 'phone crashed' && later === 'phone crashed' && writes.length === 1); stream.destroy();
  // A reply split over many chunks (a `state` carrying a large surface record), and two in one chunk.
  const split = new (await import('node:stream')).PassThrough(), host = [], replies = jsonLines(split, {write() {}}, host);
  const big = replies.ask({op:'state'}), next = replies.ask({op:'logs'}), third = replies.ask({op:'tree'});
  const text = JSON.stringify({record: 'x'.repeat(4 << 20)}) + '\n';
  for (let i = 0; i < text.length; i += 65536) split.write(text.slice(i, i + 65536));
  split.write('{"n":2}\n{"n":3}\nstray');
  const [a, b, c] = await Promise.all([big, next, third]); split.write('\n');
  result('agent replies split over chunks or sharing one are each read whole', a.record.length === 4 << 20 && b.n === 2 && c.n === 3 && host[0] === 'app: stray'); split.destroy();
  const inherited = { EXACT_DEV_PLAN: '/tmp/local.plan', PRESERVED: 'yes' };
  const launched = developmentLaunchEnvironment(['--run', '--url', 'http://192.168.1.20:8765'], inherited);
  const invalid = [['--url', 'https://example.test'], ['--run', '--url'], ['--run', '--url', '/tmp/app.plan'], ['--run', '--url', 'file:///tmp/app.plan'], ['--run', '--url', 'https://a.test', '--url', 'https://b.test']];
  const rejected = invalid.every((args) => { try { developmentLaunchEnvironment(args, inherited); return false; } catch { return true; } });
  result('explicit development URLs override only the launch locator and reject invalid arguments', rejected
    && launched.PRESERVED === 'yes' && inherited.EXACT_DEV_PLAN === '/tmp/local.plan'
    && developmentLaunchEnvironment([], inherited).EXACT_DEV_PLAN === inherited.EXACT_DEV_PLAN);
  const remote = deviceLaunchArgs('PHONE', 'com.example.app', launched);
  const baked = deviceLaunchArgs('PHONE', 'com.example.app', {});
  const refused = await rejects(() => deviceLaunchArgs('PHONE', 'com.example.app', { EXACT_DEV_PLAN: '/tmp/app.plan' }), e => /http\(s\) URL/.test(e.message));
  result('device launch carries only a reachable dev-plan URL',
    remote.at(-3) === '--environment-variables'
      && remote.at(-2) === '{"EXACT_DEV_PLAN":"http://192.168.1.20:8765/"}'
      && remote.at(-1) === 'com.example.app'
      && !baked.includes('--environment-variables')
      && refused);
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
  const plan = Buffer.from('the archived plan');
  const receipt = { embedded: { plan: { sha256: sha256(plan), bytes: plan.length }, assets: [
    { name: 'assets/logo.png', sha256: sha256(Buffer.from('image')), bytes: 5 },
    { name: 'shaders/surface.wgsl', sha256: sha256(Buffer.from('shader')), bytes: 6 },
  ] } };
  let matched = true;
  try { verifyBakeFiles(receipt, plan, listAssets(bundle).reverse()); }
  catch { matched = false; }
  const refuses = (bytes = plan) => {
    try { verifyBakeFiles(receipt, bytes, listAssets(bundle)); return false; }
    catch { return true; }
  };
  const changedPlan = refuses(Buffer.from('a changed plan'));
  rmSync(join(bundle, 'assets', 'logo.png'));
  const removed = refuses();
  writeFileSync(join(bundle, 'assets', 'logo.png'), 'other');
  const changed = refuses();
  writeFileSync(join(bundle, 'assets', 'logo.png'), 'image');
  writeFileSync(join(bundle, 'assets', 'extra.png'), 'extra');
  const added = refuses();
  rmSync(join(bundle, 'assets', 'extra.png'));
  result('Apple and web packaging enforce the baked plan and complete static roster', matched && changedPlan && removed && changed && added);
  symlinkSync(outside, join(source, 'assets', 'linked'));
  const appRefused = await rejects(() => copyAppleStaticTrees(source, join(dir, 'refused-app'), appTrees), error => error.message.includes('cannot be symlinks'));
  symlinkSync(outside, join(bundle, 'assets', 'linked'));
  const hostRefused = await rejects(() => copyAppleStaticTrees(bundle, join(dir, 'refused-host')), error => error.message.includes('cannot be symlinks'));
  rmSync(join(source, 'assets', 'linked'));
  const missingRoot = join(dir, 'missing-deck');
  symlinkSync(missingRoot, join(source, 'deck'));
  const danglingRootRefused = await rejects(() => copyAppleStaticTrees(source, join(dir, 'refused-root'), appTrees), error => error.message.includes('must be a real directory'));
  rmSync(dir, { recursive: true, force: true });
  result('Apple packages reject linked static files and roots', copied && appRefused && hostRefused && danglingRootRefused);
}
