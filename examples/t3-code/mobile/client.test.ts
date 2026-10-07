import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { mobileCommand, mobileNative, mobilePairingFields, mobilePairingTarget, mobilePairingUrl, mobileSnapshot } from './client';
import { ClientError, type Files, type Native } from './shared/protocol';

const unusedStorage: Files = { fs: {
  async mkdir() { throw new Error('Unexpected storage call'); },
  async readFile() { throw new Error('Unexpected storage call'); },
  async atomicWriteFile() { throw new Error('Unexpected storage call'); },
} };

describe('pinned shared sources', () => {
  test('every TS copy retains its pin and full body apart from explicit mobile send admission and selection', () => {
    const directory = new URL('./shared/', import.meta.url).pathname;
    const names: string[] = [];
    function visit(dir: string) {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) visit(join(dir, entry.name));
        else if (entry.name.endsWith('.ts')) names.push(relative(directory, join(dir, entry.name)));
      }
    }
    visit(directory);
    expect(names.length).toBe(297);
    for (const name of names) {
      const local = readFileSync(join(directory, name), 'utf8').split('\n');
      expect(local[0]).toContain('GAP 001');
      const pin = ['client.ts', 'local-backend.ts', 'timestamp-format.ts'].includes(name)
        ? '38352ceaf4cd35a40b7b24ce992db87c2357a99b' : '887b2491b182f851b11253655f6aa84fe2a26708';
      const adapted = name === 'client-ops-composer.ts';
      expect(local[1]).toBe(`// ${adapted ? 'Adapted' : 'Unchanged'} body from examples/t3-code/${name} at ${pin}.`);
      let expected = readFileSync(new URL(`../${name}`, import.meta.url), 'utf8');
      if (adapted) expected = "// Mobile 365aa87982: send admission and retained model options differ from this desktop copy.\nimport { mobileModelSelectionUnavailable } from '../model-availability';\nimport { mobileDispatchSelection as dispatchSelection } from '../model-send-selection';\n" + expected
        .replace("import { dispatchSelection, promptForSend, ultrathinkChoice }", "import { promptForSend, ultrathinkChoice }")
        .replace("if (!arr(provider.models).some(model => model.slug === this.modelId)) throw new ClientError('Choose one of the models advertised by T3.');",
          "if (!this.modelId || mobileModelSelectionUnavailable(this.config, { instanceId: this.providerId, model: this.modelId })) throw new ClientError('Model unavailable. Open model settings.');");
      expect(local.slice(2).join('\n')).toBe(expected);
    }
  });
});

describe('upstream mobile pairing forms', () => {
  test('unwraps mobile QR/deep links before the shared hosted parser', () => {
    const nested = 'https://t3.codes/pair?host=https%3A%2F%2Fmy-server.example#token=one-time';
    const input = `t3code://connect?pairingUrl=${encodeURIComponent(nested)}`;
    expect(mobilePairingUrl(input)).toBe(nested);
    expect(mobilePairingFields(input)).toEqual({ source: input, host: 'https://my-server.example', code: 'one-time' });
    expect(mobilePairingTarget(input, '')).toEqual({ origin: 'https://my-server.example', credential: 'one-time' });
  });
  test('accepts direct URLs, hash before query tokens, and a URL pasted into code', () => {
    const input = 'https://server.example/path?token=query#token=hash';
    expect(mobilePairingFields(input)).toEqual({ source: input, host: 'https://server.example', code: 'hash' });
    expect(mobilePairingTarget('unused.example', input)).toEqual({ origin: 'https://server.example', credential: 'hash' });
  });
  test('uses HTTP for bare IP + code, HTTPS for a bare DNS name', () => {
    expect(mobilePairingTarget('127.0.0.1:3773', 'token')).toEqual({ origin: 'http://127.0.0.1:3773', credential: 'token' });
    expect(mobilePairingTarget('[::1]:3773', 'token')).toEqual({ origin: 'http://[::1]:3773', credential: 'token' });
    expect(mobilePairingTarget('server.example:3773', 'token')).toEqual({ origin: 'https://server.example:3773', credential: 'token' });
    expect(mobilePairingTarget('https://127.0.0.1:3773', 'token').origin).toBe('https://127.0.0.1:3773');
  });
  test('host-only parsing preserves mobile form text and strips URL paths', () => {
    expect(mobilePairingFields('server.example')).toEqual({ source: 'server.example', host: 'server.example', code: '' });
    expect(mobilePairingFields('https://server.example/path?query=yes')).toEqual({ source: 'https://server.example/path?query=yes', host: 'https://server.example', code: '' });
    expect(mobilePairingFields('')).toEqual({ source: '', host: '', code: '' });
  });
  test('invalid addresses do not echo a pairing code in an error', () => {
    expect(() => mobilePairingTarget('not a host', 'secret-code')).toThrow('Enter a valid T3 server address or pairing link.');
    expect(() => mobilePairingTarget('ftp://server.example', 'secret-code')).toThrow(ClientError);
  });
});

