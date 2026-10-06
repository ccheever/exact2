#!/usr/bin/env bun
// Reference test map for the T3 Code clone (task 20261005-reference-logic-tests-done-areas).
// Reads the read-only T3 Code checkout (pin 1e2ecbd975) and the clone's bun tests; the app
// build never runs it.
//
//   bun test-map.mjs list                      reference test files of the four packages, with case counts
//   bun test-map.mjs titles <ref-file>         describe/it/test titles of one reference file
//   bun test-map.mjs compare <ref-file> <clone-test>...   reference titles missing from the clone tests,
//                                              then clone-only titles
//   bun test-map.mjs scan                      for each reference file, the clone test with the most of its titles
//   bun test-map.mjs check <REFERENCE-TESTS.md>   completeness, one class per file, notes, ticket names,
//                                              and a compare for every done-equivalent row
//
// T3_REF names the reference checkout (default: <repo>/target/t3-ref/src-1e2ecbd975).
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { basename, dirname, join, relative, resolve } from 'node:path';

const here = dirname(new URL(import.meta.url).pathname);
const repo = process.env.T3_REPO ? resolve(process.env.T3_REPO) : (() => { let d = here; while (d !== '/' && !existsSync(join(d, 'Cargo.lock'))) d = dirname(d); return d; })();
const clone = join(repo, 'examples/t3-code');
const ref = resolve(process.env.T3_REF ?? join(repo, 'target/t3-ref/src-1e2ecbd975'));
const tasks = join(clone, '.exact/implementation/20261005-t3code-macos-parity/tasks');
const PACKAGES = ['apps/web/src', 'packages/client-runtime/src', 'packages/shared/src', 'apps/desktop/src'];
const GROUPS = ['packages/contracts', 'apps/server'];
const CLASSES = ['done-equivalent', 'port', 'swift', 'n/a-ui', 'n/a-server', 'n/a-excluded', 'later-ticket'];

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'dist' || name.startsWith('.')) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.test\.tsx?$/.test(name)) out.push(p);
  }
  return out;
}

// `git ls-files` when the checkout is a repository; a plain walk of the exported tree otherwise.
export function referenceFiles(dirs = PACKAGES) {
  if (existsSync(join(ref, '.git'))) {
    const out = execFileSync('git', ['-C', ref, 'ls-files', '--', ...dirs.flatMap((d) => [`${d}/**/*.test.ts`, `${d}/**/*.test.tsx`])], { encoding: 'utf8' });
    return out.split('\n').filter(Boolean).sort();
  }
  return dirs.flatMap((d) => walk(join(ref, d)).map((p) => relative(ref, p))).sort();
}

