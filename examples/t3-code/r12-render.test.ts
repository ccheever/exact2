import { describe, expect, test } from 'bun:test';
import { highlight, messageCodeBlocks } from './timeline-highlight';
import { shikiLanguage, shikiTokens, SHIKI_MAX_CHARS } from './r12-render-highlight';
import { lineTokens } from './timeline-diff-syntax';
import { codeLines } from './r4-surfaces-files';

// Lane r12-render: the shared highlighter runs the reference's Shiki 4.2 grammars and pierre themes.
// Each expectation is the light/dark colour pair real Shiki 4.2 gives every non-blank character
// (generated with the reference's own shiki and @pierre/theme packages).
const PAIR: Record<string, string> = {
  kw: '#d32a61/#ff678d', decl: '#a631be/#d568ea', type: '#a631be/#d568ea', fn: '#693acf/#9d6afb', var: '#d47628/#ffa359',
  const: '#d5901c/#ffab16', param: '#636363/#a3a3a3', flag: '#d5a910/#ffd452', pyparam: '#d5a910/#ffd452', bold: '#d5a910/#ffd452',
  interp: '#d5a910/#ffd452', str: '#199f43/#5ecc71', esc: '#16a994/#61d5c0', num: '#1ca1c7/#68cdf2', op: '#08c0ef/#08c0ef',
  nul: '#08c0ef/#08c0ef', builtin: '#08c0ef/#08c0ef', punct: '#636363/#636363', com: '#737373/#737373', key: '#d5512f/#ff855e',
  tag: '#d5512f/#ff855e', heading: '#d5512f/#ff855e', attr: '#18a46c/#60d199', regex: '#17a5af/#64d1db', deco: '#1a85d4/#69b1ff',
  '': '#0a0a0a/#fafafa',
};
const paint = (tokens: { text: string; cls: string }[]) =>
  tokens.flatMap(token => [...token.text].filter(ch => !/\s/.test(ch)).map(ch => ch + PAIR[token.cls])).join(' ');
