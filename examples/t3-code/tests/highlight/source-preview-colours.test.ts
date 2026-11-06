import { afterEach, describe, expect, test } from 'bun:test';
import { codeLines } from '../../src/panel/files-surface';
import { resetHighlights } from '../../src/highlight/highlights';
import { highlighted, settleHighlights } from '../support/highlights-fixture';

// The media fixture's page.html, which the reference's ReadOnlySourcePreview colours with @pierre/highlights' html lexer.
const PAGE = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Fixture page</title></head>\n<body>\n<h1>Fixture page</h1>\n<p>A sandboxed HTML attachment rendered by its own document.</p>\n<p id=\"script\">Script has not run.</p>\n<button onclick=\"document.getElementById('count').textContent=String(Number(document.getElementById('count').textContent)+1)\">Count</button>\n<span id=\"count\">0</span>\n<script>document.getElementById('script').textContent='Script ran in the frame.';</script>\n</body></html>";
afterEach(() => resetHighlights());

describe('HTML source in the reference\'s lexers', () => {
  test('the attachment source lines keep their text and split tokens at line ends', () => {
    codeLines('page.html', PAGE);
    settleHighlights();
    const lines = codeLines('page.html', PAGE);
    expect(lines.map(line => line.runs.map(run => run.text).join(''))).toEqual(PAGE.split('\n'));
    expect(lines[0]!.runs.map(run => [run.text, run.syntax])).toEqual([['<!doctype html>', 'tag']]);
    expect(lines.slice(1).some(line => line.runs.some(run => run.syntax !== ''))).toBe(true);
  });
  test('script bodies are JavaScript: classes, constructors, parameters and template literals', () => {
    const tokens = highlighted('class A extends B { constructor(x) { super(x); } }\nconst f = (a, b) => `${a}!`;', 'ts');
    const classOf = (text: string) => tokens.find(token => token.text.trim() === text)?.cls;
    expect([classOf('B'), classOf('constructor'), classOf('x'), classOf('super'), classOf('`'), classOf('${')])
      .toEqual(['type', 'fn', 'param', 'label', 'str', 'kw']);
  });
});

describe('Files breadcrumbs scrolled to their end', () => {
  test('a fitting trail has no fade or shift; an overflowing one fades its left edge over min(24, overflow)', async () => {
    const { crumbsMask, crumbsShift } = await import('../../src/panel/files-crumbs-scroll');
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
    const { searchSettings } = await import('../../src/settings/settings-search');
    const titles = (query: string) => searchSettings(query, { connected: true, autoSettle: true, scopeKind: 'environment' }).map((hit: { title: string }) => hit.title);
    expect(titles('publish agent activity')).not.toContain('Publish agent activity');
    expect(titles('T3 Connect')).not.toContain('T3 Connect');
  });
});
