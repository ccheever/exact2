// js/src/binary.js's BufferSource reads in a realm whose engine has shared, resizable and detachable buffers, as a
// browser's does: the wasm target's module realms run the same prelude, and Hermes has none of these buffers.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createContext, runInContext } from 'node:vm';

const binary = readFileSync(new URL('../../js/src/binary.js', import.meta.url), 'utf8');

function realm() {
  const context = createContext({ Headers, URL, URLSearchParams, TextEncoder, TextDecoder });
  runInContext(binary, context);
  runInContext('globalThis.said = f => { try { return f(); } catch (e) { return `${e.name}: ${e.message}`; } };', context);
  return context;
}

test('a body on a detached buffer is no bytes, its view\'s range unread, and the init is still converted', async () => {
  const [order, status, text] = await runInContext(`(async () => {
    const { Response } = __exact_binary.responses(), order = [];
    const buffer = new ArrayBuffer(4), view = new DataView(buffer, 1, 2);
    buffer.transfer();
    const response = new Response(view, { get status() { order.push('status'); return 201; } });
    return [order, response.status, await response.text()];
  })()`, realm());
  expect([order, status, text]).toEqual([['status'], 201, '']);
});

test('a body on a SharedArrayBuffer is refused before the init is read', () => {
  const [refused, order] = runInContext(`(() => {
    const { Response } = __exact_binary.responses(), order = [];
    return [said(() => new Response(new Uint8Array(new SharedArrayBuffer(1)), { get status() { order.push('status'); return 200; } })), order];
  })()`, realm());
  expect([refused, order]).toEqual(['TypeError: new Response(): data on a SharedArrayBuffer is not a BufferSource', []]);
});

test('copyBytes, as crypto.subtle takes its data, refuses a shared or resizable buffer and gives a detached one no bytes', () => {
  const copied = runInContext(`(() => {
    const copy = data => Array.from(new Uint8Array(__exact_binary.copyBytes(data, 'crypto.subtle.digest()')));
    const detached = new ArrayBuffer(2), detachedView = new DataView(detached);
    detached.transfer();
    return [said(() => copy(new Uint8Array(new SharedArrayBuffer(1)))), said(() => copy(new Uint8Array(new ArrayBuffer(2, { maxByteLength: 4 })))),
      said(() => copy(detachedView)), said(() => copy(detached)), said(() => copy(new Uint8Array([1, 2, 3]).subarray(1)))];
  })()`, realm());
  expect(copied).toEqual([
    'TypeError: crypto.subtle.digest(): data on a SharedArrayBuffer is not a BufferSource',
    'TypeError: crypto.subtle.digest(): data on a resizable ArrayBuffer is not a BufferSource',
    [], [], [2, 3],
  ]);
});
