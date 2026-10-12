import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { highlight, messageCodeBlocks } from './timeline-highlight';
import { CLASS_OF_PAIR, SLICE_BUDGET_CHARS, SLOW_LANGUAGES, TOKENIZE_MAX_LINE_LENGTH, fileLanguage, highlightPending, highlightSlice, resetHighlightSlicing, shikiLanguage, shikiTokens, startHighlightTurn } from './r12-render-highlight';
import { grammarId, grammarWork, loadedGrammarChunks, tokenizeLine, tokenizeLineWithin } from './r12-render-textmate';
import { GRAMMAR_CHUNKS, GRAMMAR_INDEX, GRAMMAR_LANGUAGES } from './r12-render-grammar';
import { FILE_TYPES } from './r12-render-filetypes';

/** tools/grammar/gen-grammar.mjs LICENCE_HELD: the grammars left out for their licences (GPL-3.0, or none granted
 *  upstream; the coordinator's decision of 2026-10-12. The generator is a developer tool with its own packages;
 *  gen-grammar.test.ts checks the committed data is what it makes). */
const LICENCE_HELD = ['ada', 'dax', 'dream-maker', 'glsl', 'gnuplot', 'nginx', 'racket', 'tcl'];
import { lineTokens } from './timeline-diff-syntax';

