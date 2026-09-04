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
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { appManifestDigest, builtAppMatches, copyStaticTree, installStaticCandidate, listPublicFiles, publicFileCards, staticFile, webEnvelope } from '../host/web/serve.mjs';
import { assertWebDistApp } from './agent.mjs';
import { copyAppleStaticTrees, deviceLaunchArgs } from '../host/apple/build.mjs';
import { canonicalBytes, classify, defaultRelease, deployRun, inspectHead, publishStream, streamHead } from './deploy.mjs';
import { blobPath, DirectoryOrigin, HttpsOrigin, OriginUnavailable } from './origin.mjs';

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

  result('static candidates reject links and preserve last-good bytes', treeRefused && invalidRefused
    && linkRefused && missingRefused && readFileSync(join(target, 'ok.txt'), 'utf8') === 'good'
    && installed.toString() === 'next good' && readFileSync(join(target, 'live.txt'), 'utf8') === 'next good');
  rmSync(dir, { recursive: true, force: true });
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
  rmSync(dir, { recursive: true, force: true });
  result('Apple app and sample-host packages reject linked static files', copied && appRefused && hostRefused);
}

console.log(`\n${total - failed}/${total} passed`);
process.exit(failed ? 1 : 0);
