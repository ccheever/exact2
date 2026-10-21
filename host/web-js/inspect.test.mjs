// Inspect mode's runtime hooks on the JS target (LLP 1122 T2b) are a
// development build's alone: `bun test host/web-js/inspect.test.mjs`.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { withInspectHooks } from './module.mjs';

// rt.js itself has no inspect hook, so every build but `--dev` copies it as
// it is; a `--dev` build's copy (build.mjs, module.mjs `withInspectHooks`)
// gains the two, and fails loudly when rt.js loses the lines they hang on.
test('rt.js has no inspect hook; a development copy gains both, and the rest of it is as it was', () => {
  const source = readFileSync(new URL('./rt.js', import.meta.url), 'utf8');
  for (const hook of ['$exactViewId', '$exactEaches']) expect(source).not.toContain(hook);
  const dev = withInspectHooks(source);
  expect(dev).toContain('globalThis.$exactViewId = viewId;');
  expect(dev).toContain('(globalThis.$exactEaches ??= new Set()).add(e); onEnd(() => globalThis.$exactEaches.delete(e));');
  // Two lines added, nothing else changed.
  const lines = dev.split('\n');
  expect(lines.length).toBe(source.split('\n').length + 2);
  expect(lines.filter((l) => !l.includes('$exactViewId') && !l.includes('$exactEaches')).join('\n')).toBe(source);
  expect(() => withInspectHooks('export const nothing = 1;\n')).toThrow(/no line to hang on/);
});
