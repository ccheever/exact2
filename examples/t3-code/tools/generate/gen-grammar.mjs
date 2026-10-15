// The clone's Shiki grammar data, from pinned published packages (package.json and bun.lock next to
// this file). The reference T3 Code (1e2ecbd975) colours code with Shiki 4.2 (@pierre/diffs' shared
// highlighter), the @shikijs/langs grammars and the @pierre/theme pierre-light / pierre-dark themes;
// this compiles those grammars the way vscode-textmate's RuleFactory does (one id per rule, includes
// resolved ahead of time) and converts each regex the way Shiki's JavaScript engine does
// (oniguruma-to-es with Shiki's options, target ES2018 for Hermes), so r12-render-textmate.ts can run
// them with no Oniguruma and no regex conversion. generate.mjs writes the files (the app build runs it).
//
// Every grammar of @shikijs/langs is here (the reference loads any
// language of Shiki's bundle). They share one rule space, so an include of another language
// resolves as a registry that has it loaded resolves it, and each grammar's rules, names and
// regexes are one chunk the app parses only when a language first needs it. Output:
// r12-render-grammar.ts (the index: languages, aliases, injections, chunk ranges, the themes) and
// r12-render-grammar-<n>.ts (the chunks, packed in order, each file under the line cap).

import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { languageNames } from '@shikijs/langs';
import { parse } from 'oniguruma-parser/parser';
import { generate as generateOniguruma } from 'oniguruma-parser/generator';
import { toRegExpDetails } from 'oniguruma-to-es';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const pkg = name => JSON.parse(readFileSync(join(here, 'node_modules', name, 'package.json'), 'utf8'));

/** @shikijs/langs grammars that are not languages of Shiki's bundle (shiki 4.2 `bundledLanguagesInfo`): other
 *  grammars embed or inject them, but no fence word or file name selects them. tools/shiki-compare checks the list. */
export const EMBEDDED_ONLY = ['angular-expression', 'angular-inline-style', 'angular-inline-template', 'angular-let-declaration',
  'angular-template', 'angular-template-blocks', 'cpp-macro', 'es-tag-css', 'es-tag-glsl', 'es-tag-html', 'es-tag-sql', 'es-tag-xml',
  'jinja-html', 'markdown-nix', 'markdown-vue', 'vue-directives', 'vue-interpolations', 'vue-sfc-style-variable-injection'];

/** Grammars left out for their licences (LICENSE-T3
 *  lists each grammar's licence as its upstream states it): GPL-3.0 ada, gnuplot, nginx and racket, and dax, dream-maker,
 *  glsl and tcl, whose upstream grants no licence. A held language is no fence word or file language (it paints plain),
 *  and a held grammar is an empty grammar of its scope name: an include of it matches nothing, so the rule that embeds it
 *  stays and its content paints plain (cpp's R"glsl(…)glsl" strings, elm's [glsl| … |], ts-tags' glsl templates, latex's
 *  gnuplot code). Reversible: taking a name off the list and running the generator brings it back. */
export const LICENCE_HELD = ['ada', 'dax', 'dream-maker', 'glsl', 'gnuplot', 'nginx', 'racket', 'tcl'];
/** The grammars generated: every one of @shikijs/langs but those held. */
const NAMES = languageNames.filter(name => !LICENCE_HELD.includes(name));
/** Grammars under the MPL-2.0 (bird2, hcl, terraform): their chunk carries the licence's notice (LICENSE-T3). */
const MPL = { bird2: 'https://github.com/bird-chinese-community/BIRD-tm-language-grammar', hcl: 'https://github.com/hashicorp/syntax', terraform: 'https://github.com/hashicorp/syntax' };

