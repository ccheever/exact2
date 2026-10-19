// The design tool's reading of the wire's scopes (apps/design-tool/wire.ts
// NEEDS) is the server's table (scripts/tamber-pairing.mjs METHOD_SCOPE),
// so a paired tool greys and holds back exactly what the server refuses
// (LLP 1122 D12, T6).
import { expect, test } from 'bun:test';
import { METHOD_SCOPE } from './tamber-pairing.mjs';
import { above, doorOf, NEEDS } from '../apps/design-tool/wire.ts';

test('the tool\'s scope table is the server\'s, method for method', () => {
  expect(NEEDS).toEqual({ ...METHOD_SCOPE });
});

test('a paired client holds back a request above its scope, and the local tool\'s methods always', () => {
  expect(above('propose', 'write', 'transaction')).toContain('needs write');
  expect(above('propose', 'propose', 'propose')).toBe('');
  expect(above('write', 'local', 'pair-code')).toContain('local tool\'s alone');
});

test('a pairing goes only to an https door or a tunnel on this machine\'s loopback', () => {
  expect(doorOf('https://192.168.1.20:7743/__tamber/tool/').origin).toBe('https://192.168.1.20:7743');
  expect(doorOf('http://127.0.0.1:7744').origin).toBe('http://127.0.0.1:7744');
  expect(doorOf('http://[::1]:7744').origin).toBe('http://[::1]:7744');
  for (const bad of ['http://192.168.1.20:7743', 'http://example.test', 'ftp://127.0.0.1', 'https://ada:pw@192.168.1.20:7743', 'not a url', 'http://127.0.0.1.example.test']) expect(doorOf(bad).error).not.toBe('');
});
