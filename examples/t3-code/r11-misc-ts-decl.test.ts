import { test, expect, describe } from 'bun:test';
import { highlight, type Token } from './timeline-highlight';
import { ifStatementBegins, inImportLine } from './r11-misc-ts-decl';

// Lane r11-misc: the shared JavaScript / TypeScript tokenizer against Shiki 4.2 (pierre-light) runs
// recorded from the HEAD source's own Shiki (f870c419fc node_modules), as `character+colour` for
// every non-space character, mapped from the app's classes by synColor (markdown.contract).
const COLOR: Record<string, string> = { kw: '#d32a61', decl: '#a631be', type: '#a631be', fn: '#693acf', var: '#d47628', const: '#d5901c', param: '#636363',
  str: '#199f43', esc: '#16a994', num: '#1ca1c7', op: '#08c0ef', punct: '#636363', com: '#737373', tag: '#d5512f', attr: '#18a46c', regex: '#17a5af', flag: '#d5a910', '': '#0a0a0a' };
const colours = (tokens: Token[]) => tokens.flatMap(token => [...token.text].filter(ch => !/\s/.test(ch)).map(ch => `${ch}${COLOR[token.cls] ?? token.cls}`)).join(' ');
const SHIKI: [string, string, string, string][] = [
  ["declaration lists: the second const name is a constant, let names variables", "typescript", "const a = 1, m = 2;\nlet b = 1, n = 2;", "c#a631be o#a631be n#a631be s#a631be t#a631be a#d5901c =#08c0ef 1#1ca1c7 ,#636363 m#d5901c =#08c0ef 2#1ca1c7 ;#636363 l#a631be e#a631be t#a631be b#d47628 =#08c0ef 1#1ca1c7 ,#636363 n#d47628 =#08c0ef 2#1ca1c7 ;#636363"],
  ["a list's declarators after calls, arrays and an arrow function", "typescript", "const c = f(1, 2), k = [3, 4], g = () => 1;", "c#a631be o#a631be n#a631be s#a631be t#a631be c#d5901c =#08c0ef f#693acf (#636363 1#1ca1c7 ,#636363 2#1ca1c7 )#636363 ,#636363 k#d5901c =#08c0ef [#636363 3#1ca1c7 ,#636363 4#1ca1c7 ]#636363 ,#636363 g#693acf =#08c0ef (#636363 )#636363 =#a631be >#a631be 1#1ca1c7 ;#636363"],
  ["annotated, object-valued and for-of const names", "typescript", "const d: number = 1, e = {x: 1}, h = 3\nfor (const x of xs) {}", "c#a631be o#a631be n#a631be s#a631be t#a631be d#d5901c :#636363 n#a631be u#a631be m#a631be b#a631be e#a631be r#a631be =#08c0ef 1#1ca1c7 ,#636363 e#d5901c =#08c0ef {#636363 x#d47628 :#636363 1#1ca1c7 }#636363 ,#636363 h#d5901c =#08c0ef 3#1ca1c7 f#d32a61 o#d32a61 r#d32a61 (#636363 c#a631be o#a631be n#a631be s#a631be t#a631be x#d5901c o#d32a61 f#d32a61 x#d47628 s#d47628 )#636363 {#636363 }#636363"],
  ["a list continued after a trailing comma; destructured bindings", "typescript", "const p = 1,\n  q = 2;\nconst {r, s: t} = o, [u] = v;", "c#a631be o#a631be n#a631be s#a631be t#a631be p#d5901c =#08c0ef 1#1ca1c7 ,#636363 q#d5901c =#08c0ef 2#1ca1c7 ;#636363 c#a631be o#a631be n#a631be s#a631be t#a631be {#636363 r#d5901c ,#636363 s#d47628 :#636363 t#d5901c }#636363 =#08c0ef o#d47628 ,#636363 [#636363 u#d5901c ]#636363 =#08c0ef v#d47628 ;#636363"],
  ["object keys and case labels are not type annotations", "typescript", "const o = {a: b, c: d};\nswitch (x) { case y: z(); }", "c#a631be o#a631be n#a631be s#a631be t#a631be o#d5901c =#08c0ef {#636363 a#d47628 :#636363 b#d47628 ,#636363 c#d47628 :#636363 d#d47628 }#636363 ;#636363 s#d32a61 w#d32a61 i#d32a61 t#d32a61 c#d32a61 h#d32a61 (#636363 x#d47628 )#636363 {#636363 c#d32a61 a#d32a61 s#d32a61 e#d32a61 y#d47628 :#636363 z#693acf (#636363 )#636363 ;#636363 }#636363"],
  ["import lines: * and type", "typescript", "import * as ns from \"y\";\nimport type {A} from \"b\";", "i#d32a61 m#d32a61 p#d32a61 o#d32a61 r#d32a61 t#d32a61 *#1ca1c7 a#d32a61 s#d32a61 n#d47628 s#d47628 f#d32a61 r#d32a61 o#d32a61 m#d32a61 \"#199f43 y#199f43 \"#199f43 ;#636363 i#d32a61 m#d32a61 p#d32a61 o#d32a61 r#d32a61 t#d32a61 t#d32a61 y#d32a61 p#d32a61 e#d32a61 {#636363 A#d47628 }#636363 f#d32a61 r#d32a61 o#d32a61 m#d32a61 \"#199f43 b#199f43 \"#199f43 ;#636363"],
  ["type literals keep typed keys; a member named if is a call", "typescript", "type T = {a: Foo};\nlet v: Extract<U, {k: Bar}> = w;\nx.if (a);", "t#a631be y#a631be p#a631be e#a631be T#a631be =#08c0ef {#636363 a#d47628 :#636363 F#a631be o#a631be o#a631be }#636363 ;#636363 l#a631be e#a631be t#a631be v#d47628 :#636363 E#a631be x#a631be t#a631be r#a631be a#a631be c#a631be t#a631be <#636363 U#a631be ,#636363 {#636363 k#d47628 :#636363 B#a631be a#a631be r#a631be }#636363 >#636363 =#08c0ef w#d47628 ;#636363 x#d47628 .#636363 i#693acf f#693acf (#636363 a#d47628 )#636363 ;#636363"],
  ["HTML: an if statement keeps the script open past its block (runaway script)", "html", "<script>if (a) {</script><p>x</p>\n<p class=\"z\">y</p>\n}</script>\n<p>q</p>", "<#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 i#d32a61 f#d32a61 (#636363 a#d47628 )#636363 {#636363 <#08c0ef /#08c0ef s#d47628 c#d47628 r#d47628 i#d47628 p#d47628 t#d47628 >#08c0ef <#636363 p#d5512f >#636363 x#636363 <#636363 /#636363 p#d5512f >#636363 <#636363 p#d5512f c#18a46c l#18a46c a#18a46c s#18a46c s#18a46c =#08c0ef \"#199f43 z#199f43 \"#199f43 >#636363 y#636363 <#636363 /#636363 p#d5512f >#636363 }#636363 <#08c0ef /#08c0ef s#d47628 c#d47628 r#d47628 i#d47628 p#d47628 t#d47628 >#08c0ef <#636363 p#d5512f >#636363 q#636363 <#636363 /#636363 p#d5512f >#636363"],
  ["HTML: an object literal's value position: < is an operator", "html", "<script>x = {a: </script><p>x</p>\n}</script><p>q</p>", "<#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 x#d47628 =#08c0ef {#636363 a#d47628 :#636363 <#08c0ef /#08c0ef s#d47628 c#d47628 r#d47628 i#d47628 p#d47628 t#d47628 >#08c0ef <#636363 p#d5512f >#636363 x#636363 <#636363 /#636363 p#d5512f >#636363 }#636363 <#636363 /#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 <#636363 p#d5512f >#636363 q#0a0a0a <#636363 /#636363 p#d5512f >#636363"],
  ["HTML: an import declaration runs to its line's end over the closing tag", "html", "<script>import x from \"y\"</script><p>x</p>\n<p>y</p>", "<#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 i#d32a61 m#d32a61 p#d32a61 o#d32a61 r#d32a61 t#d32a61 x#d47628 f#d32a61 r#d32a61 o#d32a61 m#d32a61 \"#199f43 y#199f43 \"#199f43 <#0a0a0a /#0a0a0a s#d47628 c#d47628 r#d47628 i#d47628 p#d47628 t#d47628 >#0a0a0a <#0a0a0a p#d47628 >#0a0a0a x#d47628 <#0a0a0a /#0a0a0a p#d47628 >#0a0a0a <#636363 p#d5512f >#636363 y#636363 <#636363 /#636363 p#d5512f >#636363"],
  ["HTML: a line comment inside a declaration runs over the closing tag", "html", "<script>var a = 1 // </script>\n<p>q</p>", "<#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 v#a631be a#a631be r#a631be a#d47628 =#08c0ef 1#1ca1c7 /#737373 /#737373 <#737373 /#737373 s#737373 c#737373 r#737373 i#737373 p#737373 t#737373 >#737373 <#636363 p#d5512f >#636363 q#636363 <#636363 /#636363 p#d5512f >#636363"],
  ["HTML: if (a){ with no space is no if-statement rule: the script closes", "html", "<script>if (a){\n}</script><p>q</p>", "<#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 i#d32a61 f#d32a61 (#636363 a#d47628 )#636363 {#636363 }#636363 <#636363 /#636363 s#d5512f c#d5512f r#d5512f i#d5512f p#d5512f t#d5512f >#636363 <#636363 p#d5512f >#636363 q#0a0a0a <#636363 /#636363 p#d5512f >#636363"],
];

describe('tsLike against Shiki (every surface that tokenizes JavaScript: chat code, Files, diffs, HTML scripts)', () => {
  for (const [name, lang, code, expected] of SHIKI) test(name, () => {
    const tokens = highlight(code, lang);
    expect(tokens.map(token => token.text).join('')).toBe(code);
    expect(colours(tokens)).toBe(expected);
  });
});

test('the if-statement rule and import lines', () => {
  expect(ifStatementBegins('if (a) {', 0)).toBe(true);
  expect(ifStatementBegins('if (a){', 0)).toBe(false);
  expect(ifStatementBegins('if (f(g(x))) y()', 0)).toBe(true);
  expect(ifStatementBegins('x.if (a) {', 2)).toBe(false);
  expect(inImportLine('import x from "y"</script><p>', 26)).toBe(true);
  expect(inImportLine('import x from "y"; <p>', 19)).toBe(false);
  expect(inImportLine('await import("y") <p>', 18)).toBe(false);
});
