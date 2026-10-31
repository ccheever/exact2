import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// A data module's `scheduler` on the JS target (LLP 1141.001 D10), ts-fetch.js as it is: with no
// browser `scheduler.yield` (Bun has none, nor has Safari), a yield resolves in a task of its own, after
// the microtasks queued before it; `postTask` refuses, naming the yield; and an overlay may not yield.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('a yield without the browser scheduler resolves in a later task; postTask and an overlay refuse', async () => {
  expect(globalThis.scheduler).toBeUndefined();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-scheduler-'));
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js'));
  writeFileSync(resolve(dir, 'admission.js'), `export { ownWithout, redirectOf, signalDeadline } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)};
export class FetchError extends Error {} export const overlaying = { on: false }; export const inOverlay = what => new Error(what + ' in an overlay');
export const fetchWith = () => Promise.reject(new Error('no fetch'));\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  try {
    const { scheduler } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const { overlaying } = await import(pathToFileURL(resolve(dir, 'admission.js')).href);
    const order = [];
    const yielded = scheduler.yield().then(() => order.push('yield'));
    let chained = Promise.resolve();
    for (let i = 0; i < 64; i++) chained = chained.then(() => order.push(i));
    await yielded;
    expect(order.at(-1)).toBe('yield');
    expect(() => scheduler.postTask(() => {})).toThrow('await scheduler.yield()');
    overlaying.on = true;
    await expect(scheduler.yield()).rejects.toThrow('scheduler.yield() in an overlay');
    overlaying.on = false;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