const SHIKI: [string, string, string, string][] = [["type-alias right-hand sides and | in types","typescript","type Handler<T> = (event: T) => Promise<void> | void;\nexport type R = { ok: true } | { ok: false; error: E };","t#a631be/#d568ea y#a631be/#d568ea p#a631be/#d568ea e#a631be/#d568ea H#a631be/#d568ea a#a631be/#d568ea n#a631be/#d568ea d#a631be/#d568ea l#a631be/#d568ea e#a631be/#d568ea r#a631be/#d568ea <#636363/#636363 T#a631be/#d568ea >#636363/#636363 =#08c0ef/#08c0ef (#636363/#636363 e#636363/#a3a3a3 v#636363/#a3a3a3 e#636363/#a3a3a3 n#636363/#a3a3a3 t#636363/#a3a3a3 :#636363/#636363 T#a631be/#d568ea )#636363/#636363 =#a631be/#d568ea >#a631be/#d568ea P#a631be/#d568ea r#a631be/#d568ea o#a631be/#d568ea m#a631be/#d568ea i#a631be/#d568ea s#a631be/#d568ea e#a631be/#d568ea <#636363/#636363 v#a631be/#d568ea o#a631be/#d568ea i#a631be/#d568ea d#a631be/#d568ea >#636363/#636363 |#636363/#636363 v#a631be/#d568ea o#a631be/#d568ea i#a631be/#d568ea d#a631be/#d568ea ;#636363/#636363 e#d32a61/#ff678d x#d32a61/#ff678d p#d32a61/#ff678d o#d32a61/#ff678d r#d32a61/#ff678d t#d32a61/#ff678d t#a631be/#d568ea y#a631be/#d568ea p#a631be/#d568ea e#a631be/#d568ea R#a631be/#d568ea =#08c0ef/#08c0ef {#636363/#636363 o#d47628/#ffa359 k#d47628/#ffa359 :#636363/#636363 t#a631be/#d568ea r#a631be/#d568ea u#a631be/#d568ea e#a631be/#d568ea }#636363/#636363 |#636363/#636363 {#636363/#636363 o#d47628/#ffa359 k#d47628/#ffa359 :#636363/#636363 f#a631be/#d568ea a#a631be/#d568ea l#a631be/#d568ea s#a631be/#d568ea e#a631be/#d568ea ;#636363/#636363 e#d47628/#ffa359 r#d47628/#ffa359 r#d47628/#ffa359 o#d47628/#ffa359 r#d47628/#ffa359 :#636363/#636363 E#a631be/#d568ea }#636363/#636363 ;#636363/#636363"],
  ["an initializer ending at a line-final operator","typescript","const hit = cache.get(key) ??\n  fallback;\nexport const total = [1, 2].reduce((s, n) => s + n, 0) *\n  2;","c#a631be/#d568ea o#a631be/#d568ea n#a631be/#d568ea s#a631be/#d568ea t#a631be/#d568ea h#d5901c/#ffab16 i#d5901c/#ffab16 t#d5901c/#ffab16 =#08c0ef/#08c0ef c#d47628/#ffa359 a#d47628/#ffa359 c#d47628/#ffa359 h#d47628/#ffa359 e#d47628/#ffa359 .#636363/#636363 g#693acf/#9d6afb e#693acf/#9d6afb t#693acf/#9d6afb (#636363/#636363 k#d47628/#ffa359 e#d47628/#ffa359 y#d47628/#ffa359 )#636363/#636363 ?#08c0ef/#08c0ef ?#08c0ef/#08c0ef f#d47628/#ffa359 a#d47628/#ffa359 l#d47628/#ffa359 l#d47628/#ffa359 b#d47628/#ffa359 a#d47628/#ffa359 c#d47628/#ffa359 k#d47628/#ffa359 ;#636363/#636363 e#d32a61/#ff678d x#d32a61/#ff678d p#d32a61/#ff678d o#d32a61/#ff678d r#d32a61/#ff678d t#d32a61/#ff678d c#a631be/#d568ea o#a631be/#d568ea n#a631be/#d568ea s#a631be/#d568ea t#a631be/#d568ea t#d5901c/#ffab16 o#d5901c/#ffab16 t#d5901c/#ffab16 a#d5901c/#ffab16 l#d5901c/#ffab16 =#08c0ef/#08c0ef [#636363/#636363 1#1ca1c7/#68cdf2 ,#636363/#636363 2#1ca1c7/#68cdf2 ]#636363/#636363 .#636363/#636363 r#693acf/#9d6afb e#693acf/#9d6afb d#693acf/#9d6afb u#693acf/#9d6afb c#693acf/#9d6afb e#693acf/#9d6afb (#636363/#636363 (#636363/#636363 s#636363/#a3a3a3 ,#636363/#636363 n#636363/#a3a3a3 )#636363/#636363 =#a631be/#d568ea >#a631be/#d568ea s#d47628/#ffa359 +#08c0ef/#08c0ef n#d47628/#ffa359 ,#636363/#636363 0#1ca1c7/#68cdf2 )#636363/#636363 *#08c0ef/#08c0ef 2#1ca1c7/#68cdf2 ;#636363/#636363"],
  ["decorators paint in their own blue","typescript","@Injectable({ scope: \"x\" })\nclass A { @Input() name?: string; }","@#1a85d4/#69b1ff I#693acf/#9d6afb n#693acf/#9d6afb j#693acf/#9d6afb e#693acf/#9d6afb c#693acf/#9d6afb t#693acf/#9d6afb a#693acf/#9d6afb b#693acf/#9d6afb l#693acf/#9d6afb e#693acf/#9d6afb (#636363/#636363 {#636363/#636363 s#d47628/#ffa359 c#d47628/#ffa359 o#d47628/#ffa359 p#d47628/#ffa359 e#d47628/#ffa359 :#636363/#636363 \"#199f43/#5ecc71 x#199f43/#5ecc71 \"#199f43/#5ecc71 }#636363/#636363 )#636363/#636363 c#a631be/#d568ea l#a631be/#d568ea a#a631be/#d568ea s#a631be/#d568ea s#a631be/#d568ea A#a631be/#d568ea {#636363/#636363 @#1a85d4/#69b1ff I#693acf/#9d6afb n#693acf/#9d6afb p#693acf/#9d6afb u#693acf/#9d6afb t#693acf/#9d6afb (#636363/#636363 )#636363/#636363 n#d47628/#ffa359 a#d47628/#ffa359 m#d47628/#ffa359 e#d47628/#ffa359 ?#d32a61/#ff678d :#636363/#636363 s#a631be/#d568ea t#a631be/#d568ea r#a631be/#d568ea i#a631be/#d568ea n#a631be/#d568ea g#a631be/#d568ea ;#636363/#636363 }#636363/#636363"],
  ["JSX in a .tsx file","tsx","const v = <div className=\"row\" onClick={() => go(1)}>{total}</div>;","c#a631be/#d568ea o#a631be/#d568ea n#a631be/#d568ea s#a631be/#d568ea t#a631be/#d568ea v#d5901c/#ffab16 =#08c0ef/#08c0ef <#636363/#636363 d#d5512f/#ff855e i#d5512f/#ff855e v#d5512f/#ff855e c#18a46c/#60d199 l#18a46c/#60d199 a#18a46c/#60d199 s#18a46c/#60d199 s#18a46c/#60d199 N#18a46c/#60d199 a#18a46c/#60d199 m#18a46c/#60d199 e#18a46c/#60d199 =#08c0ef/#08c0ef \"#199f43/#5ecc71 r#199f43/#5ecc71 o#199f43/#5ecc71 w#199f43/#5ecc71 \"#199f43/#5ecc71 o#18a46c/#60d199 n#18a46c/#60d199 C#18a46c/#60d199 l#18a46c/#60d199 i#18a46c/#60d199 c#18a46c/#60d199 k#18a46c/#60d199 =#08c0ef/#08c0ef {#d32a61/#ff678d (#636363/#636363 )#636363/#636363 =#a631be/#d568ea >#a631be/#d568ea g#693acf/#9d6afb o#693acf/#9d6afb (#636363/#636363 1#1ca1c7/#68cdf2 )#636363/#636363 }#d32a61/#ff678d >#636363/#636363 {#d32a61/#ff678d t#d47628/#ffa359 o#d47628/#ffa359 t#d47628/#ffa359 a#d47628/#ffa359 l#d47628/#ffa359 }#d32a61/#ff678d <#636363/#636363 /#636363/#636363 d#d5512f/#ff855e i#d5512f/#ff855e v#d5512f/#ff855e >#636363/#636363 ;#636363/#636363"],
  ["a Python decorator, f-string and keyword arguments","python","@dataclass\ndef run(self, *args, **kw) -> bool:\n    return f\"{self.name!r}\"","@#1a85d4/#69b1ff d#1a85d4/#69b1ff a#1a85d4/#69b1ff t#1a85d4/#69b1ff a#1a85d4/#69b1ff c#1a85d4/#69b1ff l#1a85d4/#69b1ff a#1a85d4/#69b1ff s#1a85d4/#69b1ff s#1a85d4/#69b1ff d#a631be/#d568ea e#a631be/#d568ea f#a631be/#d568ea r#693acf/#9d6afb u#693acf/#9d6afb n#693acf/#9d6afb (#636363/#636363 s#d5901c/#ffab16 e#d5901c/#ffab16 l#d5901c/#ffab16 f#d5901c/#ffab16 ,#636363/#636363 *#636363/#636363 a#d5a910/#ffd452 r#d5a910/#ffd452 g#d5a910/#ffd452 s#d5a910/#ffd452 ,#636363/#636363 *#636363/#636363 *#636363/#636363 k#d5a910/#ffd452 w#d5a910/#ffd452 )#636363/#636363 -#636363/#636363 >#636363/#636363 b#08c0ef/#08c0ef o#08c0ef/#08c0ef o#08c0ef/#08c0ef l#08c0ef/#08c0ef :#636363/#636363 r#d32a61/#ff678d e#d32a61/#ff678d t#d32a61/#ff678d u#d32a61/#ff678d r#d32a61/#ff678d n#d32a61/#ff678d f#a631be/#d568ea \"#199f43/#5ecc71 {#d5a910/#ffd452 s#d5901c/#ffab16 e#d5901c/#ffab16 l#d5901c/#ffab16 f#d5901c/#ffab16 .#636363/#636363 n#0a0a0a/#fafafa a#0a0a0a/#fafafa m#0a0a0a/#fafafa e#0a0a0a/#fafafa !#a631be/#d568ea r#a631be/#d568ea }#d5a910/#ffd452 \"#199f43/#5ecc71"],
  ["shell: an if with a quoted command word (a lookbehind Hermes needs rewritten)","shellscript","if \"$interactive\" && [ -z \"${NO_COLOR:-}\" ]; then\n  echo hi >&2\nfi","i#d32a61/#ff678d f#d32a61/#ff678d \"#693acf/#9d6afb $#d47628/#ffa359 i#d47628/#ffa359 n#d47628/#ffa359 t#d47628/#ffa359 e#d47628/#ffa359 r#d47628/#ffa359 a#d47628/#ffa359 c#d47628/#ffa359 t#d47628/#ffa359 i#d47628/#ffa359 v#d47628/#ffa359 e#d47628/#ffa359 \"#693acf/#9d6afb &#636363/#636363 &#636363/#636363 [#636363/#636363 -#08c0ef/#08c0ef z#08c0ef/#08c0ef \"#199f43/#5ecc71 $#636363/#636363 {#636363/#636363 N#d47628/#ffa359 O#d47628/#ffa359 _#d47628/#ffa359 C#d47628/#ffa359 O#d47628/#ffa359 L#d47628/#ffa359 O#d47628/#ffa359 R#d47628/#ffa359 :#636363/#636363 -#636363/#636363 }#636363/#636363 \"#199f43/#5ecc71 ]#636363/#636363 ;#636363/#636363 t#d32a61/#ff678d h#d32a61/#ff678d e#d32a61/#ff678d n#d32a61/#ff678d e#693acf/#9d6afb c#693acf/#9d6afb h#693acf/#9d6afb o#693acf/#9d6afb h#199f43/#5ecc71 i#199f43/#5ecc71 >#636363/#636363 &#636363/#636363 2#636363/#636363 f#d32a61/#ff678d i#d32a61/#ff678d"],
  ["JSON keys and values","json","{ \"name\": \"x\", \"n\": 1, \"ok\": true, \"none\": null }","{#636363/#636363 \"#d5512f/#ff855e n#d5512f/#ff855e a#d5512f/#ff855e m#d5512f/#ff855e e#d5512f/#ff855e \"#d5512f/#ff855e :#636363/#636363 \"#199f43/#5ecc71 x#199f43/#5ecc71 \"#199f43/#5ecc71 ,#636363/#636363 \"#d5512f/#ff855e n#d5512f/#ff855e \"#d5512f/#ff855e :#636363/#636363 1#1ca1c7/#68cdf2 ,#636363/#636363 \"#d5512f/#ff855e o#d5512f/#ff855e k#d5512f/#ff855e \"#d5512f/#ff855e :#636363/#636363 t#1ca1c7/#68cdf2 r#1ca1c7/#68cdf2 u#1ca1c7/#68cdf2 e#1ca1c7/#68cdf2 ,#636363/#636363 \"#d5512f/#ff855e n#d5512f/#ff855e o#d5512f/#ff855e n#d5512f/#ff855e e#d5512f/#ff855e \"#d5512f/#ff855e :#636363/#636363 n#1ca1c7/#68cdf2 u#1ca1c7/#68cdf2 l#1ca1c7/#68cdf2 l#1ca1c7/#68cdf2 }#636363/#636363"]];