// code-highlighting-languages: every language of the reference's Shiki 4.2 bundle has its grammar.
// Each expectation is the pierre-light / pierre-dark colour pair (and "+i" for italic) real Shiki 4.2
// with the Oniguruma engine gives every non-blank character (tools/shiki-compare, the reference's pins).
const PAIR_OF = Object.fromEntries(Object.entries(CLASS_OF_PAIR).map(([pair, cls]) => [cls, pair]));
const paint = (tokens: { text: string; cls: string }[]) => tokens.flatMap(token => {
  const italic = token.cls.endsWith('+i'), base = italic ? token.cls.slice(0, -2) : token.cls;
  return [...token.text].filter(ch => !/\s/.test(ch)).map(ch => ch + PAIR_OF[base] + (italic ? '+i' : ''));
}).join(' ');
const SHIKI: [string, string, string, string][] = [
  ["cpp as Shiki paints it", "cpp", "template <typename T>\nstd::vector<T> twice(const T& x) { return {x, x}; } // pair", "t#a631be/#d568ea e#a631be/#d568ea m#a631be/#d568ea p#a631be/#d568ea l#a631be/#d568ea a#a631be/#d568ea t#a631be/#d568ea e#a631be/#d568ea <#636363/#636363 t#a631be/#d568ea y#a631be/#d568ea p#a631be/#d568ea e#a631be/#d568ea n#a631be/#d568ea a#a631be/#d568ea m#a631be/#d568ea e#a631be/#d568ea T#a631be/#d568ea >#636363/#636363 s#0a0a0a/#fafafa t#0a0a0a/#fafafa d#0a0a0a/#fafafa :#636363/#636363 :#636363/#636363 v#a631be/#d568ea e#a631be/#d568ea c#a631be/#d568ea t#a631be/#d568ea o#a631be/#d568ea r#a631be/#d568ea <#636363/#636363 T#a631be/#d568ea >#636363/#636363 t#693acf/#9d6afb w#693acf/#9d6afb i#693acf/#9d6afb c#693acf/#9d6afb e#693acf/#9d6afb (#636363/#636363 c#d32a61/#ff678d o#d32a61/#ff678d n#d32a61/#ff678d s#d32a61/#ff678d t#d32a61/#ff678d T#a631be/#d568ea &#d32a61/#ff678d x#636363/#a3a3a3 )#636363/#636363 {#636363/#636363 r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d {#636363/#636363 x#0a0a0a/#fafafa ,#636363/#636363 x#0a0a0a/#fafafa }#636363/#636363 ;#636363/#636363 }#636363/#636363 /#737373/#737373 /#737373/#737373 p#737373/#737373 a#737373/#737373 i#737373/#737373 r#737373/#737373"],
  ["haskell as Shiki paints it (a class intersection, rewritten for ES2018)", "haskell", "module Main where\n\n(<+>) :: Int -> Int -> Int\nx <+> y = x + y\n\nmain = print (1 <+> 2)", "m#d32a61/#ff678d o#d32a61/#ff678d d#d32a61/#ff678d u#d32a61/#ff678d l#d32a61/#ff678d e#d32a61/#ff678d M#d5901c/#ffab16 a#d5901c/#ffab16 i#d5901c/#ffab16 n#d5901c/#ffab16 w#d32a61/#ff678d h#d32a61/#ff678d e#d32a61/#ff678d r#d32a61/#ff678d e#d32a61/#ff678d (#636363/#636363 <#693acf/#9d6afb +#693acf/#9d6afb >#693acf/#9d6afb )#636363/#636363 :#636363/#636363 :#636363/#636363 I#d5a910/#ffd452 n#d5a910/#ffd452 t#d5a910/#ffd452 -#636363/#636363 >#636363/#636363 I#d5a910/#ffd452 n#d5a910/#ffd452 t#d5a910/#ffd452 -#636363/#636363 >#636363/#636363 I#d5a910/#ffd452 n#d5a910/#ffd452 t#d5a910/#ffd452 x#0a0a0a/#fafafa <#636363/#636363 +#636363/#636363 >#636363/#636363 y#0a0a0a/#fafafa =#636363/#636363 x#0a0a0a/#fafafa +#636363/#636363 y#0a0a0a/#fafafa m#0a0a0a/#fafafa a#0a0a0a/#fafafa i#0a0a0a/#fafafa n#0a0a0a/#fafafa =#636363/#636363 p#0a0a0a/#fafafa r#0a0a0a/#fafafa i#0a0a0a/#fafafa n#0a0a0a/#fafafa t#0a0a0a/#fafafa (#636363/#636363 1#1ca1c7/#68cdf2 <#636363/#636363 +#636363/#636363 >#636363/#636363 2#1ca1c7/#68cdf2 )#636363/#636363"],
  ["julia as Shiki paints it (a class intersection, rewritten for ES2018)", "julia", "function twice(x::Int)::Int\n    return 2x  # double\nend\nconst π2 = 2π", "f#d32a61/#ff678d u#d32a61/#ff678d n#d32a61/#ff678d c#d32a61/#ff678d t#d32a61/#ff678d i#d32a61/#ff678d o#d32a61/#ff678d n#d32a61/#ff678d t#693acf/#9d6afb w#693acf/#9d6afb i#693acf/#9d6afb c#693acf/#9d6afb e#693acf/#9d6afb (#0a0a0a/#fafafa x#0a0a0a/#fafafa :#636363/#636363 :#636363/#636363 I#a631be/#d568ea n#a631be/#d568ea t#a631be/#d568ea )#0a0a0a/#fafafa :#636363/#636363 :#636363/#636363 I#a631be/#d568ea n#a631be/#d568ea t#a631be/#d568ea r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d 2#1ca1c7/#68cdf2 x#0a0a0a/#fafafa ##737373/#737373 d#737373/#737373 o#737373/#737373 u#737373/#737373 b#737373/#737373 l#737373/#737373 e#737373/#737373 e#d32a61/#ff678d n#d32a61/#ff678d d#d32a61/#ff678d c#d32a61/#ff678d o#d32a61/#ff678d n#d32a61/#ff678d s#d32a61/#ff678d t#d32a61/#ff678d π#0a0a0a/#fafafa 2#0a0a0a/#fafafa =#636363/#636363 2#1ca1c7/#68cdf2 π#1ca1c7/#68cdf2"],
  ["php embedding html as Shiki paints it", "php", "<p class=\"a\"><?php echo strlen($name); ?></p>", "<#08c0ef/#08c0ef p#d5a910/#ffd452 c#d5901c/#ffab16 l#d5901c/#ffab16 a#d5901c/#ffab16 s#d5901c/#ffab16 s#d5901c/#ffab16 =#08c0ef/#08c0ef \"#199f43/#5ecc71 a#199f43/#5ecc71 \"#199f43/#5ecc71 >#08c0ef/#08c0ef <#08c0ef/#08c0ef ?#d32a61/#ff678d p#d5a910/#ffd452 h#d5a910/#ffd452 p#d5a910/#ffd452 e#693acf/#9d6afb c#693acf/#9d6afb h#693acf/#9d6afb o#693acf/#9d6afb s#693acf/#9d6afb t#693acf/#9d6afb r#693acf/#9d6afb l#693acf/#9d6afb e#693acf/#9d6afb n#693acf/#9d6afb (#636363/#636363 $#636363/#636363 n#d47628/#ffa359 a#d47628/#ffa359 m#d47628/#ffa359 e#d47628/#ffa359 )#636363/#636363 ;#636363/#636363 ?#d32a61/#ff678d >#08c0ef/#08c0ef <#08c0ef/#08c0ef /#08c0ef/#08c0ef p#d5a910/#ffd452 >#08c0ef/#08c0ef"],
  ["sql as Shiki paints it", "sql", "SELECT id, name FROM users WHERE id > 10 ORDER BY name; -- all", "S#d32a61/#ff678d E#d32a61/#ff678d L#d32a61/#ff678d E#d32a61/#ff678d C#d32a61/#ff678d T#d32a61/#ff678d i#0a0a0a/#fafafa d#0a0a0a/#fafafa ,#0a0a0a/#fafafa n#d32a61/#ff678d a#d32a61/#ff678d m#d32a61/#ff678d e#d32a61/#ff678d F#d32a61/#ff678d R#d32a61/#ff678d O#d32a61/#ff678d M#d32a61/#ff678d u#0a0a0a/#fafafa s#0a0a0a/#fafafa e#0a0a0a/#fafafa r#0a0a0a/#fafafa s#0a0a0a/#fafafa W#d32a61/#ff678d H#d32a61/#ff678d E#d32a61/#ff678d R#d32a61/#ff678d E#d32a61/#ff678d i#0a0a0a/#fafafa d#0a0a0a/#fafafa >#08c0ef/#08c0ef 1#1ca1c7/#68cdf2 0#1ca1c7/#68cdf2 O#d32a61/#ff678d R#d32a61/#ff678d D#d32a61/#ff678d E#d32a61/#ff678d R#d32a61/#ff678d B#d32a61/#ff678d Y#d32a61/#ff678d n#d32a61/#ff678d a#d32a61/#ff678d m#d32a61/#ff678d e#d32a61/#ff678d ;#0a0a0a/#fafafa -#737373/#737373 -#737373/#737373 a#737373/#737373 l#737373/#737373 l#737373/#737373"],
  ["lua as Shiki paints it", "lua", "local function add(a, b)\n  return a + b -- sum\nend", "l#d32a61/#ff678d o#d32a61/#ff678d c#d32a61/#ff678d a#d32a61/#ff678d l#d32a61/#ff678d f#d32a61/#ff678d u#d32a61/#ff678d n#d32a61/#ff678d c#d32a61/#ff678d t#d32a61/#ff678d i#d32a61/#ff678d o#d32a61/#ff678d n#d32a61/#ff678d a#693acf/#9d6afb d#693acf/#9d6afb d#693acf/#9d6afb (#636363/#636363 a#636363/#a3a3a3 ,#636363/#636363 b#636363/#a3a3a3 )#636363/#636363 r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d a#d47628/#ffa359 +#636363/#636363 b#d47628/#ffa359 -#737373/#737373 -#737373/#737373 s#737373/#737373 u#737373/#737373 m#737373/#737373 e#d32a61/#ff678d n#d32a61/#ff678d d#d32a61/#ff678d"],
  ["vue with its own injections as Shiki paints it", "vue", "<template>\n  <p :title=\"msg\">{{ msg }}</p>\n</template>", "<#636363/#636363 t#d5512f/#ff855e e#d5512f/#ff855e m#d5512f/#ff855e p#d5512f/#ff855e l#d5512f/#ff855e a#d5512f/#ff855e t#d5512f/#ff855e e#d5512f/#ff855e >#636363/#636363 <#636363/#636363 p#d5512f/#ff855e :#18a46c/#60d199 t#18a46c/#60d199 i#18a46c/#60d199 t#18a46c/#60d199 l#18a46c/#60d199 e#18a46c/#60d199 =#636363/#636363 \"#199f43/#5ecc71 m#199f43/#5ecc71 s#199f43/#5ecc71 g#199f43/#5ecc71 \"#199f43/#5ecc71 >#636363/#636363 {#0a0a0a/#fafafa {#0a0a0a/#fafafa m#0a0a0a/#fafafa s#0a0a0a/#fafafa g#0a0a0a/#fafafa }#0a0a0a/#fafafa }#0a0a0a/#fafafa <#636363/#636363 /#636363/#636363 p#d5512f/#ff855e >#636363/#636363 <#636363/#636363 /#636363/#636363 t#d5512f/#ff855e e#d5512f/#ff855e m#d5512f/#ff855e p#d5512f/#ff855e l#d5512f/#ff855e a#d5512f/#ff855e t#d5512f/#ff855e e#d5512f/#ff855e >#636363/#636363"],
  ["terraform as Shiki paints it (MPL-2.0, back)", "terraform", "resource \"aws_s3_bucket\" \"b\" {\n  bucket = \"my-bucket\" # name\n}", "r#a631be/#d568ea e#a631be/#d568ea s#a631be/#d568ea o#a631be/#d568ea u#a631be/#d568ea r#a631be/#d568ea c#a631be/#d568ea e#a631be/#d568ea \"#d47628/#ffa359 a#d47628/#ffa359 w#d47628/#ffa359 s#d47628/#ffa359 _#d47628/#ffa359 s#d47628/#ffa359 3#d47628/#ffa359 _#d47628/#ffa359 b#d47628/#ffa359 u#d47628/#ffa359 c#d47628/#ffa359 k#d47628/#ffa359 e#d47628/#ffa359 t#d47628/#ffa359 \"#d47628/#ffa359 \"#d47628/#ffa359 b#d47628/#ffa359 \"#d47628/#ffa359 {#636363/#636363 b#d47628/#ffa359 u#d47628/#ffa359 c#d47628/#ffa359 k#d47628/#ffa359 e#d47628/#ffa359 t#d47628/#ffa359 =#08c0ef/#08c0ef \"#199f43/#5ecc71 m#199f43/#5ecc71 y#199f43/#5ecc71 -#199f43/#5ecc71 b#199f43/#5ecc71 u#199f43/#5ecc71 c#199f43/#5ecc71 k#199f43/#5ecc71 e#199f43/#5ecc71 t#199f43/#5ecc71 \"#199f43/#5ecc71 ##737373/#737373 n#737373/#737373 a#737373/#737373 m#737373/#737373 e#737373/#737373 }#636363/#636363"],
  ["hcl as Shiki paints it (MPL-2.0, back)", "hcl", "block \"x\" {\n  enabled = true\n}", "b#a631be/#d568ea l#a631be/#d568ea o#a631be/#d568ea c#a631be/#d568ea k#a631be/#d568ea \"#d47628/#ffa359 x#d47628/#ffa359 \"#d47628/#ffa359 {#636363/#636363 e#d47628/#ffa359 n#d47628/#ffa359 a#d47628/#ffa359 b#d47628/#ffa359 l#d47628/#ffa359 e#d47628/#ffa359 d#d47628/#ffa359 =#08c0ef/#08c0ef t#1ca1c7/#68cdf2 r#1ca1c7/#68cdf2 u#1ca1c7/#68cdf2 e#1ca1c7/#68cdf2 }#636363/#636363"],
  ["bird2 as Shiki paints it (MPL-2.0, back)", "bird2", "protocol bgp {\n  local as 65000;\n}", "p#d32a61/#ff678d r#d32a61/#ff678d o#d32a61/#ff678d t#d32a61/#ff678d o#d32a61/#ff678d c#d32a61/#ff678d o#d32a61/#ff678d l#d32a61/#ff678d b#a631be/#d568ea g#a631be/#d568ea p#a631be/#d568ea {#0a0a0a/#fafafa l#d32a61/#ff678d o#d32a61/#ff678d c#d32a61/#ff678d a#d32a61/#ff678d l#d32a61/#ff678d a#d32a61/#ff678d s#d32a61/#ff678d 6#1ca1c7/#68cdf2 5#1ca1c7/#68cdf2 0#1ca1c7/#68cdf2 0#1ca1c7/#68cdf2 0#1ca1c7/#68cdf2 ;#636363/#636363 }#636363/#636363"],
];

