// The extension's problem matcher reads what the Contract compiler prints, and
// the repository's .vscode/tasks.json names scripts that exist. VS Code runs
// the matcher's regexp as a JavaScript RegExp, as this does.
import { spawnSync } from 'node:child_process';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '../..');
const matcher = JSON.parse(readFileSync(resolve(import.meta.dir, 'package.json'), 'utf8')).contributes.problemMatchers
  .find(m => m.name === 'exact-contract');

test('the problem matcher reads each diagnostic the compiler prints, and only those', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-vscode-'));
  try {
    writeFileSync(resolve(dir, 'bad.contract'), 'shape A\n  x: string\n\nshape A\n  y: string\n\ncomponent Main\n  view\n    div\n');
    // Relative to where it runs, as a task in an app's folder runs it.
    const built = spawnSync('bun', [resolve(ROOT, 'scripts/exact.mjs'), 'contract', 'build', 'bad.contract'], { cwd: dir, encoding: 'utf8' });
    assert.equal(built.status, 1, built.stderr);
    const { regexp, file, line, column, code, message } = matcher.pattern, pattern = new RegExp(regexp);
    const found = built.stderr.split('\n').map(l => pattern.exec(l)).filter(Boolean)
      .map(m => ({ file: m[file], line: m[line], column: m[column], code: m[code], message: m[message] }));
    assert.deepEqual(found.map(({ message: _, ...rest }) => rest), [
      { file: 'bad.contract', line: '4', column: '1', code: 'type-duplicate-shape' },
      { file: 'bad.contract', line: '9', column: '5', code: 'lower-unknown-tag' },
    ]);
    assert.match(found[1].message, /^unknown tag `div`/);
    // A related note (CompileError's Display: indented, `file:line:col: note`) is not a diagnostic of its own.
    assert.equal(pattern.exec('  bad.contract:1:1: first declared here'), null);
    // A Windows path keeps its drive letter.
    assert.equal(pattern.exec('C:\\app\\app.contract:2:3 [syntax-x] y')?.[file], 'C:\\app\\app.contract');
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 600_000); // A cold checkout builds the compiler first.

test('every script the repository tasks run exists, and each matcher is one VS Code can resolve', () => {
  const { tasks } = JSON.parse(readFileSync(resolve(ROOT, '.vscode/tasks.json'), 'utf8'));
  const known = new Set(['$exact-contract', '$rustc']);
  for (const t of tasks) {
    for (const [, script] of (t.command ?? '').matchAll(/\bbun (\S+\.mjs)/g)) assert.ok(existsSync(resolve(ROOT, script)), `${t.label}: ${script}`);
    for (const m of [t.problemMatcher ?? []].flat()) assert.ok(known.has(m), `${t.label}: ${m}`);
    for (const d of t.dependsOn ?? []) assert.ok(tasks.some(other => other.label === d), `${t.label}: no task ${d}`);
  }
});