describe('the shared highlighter against Shiki 4.2 (r12-render)', () => {
  for (const [name, lang, code, expected] of SHIKI) test(name, () => {
    const tokens = highlight(code, lang);
    expect(tokens.map(token => token.text).join('')).toBe(code);
    expect(paint(tokens)).toBe(expected);
  });
  test('fence names and file paths pick the grammar @pierre/diffs and Shiki pick', () => {
    expect(['ts', 'tsx', 'js', 'sh', 'zsh', 'py', 'yml', 'md', 'rs', 'go', 'swift', 'jsonc'].map(shikiLanguage))
      .toEqual(['typescript', 'tsx', 'javascript', 'shellscript', 'shellscript', 'python', 'yaml', 'markdown', 'rust', 'go', 'swift', 'jsonc']);
    expect(['src/a.mts', 'a/b.tsx', 'x.cjs', 'run.bash', 'c.yaml', 'Cargo.toml', 'page.htm', 'notes.unknown', 'Makefile', 'ruby'].map(shikiLanguage))
      .toEqual(['typescript', 'tsx', 'javascript', 'shellscript', 'yaml', 'toml', 'html', '', 'make', 'ruby']);
    // shiki-residuals: the added grammars, by fence word and by file name.
    expect(['c', 'java', 'kt', 'cs', 'c#', 'xml', 'diff', 'dockerfile', 'makefile', 'rb', 'cobol'].map(shikiLanguage))
      .toEqual(['c', 'java', 'kotlin', 'csharp', 'csharp', 'xml', 'diff', 'docker', 'make', 'ruby', '']);
    expect(['src/main.c', 'A.java', 'b/C.kt', 'D.cs', 'pom.xml', 'fix.patch', 'docker/Dockerfile', 'lib/x.mk', 'Gemfile', 'a/b.gemspec', 'x.h'].map(shikiLanguage))
      .toEqual(['c', 'java', 'kotlin', 'csharp', 'xml', 'diff', 'docker', 'make', 'ruby', 'ruby', '']);
  });
  test('every surface reaches it: chat fences, the diff panel, the Files panel', () => {
    const block = messageCodeBlocks('```tsx\nconst v = <b>{x}</b>;\n```')[0]!;
    expect(block.tokens.map(token => token.text).join('')).toBe('const v = <b>{x}</b>;');
    expect(block.tokens.find(token => token.text === 'b')?.cls).toBe('tag');
    // A .tsx diff side is tokenized as tsx, not typescript (the path picks the grammar).
    expect(lineTokens(['const v = <b>{x}</b>;'], 'a/View.tsx')[0]!.find(token => token.text === 'b')?.cls).toBe('tag');
    expect(codeLines('Cargo.toml', '[package]\nname = "x"')[1]!.runs.map(run => run.syntax)).toEqual(['var', '', 'punct', '', 'str']);
  });
  test("a code line is keyed by its runs, so another file's line remounts", () => {
    expect(codeLines('a.ts', 'const a = 1;')[0]!.id).not.toBe(codeLines('b.py', 'import os')[0]!.id);
    expect(codeLines('a.ts', 'const a = 1;')[0]!.id).toBe(codeLines('a.ts', 'const a = 1;')[0]!.id);
  });
  test('a streaming block resumes from its last unchanged line and agrees with a fresh run', () => {
    const lines = Array.from({ length: 40 }, (_, at) => `const v${at}: Map<string, number> = new Map(); // ${at}`);
    let grown = '';
    for (const line of lines) { grown += (grown ? '\n' : '') + line; shikiTokens(grown, 'ts'); }
    const resumed = shikiTokens(grown + '\n/* tail', 'ts')!;
    expect(paint(resumed)).toBe(paint(highlight(grown + '\n/* tail', 'typescript')));
  });
  test('text over the budget keeps the heuristic tokenizer', () => {
    expect(shikiTokens('x'.repeat(SHIKI_MAX_CHARS + 1), 'ts')).toBeNull();
    expect(highlight('const a = 1;\n'.repeat(4000), 'ts').length).toBeGreaterThan(1);
  });
});
