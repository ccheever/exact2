#!/usr/bin/env bun
/**
 * boot — the fifth check (rules/RULES.md §The five checks): parse and count
 * the module graph reachable before first pixel. A count cannot flake like a
 * timer, and parsing keeps valid HTML/ESM spellings from bypassing the count.
 */
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import vm from 'node:vm';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ALLOWED = new Set(['host/web/glue.js']);
const ROOT = resolve(new URL('..', import.meta.url).pathname);

// The page is deliberately small, so a fail-closed tokenizer is preferable
// to a dependency. It recognizes HTML comments, quoted and unquoted
// attributes, and reports malformed script tags instead of ignoring them.
function scriptTags(html, problems) {
  const scripts = [];
  let at = 0;
  while (at < html.length) {
    const lt = html.indexOf('<', at);
    if (lt < 0) break;
    if (html.startsWith('<!--', lt)) {
      const end = html.indexOf('-->', lt + 4);
      if (end < 0) { problems.push('malformed HTML: unterminated comment'); break; }
      at = end + 3;
      continue;
    }
    const head = html.slice(lt, lt + 8).toLowerCase();
    if (!head.startsWith('<script') || !/[\s/>]/.test(html[lt + 7] ?? '')) { at = lt + 1; continue; }
    let quote = null;
    let end = lt + 7;
    for (; end < html.length; end++) {
      const ch = html[end];
      if (quote) { if (ch === quote) quote = null; continue; }
      if (ch === '"' || ch === "'") quote = ch;
      else if (ch === '>') break;
    }
    if (end >= html.length || quote) { problems.push('malformed HTML: unterminated <script> tag'); break; }
    const text = html.slice(lt + 7, end);
    const attrs = new Map();
    let i = 0;
    while (i < text.length) {
      while (/\s/.test(text[i] ?? '')) i++;
      if (i >= text.length || text[i] === '/') break;
      const start = i;
      while (i < text.length && !/[\s=/>]/.test(text[i])) i++;
      if (i === start) { problems.push('malformed HTML: unsupported <script> attribute syntax'); break; }
      const name = text.slice(start, i).toLowerCase();
      while (/\s/.test(text[i] ?? '')) i++;
      let value = '';
      if (text[i] === '=') {
        i++;
        while (/\s/.test(text[i] ?? '')) i++;
        if (i >= text.length) { problems.push(`malformed HTML: ${name}= has no value`); break; }
        if (text[i] === '"' || text[i] === "'") {
          const q = text[i++];
          const valueStart = i;
          while (i < text.length && text[i] !== q) i++;
          if (i >= text.length) { problems.push(`malformed HTML: unterminated ${name} value`); break; }
          value = text.slice(valueStart, i++);
        } else {
          const valueStart = i;
          while (i < text.length && !/[\s>]/.test(text[i])) i++;
          value = text.slice(valueStart, i);
        }
      }
      if (attrs.has(name)) problems.push(`malformed HTML: duplicate <script> ${name} attribute`);
      else attrs.set(name, value);
    }
    scripts.push(attrs);
    const lower = html.toLowerCase();
    const close = lower.indexOf('</script', end + 1);
    if (close < 0 || !/[\s>]/.test(html[close + 8] ?? '') || html.indexOf('>', close + 8) < 0) {
      problems.push('malformed HTML: <script> has no closing tag');
      break;
    }
    at = html.indexOf('>', close + 8) + 1;
  }
  return scripts;
}

// Remove comments and literal bodies while retaining code in ${...}; this is
// only for detecting dynamic import. Static imports and all syntax are parsed
// by the runtime's module parser below; none of the source is executed.
function codeOnly(source) {
  const out = [...source].fill(' ');
  const templates = [];
  let state = 'code';
  let quote = null;
  let braces = 0;
  for (let i = 0; i < source.length; i++) {
    const ch = source[i], next = source[i + 1];
    if (state === 'line') { if (ch === '\n') { state = 'code'; out[i] = ch; } continue; }
    if (state === 'block') { if (ch === '*' && next === '/') { state = 'code'; i++; } continue; }
    if (state === 'string') { if (ch === '\\') { i++; continue; } if (ch === quote) state = 'code'; continue; }
    if (state === 'template') {
      if (ch === '\\') { i++; continue; }
      if (ch === '`') { state = 'code'; continue; }
      if (ch === '$' && next === '{') { templates.push(braces); braces++; state = 'code'; i++; }
      continue;
    }
    if (ch === '/' && next === '/') { state = 'line'; i++; continue; }
    if (ch === '/' && next === '*') { state = 'block'; i++; continue; }
    if (ch === '"' || ch === "'") { state = 'string'; quote = ch; continue; }
    if (ch === '`') { state = 'template'; continue; }
    if (ch === '{') braces++;
    if (ch === '}') {
      braces--;
      if (templates.length && braces === templates[templates.length - 1]) { templates.pop(); state = 'template'; continue; }
    }
    out[i] = ch;
  }
  return out.join('');
}

