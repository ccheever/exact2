import { afterEach, describe, expect, test } from 'bun:test';
import { highlight, messageCodeBlocks } from './timeline-highlight';
import { CLASS_OF_PAIR, highlightPending, highlightSlice, resetHighlightSlicing, shikiTokens, startHighlightTurn } from './r12-render-highlight';
import { lineTokens } from './timeline-diff-syntax';
import { codeLines } from './r4-surfaces-files';

// shiki-residuals: more grammars, italics, and long texts in slices. Each expectation is the
// pierre-light / pierre-dark colour pair (and "+i" for italic) real Shiki 4.2 with the Oniguruma
// engine gives every non-blank character (tools/shiki-compare/node_modules, the reference's pins).
const PAIR_OF = Object.fromEntries(Object.entries(CLASS_OF_PAIR).map(([pair, cls]) => [cls, pair]));
const paint = (tokens: { text: string; cls: string }[]) => tokens.flatMap(token => {
  const italic = token.cls.endsWith('+i'), base = italic ? token.cls.slice(0, -2) : token.cls;
  return [...token.text].filter(ch => !/\s/.test(ch)).map(ch => ch + PAIR_OF[base] + (italic ? '+i' : ''));
}).join(' ');
const SHIKI: [string, string, string, string][] = [
  ["c as Shiki paints it", "c", "#include <stdio.h>\nint main(void) { printf(\"%d\\n\", 42); return 0; }", "##636363/#636363 i#d32a61/#ff678d n#d32a61/#ff678d c#d32a61/#ff678d l#d32a61/#ff678d u#d32a61/#ff678d d#d32a61/#ff678d e#d32a61/#ff678d <#199f43/#5ecc71 s#199f43/#5ecc71 t#199f43/#5ecc71 d#199f43/#5ecc71 i#199f43/#5ecc71 o#199f43/#5ecc71 .#199f43/#5ecc71 h#199f43/#5ecc71 >#199f43/#5ecc71 i#a631be/#d568ea n#a631be/#d568ea t#a631be/#d568ea m#693acf/#9d6afb a#693acf/#9d6afb i#693acf/#9d6afb n#693acf/#9d6afb (#636363/#636363 v#a631be/#d568ea o#a631be/#d568ea i#a631be/#d568ea d#a631be/#d568ea )#636363/#636363 {#636363/#636363 p#693acf/#9d6afb r#693acf/#9d6afb i#693acf/#9d6afb n#693acf/#9d6afb t#693acf/#9d6afb f#693acf/#9d6afb (#636363/#636363 \"#199f43/#5ecc71 %#d5a910/#ffd452 d#d5a910/#ffd452 \\#16a994/#61d5c0 n#16a994/#61d5c0 \"#199f43/#5ecc71 ,#636363/#636363 4#1ca1c7/#68cdf2 2#1ca1c7/#68cdf2 )#636363/#636363 ;#636363/#636363 r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d 0#1ca1c7/#68cdf2 ;#636363/#636363 }#636363/#636363"],
  ["java as Shiki paints it", "java", "public final class A { @Override public String toString() { return \"a\" + 1; } }", "p#d32a61/#ff678d u#d32a61/#ff678d b#d32a61/#ff678d l#d32a61/#ff678d i#d32a61/#ff678d c#d32a61/#ff678d f#d32a61/#ff678d i#d32a61/#ff678d n#d32a61/#ff678d a#d32a61/#ff678d l#d32a61/#ff678d c#d32a61/#ff678d l#d32a61/#ff678d a#d32a61/#ff678d s#d32a61/#ff678d s#d32a61/#ff678d A#a631be/#d568ea {#636363/#636363 @#636363/#636363 O#d5901c/#ffab16 v#d5901c/#ffab16 e#d5901c/#ffab16 r#d5901c/#ffab16 r#d5901c/#ffab16 i#d5901c/#ffab16 d#d5901c/#ffab16 e#d5901c/#ffab16 p#d32a61/#ff678d u#d32a61/#ff678d b#d32a61/#ff678d l#d32a61/#ff678d i#d32a61/#ff678d c#d32a61/#ff678d S#d5901c/#ffab16 t#d5901c/#ffab16 r#d5901c/#ffab16 i#d5901c/#ffab16 n#d5901c/#ffab16 g#d5901c/#ffab16 t#693acf/#9d6afb o#693acf/#9d6afb S#693acf/#9d6afb t#693acf/#9d6afb r#693acf/#9d6afb i#693acf/#9d6afb n#693acf/#9d6afb g#693acf/#9d6afb (#636363/#636363 )#636363/#636363 {#636363/#636363 r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d \"#199f43/#5ecc71 a#199f43/#5ecc71 \"#199f43/#5ecc71 +#08c0ef/#08c0ef 1#1ca1c7/#68cdf2 ;#636363/#636363 }#636363/#636363 }#636363/#636363"],
  ["kotlin as Shiki paints it", "kt", "data class P(val x: Int) { fun twice() = x * 2 }", "d#d32a61/#ff678d a#d32a61/#ff678d t#d32a61/#ff678d a#d32a61/#ff678d c#d32a61/#ff678d l#d32a61/#ff678d a#d32a61/#ff678d s#d32a61/#ff678d s#d32a61/#ff678d P#a631be/#d568ea (#0a0a0a/#fafafa v#d32a61/#ff678d a#d32a61/#ff678d l#d32a61/#ff678d x#0a0a0a/#fafafa :#0a0a0a/#fafafa I#a631be/#d568ea n#a631be/#d568ea t#a631be/#d568ea )#0a0a0a/#fafafa {#0a0a0a/#fafafa f#d32a61/#ff678d u#d32a61/#ff678d n#d32a61/#ff678d t#693acf/#9d6afb w#693acf/#9d6afb i#693acf/#9d6afb c#693acf/#9d6afb e#693acf/#9d6afb (#0a0a0a/#fafafa )#0a0a0a/#fafafa =#08c0ef/#08c0ef x#0a0a0a/#fafafa *#08c0ef/#08c0ef 2#1ca1c7/#68cdf2 }#0a0a0a/#fafafa"],
  ["csharp as Shiki paints it", "cs", "namespace N { public record R(int X); var s = $\"{X}\"; }", "n#a631be/#d568ea a#a631be/#d568ea m#a631be/#d568ea e#a631be/#d568ea s#a631be/#d568ea p#a631be/#d568ea a#a631be/#d568ea c#a631be/#d568ea e#a631be/#d568ea N#d5901c/#ffab16 {#636363/#636363 p#d32a61/#ff678d u#d32a61/#ff678d b#d32a61/#ff678d l#d32a61/#ff678d i#d32a61/#ff678d c#d32a61/#ff678d r#a631be/#d568ea e#a631be/#d568ea c#a631be/#d568ea o#a631be/#d568ea r#a631be/#d568ea d#a631be/#d568ea R#a631be/#d568ea (#636363/#636363 i#d32a61/#ff678d n#d32a61/#ff678d t#d32a61/#ff678d X#0a0a0a/#fafafa )#636363/#636363 ;#636363/#636363 v#0a0a0a/#fafafa a#0a0a0a/#fafafa r#0a0a0a/#fafafa s#0a0a0a/#fafafa =#0a0a0a/#fafafa $#0a0a0a/#fafafa \"#0a0a0a/#fafafa {#0a0a0a/#fafafa X#0a0a0a/#fafafa }#636363/#636363 \"#199f43/#5ecc71 ;#199f43/#5ecc71 }#0a0a0a/#fafafa"],
  ["xml as Shiki paints it", "xml", "<?xml version=\"1.0\"?>\n<a b=\"c\"><!-- n --><d/></a>", "<#636363/#636363 ?#636363/#636363 x#d5512f/#ff855e m#d5512f/#ff855e l#d5512f/#ff855e v#18a46c/#60d199 e#18a46c/#60d199 r#18a46c/#60d199 s#18a46c/#60d199 i#18a46c/#60d199 o#18a46c/#60d199 n#18a46c/#60d199 =#636363/#636363 \"#199f43/#5ecc71 1#199f43/#5ecc71 .#199f43/#5ecc71 0#199f43/#5ecc71 \"#199f43/#5ecc71 ?#636363/#636363 >#636363/#636363 <#636363/#636363 a#d5512f/#ff855e b#18a46c/#60d199 =#636363/#636363 \"#199f43/#5ecc71 c#199f43/#5ecc71 \"#199f43/#5ecc71 >#636363/#636363 <#737373/#737373 !#737373/#737373 -#737373/#737373 -#737373/#737373 n#737373/#737373 -#737373/#737373 -#737373/#737373 >#737373/#737373 <#636363/#636363 d#d5512f/#ff855e /#636363/#636363 >#636363/#636363 <#636363/#636363 /#636363/#636363 a#d5512f/#ff855e >#636363/#636363"],
  ["diff as Shiki paints it", "diff", "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new", "-#693acf/#9d6afb -#693acf/#9d6afb -#693acf/#9d6afb a#693acf/#9d6afb /#693acf/#9d6afb x#693acf/#9d6afb +#693acf/#9d6afb +#693acf/#9d6afb +#693acf/#9d6afb b#693acf/#9d6afb /#693acf/#9d6afb x#693acf/#9d6afb @#636363/#636363 @#636363/#636363 -#0a0a0a/#fafafa 1#0a0a0a/#fafafa +#0a0a0a/#fafafa 1#0a0a0a/#fafafa @#636363/#636363 @#636363/#636363 -#636363/#636363 o#d5512f/#ff855e l#d5512f/#ff855e d#d5512f/#ff855e +#636363/#636363 n#199f43/#5ecc71 e#199f43/#5ecc71 w#199f43/#5ecc71"],
  ["docker as Shiki paints it", "dockerfile", "FROM node:20 AS build\nRUN npm ci && npm run build # go\nENV A=1", "F#693acf/#9d6afb R#693acf/#9d6afb O#693acf/#9d6afb M#693acf/#9d6afb n#0a0a0a/#fafafa o#0a0a0a/#fafafa d#0a0a0a/#fafafa e#0a0a0a/#fafafa :#0a0a0a/#fafafa 2#0a0a0a/#fafafa 0#0a0a0a/#fafafa A#693acf/#9d6afb S#693acf/#9d6afb b#0a0a0a/#fafafa u#0a0a0a/#fafafa i#0a0a0a/#fafafa l#0a0a0a/#fafafa d#0a0a0a/#fafafa R#693acf/#9d6afb U#693acf/#9d6afb N#693acf/#9d6afb n#0a0a0a/#fafafa p#0a0a0a/#fafafa m#0a0a0a/#fafafa c#0a0a0a/#fafafa i#0a0a0a/#fafafa &#0a0a0a/#fafafa &#0a0a0a/#fafafa n#0a0a0a/#fafafa p#0a0a0a/#fafafa m#0a0a0a/#fafafa r#0a0a0a/#fafafa u#0a0a0a/#fafafa n#0a0a0a/#fafafa b#0a0a0a/#fafafa u#0a0a0a/#fafafa i#0a0a0a/#fafafa l#0a0a0a/#fafafa d#0a0a0a/#fafafa ##0a0a0a/#fafafa g#0a0a0a/#fafafa o#0a0a0a/#fafafa E#693acf/#9d6afb N#693acf/#9d6afb V#693acf/#9d6afb A#0a0a0a/#fafafa =#0a0a0a/#fafafa 1#0a0a0a/#fafafa"],
  ["make as Shiki paints it", "makefile", "all: $(OBJ)\n\t$(CC) -o $@ $^ # link", "a#693acf/#9d6afb l#693acf/#9d6afb l#693acf/#9d6afb :#636363/#636363 $#636363/#636363 (#636363/#636363 O#d47628/#ffa359 B#d47628/#ffa359 J#d47628/#ffa359 )#636363/#636363 $#636363/#636363 (#636363/#636363 C#d47628/#ffa359 C#d47628/#ffa359 )#636363/#636363 -#d5901c/#ffab16 o#d5901c/#ffab16 $#d5901c/#ffab16 @#d5901c/#ffab16 $#d5901c/#ffab16 ^#d5901c/#ffab16 ##d5901c/#ffab16 l#d5901c/#ffab16 i#d5901c/#ffab16 n#d5901c/#ffab16 k#d5901c/#ffab16"],
  ["ruby as Shiki paints it", "rb", "class A < B\n  def go(x = 1) = puts \"#{x}\" if x\nend", "c#d32a61/#ff678d l#d32a61/#ff678d a#d32a61/#ff678d s#d32a61/#ff678d s#d32a61/#ff678d A#a631be/#d568ea <#636363/#636363 B#a631be/#d568ea d#d32a61/#ff678d e#d32a61/#ff678d f#d32a61/#ff678d g#693acf/#9d6afb o#693acf/#9d6afb (#636363/#636363 x#636363/#a3a3a3 =#08c0ef/#08c0ef 1#1ca1c7/#68cdf2 )#636363/#636363 =#08c0ef/#08c0ef p#693acf/#9d6afb u#693acf/#9d6afb t#693acf/#9d6afb s#693acf/#9d6afb \"#199f43/#5ecc71 ##d32a61/#ff678d {#d32a61/#ff678d x#199f43/#5ecc71 }#d32a61/#ff678d \"#199f43/#5ecc71 i#d32a61/#ff678d f#d32a61/#ff678d x#0a0a0a/#fafafa e#d32a61/#ff678d n#d32a61/#ff678d d#d32a61/#ff678d"],
  ["markdown as Shiki paints it", "md", "Some *soft* and **loud** words, _here_ too.", "S#0a0a0a/#fafafa o#0a0a0a/#fafafa m#0a0a0a/#fafafa e#0a0a0a/#fafafa *#d32a61/#ff678d+i s#d32a61/#ff678d+i o#d32a61/#ff678d+i f#d32a61/#ff678d+i t#d32a61/#ff678d+i *#d32a61/#ff678d+i a#0a0a0a/#fafafa n#0a0a0a/#fafafa d#0a0a0a/#fafafa *#d5a910/#ffd452 *#d5a910/#ffd452 l#d5a910/#ffd452 o#d5a910/#ffd452 u#d5a910/#ffd452 d#d5a910/#ffd452 *#d5a910/#ffd452 *#d5a910/#ffd452 w#0a0a0a/#fafafa o#0a0a0a/#fafafa r#0a0a0a/#fafafa d#0a0a0a/#fafafa s#0a0a0a/#fafafa ,#0a0a0a/#fafafa _#d32a61/#ff678d+i h#d32a61/#ff678d+i e#d32a61/#ff678d+i r#d32a61/#ff678d+i e#d32a61/#ff678d+i _#d32a61/#ff678d+i t#0a0a0a/#fafafa o#0a0a0a/#fafafa o#0a0a0a/#fafafa .#0a0a0a/#fafafa"]];

