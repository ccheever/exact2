import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// The JS target's count of held fetches (LLP 1016.000 D9), ts-fetch.js and held.js as they are, over a stand-in
// `fetchWith` whose replies the test lets through: an answer counts for no clock only while every piece of work it
// has out is a held fetch, and counts again for what follows the held reply.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('an answer is held only while its held fetch is all it awaits, and counts again for the work after it', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-held-count-'));
  for (const name of ['ts-fetch.js', 'held.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `export { ownWithout, redirectOf, signalDeadline } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)};
export class FetchError extends Error {} export const overlaying = { on: false }; export const inOverlay = what => new Error(what);
export const fetchWith = (grants, input) => globalThis.heldGates(String(input));\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  const gates = new Map();
  globalThis.heldGates = input => new Promise(ok => gates.set(input, () => ok(new Response('x'))));
  try {
    const tsf = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const held = await import(pathToFileURL(resolve(dir, 'held.js')).href);
    Object.assign(tsf.answering, { loose: held.looseFetch, heldReply: held.heldReply });
    const inflight = { n: 0 }, owner = {}, call = {}, t = { id: 1, name: 'item', owner, call, waiting: true };
    owner.ticket = t;
    held.counted(t, inflight);
    expect(inflight.n).toBe(1);
    // In the answer's turn: a held fetch and an ordinary piece of work beside it.
    tsf.answering.call = call;
    const poll = tsf.fetch('https://x.test/changes', { exactHeld: true });
    let finish;
    const work = tsf.ordinary(() => new Promise(ok => { finish = ok; }));
    const digest = tsf.crypto.subtle.digest('SHA-256', new Uint8Array(1));
    tsf.answering.call = null;
    expect(inflight.n).toBe(1);
    expect(held.heldAsks()).toEqual([]);
    await digest;
    finish();
    await work;
    // Only the held fetch out: no clock waits on the answer, and it is named.
    expect(inflight.n).toBe(0);
    expect(held.heldAsks()).toEqual([{ name: 'item', ticket: 1 }]);
    // A fetch tied to no answer (a plain `.then`): held candidates count again while it is out.
    const loose = tsf.fetch('https://x.test/other');
    expect(inflight.n).toBe(1);
    gates.get('https://x.test/other')();
    await loose;
    expect(inflight.n).toBe(0);
    // The held reply lands: one reply counted, and the answer's work after it counts again.
    gates.get('https://x.test/changes')();
    await poll;
    expect(held.heldReplies()).toEqual({ item: 1 });
    expect(inflight.n).toBe(1);
    expect(held.heldAsks()).toEqual([]);
    // Superseded while held: unnamed, and its late reply is no reply.
    tsf.answering.call = call;
    const again = tsf.fetch('https://x.test/changes2', { exactHeld: true });
    tsf.answering.call = null;
    owner.ticket = { id: 2 };
    expect(held.heldAsks()).toEqual([]);
    gates.get('https://x.test/changes2')();
    await again;
    expect(held.heldReplies()).toEqual({ item: 1 });
  } finally {
    delete globalThis.heldGates;
    rmSync(dir, { recursive: true, force: true });
  }
});
