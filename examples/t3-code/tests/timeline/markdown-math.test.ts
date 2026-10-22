import { describe, expect, test } from 'bun:test';
import { mathFrames, noteMathFrame } from '../../src/timeline/markdown-math';
import { appearanceSections } from '../../src/appearance/settings-appearance';
import { decodeClientPrefs, DEVICE_DEFAULTS, restoreLabels } from '../../src/settings/settings-core';
import { searchSettings } from '../../src/settings/settings-search';

// Settings › Appearance › Render math (AppearanceSettingsPanel.tsx at 74541bab17) and the math pages' reports
// (MathTypeset's cache): display formulas are ```math fences (data/src/markdown_math.rs, its own tests).
describe('Render math', () => {
  test('the Appearance row is off by default, after Chat width, and resets to off', () => {
    const rows = (math: boolean) => appearanceSections(decodeClientPrefs({ mathRenderingEnabled: math }), false).flatMap(section => section.rows);
    const titles = rows(false).map(row => row.title);
    expect(titles.indexOf('Render math')).toBe(titles.indexOf('Chat width') + 1);
    expect(rows(false).find(row => row.id === 'math-rendering')).toMatchObject({ kind: 'switch', checked: false, resettable: false, target: 'mathRenderingEnabled',
      description: 'Typeset LaTeX math, such as $x^2$ or \\[x^2\\], in messages and Markdown previews. The composer keeps TeX source editable.' });
    expect(rows(true).find(row => row.id === 'math-rendering')).toMatchObject({ checked: true, resettable: true, resetLabel: 'math rendering' });
    expect(decodeClientPrefs({}).mathRenderingEnabled).toBe(false);
    expect(restoreLabels({ deviceSettings: { ...DEVICE_DEFAULTS }, clientSettings: decodeClientPrefs({ mathRenderingEnabled: true }), groupingMode: 'repository' }, null)).toContain('Render math');
  });
  test('search finds it by its terms', () => {
    const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [] };
    expect(searchSettings('katex', context).map(item => item.title)).toEqual(['Render math']);
  });
  test('a math page\'s report lands per theme, size and TeX; one KaTeX cannot parse keeps its source', () => {
    const tex = 'E = mc^2';
    noteMathFrame(JSON.stringify({ status: 'rendered', width: 80.5, height: 24, theme: 'dark', size: 14, tex }));
    noteMathFrame(JSON.stringify({ status: 'error', theme: 'light', size: 14, tex: '\\frac{' }));
    expect(mathFrames().filter(frame => frame.tex === tex || frame.tex === '\\frac{')).toEqual([
      { theme: 'dark', size: 14, tex, status: 'rendered', width: 80.5, height: 24 },
      { theme: 'light', size: 14, tex: '\\frac{', status: 'error', width: 0, height: 0 },
    ]);
    // TeX keeps its backslashes through the report.
    const sum = '\\sum_{k=1}^{n} k = \\frac{n(n+1)}{2}';
    noteMathFrame(JSON.stringify({ status: 'rendered', width: 130, height: 56, theme: 'light', size: 14, tex: sum }));
    expect(mathFrames().find(frame => frame.tex === sum)).toMatchObject({ status: 'rendered', width: 130 });
    // A report that is not JSON, names no TeX or size, or a rendered one with no size changes nothing.
    const before = mathFrames().length;
    for (const junk of ['nope', JSON.stringify({ status: 'rendered', width: 1, height: 1, theme: 'dark', size: 14, tex: '' }),
      JSON.stringify({ status: 'rendered', width: 1, height: 1, theme: 'dark', size: 0, tex }), JSON.stringify({ status: 'rendered', width: 0, height: 5, theme: 'dark', size: 14, tex: 'x' })]) noteMathFrame(junk);
    expect(mathFrames().length).toBe(before);
  });
});
