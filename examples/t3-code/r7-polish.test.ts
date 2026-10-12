import { describe, expect, test } from 'bun:test';
import { highlight, type Cls } from './timeline-highlight';
import { codeLines } from './r4-surfaces-files';

// Lane r7-polish. Pierre-light colours Shiki 4.2 painted (the reference's ReadOnlySourcePreview theme)
// for the media fixture's page.html and a style/script snippet: [non-space text, colour] runs.
const LIGHT: Record<Cls, string> = { '': '#0a0a0a', kw: '#d32a61', decl: '#a631be', type: '#a631be', fn: '#693acf', var: '#d47628', const: '#d5901c',
  param: '#636363', pyparam: '#d5a910', str: '#199f43', esc: '#16a994', num: '#1ca1c7', nul: '#08c0ef', op: '#08c0ef', punct: '#636363', com: '#737373',
  key: '#d5512f', flag: '#d5a910', builtin: '#08c0ef', interp: '#d5a910', heading: '#d5512f', bold: '#d5a910', tag: '#d5512f', attr: '#18a46c' };
const painted = (runs: [string, string][]) => runs.flatMap(([text, color]) => [...text].filter(character => character.trim()).map(() => color));
const PAGE = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Fixture page</title></head>\n<body>\n<h1>Fixture page</h1>\n<p>A sandboxed HTML attachment rendered by its own document.</p>\n<p id=\"script\">Script has not run.</p>\n<button onclick=\"document.getElementById('count').textContent=String(Number(document.getElementById('count').textContent)+1)\">Count</button>\n<span id=\"count\">0</span>\n<script>document.getElementById('script').textContent='Script ran in the frame.';</script>\n</body></html>";
const PAGE_RUNS: [string, string][] = [["<!","#636363"],["doctype","#d5512f"],["html","#18a46c"],["><","#636363"],["html","#d5512f"],["><","#636363"],["head","#d5512f"],["><","#636363"],["meta","#d5512f"],["charset","#18a46c"],["=","#636363"],["\"utf-8\"","#199f43"],["><","#636363"],["title","#d5512f"],[">","#636363"],["Fixturepage","#0a0a0a"],["</","#636363"],["title","#d5512f"],["></","#636363"],["head","#d5512f"],["><","#636363"],["body","#d5512f"],["><","#636363"],["h1","#d5512f"],[">","#636363"],["Fixturepage","#0a0a0a"],["</","#636363"],["h1","#d5512f"],["><","#636363"],["p","#d5512f"],[">","#636363"],["AsandboxedHTMLattachmentrenderedbyitsowndocument.","#0a0a0a"],["</","#636363"],["p","#d5512f"],["><","#636363"],["p","#d5512f"],["id","#18a46c"],["=","#636363"],["\"script\"","#199f43"],[">","#636363"],["Scripthasnotrun.","#0a0a0a"],["</","#636363"],["p","#d5512f"],["><","#636363"],["button","#d5512f"],["onclick","#18a46c"],["=","#636363"],["\"","#199f43"],["document","#d47628"],[".","#636363"],["getElementById","#693acf"],["(","#636363"],["'count'","#199f43"],[").","#636363"],["textContent","#d47628"],["=","#08c0ef"],["String","#693acf"],["(","#636363"],["Number","#693acf"],["(","#636363"],["document","#d47628"],[".","#636363"],["getElementById","#693acf"],["(","#636363"],["'count'","#199f43"],[").","#636363"],["textContent","#d47628"],[")","#636363"],["+","#08c0ef"],["1","#1ca1c7"],[")","#636363"],["\"","#199f43"],[">","#636363"],["Count","#0a0a0a"],["</","#636363"],["button","#d5512f"],["><","#636363"],["span","#d5512f"],["id","#18a46c"],["=","#636363"],["\"count\"","#199f43"],[">","#636363"],["0","#0a0a0a"],["</","#636363"],["span","#d5512f"],["><","#636363"],["script","#d5512f"],[">","#636363"],["document","#d47628"],[".","#636363"],["getElementById","#693acf"],["(","#636363"],["'script'","#199f43"],[").","#636363"],["textContent","#d47628"],["=","#08c0ef"],["'Scriptranintheframe.'","#199f43"],[";</","#636363"],["script","#d5512f"],["></","#636363"],["body","#d5512f"],["></","#636363"],["html","#d5512f"],[">","#636363"]];
const SNIPPET = "<style>.card > h1:hover { padding: 4px !important; color: #333; font-family: system-ui, foo; }</style>\n<a href=# onclick=\"go(`n ${i}`)\" style=\"color: red\">A &amp; B</a>";
const SNIPPET_RUNS: [string, string][] = [["<","#636363"],["style","#d5512f"],[">.","#636363"],["card","#18a46c"],[">","#636363"],["h1","#d5512f"],[":","#636363"],["hover","#08c0ef"],["{","#636363"],["padding","#08c0ef"],[":","#636363"],["4","#1ca1c7"],["px","#d5512f"],["!important","#d32a61"],[";","#636363"],["color","#08c0ef"],[":","#636363"],["#333","#d5a910"],[";","#636363"],["font-family","#08c0ef"],[":","#636363"],["system-ui","#d5a910"],[",","#636363"],["foo","#0a0a0a"],[";}</","#636363"],["style","#d5512f"],["><","#636363"],["a","#d5512f"],["href","#18a46c"],["=","#636363"],["#","#199f43"],["onclick","#18a46c"],["=","#636363"],["\"","#199f43"],["go","#693acf"],["(`","#636363"],["n","#199f43"],["${","#d32a61"],["i","#d47628"],["}","#d32a61"],["`)","#636363"],["\"","#199f43"],["style","#18a46c"],["=","#636363"],["\"color:red\"","#199f43"],[">","#636363"],["A","#0a0a0a"],["&","#636363"],["amp","#d5512f"],[";","#636363"],["B","#0a0a0a"],["</","#636363"],["a","#d5512f"],[">","#636363"]];