// code-highlighting-languages: an app test may leave slicing on, and a language's first use then defers its tokens.
beforeEach(() => resetHighlightSlicing());
afterEach(() => resetHighlightSlicing());

describe('the reference\'s whole language list (code-highlighting-languages)', () => {
  test('Shiki 4.2\'s 235 languages and 97 aliases but the 8 held for their licences (tools/shiki-compare --list checks the names)', () => {
    const index = JSON.parse(GRAMMAR_INDEX) as { roots: Record<string, number>; aliases: Record<string, string>; chunks: unknown[] };
    // LICENCE_HELD (tools/grammar/gen-grammar.mjs) leaves out eight languages, none with an alias.
    expect(LICENCE_HELD.length).toBe(8);
    expect(Object.keys(index.roots).length).toBe(235 - 8);
    expect(Object.keys(index.aliases).length).toBe(97);
    expect(GRAMMAR_LANGUAGES.split(' ').length).toBe(332 - 8);
    expect(LICENCE_HELD.map(grammarId).filter(Boolean)).toEqual([]);
    // The MPL-2.0 grammars are back (LICENSE-T3 carries their notice), with their aliases.
    expect(['bird2', 'bird', 'hcl', 'terraform', 'tf', 'tfvars'].map(grammarId)).toEqual(['bird2', 'bird2', 'hcl', 'terraform', 'terraform', 'terraform']);
    // One chunk per @shikijs/langs grammar generated (18 of them only embedded in or injected into others).
    expect(index.chunks.length).toBe(253 - 8);
    expect(GRAMMAR_CHUNKS.length).toBe(253 - 8);
    expect(['cpp', 'c++', 'haskell', 'hs', 'julia', 'jl', 'php', 'sql', 'lua', 'scss', 'dart', 'scala', 'elixir', 'zig', 'vue', 'svelte', 'objective-c', 'objc', 'wolfram', 'emacs-lisp', 'elisp']
      .map(grammarId)).toEqual(['cpp', 'cpp', 'haskell', 'haskell', 'julia', 'julia', 'php', 'sql', 'lua', 'scss', 'dart', 'scala', 'elixir', 'zig', 'vue', 'svelte', 'objective-c', 'objective-c', 'wolfram', 'emacs-lisp', 'emacs-lisp']);
    // An embedded-only grammar is no fence word, and a name on Object.prototype is no language.
    expect(['cpp-macro', 'markdown-vue', 'es-tag-css', 'constructor', 'toString', 'klingon'].map(grammarId)).toEqual(['', '', '', '', '', '']);
  });
  for (const [name, lang, code, expected] of SHIKI) test(name, () => {
    const tokens = highlight(code, lang);
    expect(tokens.map(token => token.text).join('')).toBe(code);
    expect(paint(tokens)).toBe(expected);
  });
  test('a file name picks its language as @pierre/diffs does', () => {
    expect(Object.keys(FILE_TYPES).length).toBe(345);
    expect(['src/a.hpp', 'x.cc', 'Main.hs', 'base/bool.jl', 'index.php', 'q.sql', 'init.lua', 'a.scss', 'App.vue', 'CMakeLists.txt', 'app.component.ts', 'x.blade.php', 'notes.1c', 'x.h', 'run.unknown'].map(shikiLanguage))
      .toEqual(['cpp', 'cpp', 'haskell', 'julia', 'php', 'sql', 'lua', 'scss', 'vue', 'cmake', 'angular-ts', 'blade', 'bsl', 'objective-cpp', '']);
    // Every value of the table is a language (or alias) with its grammar here, but the held ones, which paint plain.
    const held = new Set(LICENCE_HELD);
    expect(Object.values(FILE_TYPES).filter(lang => !grammarId(lang) && !held.has(lang))).toEqual([]);
    expect(['shader.frag', 'a.vert', 'x.glsl', 'nginx.conf', 'x.adb', 'x.tcl', 'x.rkt'].map(shikiLanguage)).toEqual(['', '', '', '', '', '', '']);
    expect(['main.tf', 'vars.tfvars', 'x.hcl'].map(shikiLanguage)).toEqual(['terraform', 'terraform', 'hcl']);
    expect(lineTokens(['int main() { return 0; }'], 'src/main.cpp')[0]!.some(token => token.cls === 'kw')).toBe(true);
  });
  test('a fence word Shiki does not know paints plain, gitignore reads as ini, and h is no fence word', () => {
    expect(shikiLanguage('gitignore')).toBe('ini');
    expect(messageCodeBlocks('```gitignore\n# build\ntarget/\n```')[0]!.tokens.some(token => token.cls === 'com')).toBe(true);
    for (const word of ['h', 'hpp', 'klingon']) expect(messageCodeBlocks(`\`\`\`${word}\nint x = 1;\n\`\`\``)[0]!.tokens).toEqual([{ id: '0', text: 'int x = 1;', cls: '' }]);
  });
  test('a fence word is looked up as written, and a whole file name matches only as the whole path (review)', () => {
    // Shiki's bundledLanguages keys are lower case: ```Python and ```Dockerfile are plain in the reference.
    expect(['python', 'Python', 'PYTHON', 'dockerfile', 'Dockerfile', 'Makefile', 'CODEOWNERS'].map(shikiLanguage)).toEqual(['python', '', '', 'docker', '', '', '']);
    for (const word of ['Python', 'Dockerfile']) expect(messageCodeBlocks(`\`\`\`${word}\nRUN x = 1\n\`\`\``)[0]!.tokens).toEqual([{ id: '0', text: 'RUN x = 1', cls: '' }]);
    // @pierre/diffs looks the path up whole first: a root Dockerfile, CMakeLists.txt or CODEOWNERS has its
    // language, a nested one none (its simple-extension match finds nothing either).
    expect(['Dockerfile', 'CMakeLists.txt', 'CODEOWNERS', 'docker/Dockerfile', 'src/CMakeLists.txt', '.github/CODEOWNERS', 'Makefile', 'lib/Makefile'].map(fileLanguage))
      .toEqual(['docker', 'cmake', 'codeowners', '', '', '', 'make', '']);
    expect(lineTokens(['FROM node:20'], 'Dockerfile')[0]).toEqual([{ text: 'FROM', cls: 'fn' }, { text: ' node:20', cls: '' }]);
    expect(lineTokens(['FROM node:20'], 'docker/Dockerfile')[0]).toEqual([{ text: 'FROM node:20', cls: '' }]);
  });
  test('every converted regex has ES2018 flags (no v) and compiles, in its ASCII and Unicode \\b forms', () => {
    type Entry = { s: string; f: string; u?: Entry };
    let count = 0, unicode = 0;
    for (const chunk of GRAMMAR_CHUNKS) for (const rx of JSON.parse(chunk()).x as (Entry & { v?: (Entry | number)[] })[]) {
      for (const one of rx.v ? rx.v.filter(variant => typeof variant !== 'number') as Entry[] : [rx]) {
        for (const form of one.u ? [one, one.u] : [one]) {
          expect(form.f).not.toContain('v');
          expect(() => new RegExp(form.s, form.f + 'dg')).not.toThrow();
          count++;
        }
        if (one.u) unicode++;
      }
    }
    expect(count).toBeGreaterThan(25_000);
    expect(unicode).toBeGreaterThan(5_000);
  });
  test('a \\b next to a letter outside ASCII is Unicode, as Oniguruma reads it (Korean emphasis, julia 2π)', () => {
    // Shiki (Oniguruma): `__밑줄__과` is no emphasis, `**굵게**` is bold; `2π` is one number.
    const md = highlight('__밑줄__과 **굵게**', 'markdown');
    expect(md.filter(token => token.cls === 'flag').map(token => token.text).join('')).toBe('**굵게**');
    expect(highlight('x = 2π', 'julia').filter(token => token.cls === 'num').map(token => token.text)).toEqual(['2π']);
    // An ASCII line keeps the ASCII forms, which read \b the same there.
    expect(highlight('x = 2pi', 'julia').filter(token => token.cls === 'num').map(token => token.text)).toEqual(['2pi']);
  });
});