afterEach(() => resetHighlightSlicing());

describe('the added grammars and italics against Shiki 4.2 (shiki-residuals)', () => {
  for (const [name, lang, code, expected] of SHIKI) test(name, () => {
    const tokens = highlight(code, lang);
    expect(tokens.map(token => token.text).join('')).toBe(code);
    expect(paint(tokens)).toBe(expected);
  });
  test('italic reaches a chat fence, a diff side and a Files line', () => {
    const block = messageCodeBlocks('```md\nSome *soft* words\n```')[0]!;
    expect(block.tokens.filter(token => token.cls.endsWith('+i')).map(token => token.text).join('')).toBe('*soft*');
    expect(lineTokens(['Some *soft* words'], 'notes/a.md')[0]!.some(token => token.cls === 'kw+i')).toBe(true);
    expect(codeLines('a.md', 'Some *soft* words')[0]!.runs.some(run => run.syntax === 'kw+i')).toBe(true);
  });
  test('a fence language with no grammar paints as one plain run, as the reference falls back to text', () => {
    expect(messageCodeBlocks('```cobol\nDISPLAY "HI".\n```')[0]!.tokens).toEqual([{ id: '0', text: 'DISPLAY "HI".', cls: '' }]);
    // An empty fence paints nothing.
    expect(messageCodeBlocks('```ts\n\n```')[0]!.tokens.map(token => token.text).join('')).toBe('');
  });
});

