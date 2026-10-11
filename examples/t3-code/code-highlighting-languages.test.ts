import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { highlight, messageCodeBlocks } from './timeline-highlight';
import { CLASS_OF_PAIR, SLOW_LANGUAGES, highlightPending, highlightSlice, resetHighlightSlicing, shikiLanguage, shikiTokens, startHighlightTurn } from './r12-render-highlight';
import { grammarId, grammarWork, loadedGrammarChunks } from './r12-render-textmate';
import { GRAMMAR_CHUNKS, GRAMMAR_INDEX, GRAMMAR_LANGUAGES } from './r12-render-grammar';
import { FILE_TYPES } from './r12-render-filetypes';
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
  ["vue with its own injections as Shiki paints it", "vue", "<template>\n  <p :title=\"msg\">{{ msg }}</p>\n</template>", "<#636363/#636363 t#d5512f/#ff855e e#d5512f/#ff855e m#d5512f/#ff855e p#d5512f/#ff855e l#d5512f/#ff855e a#d5512f/#ff855e t#d5512f/#ff855e e#d5512f/#ff855e >#636363/#636363 <#636363/#636363 p#d5512f/#ff855e :#18a46c/#60d199 t#18a46c/#60d199 i#18a46c/#60d199 t#18a46c/#60d199 l#18a46c/#60d199 e#18a46c/#60d199 =#636363/#636363 \"#199f43/#5ecc71 m#199f43/#5ecc71 s#199f43/#5ecc71 g#199f43/#5ecc71 \"#199f43/#5ecc71 >#636363/#636363 {#0a0a0a/#fafafa {#0a0a0a/#fafafa m#0a0a0a/#fafafa s#0a0a0a/#fafafa g#0a0a0a/#fafafa }#0a0a0a/#fafafa }#0a0a0a/#fafafa <#636363/#636363 /#636363/#636363 p#d5512f/#ff855e >#636363/#636363 <#636363/#636363 /#636363/#636363 t#d5512f/#ff855e e#d5512f/#ff855e m#d5512f/#ff855e p#d5512f/#ff855e l#d5512f/#ff855e a#d5512f/#ff855e t#d5512f/#ff855e e#d5512f/#ff855e >#636363/#636363"]];

// code-highlighting-languages: an app test may leave slicing on, and a language's first use then defers its tokens.
beforeEach(() => resetHighlightSlicing());
afterEach(() => resetHighlightSlicing());

describe('the reference\'s whole language list (code-highlighting-languages)', () => {
  test('235 languages and 97 aliases, as Shiki 4.2 bundles them (tools/shiki-compare --list checks the names)', () => {
    const index = JSON.parse(GRAMMAR_INDEX) as { roots: Record<string, number>; aliases: Record<string, string>; chunks: unknown[] };
    expect(Object.keys(index.roots).length).toBe(235);
    expect(Object.keys(index.aliases).length).toBe(97);
    expect(GRAMMAR_LANGUAGES.split(' ').length).toBe(332);
    // One chunk per @shikijs/langs grammar (18 of them only embedded in or injected into others).
    expect(index.chunks.length).toBe(253);
    expect(GRAMMAR_CHUNKS.length).toBe(253);
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
    expect(['src/a.hpp', 'x.cc', 'Main.hs', 'base/bool.jl', 'index.php', 'q.sql', 'init.lua', 'a.scss', 'App.vue', 'CMakeLists.txt', 'app.component.ts', 'x.blade.php', 'main.tf', 'notes.1c', 'x.h', 'run.unknown'].map(shikiLanguage))
      .toEqual(['cpp', 'cpp', 'haskell', 'julia', 'php', 'sql', 'lua', 'scss', 'vue', 'cmake', 'angular-ts', 'blade', 'terraform', 'bsl', 'objective-cpp', '']);
    // Every value of the table is a language (or alias) with its grammar here.
    expect(Object.values(FILE_TYPES).filter(lang => !grammarId(lang))).toEqual([]);
    expect(lineTokens(['int main() { return 0; }'], 'src/main.cpp')[0]!.some(token => token.cls === 'kw')).toBe(true);
  });
  test('a fence word Shiki does not know paints plain, gitignore reads as ini, and h is no fence word', () => {
    expect(shikiLanguage('gitignore')).toBe('ini');
    expect(messageCodeBlocks('```gitignore\n# build\ntarget/\n```')[0]!.tokens.some(token => token.cls === 'com')).toBe(true);
    for (const word of ['h', 'hpp', 'klingon']) expect(messageCodeBlocks(`\`\`\`${word}\nint x = 1;\n\`\`\``)[0]!.tokens).toEqual([{ id: '0', text: 'int x = 1;', cls: '' }]);
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
