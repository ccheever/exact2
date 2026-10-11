// An overlay on the JS target refuses the same calls as the prelude's executors (js/tests/it/overlay.rs), by
// name: a promise-returning call rejects, any other throws. The page's own `crypto`, the store facade and `fetch`
// are the ones a data module is given (ts-fetch.js, admission.js).
import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

// The real modules, beside the grant data a build writes (host/web-js/build.mjs).
const dir = mkdtempSync(resolve(tmpdir(), 'exact-overlay-guards-'));
const here = new URL('../..', import.meta.url).pathname;
// Their scripts only: what they import, without builds, tests or Rust.
const scripts = path => !/\/(dist|tests|src|target|node_modules)(\/|$)/.test(path);
for (const sub of ['web-js', 'web']) cpSync(resolve(here, sub), resolve(dir, sub), { recursive: true, filter: scripts });
writeFileSync(resolve(dir, 'web-js/admission-data.js'), "import {createGrantSet} from './admission.js'; export const tsGrantSet = createGrantSet(''), rustGrantSet = tsGrantSet;\n");
const { createSecretFacade, overlaying } = await import(resolve(dir, 'web-js/admission.js'));
const { crypto, fetch } = await import(resolve(dir, 'web-js/ts-fetch.js'));

// Each call, as its error names it, and how it refuses: the prelude's table (js/tests/it/overlay.rs) but storage,
// which js-runtime.test.mjs tries.
const REFUSED = [
  ['crypto.getRandomValues()', 'throws', crypto => crypto.getRandomValues(new Uint8Array(1))],
  ['crypto.randomUUID()', 'throws', crypto => crypto.randomUUID()],
  ...['digest', 'generateKey', 'sign', 'verify', 'importKey', 'exportKey', 'encrypt', 'decrypt', 'deriveBits']
    .map(m => [`crypto.subtle.${m}()`, 'rejects', crypto => crypto.subtle[m]('SHA-256', new Uint8Array(0))]),
  ['fetch()', 'rejects', (_, fetch) => fetch('https://fixture.exact.test/value')],
  ['store.get()', 'throws', (_, __, store) => store.get('x')],
  ['store.set()', 'throws', (_, __, store) => store.set('x', 'y')],
  ['store.forget()', 'throws', (_, __, store) => store.forget('x')],
  ['store.keepKey()', 'rejects', (_, __, store) => store.keepKey('x', null)],
  ['store.key()', 'rejects', (_, __, store) => store.key('x')],
];

const outcome = async call => {
  let value;
  try { value = call(); } catch (e) { return `throws ${e.message}`; }
  if (!value || typeof value.then !== 'function') return 'runs';
  try { await value; return 'resolves'; } catch (e) { return `rejects ${e.message}`; }
};

test('every effect in an overlay refuses by name, as the prelude refuses it', async () => {
  const store = createSecretFacade({ get: () => null, set: () => {} }, 'secret.keep x', () => Promise.resolve(null));
  overlaying.on = true;
  try {
    for (const [name, how, call] of REFUSED) {
      expect(await outcome(() => call(crypto, fetch, store))).toStartWith(`${how} ${name} is unavailable in an overlay`);
    }
  } finally { overlaying.on = false; }
});