describe('mobile wire attribution', () => {
  test('changes only newly authored orchestration payloads without mutating shared state', async () => {
    const requests: unknown[] = [], topics: string[] = [];
    const native = mobileNative({ available: true, watch: topic => topics.push(topic), later: async request => { requests.push(request); return { ok: true, generation: 9, value: {} }; } });
    const request = { op: 'request', method: 'orchestration.dispatchCommand', generation: 9, payload: { type: 'message.dispatch', creationSource: 'web', commandId: 'same-id', text: 'hello' } };
    const result = await native.later(request);
    expect(request.payload.creationSource).toBe('web');
    expect(requests[0]).toEqual({ ...request, payload: { ...request.payload, creationSource: 'mobile' } });
    expect(result).toEqual({ ok: true, generation: 9, value: {} });
    const launch = { op: 'request', method: 'orchestration.launchThread', payload: { creationSource: 'web', threadId: 'thread' } };
    await native.later(launch);
    expect(requests[1]).toEqual({ ...launch, payload: { ...launch.payload, creationSource: 'mobile' } });
    const other = { op: 'request', method: 'server.updateSettings', payload: { creationSource: 'web' } };
    await native.later(other);
    expect(requests[2]).toBe(other);
    const serverCreated = { ...request, payload: { ...request.payload, creationSource: 'server' } };
    await native.later(serverCreated);
    expect(requests[3]).toBe(serverCreated);
    native.watch('t3.events');
    expect(topics).toEqual(['t3.events']);
  });
  test('preserves native rejection identity for let-go handling', async () => {
    const error = { name: 'FetchError', kind: 'Aborted' };
    const native = mobileNative({ available: true, watch() {}, later: async () => { throw error; } });
    await expect(native.later({ op: 'status' })).rejects.toBe(error);
  });
});

describe('bake and command boundary', () => {
  test('bake snapshot is disconnected without reading files or manufacturing environments', async () => {
    const snapshot = await mobileSnapshot(undefined, unusedStorage);
    expect(snapshot.nativeAvailable).toBe(false);
    expect(snapshot.ready).toBe(false);
    expect(snapshot.environments).toEqual([]);
    expect(snapshot.projects).toEqual([]);
    expect(snapshot.origin).toBe('');
    expect(JSON.stringify(snapshot)).not.toContain('access_token');
  });
  test('unavailable host and invalid mobile addresses never dispatch native work', async () => {
    expect((await mobileCommand(['connect'], undefined, unusedStorage)).message).toContain('iPhone or iPad');
    const requests: unknown[] = [];
    const native: Native = { available: true, watch() {}, later: async request => { requests.push(request); throw new Error('unexpected'); } };
    expect((await mobileCommand(['connect', 'not a host', 'secret-code'], native, unusedStorage)).message).toContain('valid T3 server');
    expect((await mobileCommand(['environment-ssh-connect'], native, unusedStorage)).message).toContain('desktop app');
    expect(requests).toEqual([]);
  });
});