// Titles of describe/it/test calls (with .each/.effect/.skip/... modifiers), as written.
export function titles(source) {
  const re = /\b(describe|it|test)((?:\.(?:each|effect|live|scoped|skip|only|todo|concurrent|sequential|skipIf|runIf|layer|prop|fails|flakyTest))*)(?:\([^()]*(?:\([^()]*\)[^()]*)*\))?(?:\.(?:effect|live|scoped|skip|only|prop))*\(\s*(?:(['"])((?:\\.|(?!\3).)*)\3|`((?:\\.|[^`])*)`)/g;
  const out = [];
  for (const m of source.matchAll(re)) out.push({ kind: m[1], title: (m[4] ?? m[5] ?? '').replace(/\\(.)/g, '$1') });
  return out;
}

export const norm = (t) => t.toLowerCase().replace(/\$\{[^}]*\}|%[sdifjo#]/g, '*').replace(/[^a-z0-9*]+/g, ' ').trim();
const read = (p) => readFileSync(p, 'utf8');
const refTitles = (file) => titles(read(join(ref, file)));
const caseCount = (file) => refTitles(file).length;
const cloneTests = () => readdirSync(clone).filter((n) => n.endsWith('.test.ts')).sort();

export function compare(refFile, cloneFiles) {
  const want = refTitles(refFile);
  const have = cloneFiles.flatMap((f) => titles(read(join(clone, f))));
  const haveSet = new Set(have.map((t) => norm(t.title)));
  const wantSet = new Set(want.map((t) => norm(t.title)));
  return {
    reference: want.length,
    missing: want.filter((t) => !haveSet.has(norm(t.title))).map((t) => `${t.kind} ${t.title}`),
    cloneOnly: have.filter((t) => !wantSet.has(norm(t.title))).map((t) => `${t.kind} ${t.title}`),
  };
}

function parseMap(path) {
  const rows = [];
  for (const line of read(path).split('\n')) {
    if (!line.startsWith('| `') ) continue;
    const cells = line.split('|').slice(1, -1).map((c) => c.trim());
    if (cells.length < 5) continue;
    const [file, , cls, cloneCell, notes] = cells;
    rows.push({ file: file.replace(/`/g, ''), cls, clone: cloneCell, notes, line });
  }
  return rows;
}

function check(mapPath) {
  const rows = parseMap(mapPath);
  const files = referenceFiles();
  const problems = [];
  const byFile = new Map();
  for (const r of rows) byFile.set(r.file, [...(byFile.get(r.file) ?? []), r]);
  for (const f of files) if (!byFile.has(f)) problems.push(`unmapped: ${f}`);
  for (const [f, rs] of byFile) {
    if (rs.length > 1) problems.push(`${rs.length} rows: ${f}`);
    if (!files.includes(f) && !GROUPS.includes(f)) problems.push(`not a reference test file: ${f}`);
  }
  for (const g of GROUPS) if (!byFile.has(g)) problems.push(`missing group row: ${g}`);
  const ticketNames = new Set(readdirSync(tasks).map((n) => n.replace(/\.md$/, '')));
  const counts = Object.fromEntries(CLASSES.map((c) => [c, 0]));
  const proofs = [];
  for (const r of rows) {
    const base = r.cls.startsWith('later-ticket') ? 'later-ticket' : r.cls;
    if (!CLASSES.includes(base)) { problems.push(`bad class "${r.cls}": ${r.file}`); continue; }
    if (!GROUPS.includes(r.file)) counts[base]++;
    if (!r.notes || r.notes === '—') problems.push(`no note: ${r.file}`);
    if (base === 'later-ticket') {
      const t = r.cls.replace(/^later-ticket:\s*/, '').replace(/`/g, '');
      if (!ticketNames.has(t)) problems.push(`unknown ticket "${t}": ${r.file}`);
    }
    for (const t of r.notes.matchAll(/`(20261005-[a-z0-9-]+)`/g)) if (!ticketNames.has(t[1])) problems.push(`unknown ticket in note "${t[1]}": ${r.file}`);
    if (base === 'done-equivalent') {
      const named = [...r.clone.matchAll(/`([^`]+\.test\.ts)`/g)].map((m) => m[1]);
      if (!named.length) { problems.push(`done-equivalent without a clone test: ${r.file}`); continue; }
      const c = compare(r.file, named);
      proofs.push(`${r.file} -> ${named.join(', ')}: ${c.reference - c.missing.length}/${c.reference} reference titles present; ${c.cloneOnly.length} clone-only`);
      for (const m of c.missing) problems.push(`done-equivalent missing title: ${r.file}: ${m}`);
    }
  }
  console.log(`reference ${ref}`);
  console.log(`files ${files.length}; rows ${rows.length} (${rows.length - GROUPS.length} file rows + ${GROUPS.length} group rows)`);
  console.log(`classes ${CLASSES.map((c) => `${c} ${counts[c]}`).join(', ')}`);
  for (const p of proofs) console.log(`proof ${p}`);
  for (const p of problems) console.log(`problem ${p}`);
  console.log(problems.length ? `FAIL ${problems.length} problems` : 'OK');
  process.exit(problems.length ? 1 : 0);
}

const [cmd, ...args] = process.argv.slice(2);
if (cmd === 'list') {
  for (const f of referenceFiles()) console.log(`${f}\t${caseCount(f)}`);
} else if (cmd === 'titles') {
  for (const t of refTitles(args[0])) console.log(`${t.kind}\t${t.title}`);
} else if (cmd === 'compare') {
  const c = compare(args[0], args.slice(1));
  console.log(`${args[0]}: ${c.reference - c.missing.length}/${c.reference} reference titles present`);
  for (const m of c.missing) console.log(`missing ${m}`);
  for (const m of c.cloneOnly) console.log(`clone-only ${m}`);
  process.exit(c.missing.length ? 1 : 0);
} else if (cmd === 'scan') {
  const index = new Map();
  for (const f of cloneTests()) for (const t of titles(read(join(clone, f)))) {
    const k = norm(t.title); index.set(k, new Set([...(index.get(k) ?? []), f]));
  }
  for (const f of referenceFiles()) {
    const ts = refTitles(f).filter((t) => t.kind !== 'describe');
    const hits = new Map();
    let any = 0;
    for (const t of ts) { const s = index.get(norm(t.title)); if (s) { any++; for (const c of s) hits.set(c, (hits.get(c) ?? 0) + 1); } }
    const best = [...hits].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([c, n]) => `${c}:${n}`).join(' ');
    console.log(`${f}\t${ts.length}\t${any}\t${best}`);
  }
} else if (cmd === 'check') {
  check(args[0] ?? join(clone, 'REFERENCE-TESTS.md'));
} else {
  console.log('usage: test-map.mjs list | titles <ref-file> | compare <ref-file> <clone-test>... | scan | check [map]');
  process.exit(2);
}
