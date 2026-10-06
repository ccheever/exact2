// Developer command (the app build never runs it): generates the clone's Shiki grammar data from
// pinned published packages (package.json and bun.lock next to this file; install them with
// `bun install --frozen-lockfile` here). The reference T3 Code (1e2ecbd975) colours code with
// Shiki 4.2's JavaScript regex engine, the @shikijs/langs grammars and the @pierre/theme
// pierre-light / pierre-dark themes; this compiles those grammars the way vscode-textmate's
// RuleFactory does (one id per rule, includes resolved ahead of time) and converts each regex the
// way Shiki's JavaScript engine does (oniguruma-to-es with Shiki's options, target ES2018 for
// Hermes), so r12-render-textmate.ts can run them with no Oniguruma and no regex conversion.
//
//   bun examples/t3-code/tools/grammar/gen-grammar.mjs          write the data files
//   bun examples/t3-code/tools/grammar/gen-grammar.mjs --check  exit 1 when they differ
//
// Output: one data module per language group (GROUPS below), each `r12-render-grammar*.ts`.

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { toRegExpDetails } from 'oniguruma-to-es';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const pkg = name => JSON.parse(readFileSync(join(here, 'node_modules', name, 'package.json'), 'utf8'));

/** The language groups: each is one generated file, its languages compiled in this order. */
//  Rule ids are per group, so a grammar includes only grammars of its own group compiled before it
//  (an include of any other grammar matches nothing); the first group also holds the themes.
export const GROUPS = [
  { file: 'r12-render-grammar.ts', langs: ['typescript', 'tsx', 'javascript', 'jsx', 'json', 'jsonc', 'shellscript', 'python', 'css', 'html', 'markdown', 'yaml', 'toml', 'rust', 'go', 'swift'] },
  // Decision U15 (proposed list): languages with a real-file corpus here, each checked against Shiki.
  { file: 'r12-render-grammar-more.ts', langs: ['c', 'java', 'kotlin', 'csharp', 'xml', 'diff', 'docker', 'make', 'ruby'] },
];

// Shiki 4.2's createJavaScriptRegexEngine options (@shikijs/engine-javascript), target ES2018
// so the patterns compile in Hermes (no `v` flag, no ES2025 modifiers).
const ENGINE = {
  global: true, hasIndices: true, target: 'ES2018',
  rules: { allowOrphanBackrefs: true, asciiWordBoundaries: true, captureGroup: true, recursionLimit: 5, singleline: true },
};

// ── vscode-textmate's RegExpSource: \z rewritten, \A / \G noted, back-references noted ─────────
function regExpSource(source) {
  let hasAnchor = false, out = '', last = 0;
  for (let pos = 0; pos < source.length; pos++) {
    if (source[pos] !== '\\' || pos + 1 >= source.length) continue;
    const next = source[pos + 1];
    if (next === 'z') { out += source.slice(last, pos) + '$(?!\\n)(?<!\\n)'; last = pos + 2; }
    else if (next === 'A' || next === 'G') hasAnchor = true;
    pos++;
  }
  out += source.slice(last);
  return { source: out, hasAnchor, hasBackReferences: /\\(\d+)/.test(out) };
}
const NEVER = String.fromCharCode(0xffff);
/** vscode-textmate's resolveAnchors: a disallowed \A or \G becomes U+FFFF (never matches). */
function resolveAnchors(source, allowA, allowG) {
  let out = '';
  for (let pos = 0; pos < source.length; pos++) {
    const ch = source[pos];
    out += ch;
    if (ch === '\\' && pos + 1 < source.length) {
      const next = source[++pos];
      if (next === 'A') out = out.slice(0, -1) + (allowA ? '\\A' : NEVER);
      else if (next === 'G') out = out.slice(0, -1) + (allowG ? '\\G' : NEVER);
      else out += next;
    }
  }
  return out;
}