function localModule(root, from, specifier, problems) {
  if (specifier.startsWith('./') || specifier.startsWith('../')) return resolve(dirname(from), specifier);
  if (specifier.startsWith('/')) return resolve(root, '.' + specifier);
  problems.push(`unsupported non-local module specifier in ${from.slice(root.length + 1)}: ${specifier}`);
  return null;
}

function run() {
  const root = ROOT;
  const page = resolve(root, 'host/web/index.html');
  const problems = [];
  if (!existsSync(page)) {
    console.log('boot — host/web/index.html is missing; nothing to count, and the check must not pass on nothing.');
    return 1;
  }
  const html = readFileSync(page, 'utf8');
  if (!existsSync(resolve(root, 'rules/RULES.md')) || !/App JS executed before first pixel\s*\|\s*none/i.test(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))) {
    problems.push('rules/RULES.md no longer declares "App JS executed before first pixel | none"; boot has no budget.');
  }
  const scripts = scriptTags(html, problems);
  const external = scripts.filter((attrs) => attrs.has('src'));
  const inline = scripts.length - external.length;
  if (!external.length) problems.push('the page has no external host-glue script to count');
  if (inline) problems.push(`${inline} inline <script> block(s) in the page; only the glue module may run.`);
  const queue = [];
  for (const attrs of external) {
    const specifier = attrs.get('src');
    if (!specifier) { problems.push('a <script src> has an empty source'); continue; }
    const file = localModule(root, page, specifier, problems);
    if (file) queue.push(file);
  }

  const seen = new Set();
  const sources = new Map();
  while (queue.length) {
    const file = queue.shift();
    if (seen.has(file)) continue;
    seen.add(file);
    const rel = file.slice(root.length + 1);
    if (!ALLOWED.has(rel)) problems.push(`module before first pixel is outside the allowed host paths: ${rel}`);
    if (rel.startsWith('apps/')) problems.push(`app JS before first pixel: ${rel}`);
    if (!existsSync(file)) { problems.push(`missing module: ${rel}`); continue; }
    const source = readFileSync(file, 'utf8');
    sources.set(file, source);
    let requests;
    try { requests = new vm.SourceTextModule(source, { identifier: file }).dependencySpecifiers; }
    catch (error) { problems.push(`invalid module syntax in ${rel}: ${error.message}`); continue; }
    if (/\bimport\s*\(/.test(codeOnly(source))) problems.push(`dynamic import before first pixel in ${rel}`);
    for (const request of requests) {
      const next = localModule(root, file, request, problems);
      if (next) queue.push(next);
    }
  }
  const wasm = (html.match(/\.wasm/g) ?? []).length + [...sources.values()].reduce((n, source) => n + (source.match(/\.wasm/g) ?? []).length, 0);
  const modules = [...sources].map(([file, source]) => ({
    path: file.slice(root.length + 1), bytes: Buffer.byteLength(source),
    sha256: createHash('sha256').update(source).digest('hex'),
  }));
  const report = { modules: seen.size, javascript_bytes: modules.reduce((n, m) => n + m.bytes, 0),
    html_bytes: Buffer.byteLength(html), files: modules, wasm_references: wasm, problems };
  if (process.argv.includes('--json')) console.log(JSON.stringify(report));
  else {
    console.log(`boot — modules reachable before first pixel: ${seen.size} (${[...seen].map((file) => file.slice(root.length + 1)).join(', ') || 'none'}); wasm references: ${wasm}`);
    console.log(`  reachable JavaScript: ${report.javascript_bytes} B; page: ${report.html_bytes} B (diagnostic sizes, no byte budget)`);
    for (const m of modules) console.log(`  ${m.path}: ${m.bytes} B; sha256 ${m.sha256}`);
    for (const problem of problems) console.log('  ' + problem);
    console.log(problems.length ? `${problems.length} violation(s).` : 'Allowed import paths only. This does not prove generic content, constant startup work, or absence of runtime-loaded code; metrics measures built artifacts and browser work.');
  }
  return problems.length ? 1 : 0;
}

const entry = process.argv[1]
  && realpathSync(resolve(process.argv[1])) === realpathSync(fileURLToPath(import.meta.url));
if (entry) process.exitCode = run();
