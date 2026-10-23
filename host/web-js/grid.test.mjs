// Load grid.js with local runtime helpers so no module mock leaks into other tests.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';

const placements = [
  String.raw`\31 99999`, String.raw`span 2 \31 99999`, String.raw`\00003199999 / 2`,
  String.raw`foo\(bar`, String.raw`calc\(2\)`, String.raw`foo\/99999`,
  String.raw`foo\ 99999`, String.raw`foo\\99999`, '10000 / -10000', 'span 10000',
];
const largePlacements = ['10001', '-10001', '+10001', 'auto / span 10001', String.raw`\31 99999 / 10001`];
const functions = ['var(--foo)', 'var(--x-card-pad)', 'var(--exact-space-md)', 'var(--exact-radius-full)', 'env(foo)', String.raw`v\61 r(--foo)`];
const calculations = ['calc(2)', 'calc(1 + 1)', 'min(2, 3)', String.raw`c\61 lc(2)`];

test('grid values admit supported tokens and refuse unsupported forms', async () => {
  const said = (globalThis.__gridSaid = []);
  const text = readFileSync(new URL('./grid.js', import.meta.url), 'utf8')
    .replace(/^import \{ normal, say \} from "\.\/rt\.js";$/m, 'const normal = (_p, v) => v, say = line => globalThis.__gridSaid.push(line);');
  expect(text).not.toContain('from "./rt.js"');
  const { gridValue } = await import(`data:text/javascript;base64,${Buffer.from(text).toString('base64')}`);
  for (const v of ['var(--exact-space-md)', 'var(--exact-radius-full)', ' var(--exact-space-xl) ', 'var(--x-card-pad)', 'calc(50% - 10px)', 'repeat(auto-fill, minmax(min(100%, 200px), 1fr))', 'clamp(20px, 25%, 80px) max(10px, 20%)']) {
    expect(gridValue('tracks', v)).toBe(v);
  }
  for (const v of ['var(--exact-accent)', 'var(--exact-space-huge)', 'var(--exact-space-md) 1fr', 'fit-content(min(100%, 200px))', 'min(1vw, 20px)', 'calc(env(safe-area-inset-top) + 10px)']) {
    expect(gridValue('tracks', v)).toBe('unset');
  }
  expect(said.length).toBe(6);
  const accepted = placements.map(v => gridValue('placement', v));
  const refused = [
    ...largePlacements.map(v => ['placement', v]),
    ...calculations.map(v => ['placement', v]),
    ...['placement', 'flow', 'justify'].flatMap(kind => functions.map(v => [kind, v])),
  ];
  said.length = 0;
  const filtered = refused.map(([kind, v]) => gridValue(kind, v));
  expect({ accepted, filtered }).toEqual({ accepted: placements, filtered: refused.map(() => 'unset') });
  expect(said).toHaveLength(refused.length);
  for (const [i, [kind, v]] of refused.entries()) {
    const prop = kind === 'placement' ? 'grid-column' : kind === 'flow' ? 'grid-auto-flow' : 'justify-items';
    expect(said[i]).toStartWith(`unset ${prop}: ${JSON.stringify(v)} (`);
  }
});

// Browser serialization can fold an unsupported function or unit into calc(px).
test('the grid filter checks browser tokens and unsupported calculations', async () => {
  const { suiteBrowser } = await import('../../scripts/agent-launch.mjs');
  const suite = await suiteBrowser(), page = await suite.newPage();
  try {
    const source = readFileSync(new URL('./grid.js', import.meta.url), 'utf8')
      .replace(/^import \{ normal, say \} from "\.\/rt\.js";$/m, `const said = [], say = line => said.push(line), normal = (p, v) => { const s = document.createElement('i').style; s.setProperty(p, v); return s.getPropertyValue(p); };`)
      .replace('export function gridValue', 'function gridValue');
    const result = await page.evaluate(source + `; [
      'min(1cm, 20px /* [ */)', 'calc(sin(1) * 20px)', 'fit-content(calc(10px + 20px))',
      'calc(1e-1px + 50%)', 'MIN(100%, 200PX)', 'min(100%, 200px /* card */)',
      String.raw\`m\\69 n(100%, 200px)\`, String.raw\`min(1c\\6d , 20px)\`
    ].map(v => gridValue('tracks', v));`);
    expect(result.slice(0, 3)).toEqual(['unset', 'unset', 'unset']);
    expect(result.slice(3, 7)).not.toContain('unset');
    expect(result[7]).toBe('unset');
    const scientific = await page.evaluate(source + `; [
      '1e-1%', '1e+1%', '1E-1%', '.1e-1%', '1e2%',
      'calc(1e-1 * 20px)', 'calc(20px / 1e+1)', 'calc(1e-1 * 20%)',
      'calc(1e-1 * 1e+2px)', '1e-1fr', '1e+1px'
    ].map(v => [normal('grid-template-columns', v), gridValue('tracks', v)]);`);
    for (const [normalized, filtered] of scientific) {
      expect(normalized).not.toBe('');
      expect(filtered).toBe(normalized);
    }
    const unsupported = await page.evaluate(source + `; [
      '1e-1em', 'min(1e-1cm, 20px)', 'calc(1e+1 * 2vw)'
    ].map(v => gridValue('tracks', v));`);
    expect(unsupported).toEqual(['unset', 'unset', 'unset']);
    const cases = [
      ...['placement', 'flow', 'justify'].flatMap(kind => functions.map(v => [kind, v, false])),
      ...['placement', 'flow', 'justify'].flatMap(kind => calculations.map(v => [kind, v, false])),
      ...largePlacements.map(v => ['placement', v, false]),
      ...placements.map(v => ['placement', v, true]),
      ['flow', String.raw`r\6f w dense`, true], ['justify', String.raw`safe c\65 nter`, true],
    ];
    const checked = await page.evaluate(source + `; ${JSON.stringify(cases)}.map(([kind, value]) => {
      const prop = kind === 'placement' ? 'grid-column' : kind === 'flow' ? 'grid-auto-flow' : 'justify-items';
      const normalized = normal(prop, value), before = said.length, filtered = gridValue(kind, value);
      const style = document.createElement('i').style;
      style.setProperty(prop, kind === 'placement' ? '2' : kind === 'flow' ? 'column' : 'center');
      style.setProperty(prop, filtered);
      return { normalized, filtered, applied: style.getPropertyValue(prop), messages: said.slice(before) };
    });`);
    expect(checked.map(({ filtered }, i) => [cases[i][0], cases[i][1], filtered])).toEqual(
      cases.map(([kind, v, accepts], i) => [kind, v, accepts ? checked[i].normalized : 'unset'])
    );
    for (const [i, { normalized, filtered, applied, messages }] of checked.entries()) {
      const [kind, v, accepts] = cases[i];
      if (kind !== 'placement' && calculations.includes(v)) {
        expect(normalized).toBe('');
        expect(messages[0]).toContain('the browser rejected it');
      } else expect(normalized).not.toBe('');
      expect(applied).toBe(filtered);
      expect(messages).toHaveLength(accepts ? 0 : 1);
      if (!accepts) expect(messages[0]).toContain(JSON.stringify(v));
    }
  } finally { await page.close(); await suite.close(); }
});