describe('a grammar held for its licence is an empty grammar (code-highlighting-languages, 2026-10-12)', () => {
  // Shiki 4.2 (Oniguruma) with each held grammar replaced by an empty grammar of its scope name (tools/shiki-compare does
  // the same): the rule that embeds glsl stays, and the GLSL inside paints plain.
  const EMBEDS: [string, string, string, string][] = [
    ["cpp embedding glsl", "cpp", "auto s = R\"glsl(void main() { gl_FragColor = vec4(1.0); })glsl\"; // shader", "a#a631be/#d568ea u#a631be/#d568ea t#a631be/#d568ea o#a631be/#d568ea s#0a0a0a/#fafafa =#d32a61/#ff678d R#199f43/#5ecc71 \"#199f43/#5ecc71 g#199f43/#5ecc71 l#199f43/#5ecc71 s#199f43/#5ecc71 l#199f43/#5ecc71 (#199f43/#5ecc71 v#0a0a0a/#fafafa o#0a0a0a/#fafafa i#0a0a0a/#fafafa d#0a0a0a/#fafafa m#0a0a0a/#fafafa a#0a0a0a/#fafafa i#0a0a0a/#fafafa n#0a0a0a/#fafafa (#0a0a0a/#fafafa )#0a0a0a/#fafafa {#0a0a0a/#fafafa g#0a0a0a/#fafafa l#0a0a0a/#fafafa _#0a0a0a/#fafafa F#0a0a0a/#fafafa r#0a0a0a/#fafafa a#0a0a0a/#fafafa g#0a0a0a/#fafafa C#0a0a0a/#fafafa o#0a0a0a/#fafafa l#0a0a0a/#fafafa o#0a0a0a/#fafafa r#0a0a0a/#fafafa =#0a0a0a/#fafafa v#0a0a0a/#fafafa e#0a0a0a/#fafafa c#0a0a0a/#fafafa 4#0a0a0a/#fafafa (#0a0a0a/#fafafa 1#0a0a0a/#fafafa .#0a0a0a/#fafafa 0#0a0a0a/#fafafa )#0a0a0a/#fafafa ;#0a0a0a/#fafafa }#0a0a0a/#fafafa )#199f43/#5ecc71 g#199f43/#5ecc71 l#199f43/#5ecc71 s#199f43/#5ecc71 l#199f43/#5ecc71 \"#199f43/#5ecc71 ;#636363/#636363 /#737373/#737373 /#737373/#737373 s#737373/#737373 h#737373/#737373 a#737373/#737373 d#737373/#737373 e#737373/#737373 r#737373/#737373"],
    ["elm embedding glsl", "elm", "shader = [glsl| void main() { float x = 1.0; } |]", "s#693acf/#9d6afb h#693acf/#9d6afb a#693acf/#9d6afb d#693acf/#9d6afb e#693acf/#9d6afb r#693acf/#9d6afb =#d32a61/#ff678d [#0a0a0a/#fafafa g#0a0a0a/#fafafa l#0a0a0a/#fafafa s#0a0a0a/#fafafa l#0a0a0a/#fafafa |#0a0a0a/#fafafa v#0a0a0a/#fafafa o#0a0a0a/#fafafa i#0a0a0a/#fafafa d#0a0a0a/#fafafa m#0a0a0a/#fafafa a#0a0a0a/#fafafa i#0a0a0a/#fafafa n#0a0a0a/#fafafa (#0a0a0a/#fafafa )#0a0a0a/#fafafa {#0a0a0a/#fafafa f#0a0a0a/#fafafa l#0a0a0a/#fafafa o#0a0a0a/#fafafa a#0a0a0a/#fafafa t#0a0a0a/#fafafa x#0a0a0a/#fafafa =#0a0a0a/#fafafa 1#0a0a0a/#fafafa .#0a0a0a/#fafafa 0#0a0a0a/#fafafa ;#0a0a0a/#fafafa }#0a0a0a/#fafafa |#0a0a0a/#fafafa ]#0a0a0a/#fafafa"],
  ];
  for (const [name, lang, code, expected] of EMBEDS) test(name, () => {
    const tokens = highlight(code, lang);
    expect(tokens.map(token => token.text).join('')).toBe(code);
    expect(paint(tokens)).toBe(expected);
  });
  test('a held language\'s fence word paints plain', () => {
    for (const word of LICENCE_HELD) expect(messageCodeBlocks(`\`\`\`${word}\nvoid main() { x = 1; }\n\`\`\``)[0]!.tokens).toEqual([{ id: '0', text: 'void main() { x = 1; }', cls: '' }]);
  });
});