// Shiki 4.2's createJavaScriptRegexEngine options (@shikijs/engine-javascript), target ES2018
// so the patterns compile in Hermes (no `v` flag, no ES2025 modifiers).
const ENGINE = {
  global: true, hasIndices: true, target: 'ES2018',
  rules: { allowOrphanBackrefs: true, asciiWordBoundaries: true, captureGroup: true, recursionLimit: 5, singleline: true },
};
// Shiki's JavaScript engine reads \b as ASCII; the reference runs
// Oniguruma, whose \b is Unicode (`__밑줄__과` is no emphasis there, `2π` is one number). On a line
// of ASCII characters the two agree, and the ASCII form is several times faster in Hermes, so a
// regex whose Unicode form differs carries it as `u`, which the engine uses on lines with other characters.
const UNICODE_ENGINE = { ...ENGINE, rules: { ...ENGINE.rules, asciiWordBoundaries: false } };

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

// ── ES2024 classes in ES2018 ────────────────────────────────────────────────────────────────
// A few grammars (haskell, purescript, stata, julia, matlab, racket)
// use Oniguruma class intersection (`[[\p{S}\p{P}]&&[^"'(),;]]`) or a class oniguruma-to-es can
// write only with nested classes (`[[^\w\s]\d]`, `[^\W\d]`), which need the ES2024 `v` flag that
// Hermes lacks. Such a class matches one character, so it is rewritten as lookarounds over plain
// classes: `[A&&B]` → `(?:(?=B)A)`, `[^A&&B]` → `(?:(?!(?=B)A)[\s\S])`, `[a[B]]` → `(?:[a]|B)`,
// `[^\W\d]` → `(?:(?!\W|\d)[\s\S])`. Oniguruma refuses a lookaround inside a lookbehind, so each such
// class is converted alone and spliced into the converted pattern at a placeholder character
// (JavaScript allows it there, and a lookahead in a lookbehind still tests the one character the
// class stands for).
const PLACEHOLDER = 0xf0000;
const PARSE = { rules: { captureGroup: true, singleline: true }, skipBackrefValidation: true };
const needsV = error => /requires min target ES2024/.test(String(error?.message));
/** The converted (JavaScript, `u` mode) pattern of one AST node standing alone. */
function alone(node, engine) {
  const ast = parse('a', PARSE);
  ast.body[0].body[0] = node;
  return toRegExpDetails(generateOniguruma(ast).pattern, engine).pattern;
}
/** The converted pattern of a class with these elements, or null when it needs the `v` flag. */
function plainClass(negate, body, engine) {
  try { return alone({ type: 'CharacterClass', kind: 'union', negate, body }, engine); } catch (error) { if (needsV(error)) return null; throw error; }
}
/** A JavaScript pattern matching the one character a class (or class element) matches. */
function classMatcher(node, engine) {
  if (node.type === 'CharacterSet') return alone(node, engine);
  if (node.type !== 'CharacterClass') return plainClass(false, [node], engine);
  if (node.kind === 'intersection') {
    const [first, ...rest] = node.body.map(kid => classMatcher(kid, engine));
    const core = rest.map(operand => `(?=${operand})`).join('') + first;
    return node.negate ? `(?:(?!${core})[\\s\\S])` : `(?:${core})`;
  }
  const plain = plainClass(node.negate, node.body, engine);
  if (plain !== null) return plain;
  const parts = node.body.map(kid => classMatcher(kid, engine));
  const any = parts.length === 1 ? parts[0] : `(?:${parts.join('|')})`;
  return node.negate ? `(?:(?!${any})[\\s\\S])` : any;
}
/** toRegExpDetails, with the classes that need the `v` flag rewritten (see above) when the plain conversion refuses them. */
function convertDetails(source, engine) {
  try { return toRegExpDetails(source, engine); } catch (error) { if (!needsV(error)) throw error; }
  // The rewrite converts each class alone, outside any flag group: refuse a pattern where that would change its meaning.
  if (/\(\?[imx-]*[ix][imx-]*[:)]/.test(source)) throw new Error(`a class to rewrite under a flag group in ${JSON.stringify(source)}`);
  const ast = parse(source, PARSE), fragments = [];
  const replace = node => {
    if (node?.type !== 'CharacterClass' || (node.kind !== 'intersection' && plainClass(node.negate, node.body, engine) !== null)) return null;
    fragments.push(classMatcher(node, engine));
    return { type: 'Character', value: PLACEHOLDER + fragments.length - 1 };
  };
  const walk = node => {
    for (const key of ['body', 'min', 'max']) {
      const value = node[key];
      if (Array.isArray(value)) value.forEach((kid, index) => { const swap = replace(kid); if (swap) value[index] = swap; else if (kid && typeof kid === 'object') walk(kid); });
      else if (value && typeof value === 'object' && value.type) { const swap = replace(value); if (swap) node[key] = swap; else walk(value); }
    }
  };
  walk(ast);
  const details = toRegExpDetails(generateOniguruma(ast).pattern, engine);
  let pattern = details.pattern;
  fragments.forEach((fragment, index) => {
    const code = PLACEHOLDER + index, forms = [String.fromCodePoint(code), `\\u{${code.toString(16).toUpperCase()}}`, `\\u{${code.toString(16)}}`];
    const form = forms.find(one => pattern.includes(one));
    if (!form) throw new Error(`class placeholder ${index} lost in ${JSON.stringify(source)}`);
    pattern = pattern.split(form).join(`(?:${fragment})`);
  });
  return { ...details, pattern };
}

