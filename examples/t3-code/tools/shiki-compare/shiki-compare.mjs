// Verification apparatus (shiki-residuals; never part of the app build). Runs real Shiki 4.2 with the
// Oniguruma engine — the reference T3 Code's highlighter (apps/web/src/lib/syntaxHighlighting.ts,
// PREFERRED_HIGHLIGHTER "shiki-wasm") — and the clone's grammar engine (r12-render-textmate.ts) on
// the same files, and compares every non-blank character's pierre-light colour, pierre-dark colour
// and italic. It also checks that each colour pair Shiki paints has a synColor class
// (r12-render-highlight.ts CLASS_OF_PAIR), since a pair without one paints as plain text.
//
//   bun install --frozen-lockfile                      (once, in this directory)
//   bun shiki-compare.mjs <language> <file>...         one language, any number of files
//   bun shiki-compare.mjs --manifest <corpus.json>     {"<language>": ["<file>", ...], ...}
//   add --js to also run Shiki's JavaScript engine (tells engine differences from grammar ones)
//
// Prints each file's size and sha256, the differing characters per file, and the first differences.
// Exit status 1 when any character differs. Corpus files stay where they are (keep them in target/).

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHighlighterCore } from 'shiki/core';
import { createOnigurumaEngine } from 'shiki/engine/oniguruma';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import { bundledLanguages } from 'shiki/langs';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const { tokenizeCode, styleOf } = await import(join(example, 'r12-render-textmate.ts'));
const { CLASS_OF_PAIR } = await import(join(example, 'r12-render-highlight.ts'));
const theme = name => JSON.parse(readFileSync(join(here, 'node_modules', '@pierre', 'theme', 'themes', `${name}.json`), 'utf8'));
const THEMES = { light: theme('pierre-light'), dark: theme('pierre-dark') };

const args = process.argv.slice(2);
const withJs = args.includes('--js');
const rest = args.filter(arg => arg !== '--js');
let corpus;
if (rest[0] === '--manifest') corpus = JSON.parse(readFileSync(rest[1], 'utf8'));
else corpus = { [rest[0]]: rest.slice(1) };

const langs = await Promise.all(Object.keys(corpus).map(async lang => (await bundledLanguages[lang]()).default));
const engines = { oniguruma: await createHighlighterCore({ themes: Object.values(THEMES), langs, engine: createOnigurumaEngine(import('shiki/wasm')) }) };
if (withJs) engines.javascript = await createHighlighterCore({ themes: Object.values(THEMES), langs, engine: createJavaScriptRegexEngine() });

/** Shiki's per-character [light, dark, italic] for `code`. */
function shikiChars(highlighter, code, lang) {
  const out = [];
  const lines = highlighter.codeToTokensWithThemes(code, { lang, themes: { light: 'pierre-light', dark: 'pierre-dark' } });
  lines.forEach((line, index) => {
    if (index) out.push(null);
    for (const token of line) {
      const light = token.variants.light, dark = token.variants.dark;
      const italic = ((light.fontStyle ?? 0) & 1) === 1;
      for (const ch of token.content) out.push([ch, (light.color ?? '').toLowerCase(), (dark.color ?? '').toLowerCase(), italic]);
    }
  });
  return out;
}
/** The clone's per-character [light, dark, italic]. */
function cloneChars(code, lang) {
  const out = [];
  for (const piece of tokenizeCode(code, lang)) {
    if (piece.text === '\n') { out.push(null); continue; }
    const light = styleOf(piece.scopes, 'light'), dark = styleOf(piece.scopes, 'dark');
    for (const ch of piece.text) out.push([ch, light.fg.toLowerCase(), dark.fg.toLowerCase(), light.italic]);
  }
  return out;
}

let failed = false;
for (const [lang, files] of Object.entries(corpus)) {
  let total = 0, differing = 0, missingPairs = new Map();
  console.log(`== ${lang}`);
  for (const file of files) {
    const code = readFileSync(file, 'utf8').replace(/\r\n?/g, '\n');
    const hash = createHash('sha256').update(code).digest('hex').slice(0, 16);
    const clone = cloneChars(code, lang);
    const results = {};
    for (const [name, highlighter] of Object.entries(engines)) {
      const reference = shikiChars(highlighter, code, lang);
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
        if (a[1] !== b[1] || a[2] !== b[2] || a[3] !== b[3]) {
          diffs++;
          if (shown.length < 4) shown.push(`${line}:${column} ${JSON.stringify(a[0])} shiki ${a.slice(1).join(' ')} clone ${b.slice(1).join(' ')}`);
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
}
process.exit(failed ? 1 : 0);