describe('Files and diffs leave a line of 1,000 characters or more plain, as @pierre/diffs does (review)', () => {
  test('the line is plain and the next one starts in the state before it; chat code has no limit', () => {
    expect(TOKENIZE_MAX_LINE_LENGTH).toBe(1_000);
    const long = '/* ' + 'x'.repeat(TOKENIZE_MAX_LINE_LENGTH - 3), short = long.slice(0, -1);
    const lines = (first: string) => lineTokens([first, 'still */ const c = 1;'], 'a.ts');
    // 1,000 characters: plain, and the comment it opens never opened (Shiki keeps the state before it).
    const [plainLine, after] = lines(long);
    expect(plainLine).toEqual([{ text: long, cls: '' }]);
    expect(after!.find(token => token.text.includes('still'))?.cls).not.toBe('com');
    expect(after!.find(token => token.text === 'const')?.cls).toBe('decl');
    // 999 characters: tokenized, so the next line is still in the comment.
    const [comment, inside] = lines(short);
    expect(comment!.every(token => token.cls === 'com')).toBe(true);
    expect(inside!.find(token => token.text.includes('still'))?.cls).toBe('com');
    // A chat code block (ChatMarkdown, Shiki with no line limit) tokenizes the long line.
    expect(highlight(`${long}\nstill */`, 'ts').every(token => token.cls === 'com' || !token.text.trim())).toBe(true);
  });
});

