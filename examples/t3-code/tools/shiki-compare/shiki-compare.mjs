// Verification apparatus (never part of the app build). Runs real Shiki 4.2 with the
// Oniguruma engine — the reference T3 Code's highlighter (apps/web/src/lib/syntaxHighlighting.ts,
// PREFERRED_HIGHLIGHTER "shiki-wasm") — and the clone's grammar engine (r12-render-textmate.ts) on
// the same files, and compares every non-blank character's pierre-light colour, pierre-dark colour
// and italic. It also checks that each colour pair Shiki paints has a synColor class
// (r12-render-highlight.ts CLASS_OF_PAIR), since a pair without one paints as plain text.
//
//   bun install --frozen-lockfile                      (once, in this directory)
//   bun shiki-compare.mjs <language> <file>...         one language, any number of files
//   bun shiki-compare.mjs --manifest <corpus.json>     {"<language>": ["<file>", ...], ...}
//   bun shiki-compare.mjs --samples <dir>              every <language>.sample in <dir> (the
//                                                      shikijs/textmate-grammars-themes samples)
//   bun shiki-compare.mjs --list                       the clone's languages and aliases against
//                                                      Shiki's bundledLanguages (exit 1 on a difference)
//   add --js to also run Shiki's JavaScript engine (tells engine differences from grammar ones)
//   add --max-line <n> to run the clone's app path (highlight(), the Files and diffs line limit) against
//                                                      Shiki's tokenizeMaxLineLength <n>, compared by synColor class
//
// Each language runs in a Shiki highlighter of its own that holds the
// language's module (its embedded grammars and injectors) and every other bundled language whose
// module brings no injector, which is the clone's state: all grammars loaded, so a markdown fence
// or an html script finds its language, and only the root language's own injectors apply.
//
// The grammars the generator leaves out for their licences (LICENCE_HELD) are left out on
// both sides: no Shiki highlighter here loads them as languages, and each is the same empty grammar of its scope name the
// generator makes it, so a language that embeds one (cpp, elm and ts-tags glsl, latex gnuplot) keeps the embedding rule and
// paints its content plain on both sides.
//
// Prints each file's size and sha256, the differing characters per file, and the first differences.
// Exit status 1 when any character differs. Corpus files stay where they are (keep them in target/).

import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHighlighterCore } from 'shiki/core';
import { createOnigurumaEngine } from 'shiki/engine/oniguruma';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import { bundledLanguages, bundledLanguagesInfo } from 'shiki/langs';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const { tokenizeCode, styleOf } = await import(join(example, 'r12-render-textmate.ts'));
const { CLASS_OF_PAIR } = await import(join(example, 'r12-render-highlight.ts'));
const { highlight } = await import(join(example, 'timeline-highlight.ts'));
const { EMBEDDED_ONLY, LICENCE_HELD } = await import(join(example, 'tools', 'generate', 'gen-grammar.mjs'));
const theme = name => JSON.parse(readFileSync(join(here, 'node_modules', '@pierre', 'theme', 'themes', `${name}.json`), 'utf8'));
const THEMES = { light: theme('pierre-light'), dark: theme('pierre-dark') };

const args = process.argv.slice(2);
const withJs = args.includes('--js');
const maxAt = args.indexOf('--max-line'), maxLine = maxAt >= 0 ? Number(args[maxAt + 1]) : 0;
const rest = args.filter((arg, at) => arg !== '--js' && (maxAt < 0 || at !== maxAt && at !== maxAt + 1));

