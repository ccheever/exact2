import { describe, expect, test } from 'bun:test';
import { highlight, type Cls } from './timeline-highlight';
import { scriptBodyEnd, styleBodyEnd } from './r9-device-html-end';

// Lane r9-device: where Shiki 4.2's html grammar ends <script> and <style> bodies. Each case is
// [source, [non-space text, pierre-light colour] runs] recorded from Shiki itself
// (target/t3-ref/src-f90b77d8 node_modules, the reference's highlighter and theme).
const LIGHT: Record<Cls, string> = { '': '#0a0a0a', kw: '#d32a61', decl: '#a631be', type: '#a631be', fn: '#693acf', var: '#d47628', const: '#d5901c',
  param: '#636363', pyparam: '#d5a910', str: '#199f43', esc: '#16a994', num: '#1ca1c7', nul: '#08c0ef', op: '#08c0ef', punct: '#636363', com: '#737373',
  key: '#d5512f', flag: '#d5a910', builtin: '#08c0ef', interp: '#d5a910', heading: '#d5512f', bold: '#d5a910', tag: '#d5512f', attr: '#18a46c' };
const painted = (runs: [string, string][]) => runs.flatMap(([text, color]) => [...text].filter(character => character.trim()).map(() => color));
const CASES: [string, string, [string, string][]][] = [
  ['uppercase </SCRIPT> after an unterminated let runs the script on (JSX, operators)', "<style>\n* , ul li + p ~ span, &:hover { color: red; border: 1px solid transparent; font: bold 12px/1.5 Helvetica, \"Inter\", foo; display: -webkit-box; color: currentColor; margin: -4px auto; z-index: 10; foo-bar: baz qux; grid-area: 1 / 2; content: \"a\\\"b\"; opacity: 50%; }\ndiv{color:blue}\n@import url(foo.css);\n@font-face { font-family: X; src: local(Y); }\n@keyframes spin { from { opacity: 0 } 50% { opacity: 1 } }\n</style>\n<script src=\"a.js\"></script>\n<SCRIPT>let A_B = 1</SCRIPT>\n<!--\nmulti\nline -->\n<textarea>raw <b> text</textarea>", [["<","#636363"],["style","#d5512f"],[">","#636363"],["*","#d5512f"],[",","#636363"],["ulli","#d5512f"],["+","#636363"],["p","#d5512f"],["~","#636363"],["span","#d5512f"],[",","#636363"],["&","#d32a61"],[":","#636363"],["hover","#08c0ef"],["{","#636363"],["color","#08c0ef"],[":","#636363"],["red","#d5a910"],[";","#636363"],["border","#08c0ef"],[":","#636363"],["1","#1ca1c7"],["px","#d5512f"],["solidtransparent","#d5a910"],[";","#636363"],["font","#08c0ef"],[":","#636363"],["bold","#d5a910"],["12","#1ca1c7"],["px","#d5512f"],["/","#0a0a0a"],["1.5","#1ca1c7"],["Helvetica","#d5a910"],[",","#636363"],["\"Inter\"","#199f43"],[",","#636363"],["foo","#0a0a0a"],[";","#636363"],["display","#08c0ef"],[":","#636363"],["-webkit-box","#0a0a0a"],[";","#636363"],["color","#08c0ef"],[":","#636363"],["currentColor","#0a0a0a"],[";","#636363"],["margin","#08c0ef"],[":","#636363"],["-4","#1ca1c7"],["px","#d5512f"],["auto","#d5a910"],[";","#636363"],["z-index","#08c0ef"],[":","#636363"],["10","#1ca1c7"],[";","#636363"],["foo-bar","#0a0a0a"],[":","#636363"],["bazqux","#0a0a0a"],[";","#636363"],["grid-area","#08c0ef"],[":","#636363"],["1","#1ca1c7"],["/","#0a0a0a"],["2","#1ca1c7"],[";","#636363"],["content","#08c0ef"],[":","#636363"],["\"a","#199f43"],["\\\"","#16a994"],["b\"","#199f43"],[";","#636363"],["opacity","#08c0ef"],[":","#636363"],["50","#1ca1c7"],["%","#d5512f"],[";}","#636363"],["div","#d5512f"],["{","#636363"],["color","#08c0ef"],[":","#636363"],["blue","#d5a910"],["}@","#636363"],["import","#d32a61"],["url","#693acf"],["(foo.css);@","#636363"],["font-face","#d32a61"],["{","#636363"],["font-family","#08c0ef"],[":","#636363"],["X","#d5a910"],[";","#636363"],["src","#08c0ef"],[":","#636363"],["local","#693acf"],["(","#636363"],["Y","#d5a910"],[");}@","#636363"],["keyframes","#d32a61"],["spin{","#636363"],["from","#0a0a0a"],["{","#636363"],["opacity","#08c0ef"],[":","#636363"],["0","#1ca1c7"],["}","#636363"],["50%","#0a0a0a"],["{","#636363"],["opacity","#08c0ef"],[":","#636363"],["1","#1ca1c7"],["}}</","#636363"],["style","#d5512f"],["><","#636363"],["script","#d5512f"],["src","#18a46c"],["=","#636363"],["\"a.js\"","#199f43"],["></","#636363"],["script","#d5512f"],["><","#636363"],["SCRIPT","#d5512f"],[">","#636363"],["let","#a631be"],["A_B","#d5901c"],["=","#08c0ef"],["1","#1ca1c7"],["</","#08c0ef"],["SCRIPT","#d5901c"],["><!--","#08c0ef"],["multiline","#d47628"],["-->","#08c0ef"],["<","#636363"],["textarea","#d5512f"],[">raw<","#636363"],["b","#d5512f"],[">text</","#636363"],["textarea","#d5512f"],[">","#636363"]]],
  ['a closing tag inside a var declaration stays JavaScript', "<script>var a = 1</script><p>x</p>\n<p>y</p>", [["<","#636363"],["script","#d5512f"],[">","#636363"],["var","#a631be"],["a","#d47628"],["=","#08c0ef"],["1","#1ca1c7"],["</","#08c0ef"],["script","#d47628"],[">","#08c0ef"],["<","#636363"],["p","#d5512f"],[">x</","#636363"],["p","#d5512f"],["><","#636363"],["p","#d5512f"],[">y</","#636363"],["p","#d5512f"],[">","#636363"]]],
  ['a line comment hides only a lower-case </script>', "<script>// c </SCRIPT>\nx</script><p>x</p>", [["<","#636363"],["script","#d5512f"],[">","#636363"],["//c</SCRIPT>","#737373"],["x","#d47628"],["</","#636363"],["script","#d5512f"],["><","#636363"],["p","#d5512f"],[">","#636363"],["x","#0a0a0a"],["</","#636363"],["p","#d5512f"],[">","#636363"]]],
  ['</script > leaves the rest plain until /script>', "<script></script ><p>x</p>\n<p>z</p></script>\n<b>w</b>", [["<","#636363"],["script","#d5512f"],["><","#636363"],["/script><p>x</p><p>z</p><","#0a0a0a"],["/","#636363"],["script","#d5512f"],["><","#636363"],["b","#d5512f"],[">","#636363"],["w","#0a0a0a"],["</","#636363"],["b","#d5512f"],[">","#636363"]]],
  ['</scriptx> ends the script body, then plain text until /script>', "<script>a</scriptx><p>x</p>\nfoo/script><i>t</i>", [["<","#636363"],["script","#d5512f"],[">","#636363"],["a","#d47628"],["<","#636363"],["/scriptx><p>x</p>foo","#0a0a0a"],["/","#636363"],["script","#d5512f"],["><","#636363"],["i","#d5512f"],[">","#636363"],["t","#0a0a0a"],["</","#636363"],["i","#d5512f"],[">","#636363"]]],
  ['a closing tag inside a string is string text', "<script>var s = \"</script>\";</script><p>x</p>", [["<","#636363"],["script","#d5512f"],[">","#636363"],["var","#a631be"],["s","#d47628"],["=","#08c0ef"],["\"</script>\"","#199f43"],[";</","#636363"],["script","#d5512f"],["><","#636363"],["p","#d5512f"],[">","#636363"],["x","#0a0a0a"],["</","#636363"],["p","#d5512f"],[">","#636363"]]],
  ['an upper-case </STYLE > with a space closes the style sheet', "<style>a{color:red}</STYLE ><p>x</p>", [["<","#636363"],["style","#d5512f"],[">","#636363"],["a","#d5512f"],["{","#636363"],["color","#08c0ef"],[":","#636363"],["red","#d5a910"],["}</","#636363"],["STYLE","#d5512f"],["><","#636363"],["p","#d5512f"],[">","#636363"],["x","#0a0a0a"],["</","#636363"],["p","#d5512f"],[">","#636363"]]],
];

