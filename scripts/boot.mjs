#!/usr/bin/env node
/**
 * boot — the fifth check (rules/RULES.md §The five checks): count the module
 * graph reachable before first pixel and fail when it grows. A count, so it
 * cannot flake the way a timer does.
 *
 * The budget is the rules file's own row: "App JS executed before first
 * pixel | none". The web host's page may load exactly its glue (host code)
 * and one wasm; any other module — and any module under apps/ — is a
 * violation. Reports every problem in one run; fails closed if the page is
 * missing.
 */
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const PAGE = resolve(ROOT, 'host/web/index.html');
const ALLOWED = new Set(['host/web/glue.js']);

const problems = [];
if (!existsSync(PAGE)) {
  console.log('boot — host/web/index.html is missing; nothing to count, and the check must not pass on nothing.');
  process.exit(1);
}
const html = readFileSync(PAGE, 'utf8');
if (!/App JS executed before first pixel\s*\|\s*none/i.test(readFileSync(resolve(ROOT, 'rules/RULES.md'), 'utf8'))) {
  problems.push('rules/RULES.md no longer declares "App JS executed before first pixel | none"; boot has no budget.');
}
// Static module graph: <script src> in the page, then static imports transitively.
const seen = new Set();
const queue = [...html.matchAll(/<script[^>]*src="([^"]+)"/g)].map((m) => resolve(dirname(PAGE), m[1]));
const inline = (html.match(/<script(?![^>]*src=)[^>]*>/g) ?? []).length;
if (inline > 0) problems.push(`${inline} inline <script> block(s) in the page; only the glue module may run.`);
while (queue.length) {
  const file = queue.shift();
  if (seen.has(file)) continue;
  seen.add(file);
  const rel = file.slice(ROOT.length + 1);
  if (!ALLOWED.has(rel)) problems.push(`module before first pixel is not host glue: ${rel}`);
  if (rel.startsWith('apps/')) problems.push(`app JS before first pixel: ${rel}`);
  if (!existsSync(file)) { problems.push(`missing module: ${rel}`); continue; }
  const src = readFileSync(file, 'utf8');
  for (const m of src.matchAll(/^\s*import\s+(?:[^'"]+from\s+)?['"]([^'"]+)['"]/gm)) queue.push(resolve(dirname(file), m[1]));
  for (const m of src.matchAll(/import\(\s*['"]([^'"]+)['"]\s*\)/g)) problems.push(`dynamic import before first pixel in ${rel}: ${m[1]}`);
}
const wasm = (html.match(/\.wasm/g) ?? []).length + [...seen].reduce((n, f) => n + (readFileSync(f, 'utf8').match(/\.wasm/g) ?? []).length, 0);
console.log(`boot — modules reachable before first pixel: ${seen.size} (${[...seen].map((f) => f.slice(ROOT.length + 1)).join(', ') || 'none'}); wasm references: ${wasm}`);
if (problems.length) { for (const p of problems) console.log('  ' + p); console.log(`${problems.length} violation(s).`); process.exit(1); }
console.log('Within budget: host glue only, no app JS.');
