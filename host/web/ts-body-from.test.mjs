import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// The file-body module (ts-body-from.js) loads on an exactBodyFrom fetch's first
// use: the request is the call's, and its deadline and abort hold while it loads.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const realHelpers = `export { ownWithout, redirectOf, within } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)}; `;

test('an exactBodyFrom fetch captures its options before loading the file-body module', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-options-'));
  for (const name of ['ts-fetch.js', 'ts-body-from.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async (set, input, init) => { globalThis.uploads.push({ method: init.method, path: await init.body.text(), timeout: init.exactTimeout }); return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n'); writeFileSync(resolve(dir, 'storage-fs.js'), 'export const requestBody = async (app, grants, path) => new Blob([path]);\n');
  globalThis.uploads = [];
  try {
    const { fetch: appFetch } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const options = Object.defineProperty({ method: 'POST', exactTimeout: 5000 }, 'exactBodyFrom', { value: 'app:/data/a', writable: true });
    const first = appFetch('https://x.test/up', options);
    options.method = 'PUT'; options.exactBodyFrom = 'app:/data/b'; options.exactTimeout = 1000;
    const second = appFetch('https://x.test/up', options);
    await Promise.all([first, second]);
    const uploads = Object.fromEntries(globalThis.uploads.map(({ method, path, timeout }) => [path, { method, timeout }]));
    expect(uploads).toMatchObject({ 'app:/data/a': { method: 'POST' }, 'app:/data/b': { method: 'PUT' } });
    expect(uploads['app:/data/a'].timeout).toBeGreaterThan(uploads['app:/data/b'].timeout);
  } finally { delete globalThis.uploads; rmSync(dir, { recursive: true, force: true }); }
});

test('an exactBodyFrom fetch times out and aborts while its module is loading', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-load-'));
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js'));
  writeFileSync(resolve(dir, 'ts-body-from.js'), 'await globalThis.bodyImportGate;\n' + readFileSync(resolve(ROOT, 'host/web-js/ts-body-from.js'), 'utf8'));
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async () => { globalThis.sentBodies++; return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  let release, timed, aborted;
  globalThis.bodyImportGate = new Promise(resolveGate => { release = resolveGate; }); globalThis.sentBodies = 0;
  try {
    const { fetch: appFetch } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    timed = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/a', exactTimeout: 10 }).catch(e => e);
    const controller = new AbortController(); aborted = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/b', signal: controller.signal }).catch(e => e);
    controller.abort(new Error('stopped'));
    const guard = promise => Promise.race([promise, new Promise(resolveGuard => setTimeout(() => resolveGuard(new Error('still pending')), 100))]);
    const [timeout, abort] = await Promise.all([guard(timed), guard(aborted)]);
    expect([timeout.kind, timeout.message, abort.message, globalThis.sentBodies]).toEqual(['Timeout', 'the request timed out after 10 ms', 'stopped', 0]);
  } finally {
    release(); await Promise.allSettled([timed, aborted].filter(Boolean));
    delete globalThis.bodyImportGate; delete globalThis.sentBodies; rmSync(dir, { recursive: true, force: true });
  }
});
