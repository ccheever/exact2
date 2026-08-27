#!/usr/bin/env node
/**
 * Proves every caps rule fires. A budget check that cannot fail is worse than
 * no check: it reports green while inspecting nothing, and everyone believes it.
 *
 * Each case builds a throwaway repository, breaks exactly one thing, and asserts
 * the matching code appears. The last case asserts a clean repository passes, so
 * the suite cannot be satisfied by a check that simply always fails.
 */

import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const CAPS = join(dirname(fileURLToPath(import.meta.url)), 'caps.mjs');

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
console.log(`\n${cases.length - failed}/${cases.length} passed`);
process.exit(failed ? 1 : 0);