// Hermes (the pinned build) mis-reads `^` inside a lookbehind (it never matches at the start of
// the input there), so inside a lookbehind, outside a class, `^` becomes the equivalent
// `(?<![\s\S])` (with the singleline rule `^` is the start of the line's text).
function hermesRewrite(pattern) {
  let out = '', inClass = false;
  const stack = []; // per open group: is it (or an enclosing group) a lookbehind
  for (let at = 0; at < pattern.length; at++) {
    const ch = pattern[at];
    if (ch === '\\') { out += ch + (pattern[at + 1] ?? ''); at++; continue; }
    if (inClass) { if (ch === ']') inClass = false; out += ch; continue; }
    if (ch === '[') { inClass = true; out += ch; continue; }
    if (ch === '(') {
      const behind = pattern.startsWith('(?<=', at) || pattern.startsWith('(?<!', at);
      stack.push(behind || (stack.length > 0 && stack[stack.length - 1]));
    } else if (ch === ')') stack.pop();
    else if (ch === '^' && stack.length && stack[stack.length - 1]) { out += '(?<![\\s\\S])'; continue; }
    out += ch;
  }
  return out;
}

/** One converted pattern as the data stores it: source, flags (without d/g), EmulatedRegExp options. */
function convert(source) {
  const details = toRegExpDetails(source, ENGINE);
  const entry = { s: hermesRewrite(details.pattern), f: details.flags.replace(/[dg]/g, '') };
  const options = details.options ?? {};
  if (options.hiddenCaptures?.length) entry.h = options.hiddenCaptures;
  if (options.transfers?.length) entry.t = options.transfers;
  if (options.strategy === 'clip_search') entry.c = 1;
  return entry;
}

// ── The data tables ─────────────────────────────────────────────────────────────────────────
function createTables() {
  const names = [], nameIndex = new Map(), regexes = [], regexIndex = new Map();
  return {
    names, regexes, rules: [], roots: {}, aliases: {}, injections: {},
    name(value) {
      if (value === undefined || value === null) return undefined;
      let index = nameIndex.get(value);
      if (index === undefined) { index = names.length; names.push(value); nameIndex.set(value, index); }
      return index;
    },
    /** `closing`: an end or while regex, whose back-references name the begin match's captures. */
    regex(raw, closing = false) {
      const parsed = regExpSource(String(raw));
      const backrefs = closing && parsed.hasBackReferences;
      const key = (backrefs ? 'closing:' : '') + raw;
      let index = regexIndex.get(key);
      if (index !== undefined) return index;
      // A back-reference (\N) becomes U+E000+N before conversion; the runtime swaps in the
      // begin match's escaped capture (r12-render-textmate.ts resolveBackrefs).
      const marked = backrefs ? parsed.source.replace(/\\(\d+)/g, (_, n) => String.fromCharCode(0xe000 + Number(n))) : parsed.source;
      let entry;
      if (parsed.hasAnchor) entry = { v: [[false, false], [false, true], [true, false], [true, true]].map(([a, g]) => convert(resolveAnchors(marked, a, g))) };
      else entry = convert(marked);
      if (backrefs) entry.b = 1;
      index = regexes.length; regexes.push(entry); regexIndex.set(key, index);
      return index;
    },
  };
}

/** Compiles one grammar into `tables`, rule ids continuing from the previous grammar's. A
 *  repository entry compiles once, in the repository that defines it; an inline pattern compiles
 *  where it is met (a `captures` table shared by begin and end compiles twice). */
