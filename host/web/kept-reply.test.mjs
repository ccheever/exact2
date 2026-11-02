// The prelude's reply (js/src/binary.js `keptReply`) keeps a frozen row's JSON only in the native runtime
// (its byte door) with the prototypes hardening froze: the web's module realm is neither, so each reply is a
// plain JSON.stringify there, whatever a prototype or a Proxy does later.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createContext, runInContext } from 'node:vm';

const binary = readFileSync(new URL('../../js/src/binary.js', import.meta.url), 'utf8');

function realm(harden) {
  const context = createContext({});
  runInContext(binary, context);
  runInContext(`globalThis.later = false;
    // A Proxy whose trap answers no toJSON at first and one later: a kept row holding it would go stale.
    globalThis.turning = new Proxy(Object.freeze({ k: 'still' }), { get: (t, k, r) => k === 'toJSON'
      ? (later ? () => ({ k: 'turned' }) : undefined) : Reflect.get(t, k, r) });
    globalThis.rows = Object.freeze(Array.from({ length: 20 }, (_, i) => Object.freeze({ id: 'r' + i,
      holes: Object.freeze([, 1]), inner: Object.freeze([i === 3 ? turning : Object.freeze({ k: 'plain' })]) })));`, context);
  if (harden) runInContext('Object.freeze(Object.prototype); Object.freeze(Array.prototype);', context);
  return context;
}
const reply = (context) => runInContext('__exact_binary.keptReply({ tag: 0, value: rows }, null)', context);
const plain = (context) => runInContext('JSON.stringify({ tag: 0, value: rows })', context);

test('a realm whose prototypes are not frozen keeps no row, so a later prototype change shows', () => {
  const context = realm(false);
  expect(reply(context)).toContain('"holes":[null,1]');
  runInContext('Array.prototype[0] = "filled";', context);
  expect(reply(context)).toBe(plain(context));
  expect(reply(context)).toContain('"holes":["filled",1]');
});

test('a hardened realm without the native door keeps no row either, so a Proxy that turns shows', () => {
  const context = realm(true);
  expect(reply(context)).toBe(plain(context));
  runInContext('later = true;', context);
  expect(reply(context)).toBe(plain(context));
  expect(reply(context)).toContain('"turned"');
});
