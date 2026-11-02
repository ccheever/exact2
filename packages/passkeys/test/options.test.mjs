import { expect, test } from 'bun:test';
import { implementation } from '../web/index.js';

test('fallback JSON conversion decodes every binary extension without changing the input', async () => {
  const previous = ['navigator', 'PublicKeyCredential'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]);
  const calls = [];
  try {
    Object.defineProperty(globalThis, 'PublicKeyCredential', { configurable: true, value: class {} });
    const credential = { id: 'credential', rawId: new Uint8Array([1]), response: { clientDataJSON: new Uint8Array(), authenticatorData: new Uint8Array(), signature: new Uint8Array(), userHandle: new Uint8Array([2]) } };
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { credentials: {
      create: async options => { calls.push(options.publicKey); return credential; },
      get: async options => { calls.push(options.publicKey); return credential; },
    } } });
    const publicKey = { challenge: 'AQI', rp: { id: 'example.test' }, user: { id: 'AwQ' }, extensions: {
      prf: { eval: { first: 'BQY', second: 'Bwg' }, evalByCredential: { credential: { first: 'CQo', second: 'Cww' } } },
      largeBlob: { write: 'DQ4' }, credBlob: 'DxA', credProps: true,
    } };
    const before = JSON.stringify(publicKey), api = implementation({ changed() {} });
    expect((await api.create({ publicKey })).error).toBeUndefined();
    expect((await api.get({ publicKey })).error).toBeUndefined();
    for (const call of calls) {
      expect([...call.extensions.prf.eval.first]).toEqual([5, 6]);
      expect([...call.extensions.prf.eval.second]).toEqual([7, 8]);
      expect([...call.extensions.prf.evalByCredential.credential.first]).toEqual([9, 10]);
      expect([...call.extensions.prf.evalByCredential.credential.second]).toEqual([11, 12]);
      expect([...call.extensions.largeBlob.write]).toEqual([13, 14]);
      expect([...call.extensions.credBlob]).toEqual([15, 16]);
      expect(call.extensions.credProps).toBe(true);
    }
    expect(JSON.stringify(publicKey)).toBe(before);
  } finally {
    for (const [key, descriptor] of previous) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key]; }
  }
});