describe('HTML source in the reference theme', () => {
  test('the media fixture page paints every character as Shiki does', () => {
    expect(painted(highlight(PAGE, 'page.html').map(token => [token.text, LIGHT[token.cls]]))).toEqual(painted(PAGE_RUNS));
  });
  test('style sheets, event handlers, style attributes and entities follow the embedded grammars', () => {
    expect(painted(highlight(SNIPPET, 'x.html').map(token => [token.text, LIGHT[token.cls]]))).toEqual(painted(SNIPPET_RUNS));
  });
  test('the attachment source lines keep their text and split tokens at line ends', () => {
    const lines = codeLines('page.html', PAGE);
    expect(lines.map(line => line.runs.map(run => run.text).join(''))).toEqual(PAGE.split('\n'));
    expect(lines[0]!.runs.map(run => [run.text, run.syntax])).toEqual([['<!', 'punct'], ['doctype', 'tag'], [' ', 'punct'], ['html', 'attr'], ['>', 'punct']]);
  });
  test('script bodies are JavaScript: constructors, inherited classes, arrow parameters and template literals', () => {
    const tokens = highlight('class A extends B { constructor(x) { super(x); } }\nconst f = (a, b) => `${a}!`;', 'ts');
    const classOf = (text: string) => tokens.find(token => token.text.trim() === text)?.cls;
    expect([classOf('B'), classOf('constructor'), classOf('x'), classOf('super'), classOf('f'), classOf('a'), classOf('`'), classOf('${')])
      .toEqual(['decl', 'decl', 'param', 'const', 'fn', 'param', 'punct', 'kw']); // lane r12-render: 'decl' paints as 'type' did
  });
});

describe('Files breadcrumbs scrolled to their end', () => {
  test('a fitting trail has no fade or shift; an overflowing one fades its left edge over min(24, overflow)', async () => {
    const { crumbsMask, crumbsShift } = await import('./r7-polish-crumbs');
    expect([crumbsShift({}), crumbsMask({})]).toEqual([0, 'none']);
    expect([crumbsShift({ anchors: { crumbs: [0, 120], 'crumbs-clip': [12, 170] } }), crumbsMask({ anchors: { crumbs: [0, 170], 'crumbs-clip': [12, 170] } })]).toEqual([0, 'none']);
    const over = { anchors: { crumbs: [-18, 188], 'crumbs-clip': [12, 170] } };
    expect([crumbsShift(over), crumbsMask(over)]).toEqual([-18, 'linear-gradient(to right, #00000000 0%, #000000 10.6%)']);
    const far = { anchors: { crumbs: [-80, 250], 'crumbs-clip': [12, 170] } };
    expect(crumbsMask(far)).toBe('linear-gradient(to right, #00000000 0%, #000000 14.1%)');
  });
});

describe('Connections without T3 Connect', () => {
  test('settings search hides the cloud-only rows (settingsSearch cloudOnly without a cloud public config)', async () => {
    const { searchSettings } = await import('./settings-search');
    const titles = (query: string) => searchSettings(query, { connected: true, autoSettle: true, scopeKind: 'environment' }).map((hit: { title: string }) => hit.title);
    expect(titles('publish agent activity')).not.toContain('Publish agent activity');
    expect(titles('T3 Connect')).not.toContain('T3 Connect');
  });
});