/** One converted pattern as the data stores it: source, flags (without d/g), EmulatedRegExp options,
 *  and its Unicode-\b form `u` when that differs. */
function convert(source) {
  const ascii = convertWith(source, ENGINE);
  if (!/\\[bB]/.test(source)) return ascii;
  const unicode = convertWith(source, UNICODE_ENGINE);
  if (JSON.stringify(unicode) !== JSON.stringify(ascii)) ascii.u = unicode;
  return ascii;
}
function convertWith(source, engine) {
  const details = convertDetails(source, engine);
  if (details.flags.includes('v')) throw new Error(`v flag in ${JSON.stringify(source)}`);
  const entry = { s: hermesRewrite(details.pattern), f: details.flags.replace(/[dg]/g, '') };
  const options = details.options ?? {};
  if (options.hiddenCaptures?.length) entry.h = options.hiddenCaptures;
  if (options.transfers?.length) entry.t = options.transfers;
  if (options.strategy === 'clip_search') entry.c = 1;
  return entry;
}
const converted = new Map();
/** A regex's data entry (`closing`: an end or while regex, whose back-references name the begin match's captures). */
function regexEntry(raw, closing) {
  const parsed = regExpSource(String(raw));
  const backrefs = closing && parsed.hasBackReferences;
  const key = (backrefs ? 'closing:' : '') + raw;
  let entry = converted.get(key);
  if (entry) return entry;
  // A back-reference (\N) becomes U+E000+N before conversion; the runtime swaps in the
  // begin match's escaped capture (r12-render-textmate.ts resolveBackrefs).
  const marked = backrefs ? parsed.source.replace(/\\(\d+)/g, (_, n) => String.fromCharCode(0xe000 + Number(n))) : parsed.source;
  if (parsed.hasAnchor) {
    // The four \A / \G variants; one equal to an earlier one is stored as that one's index.
    const variants = [[false, false], [false, true], [true, false], [true, true]].map(([a, g]) => convert(resolveAnchors(marked, a, g)));
    const texts = variants.map(one => JSON.stringify(one));
    entry = { v: variants.map((one, index) => texts.indexOf(texts[index]) < index ? texts.indexOf(texts[index]) : one) };
  }
  else entry = convert(marked);
  if (backrefs) entry.b = 1;
  converted.set(key, entry);
  return entry;
}