describe('a long line under slicing always finishes (review: the per-line work limit looped)', () => {
  test('a line retried from its start finishes, though each try makes its back-reference regexes again', () => {
    // Raw strings with hash counts no other test uses: each closes with a regex made from its begin match.
    const line = 'let a = r#######"x"#######; let b = r#########"y"#########; let c = r########"z"########;';
    let result: ReturnType<typeof tokenizeLineWithin> = null, tries = 0;
    // A limit below zero lets each try compile one grammar regex; before the fix the second
    // back-reference regex stopped every try at the same place.
    while (!result && tries < 500) { result = tokenizeLineWithin(line, 'rust', null, -1); tries++; }
    expect(result).not.toBeNull();
    expect(result!.pieces).toEqual(tokenizeLine(line, 'rust', null).pieces);
  });
  const sliced = (text: string, lang: string) => {
    resetHighlightSlicing();
    startHighlightTurn();
    let slices = 0;
    const first = shikiTokens(text, lang);
    while (!first && highlightPending() && slices < 100) { startHighlightTurn(); highlightSlice(); slices++; }
    const tokens = first ?? shikiTokens(text, lang);
    resetHighlightSlicing();
    return { slices, tokens, whole: highlight(text, lang) };
  };
  for (const [name, lang, text] of [
    ['markdown with emphasis', 'markdown', 'see **strong** and *em* words, '.repeat(102) + 'end'],
    ['Korean markdown with emphasis', 'markdown', '이것은 **굵게** 그리고 *기울임* 문장. '.repeat(60)],
    ['rust raw strings', 'rust', 'let a = r#"raw"#; '.repeat(160)],
    ['reStructuredText on its first use', 'rst', 'Some *emphasis*, **strong** and ``literal`` text, '.repeat(60)],
  ] as const) test(`${name}: one line longer than a turn, done in a few slices`, () => {
    expect(text.length).toBeGreaterThan(SLICE_BUDGET_CHARS / 2);
    const { slices, tokens, whole } = sliced(text, lang);
    expect(slices).toBeLessThan(10);
    expect(tokens).toEqual(whole);
  });
});

describe('grammars load when first used, and slow ones take more of a slice (code-highlighting-languages)', () => {
  test('a language\'s chunk is parsed on its first use, not before', () => {
    const before = loadedGrammarChunks();
    expect(before).toBeLessThan(253);
    const work = grammarWork();
    highlight('(defun f (x) (* x 2))', 'emacs-lisp');
    expect(loadedGrammarChunks()).toBe(before + 1);
    // The chunk's parse and its regexes compiled are counted as one-time work.
    expect(grammarWork()).toBeGreaterThan(work);
  });
  test('cpp characters weigh double, so the same budget tokenizes half as much', () => {
    expect(SLOW_LANGUAGES.cpp).toBe(2);
    const line = 'int add(int a, int b) { return a + b; }';
    const text = Array.from({ length: 60 }, () => line).join('\n');
    highlight(text, 'cpp'); // compile its regexes first, so the budget below counts characters only
    highlight(text, 'ts');
    const variant = (lang: string) => { resetHighlightSlicing(); startHighlightTurn(text.length + 200); return shikiTokens(text + '\n// x', lang); };
    expect(variant('ts')).not.toBeNull();
    expect(variant('cpp')).toBeNull();
    while (highlightPending()) highlightSlice();
  });
});
