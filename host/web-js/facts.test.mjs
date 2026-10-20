import { expect, test } from 'bun:test';
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

// LLP 1139.003 D2: a plan that reads only `exactViewport()` shares the
// session's one page reporter, so the agent's `prefer gpu-tier` reaches its
// `gpuTier`; with no prefer it is the agent's launch value, `high`.
test('a viewport-only plan reads the tier the agent prefers', async () => {
  Object.assign(globalThis, {
    location: { search: '?agent' },
    document: { createElement: () => ({}), addEventListener() {}, documentElement: { style: { setProperty() {} } } },
    addEventListener() {}, matchMedia: () => ({ matches: false, addEventListener() {} }),
    innerWidth: 390, innerHeight: 844,
  });
  // As the build lays them out: the web host's navigation.js beside facts.js; the runtime stubbed to its data seam.
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-facts-'));
  try {
    copyFileSync(resolve(import.meta.dir, 'facts.js'), resolve(dir, 'facts.js'));
    copyFileSync(resolve(import.meta.dir, '../web/navigation.js'), resolve(dir, 'navigation.js'));
    writeFileSync(resolve(dir, 'rt.js'), 'export const data = {}, Resources = [], commits = []; export const commit = (f, why) => { commits.push(why); f(); }; export function R() {} export const Refusal = Error;');
    const { viewport } = await import(resolve(dir, 'facts.js'));
    const { data, commits } = await import(resolve(dir, 'rt.js'));
    viewport({ v: [['gpuTier']] });
    const read = () => data.reserved.exactViewport(null, null, 'v')[0];
    expect(read()).toBe('high');
    globalThis.exact.page.prefer({ 'gpu-tier': 'low' });
    expect(read()).toBe('low');
    expect(commits).toContain('viewport');
    expect(globalThis.exact.page.read()['gpu-tier']).toBe('low');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
