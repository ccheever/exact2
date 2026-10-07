// Pairing URL cases from packages/shared/src/remote.test.ts at 1e2ecbd975,
// adapted to Exact's origin/credential result and Bun; command coverage below
// checks Welcome's form boundary against the real client command dispatcher.
import { afterEach, describe, expect, test } from 'bun:test';
import { resolveRemotePairingTarget } from './remote';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { fleet } from './settings-b-fleet';
import { toasts } from './toast';

describe('resolveRemotePairingTarget', () => {
  test.each([
    ['https://remote.example.com/pair#token=pairing-token', 'https://remote.example.com'],
    ['https://remote.example.com/pair?token=pairing-token', 'https://remote.example.com'],
    ['wss://remote.example.com/pair#token=pairing-token', 'https://remote.example.com'],
    ['ws://remote.example.com:1234/pair#token=pairing-token', 'http://remote.example.com:1234'],
    ['https://app.t3.codes/pair?host=https%3A%2F%2Fdesktop.tailnet.ts.net%3A44342%2F#token=pairing-token', 'https://desktop.tailnet.ts.net:44342'],
    ['https://app.t3.codes/pair?host=%2F%2Fremote.example.com#token=pairing-token', 'https://remote.example.com'],
    ['https://app.t3.codes/pair?host=%2F%2F%2Fremote.example.com#token=pairing-token', 'https://remote.example.com'],
    ['https://app.t3.codes/pair?host=%2F%2Fhttps%3A%2F%2Fremote.example.com#token=pairing-token', 'https://remote.example.com'],
    ['https://app.t3.codes/pair?host=myserver.com%3A3000#token=pairing-token', 'https://myserver.com:3000'],
    ['  https://remote.example.com/pair?token=ignored#token=%20pairing-token%20  ', 'https://remote.example.com'],
    ['https://remote.example.com/pair?token=pairing-token#token=%20', 'https://remote.example.com'],
  ])('resolves %s', (pairingUrl, origin) => {
    expect(resolveRemotePairingTarget({ pairingUrl })).toEqual({ origin, credential: 'pairing-token' });
  });
});

const rejected = [
  ['invalid pairing URL', 'Pairing URL is invalid.'],
  ['remote.example.com/pair#token=secret', 'Pairing URL is invalid.'],
  ['https://[invalid/pair#token=secret', 'Pairing URL is invalid.'],
  ['ftp://remote.example.com/pair#token=secret', 'Pairing URL is invalid.'],
  ['https://remote.example.com/pair', 'Pairing URL is missing its token.'],
  ['https://remote.example.com/pair#token=%20', 'Pairing URL is missing its token.'],
  ['https://app.t3.codes/pair?host=https%3A%2F%2F%5Binvalid#token=secret', 'Backend URL is invalid.'],
  ['https://app.t3.codes/pair?host=ftp%3A%2F%2Fremote.example.com#token=secret', 'Backend URL is invalid.'],
];

class PairingTransport implements Native {
  available = true;
  calls: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const good = (value: unknown) => ({ ok: true, generation: 1, value });
    if (request.op === 'connect' || request.op === 'pairEnvironment') {
      if (request.credential === 'unavailable') return { ok: false, generation: 1,
        error: { kind: 'Network', message: 'Could not connect to the server.', uncertain: false } };
      return good({ state: 'connected', origin: request.origin, environmentId: 'paired-environment', message: '' });
    }
    if (request.op === 'disconnect') return good({ state: 'disconnected', origin: '', environmentId: '', message: 'Disconnected.' });
    if (request.op === 'environments') return good({ saved: [] });
    return good({});
  }
}

function fixture(connected: boolean) {
  const client = new T3Client(), native = new PairingTransport();
  client.origin = 'http://127.0.0.1:16872';
  client.connection = connected ? 'connected' : 'disconnected';
  client.environmentId = connected ? 'existing-environment' : '';
  let written = '';
  const storage: Files = { fs: {
    async mkdir() {},
    async readFile() { throw new Error('empty fixture'); },
    async atomicWriteFile(_path, bytes) { written = new TextDecoder().decode(bytes); },
  } };
  const command = (value: string) => client.command('welcome-pair', '', value, 0, native, storage);
  return { client, native, storage, command, written: () => written };
}

afterEach(() => fleet.entries.clear());

for (const connected of [false, true]) {
  describe(`Welcome pairing while ${connected ? 'connected' : 'disconnected'}`, () => {
    test.each(rejected)('%s returns its own form error without transport', async (input, message) => {
      const { client, native, command, written } = fixture(connected);
      client.error = 'An existing unrelated error';
      const before = { origin: client.origin, environmentId: client.environmentId, connection: client.connection };
      expect(await command(input)).toMatchObject({ message });
      expect(native.calls.filter(call => ['connect', 'pairEnvironment', 'disconnect'].includes(String(call.op)))).toEqual([]);
      expect(client).toMatchObject({ ...before, error: 'An existing unrelated error', busy: false });
      expect(toasts(client)).toEqual([]);
      expect(written()).toBe('');
      expect(message).not.toContain('secret');
    });

    test('a real transport failure stays in the form and a corrected link succeeds', async () => {
      const { client, native, command } = fixture(connected);
      expect(await command('http://127.0.0.1:16874/pair#token=unavailable'))
        .toMatchObject({ message: 'Could not connect to the server.' });
      expect(client.error).toBe('');
      expect(client.busy).toBe(false);
      expect(toasts(client)).toEqual([]);
      expect(await command('invalid pairing URL')).toMatchObject({ message: 'Pairing URL is invalid.' });
      expect(await command('http://127.0.0.1:16873/pair#token=valid-fixture-code')).toMatchObject({ message: '' });
      const submitted = native.calls.filter(call => call.op === (connected ? 'pairEnvironment' : 'connect'));
      expect(submitted.map(call => [call.origin, call.credential])).toEqual([
        ['http://127.0.0.1:16874', 'unavailable'], ['http://127.0.0.1:16873', 'valid-fixture-code'],
      ]);
      expect(client).toMatchObject({ error: '', busy: false, connection: 'connected',
        environmentId: connected ? 'existing-environment' : 'paired-environment' });
      expect(toasts(client)).toEqual([]);
    });

    test('Settings still accepts a separate backend host and pairing code', async () => {
      const { client, native, storage } = fixture(connected);
      expect(await client.command('environment-add', 'http://127.0.0.1:16873', 'separate-code', 0, native, storage)).toMatchObject({ message: '' });
      expect(native.calls.find(call => call.op === (connected ? 'pairEnvironment' : 'connect')))
        .toMatchObject({ origin: 'http://127.0.0.1:16873', credential: 'separate-code' });
      expect(toasts(client)).toMatchObject([{ kind: 'success', title: 'Backend added' }]);
    });
  });
}
