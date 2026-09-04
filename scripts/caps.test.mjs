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
import { createPrivateKey, createPublicKey } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { staticFile } from '../host/web/serve.mjs';
import { classify } from './deploy.mjs';
import { DirectoryOrigin, HttpsOrigin, OriginUnavailable } from './origin.mjs';

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

{
  const dir = mkdtempSync(join(tmpdir(), 'exact-list-outage-'));
  const cohort = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
  const origin = {
    kind: 'directory', describe: () => dir, get: async () => null, head: async () => null,
    list: async () => { throw new OriginUnavailable('EACCES'); },
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

console.log(`\n${total - failed}/${total} passed`);
process.exit(failed ? 1 : 0);