describe('mobile environment editing', () => {
  test('saves local connection fields through the native catalog, never server update', async () => {
    const requests: unknown[] = [];
    const native: Native = { available: true, watch() {}, later: async request => {
      requests.push(request); return { ok: true, generation: 0, value: { saved: [] } };
    } };
    const result = await mobileCommand(['save-environment', 'environment-one', JSON.stringify({ label: ' My Mac ', url: 'https://server.example/path' })], native, unusedStorage);
    expect(result.message).toBe('');
    expect(requests).toEqual([{ op: 'mobileUpdateEnvironment', environmentId: 'environment-one', label: 'My Mac', origin: 'https://server.example' }]);
  });
  test('rejects blank labels, pairing tokens and malformed settings before native work', async () => {
    let calls = 0;
    const native: Native = { available: true, watch() {}, later: async () => { calls++; throw new Error('unexpected'); } };
    expect((await mobileCommand(['save-environment', 'env', JSON.stringify({ label: ' ', url: 'https://host.example' })], native, unusedStorage)).message).toContain('label cannot be empty');
    expect((await mobileCommand(['save-environment', 'env', JSON.stringify({ label: 'Host', url: 'https://host.example#token=secret' })], native, unusedStorage)).message).toContain('without a pairing code');
    expect((await mobileCommand(['save-environment', 'env', '{'], native, unusedStorage)).message).toBe('The environment settings are invalid.');
    expect(calls).toBe(0);
  });
});

describe('GitHub routing permissions', () => {
  test('shows a single switched-off saved environment and reads trust by its route identity', async () => {
    const { mobileRoutingRows } = await import('./client');
    const { environmentSources } = await import('./shared/connections');
    const { T3Client } = await import('./shared/client');
    const { gitHubRoutingConnectionKey } = await import('./shared/connection-routes');
    const saved = [{ origin: 'https://host.example', environmentId: 'env', label: 'Host', enabled: false, routes: [{ id: 'direct', origin: 'https://host.example', credential: 'https://host.example' }] }];
    const sources = environmentSources(new T3Client(), saved, new Map());
    const key = gitHubRoutingConnectionKey(sources[0]!);
    expect(key).not.toBeNull();
    const preferences = JSON.stringify({ githubRouting: { [key!]: 'read-write' } });
    expect(mobileRoutingRows(sources, saved, preferences, true)).toEqual([
      { id: 'https://host.example\nenv', label: 'Host', url: 'https://host.example/', permission: 'read-write', disabled: false },
    ]);
    expect(mobileRoutingRows(sources, saved, preferences, false)[0]?.disabled).toBe(true);
    const changed = [{ ...sources[0]!, routes: [{ id: 'direct', origin: 'https://new-host.example' }] }];
    expect(mobileRoutingRows(changed, saved, preferences, true)[0]?.permission).toBe('off');
  });
});

describe('mobile environment retry ownership', () => {
  test('retries only the saved background transport and preserves focus', async () => {
    const { mobileClient } = await import('./client');
    const focused = mobileClient.environmentId, origin = mobileClient.origin;
    const calls: Record<string, unknown>[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Record<string, unknown>; calls.push(request);
      return { ok: true, generation: 7, value: request.op === 'environments'
        ? { saved: [{ origin: 'https://background.example', environmentId: 'background', enabled: true }] } : {} };
    } };
    const result = await mobileCommand(['environment-reconnect', 'https://background.example\nbackground'], native, unusedStorage);
    expect(result.message).toBe('');
    expect(calls).toEqual([{ op: 'environments' }, { op: 'retry', fleet: 'https://background.example\nbackground' }]);
    expect(mobileClient.environmentId).toBe(focused);
    expect(mobileClient.origin).toBe(origin);
  });
  test('does not open an unknown or disabled saved environment', async () => {
    const calls: unknown[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      calls.push(input); return { ok: true, generation: 0, value: { saved: [
        { origin: 'https://disabled.example', environmentId: 'disabled', enabled: false },
      ] } };
    } };
    for (const key of ['https://unknown.example\nunknown', 'https://disabled.example\ndisabled']) {
      expect((await mobileCommand(['environment-reconnect', key], native, unusedStorage)).message).toContain('Switch on');
    }
    expect(calls).toEqual([{ op: 'environments' }, { op: 'environments' }]);
  });
});
