import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'bun:test';
import { answer } from './app';

describe('mobile pairing source', () => {
  test('keeps the copied parser identical to the adopted desktop source', () => {
    const local = readFileSync(new URL('./shared/r10-connect-pairing.ts', import.meta.url), 'utf8');
    const desktop = readFileSync(new URL('../r10-connect-pairing.ts', import.meta.url), 'utf8');
    expect(local.split('\n').slice(2).join('\n')).toBe(desktop);
  });
  test('uses the shared hosted-link parser', () => {
    expect(answer('pairingFields', ['https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture']))
      .toEqual({ source: 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture', host: 'https://server.example', code: 'fixture' });
  });
  test('answers empty input at bake and rejects unknown sources', () => {
    expect(answer('pairingFields', [''])).toEqual({ source: '', host: '', code: '' });
    expect(() => answer('missing', [])).toThrow('Unknown mobile source');
  });
});
