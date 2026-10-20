// grid.js's track check (LLP 1121 stage 11 B2, round 3, Astra): a bound
// track naming a base name is its property, which the check admits as it
// admits a design token's; only the scale's nine, never any `--exact-` one.
// grid.js is loaded from its text with rt.js's two helpers stood in, so no
// module mock leaks into the other files this process runs.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';

test('a base name\'s property is a track; another --exact- property is refused', async () => {
  const said = (globalThis.__gridSaid = []);
  const text = readFileSync(new URL('./grid.js', import.meta.url), 'utf8')
    .replace(/^import \{ normal, say \} from "\.\/rt\.js";$/m, 'const normal = (_p, v) => v, say = line => globalThis.__gridSaid.push(line);');
  expect(text).not.toContain('from "./rt.js"');
  const { gridValue } = await import(`data:text/javascript;base64,${Buffer.from(text).toString('base64')}`);
  for (const v of ['var(--exact-space-md)', 'var(--exact-radius-full)', ' var(--exact-space-xl) ', 'var(--x-card-pad)']) {
    expect(gridValue('tracks', v)).toBe(v);
  }
  for (const v of ['var(--exact-accent)', 'var(--exact-space-huge)', 'var(--exact-space-md) 1fr']) {
    expect(gridValue('tracks', v)).toBe('unset');
  }
  expect(said.length).toBe(3);
});