if (rest[0] === '--list') {
  // The clone's fence words (ids and aliases) are Shiki's bundledLanguages keys, and the
  // generator's EMBEDDED_ONLY grammars are exactly those of @shikijs/langs Shiki does not bundle.
  const { GRAMMAR_LANGUAGES } = await import(join(example, 'r12-render-grammar.ts'));
  const { languageNames } = await import('@shikijs/langs');
  const clone = new Set(GRAMMAR_LANGUAGES.split(' ')), shiki = new Set(Object.keys(bundledLanguages));
  const ids = new Set(bundledLanguagesInfo.map(info => info.id));
  // A held language is missing with its aliases, and nothing else is.
  const heldNames = new Set(bundledLanguagesInfo.filter(info => LICENCE_HELD.includes(info.id)).flatMap(info => [info.id, ...(info.aliases ?? [])]));
  const missing = [...shiki].filter(name => !clone.has(name)), extra = [...clone].filter(name => !shiki.has(name));
  const unheld = missing.filter(name => !heldNames.has(name)), present = [...heldNames].filter(name => clone.has(name));
  const embedded = languageNames.filter(name => !ids.has(name));
  const embeddedDiff = embedded.join(' ') !== [...EMBEDDED_ONLY].sort().join(' ');
  console.log(`shiki 4.2 bundledLanguages: ${shiki.size} keys (${ids.size} languages); clone: ${clone.size}`);
  console.log(`missing in the clone: ${missing.join(' ') || 'none'} (held for their licences: ${[...heldNames].join(' ') || 'none'}); only in the clone: ${extra.join(' ') || 'none'}`);
  console.log(`embedded-only grammars: ${embedded.length}${embeddedDiff ? ` (generator lists ${EMBEDDED_ONLY.join(' ')})` : ', as the generator lists'}`);
  process.exit(unheld.length || present.length || extra.length || embeddedDiff ? 1 : 0);
}

let corpus;
if (rest[0] === '--manifest') corpus = JSON.parse(readFileSync(rest[1], 'utf8'));
else if (rest[0] === '--samples') {
  corpus = {};
  for (const name of readdirSync(rest[1]).sort()) {
    const lang = name.replace(/\.sample$/, '');
    if (name.endsWith('.sample') && bundledLanguagesInfo.some(info => info.id === lang) && !LICENCE_HELD.includes(lang)) corpus[lang] = [join(rest[1], name)];
  }
} else corpus = { [rest[0]]: rest.slice(1) };

// Every bundled language whose module brings no injector (Shiki refuses a language without the
// grammars it embeds, and an injector would reach other roots), and per language its own module.
/** A held grammar as the generator makes it: an empty grammar of its scope name. */
const stub = grammar => LICENCE_HELD.includes(grammar.name) ? { name: grammar.name, scopeName: grammar.scopeName, patterns: [] } : grammar;
const moduleOf = async lang => (await bundledLanguages[lang]()).default.map(stub);
const shared = [];
for (const info of bundledLanguagesInfo) {
  const modules = await moduleOf(info.id);
  if (modules.some(grammar => grammar.injectTo)) continue;
  for (const grammar of modules) if (!shared.some(one => one.name === grammar.name)) shared.push(grammar);
}
const oniguruma = createOnigurumaEngine(import('shiki/wasm'));
/** Shiki highlighters for one language, as the clone holds it (see above). */
async function enginesFor(lang) {
  const own = await moduleOf(lang);
  const langs = [...shared.filter(one => !own.some(mine => mine.name === one.name)), ...own];
  const out = { oniguruma: await createHighlighterCore({ themes: Object.values(THEMES), langs, engine: oniguruma }) };
  if (withJs) out.javascript = await createHighlighterCore({ themes: Object.values(THEMES), langs, engine: createJavaScriptRegexEngine() });
  return out;
}

const FG = { light: THEMES.light.colors['editor.foreground'].toLowerCase(), dark: THEMES.dark.colors['editor.foreground'].toLowerCase() };
/** Shiki's per-character [light, dark, italic] for `code` (a token with no colour has the editor's). */
function shikiChars(highlighter, code, lang) {
  const out = [];
  const options = { lang, themes: { light: 'pierre-light', dark: 'pierre-dark' }, ...(maxLine ? { tokenizeMaxLineLength: maxLine } : {}) };
  const lines = highlighter.codeToTokensWithThemes(code, options);
  lines.forEach((line, index) => {
    if (index) out.push(null);
    for (const token of line) {
      const light = token.variants.light, dark = token.variants.dark;
      const italic = ((light.fontStyle ?? 0) & 1) === 1;
      for (const ch of token.content) out.push([ch, (light.color || FG.light).toLowerCase(), (dark.color || FG.dark).toLowerCase(), italic]);
    }
  });
  return out;
}
/** --max-line: the class Shiki's colours name, as the clone's app path paints them. */
const classOf = ([, light, dark, italic]) => { const base = CLASS_OF_PAIR[`${light}/${dark}`] ?? `?${light}/${dark}`; return italic ? `${base}+i` : base; };
/** The clone's per-character [light, dark, italic] (--max-line: [character, class] by highlight()). */
function cloneChars(code, lang) {
  const out = [];
  if (maxLine) {
    for (const token of highlight(code, lang, maxLine)) for (const ch of token.text) out.push(ch === '\n' ? null : [ch, token.cls]);
    return out;
  }
  for (const piece of tokenizeCode(code, lang)) {
    if (piece.text === '\n') { out.push(null); continue; }
    const light = styleOf(piece.scopes, 'light'), dark = styleOf(piece.scopes, 'dark');
    for (const ch of piece.text) out.push([ch, light.fg.toLowerCase(), dark.fg.toLowerCase(), light.italic]);
  }
  return out;
}