// ── Compiling every grammar into one rule space ──────────────────────────────────────────────
/** A compiled rule before numbering: names and regexes as their raw strings, rule references as ids. */
function createSpace(grammars, heldScopes) {
  const rules = [], owner = [], contexts = new Map(), byScope = new Map();
  for (const [name, { grammar }] of grammars) byScope.set(grammar.scopeName, name);
  const space = { rules, owner, context, byScope };

  /** One grammar's compile state, made when the grammar is first included or compiled. */
  function context(lang) {
    let ctx = contexts.get(lang);
    if (ctx) return ctx;
    const grammar = grammars.get(lang).grammar, ids = new Map();
    const allocate = () => { rules.push(null); owner.push(lang); return rules.length - 1; };
    const rootId = allocate();
    ids.set(grammar, rootId);
    const top = {};
    const repository = grammar.repository ?? {};
    for (const key of Object.keys(repository)) top[key] = { desc: repository[key], scope: top };

    const captures = (desc, scope) => {
      if (!desc) return undefined;
      const out = {};
      for (const key of Object.keys(desc)) {
        // A capture that is not an object (racket's strings, wikitext's nulls) names nothing.
        const capture = desc[key] !== null && typeof desc[key] === 'object' ? desc[key] : {}, entry = {};
        if (capture.name !== undefined && capture.name !== null) entry.n = capture.name;
        if (capture.contentName !== undefined && capture.contentName !== null) entry.c = capture.contentName;
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
      rules[id] = { i: compile(item.target, item.scope, true) };
      return id;
    };
    const resolveInclude = (include, scope) => {
      if (include === '$self' || include === '$base') {
        // Each self-include is its own include rule of the root (this grammar's patterns).
        const id = allocate();
        rules[id] = { i: rootId };
        return id;
      }
      if (include.startsWith('#')) {
        const item = scope[include.slice(1)];
        return item ? entry(item) : -2;
      }
      // Another grammar (or this one by its scope name), as a registry with every grammar loaded resolves it.
      const hash = include.indexOf('#');
      const scopeName = hash < 0 ? include : include.slice(0, hash);
      if (heldScopes.has(scopeName)) {
        // A held grammar is an empty one (LICENCE_HELD): its root is a rule with no patterns, so the including list
        // misses nothing and keeps its rule; a key of its repository is missing, as the empty grammar has none.
        if (hash >= 0) return -2;
        const id = allocate();
        rules[id] = { p: [] };
        return id;
      }
      const other = byScope.get(scopeName);
      if (!other) return -2;
      const target = context(other);
      return hash < 0 ? target.rootId : target.entryByKey(include.slice(hash + 1));
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
      // vscode-textmate's RuleFactory.getCompiledRuleId: a (truthy) match makes a match rule; no begin, an
      // include-only rule of its patterns (or of its include when it has none); an empty end never matches.
      if (!desc.match && desc.begin === undefined && desc.patterns === undefined && desc.include) {
        rule.i = resolveInclude(desc.include, local);
        rules[id] = rule;
        return id;
      }
      // An inline rule with neither match nor begin only lists patterns: its name is never a scope.
      if (isEntry || desc.match || desc.begin !== undefined) {
        if (desc.name !== undefined && desc.name !== null) rule.n = desc.name;
        if (desc.contentName !== undefined && desc.contentName !== null) rule.c = desc.contentName;
      }
      if (desc.match) {
        rule.m = [desc.match, false];
        const mc = captures(desc.captures, local);
        if (mc) rule.mc = mc;
      } else if (desc.begin !== undefined) {
        rule.b = [desc.begin, false];
        const bc = captures(desc.beginCaptures ?? desc.captures, local);
        if (bc) rule.bc = bc;
        if (desc.while) {
          rule.w = [desc.while, true];
          const wc = captures(desc.whileCaptures ?? desc.captures, local);
          if (wc) rule.wc = wc;
        } else {
          rule.e = [desc.end || NEVER, true];
          const ec = captures(desc.endCaptures ?? desc.captures, local);
          if (ec) rule.ec = ec;
          if (desc.applyEndPatternLast) rule.l = 1;
        }
      }
      if (desc.patterns && !desc.match) rule.p = desc.patterns.map(child => compile(child, local, false));
      rules[id] = rule;
      return id;
    }

    ctx = {
      rootId,
      entryByKey: key => top[key] ? entry(top[key]) : -2,
      /** Compiles every repository entry, the root's patterns and the grammar's own injections. */
      finish() {
        for (const key of Object.keys(repository)) entry(top[key]);
        const root = {};
        if (grammar.scopeName) root.n = grammar.scopeName;
        root.p = (grammar.patterns ?? []).map(child => compile(child, top, false));
        rules[rootId] = root;
        ctx.injections = Object.entries(grammar.injections ?? {}).map(([selector, desc]) => [selector, compile(desc, top, false)]);
      },
    };
    contexts.set(lang, ctx);
    return ctx;
  }
  return space;
}

/** vscode-textmate's RuleFactory._compilePatterns: an include that resolves to nothing is left out
 *  of a pattern list, and so is a begin/end, begin/while or include-only rule whose own list lost an
 *  entry that way and kept none (asm's strings include repository keys asm does not define, so
 *  Shiki never matches them). A rule still being compiled when a pattern list reaches it again is
 *  kept, as vscode-textmate's registry does not hold it yet; this follows its order from each root. */
function dropMissingPatterns(space, roots) {
  const state = new Map();
  const skippable = id => {
    if (id < 0) return true;
    const rule = space.rules[id];
    if (rule.m !== undefined) return false;
    const known = state.get(id);
    if (known !== undefined) return known === true;
    state.set(id, 'busy');
    const list = rule.i !== undefined ? [rule.i] : rule.p ?? [];
    const keep = list.filter(child => !skippable(child));
    if (rule.p) rule.p = keep;
    const result = keep.length === 0 && list.length > 0;
    state.set(id, result);
    return result;
  };
  for (const root of roots) skippable(root);
  space.rules.forEach((_, id) => skippable(id));
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

/** Every grammar of @shikijs/langs but the held ones, with the grammars its own module loads (Shiki's embeddedLangs
 *  closure, the held ones left out), and the held grammars' scope names. */
async function loadGrammars() {
  const grammars = new Map(), heldScopes = new Set();
  for (const name of languageNames) {
    const modules = (await import(`@shikijs/langs/${name}`)).default;
    const grammar = modules.find(one => one.name === name);
    if (!grammar) throw new Error(`@shikijs/langs has no grammar named ${name}`);
    if (LICENCE_HELD.includes(name)) { heldScopes.add(grammar.scopeName); continue; }
    const loads = [...new Set(modules.map(one => one.name))].filter(one => !LICENCE_HELD.includes(one));
    grammars.set(name, { grammar, loads });
  }
  return { grammars, heldScopes };
}

/** The data: the index (what every highlight needs) and one chunk per grammar (parsed on first use). */
export async function buildData() {
  const { grammars, heldScopes } = await loadGrammars();
  const space = createSpace(grammars, heldScopes);
  for (const name of NAMES) space.context(name).finish();
  dropMissingPatterns(space, NAMES.map(name => space.context(name).rootId));

  // Number the rules grammar by grammar (each grammar's rules one range), in languageNames order.
  const order = new Map(NAMES.map((name, index) => [name, index]));
  const sorted = space.rules.map((_, id) => id).sort((a, b) => order.get(space.owner[a]) - order.get(space.owner[b]) || a - b);
  const renumber = new Array(space.rules.length);
  sorted.forEach((old, index) => { renumber[old] = index; });
  const id = old => old < 0 ? old : renumber[old];

  const chunks = [], ranges = [];
  let ruleAt = 0, nameAt = 0, regexAt = 0;
  for (const name of NAMES) {
    const names = [], nameIndex = new Map(), regexes = [], regexIndex = new Map(), rules = [];
    const nameOf = value => {
      let index = nameIndex.get(value);
      if (index === undefined) { index = names.length; names.push(value); nameIndex.set(value, index); }
      return nameAt + index;
    };
    const regexOf = ([raw, closing]) => {
      const key = (closing ? '1' : '0') + raw;
      let index = regexIndex.get(key);
      if (index === undefined) { index = regexes.length; regexes.push(regexEntry(raw, closing)); regexIndex.set(key, index); }
      return regexAt + index;
    };
    const captures = table => {
      if (!table) return undefined;
      const out = {};
      for (const [key, capture] of Object.entries(table)) {
        const entry = {};
        if (capture.n !== undefined) entry.n = nameOf(capture.n);
        if (capture.c !== undefined) entry.c = nameOf(capture.c);
        if (capture.r !== undefined) entry.r = id(capture.r);
        out[key] = entry;
      }
      return out;
    };
    const start = ruleAt;
    while (ruleAt < sorted.length && space.owner[sorted[ruleAt]] === name) {
      const rule = space.rules[sorted[ruleAt]], out = {};
      if (rule.i !== undefined) out.i = id(rule.i);
      if (rule.n !== undefined) out.n = nameOf(rule.n);
      if (rule.c !== undefined) out.c = nameOf(rule.c);
      for (const key of ['m', 'b', 'e', 'w']) if (rule[key]) out[key] = regexOf(rule[key]);
      for (const key of ['mc', 'bc', 'ec', 'wc']) if (rule[key]) out[key] = captures(rule[key]);
      if (rule.l) out.l = 1;
      if (rule.p) out.p = rule.p.map(id);
      rules.push(out);
      ruleAt++;
    }
    ranges.push([start, nameAt, regexAt]);
    chunks.push({ name, data: { r: rules, n: names, x: regexes } });
    nameAt += names.length;
    regexAt += regexes.length;
  }

  // Languages a fence word or file name selects, their aliases, and per language the injections
  // vscode-textmate applies to it as a root: its own grammar's, then each grammar its module loads
  // whose injectTo names the root's scope or a dotted prefix of it (Shiki's Registry).
  const roots = {}, aliases = {}, injections = {};
  for (const name of NAMES) {
    if (EMBEDDED_ONLY.includes(name)) continue;
    const { grammar, loads } = grammars.get(name);
    roots[name] = id(space.context(name).rootId);
    for (const alias of grammar.aliases ?? []) aliases[alias] = name;
    const prefixes = grammar.scopeName.split('.').map((_, index, parts) => parts.slice(0, index + 1).join('.'));
    const list = space.context(name).injections.map(([selector, rule]) => [selector, id(rule)]);
    for (const prefix of prefixes) for (const loaded of loads) {
      const injector = grammars.get(loaded).grammar;
      if (injector.injectTo?.includes(prefix) && injector.injectionSelector) list.push([injector.injectionSelector, id(space.context(loaded).rootId)]);
    }
    if (list.length) injections[name] = list;
  }
  const index = {
    roots, aliases, injections, chunks: ranges, counts: [sorted.length, nameAt, regexAt],
    themes: { light: themeData('pierre-light.json'), dark: themeData('pierre-dark.json') },
  };
  return { index, chunks };
}

// ── Files ────────────────────────────────────────────────────────────────────────────────────
const CHUNK = 1000;
/** A JSON text as string literal lines of at most CHUNK characters each. */
const literalLines = json => {
  const parts = [];
  for (let at = 0; at < json.length; at += CHUNK) parts.push(JSON.stringify(json.slice(at, at + CHUNK)));
  return parts;
};
/** Each chunk file stays under the source-file cap (1,500 lines; rules/RULES.md). */
const FILE_LINES = 1400;
const DATA_FILE = /^r12-render-grammar(-\d+)?\.ts$/;

export async function generate() {
  const self = createHash('sha256').update(readFileSync(fileURLToPath(import.meta.url))).digest('hex').slice(0, 16);
  const versions = ['@shikijs/langs', '@pierre/theme', 'oniguruma-to-es', 'oniguruma-parser', 'regex', 'regex-recursion'].map(name => `${name}@${pkg(name).version}`);
  const { index, chunks } = await buildData();
  const header = (what) => `// Generated by examples/t3-code/tools/generate/gen-grammar.mjs (sha256 ${self}); do not edit.\n`
    + `// ${what}\n`
    + `// Inputs: ${versions.join(', ')} (tools/generate/bun.lock).\n`;

  // Pack the chunks in order into files of at most FILE_LINES lines.
  const files = [];
  let lines = [], names = [];
  const flush = () => {
    if (!lines.length) return;
    const number = files.length + 1;
    const text = header(`The reference's Shiki 4.2 TextMate grammars ${names[0]} to ${names[names.length - 1]} (@shikijs/langs, MIT; each grammar's own licence travels with it in its upstream repository), their Oniguruma regexes converted to JavaScript (oniguruma-to-es, MIT). See LICENSE-T3.`)
      + `/** Grammar chunks ${names.join(', ')}: each a function returning its JSON text (parsed when first used). */\n`
      + `export const CHUNKS: (() => string)[] = [\n${lines.join('\n')}\n];\n`;
    files.push({ file: join(example, `r12-render-grammar-${number}.ts`), text });
    lines = []; names = [];
  };
  for (const chunk of chunks) {
    const parts = literalLines(JSON.stringify(chunk.data));
    // An MPL-2.0 grammar's chunk is its Executable Form: the licence's Exhibit A notice and where its Source Code Form is.
    const notice = MPL[chunk.name] ? [`  // This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL`,
      `  // was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/. Source Code Form: ${MPL[chunk.name]}`,
      `  // (as @shikijs/langs 4.2.0 packages it). See LICENSE-T3.`] : [];
    const body = [`  // ${chunk.name}`, ...notice, `  () => [`, ...parts.map(part => `    ${part},`), `  ].join(''),`];
    if (lines.length && lines.length + body.length > FILE_LINES) flush();
    lines.push(...body); names.push(chunk.name);
  }
  flush();

  const languages = [...Object.keys(index.roots), ...Object.keys(index.aliases)].join(' ');
  const indexText = header(`The index of the grammar chunks (r12-render-grammar-1.ts to -${files.length}.ts): languages, aliases, injections, rule ranges, and the pierre-light / pierre-dark token colours and font styles (@pierre/theme, MIT). See LICENSE-T3.`)
    + files.map((_, at) => `import { CHUNKS as CHUNKS_${at + 1} } from './r12-render-grammar-${at + 1}';\n`).join('')
    + `/** The languages and aliases the chunks hold, readable without parsing the index. */\n`
    + `export const GRAMMAR_LANGUAGES = ${JSON.stringify(languages)};\n`
    + `/** Every grammar's chunk, in the order of the index's \`chunks\` ranges. */\n`
    + `export const GRAMMAR_CHUNKS: (() => string)[] = [${files.map((_, at) => `...CHUNKS_${at + 1}`).join(', ')}];\n`
    + `export const GRAMMAR_INDEX: string = [\n${literalLines(JSON.stringify(index)).map(part => `  ${part},`).join('\n')}\n].join('');\n`;
  return [{ file: join(example, 'r12-render-grammar.ts'), text: indexText }, ...files];
}

/** Grammar data files the generator no longer writes (fewer chunk files, the old -more file). */
export function stale(outputs) {
  const wanted = new Set(outputs.map(({ file }) => file));
  return readdirSync(example).filter(one => DATA_FILE.test(one) || one === 'r12-render-grammar-more.ts').map(one => join(example, one)).filter(file => !wanted.has(file));
}