describe('HTML script and style bodies end where Shiki ends them', () => {
  for (const [name, source, runs] of CASES) {
    test(name, () => {
      expect(painted(highlight(source, 'page.html').map(token => [token.text, LIGHT[token.cls]]))).toEqual(painted(runs));
    });
  }
  test('the body scanners skip strings, comments, templates, brackets and open declarations', () => {
    const at = (text: string) => scriptBodyEnd(text, 8);
    expect(at('<script>a();</SCRIPT>')).toBe(12);
    expect(at('<script>"</script>";</script>')).toBe(20);
    expect(at('<script>`${"</script>"}`</script>')).toBe(24);
    expect(at('<script>f(</script>)</script>')).toBe(20);
    expect(at('<script>let a = 1</script>\n</script>')).toBe(27);
    expect(at('<script>a ? </script> : b</script>')).toBe(25);
    expect(at('<script>// x </SCRIPT>\n</script>')).toBe(23);
    expect(at('<script>/* </script> */</script>')).toBe(11);
    expect(at('<script>never closed')).toBe(20);
    expect(styleBodyEnd('<style>a{b:"</style>"}</STYLE >', 7)).toBe(22);
    expect(styleBodyEnd('<style>a{b:c</style>}</style>', 7)).toBe(21);
  });
});