let failed = false;
const summary = [];
for (const [lang, files] of Object.entries(corpus)) {
  let total = 0, differing = 0, missingPairs = new Map();
  console.log(`== ${lang}`);
  const engines = await enginesFor(lang);
  for (const file of files) {
    const code = readFileSync(file, 'utf8').replace(/\r\n?/g, '\n');
    const hash = createHash('sha256').update(code).digest('hex').slice(0, 16);
    const clone = cloneChars(code, lang);
    const results = {};
    for (const [name, highlighter] of Object.entries(engines)) {
      let reference;
      // The reference paints a text whose highlighting throws as plain text (ChatMarkdown.tsx).
      try { reference = shikiChars(highlighter, code, lang); } catch (error) {
        console.log(`  shiki (${name}) throws: ${String(error?.message ?? error).slice(0, 120)}`);
        reference = [...code].map(ch => ch === '\n' ? null : [ch, THEMES.light.colors['editor.foreground'].toLowerCase(), THEMES.dark.colors['editor.foreground'].toLowerCase(), false]);
      }
      let diffs = 0, line = 1, column = 0;
      const shown = [];
      const length = Math.max(reference.length, clone.length);
      for (let at = 0; at < length; at++) {
        const a = reference[at], b = clone[at];
        if (a === null || b === null) { line++; column = 0; if (a !== b) { diffs++; } continue; }
        column++;
        if (!a || !b) { diffs++; continue; }
        if (/\s/.test(a[0])) continue;
        if (name === 'oniguruma') {
          total++;
          const pair = `${a[1]}/${a[2]}`;
          if (CLASS_OF_PAIR[pair] === undefined) missingPairs.set(pair, (missingPairs.get(pair) ?? 0) + 1);
        }
        if (maxLine ? classOf(a) !== b[1] : a[1] !== b[1] || a[2] !== b[2] || a[3] !== b[3]) {
          diffs++;
          if (shown.length < 4) shown.push(`${line}:${column} ${JSON.stringify(a[0])} shiki ${maxLine ? classOf(a) : a.slice(1).join(' ')} clone ${b.slice(1).join(' ')}`);
        }
      }
      results[name] = { diffs, shown };
    }
    const main = results.oniguruma;
    if (main.diffs) failed = true;
    differing += main.diffs;
    const js = results.javascript ? ` (javascript engine: ${results.javascript.diffs})` : '';
    console.log(`  ${file.replace(process.env.HOME ?? '\u0000', '~')}  ${code.length} chars  sha256 ${hash}  differing ${main.diffs}${js}`);
    for (const line of main.shown) console.log(`    ${line}`);
  }
  console.log(`  ${lang}: ${files.length} files, ${total} non-blank characters, ${differing} differing`);
  if (missingPairs.size) { failed = true; console.log(`  colour pairs with no synColor class: ${[...missingPairs].map(([pair, n]) => `${pair} ×${n}`).join(', ')}`); }
  summary.push([lang, files.length, total, differing, missingPairs.size]);
}
if (summary.length > 1) {
  const clean = summary.filter(row => !row[3] && !row[4]);
  console.log(`\n== summary: ${summary.length} languages, ${clean.length} with 0 differing characters and every colour pair classed; `
    + `${summary.reduce((sum, row) => sum + row[2], 0)} non-blank characters, ${summary.reduce((sum, row) => sum + row[3], 0)} differing`);
  for (const [lang, , total, differing, pairs] of summary.filter(row => row[3] || row[4])) console.log(`  ${lang}: ${differing} of ${total} differing${pairs ? `, ${pairs} unclassed colour pairs` : ''}`);
}
process.exit(failed ? 1 : 0);