function compileGrammar(tables, grammar, scopes, compiledIds) {
  const ids = new Map();
  compiledIds.set(grammar, ids);
  const rootId = tables.rules.length;
  tables.rules.push(null);
  ids.set(grammar, rootId);
  const allocate = () => { tables.rules.push(null); return tables.rules.length - 1; };

  const captures = (desc, scope) => {
    if (!desc) return undefined;
    const out = {};
    for (const key of Object.keys(desc)) {
      const capture = desc[key], entry = {};
      const n = tables.name(capture.name), c = tables.name(capture.contentName);
      if (n !== undefined) entry.n = n;
      if (c !== undefined) entry.c = c;
      if (capture.patterns) entry.r = compile(capture, scope, false);
      out[key] = entry;
    }
    return out;
  };
  /** A scope maps a repository key to its entry and the scope that defines it. */
  const entry = (item) => {
    const known = ids.get(item.desc);
    if (known !== undefined) return known;
    if (!item.nested) return compile(item.desc, item.scope, true);
    // A nested repository's entry: an include rule of the entry, compiled in its own scope.
    const id = allocate();
    ids.set(item.desc, id);
    tables.rules[id] = { i: compile(item.target, item.scope, true) };
    return id;
  };
  const resolveInclude = (include, scope) => {
    if (include === '$self' || include === '$base') {
      // Each self-include is its own include rule of the root (this grammar's patterns).
      const id = allocate();
      tables.rules[id] = { i: rootId };
      return id;
    }
    if (include.startsWith('#')) {
      const item = scope[include.slice(1)];
      return item ? entry(item) : -2;
    }
    // Another grammar of this group, compiled before this one (the grammar itself is not yet).
    const hash = include.indexOf('#');
    const other = scopes.get(hash < 0 ? include : include.slice(0, hash));
    if (!other) return -2;
    const otherIds = compiledIds.get(other);
    if (hash < 0) return otherIds.get(other);
    const target = other.repository?.[include.slice(hash + 1)];
    return target && otherIds.has(target) ? otherIds.get(target) : -2;
  };
  function compile(desc, scope, isEntry) {
    if (isEntry) { const known = ids.get(desc); if (known !== undefined) return known; }
    const id = allocate();
    if (isEntry) ids.set(desc, id);
    const rule = {};
    let local = scope;
    if (desc.repository) {
      local = { ...scope };
      for (const key of Object.keys(desc.repository)) local[key] = { nested: true, desc: {}, target: desc.repository[key], scope: local };
    }
    if (desc.include !== undefined && desc.match === undefined && desc.begin === undefined) {
      rule.i = resolveInclude(desc.include, local);
      tables.rules[id] = rule;
      return id;
    }
    // An inline rule with neither match nor begin only lists patterns: its name is never a scope.
    const n = tables.name(desc.name), c = tables.name(desc.contentName);
    if (isEntry || desc.match !== undefined || desc.begin !== undefined) {
      if (n !== undefined) rule.n = n;
      if (c !== undefined) rule.c = c;
    }
    if (desc.match !== undefined) {
      rule.m = tables.regex(desc.match);
      const mc = captures(desc.captures, local);
      if (mc) rule.mc = mc;
    } else if (desc.begin !== undefined) {
      rule.b = tables.regex(desc.begin);
      const bc = captures(desc.beginCaptures ?? desc.captures, local);
      if (bc) rule.bc = bc;
      if (desc.while !== undefined) {
        rule.w = tables.regex(desc.while, true);
        const wc = captures(desc.whileCaptures ?? desc.captures, local);
        if (wc) rule.wc = wc;
      } else {
        rule.e = tables.regex(desc.end ?? NEVER, true);
        const ec = captures(desc.endCaptures ?? desc.captures, local);
        if (ec) rule.ec = ec;
        if (desc.applyEndPatternLast) rule.l = 1;
      }
    }
    if (desc.patterns) rule.p = desc.patterns.map(child => compile(child, local, false));
    tables.rules[id] = rule;
    return id;
  }

  const repository = grammar.repository ?? {};
  const top = {};
  for (const key of Object.keys(repository)) top[key] = { desc: repository[key], scope: top };
  for (const key of Object.keys(repository)) entry(top[key]);
  const root = {};
  const n = tables.name(grammar.scopeName);
  if (n !== undefined) root.n = n;
  root.p = (grammar.patterns ?? []).map(child => compile(child, top, false));
  tables.rules[rootId] = root;
  if (grammar.injections) {
    tables.injections[grammar.name] = Object.entries(grammar.injections).map(([selector, desc]) => [selector, compile(desc, top, false)]);
  }
  return rootId;
}

