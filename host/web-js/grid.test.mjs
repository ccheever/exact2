// Load grid.js with local runtime helpers so no module mock leaks into other tests.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';

test('grid tracks admit calculations and checked tokens, and refuse unsupported forms', async () => {
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
});

// Browser serialization can fold an unsupported function or unit into calc(px).
test('the grid filter checks calculation units and functions before browser folding', async () => {
  const { suiteBrowser } = await import('../../scripts/agent-launch.mjs');
  const suite = await suiteBrowser(), page = await suite.newPage();
  try {
    const source = readFileSync(new URL('./grid.js', import.meta.url), 'utf8')
      .replace(/^import \{ normal, say \} from "\.\/rt\.js";$/m, `const say = () => {}, normal = (p, v) => { const s = document.createElement('i').style; s.setProperty(p, v); return s.getPropertyValue(p); };`)
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
  } finally { await page.close(); await suite.close(); }
});