describe('long texts are tokenized in slices (shiki-residuals)', () => {
  const long = Array.from({ length: 600 }, (_, at) => `export const v${at}: Map<string, number> = new Map([["k${at}", ${at}]]); // ${at}`).join('\n');
  test('an answer over its budget paints heuristic colours, slices finish it, and the result equals an unsliced run', () => {
    const whole = shikiTokens(long, 'ts')!;
    resetHighlightSlicing();
    startHighlightTurn(5);
    expect(shikiTokens(long, 'ts')).toBeNull();
    expect(highlightPending()).toBe(true);
    expect(highlight(long, 'ts').map(token => token.text).join('')).toBe(long); // heuristic tokens meanwhile
    let turns = 0, finished = false;
    while (highlightPending()) { const step = highlightSlice(5); finished ||= step.finished; turns++; }
    expect(finished).toBe(true);
    expect(turns).toBeGreaterThan(1);
    startHighlightTurn(5);
    expect(shikiTokens(long, 'ts')).toEqual(whole);
  });
  test('a slice for an older text never replaces a newer one', () => {
    startHighlightTurn(0);
    const older = long, newer = long + '\nexport const tail = "new";';
    expect(shikiTokens(older, 'ts')).toBeNull();
    expect(shikiTokens(newer, 'ts')).toBeNull();
    while (highlightPending()) highlightSlice(5);
    startHighlightTurn(50);
    expect(shikiTokens(newer, 'ts')!.map(token => token.text).join('')).toBe(newer);
    expect(shikiTokens(older, 'ts')!.map(token => token.text).join('')).toBe(older);
  });
  test("a chat message's blocks are asked again until their Shiki tokens are in", () => {
    const message = '```ts\n' + long + '\n```';
    startHighlightTurn(0);
    const first = messageCodeBlocks(message)[0]!;
    while (highlightPending()) highlightSlice(5);
    startHighlightTurn(50);
    const later = messageCodeBlocks(message)[0]!;
    expect(later.tokens).not.toEqual(first.tokens);
    expect(later.tokens.find(token => token.text.includes('Map'))?.cls).toBe('decl');
  });
  test('a text nobody asks for again is dropped after 300 answers', () => {
    startHighlightTurn(0);
    expect(shikiTokens(long, 'ts')).toBeNull();
    for (let answer = 0; answer < 301; answer++) startHighlightTurn(0);
    expect(highlightSlice(5)).toEqual({ finished: false, pending: false });
  });
  test('the turn budget counts characters, never the clock (a data source has none)', () => {
    const realNow = Date.now;
    Date.now = () => { throw new Error('Date.now() is unavailable in data sources'); };
    try {
      startHighlightTurn(5);
      expect(shikiTokens(long, 'ts')).toBeNull();
      expect(highlightSlice(5).pending).toBe(true);
    } finally { Date.now = realNow; }
  });
});