// ── Themes: tokenColors flattened to [selector, foreground, fontStyle?] in theme order ─────────
function themeData(file) {
  const theme = JSON.parse(readFileSync(join(here, 'node_modules', '@pierre', 'theme', 'themes', file), 'utf8'));
  const rules = [];
  for (const { scope, settings } of theme.tokenColors) {
    if (!settings || scope === undefined) continue;
    const selectors = Array.isArray(scope) ? scope : String(scope).split(',');
    const fg = settings.foreground ?? '';
    const style = typeof settings.fontStyle === 'string' ? settings.fontStyle.trim() : undefined;
    if (!fg && style === undefined) continue;
    for (const selector of selectors) {
      const trimmed = selector.trim();
      if (trimmed) rules.push(style === undefined ? [trimmed, fg] : [trimmed, fg, style]);
    }
  }
  return { fg: theme.colors['editor.foreground'], rules };
}

async function loadLanguage(name) {
  const modules = (await import(`@shikijs/langs/${name}`)).default;
  const grammar = modules.find(one => one.name === name);
  if (!grammar) throw new Error(`@shikijs/langs has no grammar named ${name}`);
  return { grammar, bundled: modules };
}

const CHUNK = 1000;
function moduleText(header, data) {
  const json = JSON.stringify(data), parts = [];
  for (let at = 0; at < json.length; at += CHUNK) parts.push(JSON.stringify(json.slice(at, at + CHUNK)));
  // The languages and aliases the file holds, readable without parsing the data.
  const languages = [...Object.keys(data.roots), ...Object.keys(data.aliases)].join(' ');
  return `${header}export const GRAMMAR_LANGUAGES = ${JSON.stringify(languages)};\n`
    + `export const GRAMMAR_DATA: string =\n  ${parts.join(' +\n  ')};\n`;
}

/** The data one group's file holds. */
export async function buildData(langs, withThemes = true) {
  const tables = createTables();
  const compiledIds = new Map(), scopes = new Map();
  for (const lang of langs) {
    const { grammar } = await loadLanguage(lang);
    // A grammar's own scope name resolves to itself, as vscode-textmate's registry resolves it.
    scopes.set(grammar.scopeName, grammar);
    tables.roots[lang] = compileGrammar(tables, grammar, scopes, compiledIds);
    for (const alias of grammar.aliases ?? []) tables.aliases[alias] = lang;
  }
  const data = { roots: tables.roots, aliases: tables.aliases, injections: tables.injections, names: tables.names, rules: tables.rules, regexes: tables.regexes };
  if (withThemes) data.themes = { light: themeData('pierre-light.json'), dark: themeData('pierre-dark.json') };
  return data;
}

export async function generate() {
  const self = createHash('sha256').update(readFileSync(fileURLToPath(import.meta.url))).digest('hex').slice(0, 16);
  const versions = ['@shikijs/langs', '@pierre/theme', 'oniguruma-to-es', 'oniguruma-parser', 'regex', 'regex-recursion'].map(name => `${name}@${pkg(name).version}`);
  const out = [];
  for (const [index, group] of GROUPS.entries()) {
    const data = await buildData(group.langs, index === 0);
    const header = `// Generated by examples/t3-code/tools/grammar/gen-grammar.mjs (sha256 ${self}); do not edit.\n`
      + `// The reference's Shiki 4.2 TextMate grammars (${group.langs.join(', ')}; MIT and other\n`
      + `// permissive licences, see @shikijs/langs) with their Oniguruma regexes converted to JavaScript\n`
      + `// (oniguruma-to-es, MIT)${index === 0 ? ', and the pierre-light / pierre-dark token colours and font styles (@pierre/theme, MIT)' : ''}. See LICENSE-T3.\n`
      + `// Inputs: ${versions.join(', ')} (tools/grammar/bun.lock).\n`;
    out.push({ file: join(example, group.file), text: moduleText(header, data) });
  }
  return out;
}

if (import.meta.main) {
  const check = process.argv.includes('--check');
  let differs = 0;
  for (const { file, text } of await generate()) {
    let current = '';
    try { current = readFileSync(file, 'utf8'); } catch {}
    if (current === text) continue;
    differs++;
    if (check) console.error(`differs: ${file}`); else { writeFileSync(file, text); console.log(`wrote ${file} (${text.length} bytes)`); }
  }
  if (check && differs) process.exit(1);
  if (check) console.log('grammar data matches the generator');
}
