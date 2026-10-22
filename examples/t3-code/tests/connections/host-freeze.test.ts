// A host that dropped because it froze says why (host-freeze.ts; the reference's hostFreeze.test.ts and
// HostFreezeToastCoordinator at 74541bab17), and a read whose session went away shows no failure (runtime.ts
// pendingOnceSessionReplaced).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { eventLoopStall, hostFreezeNotice } from '../../src/connections/host-freeze';
import { toasts } from '../../src/shell/toast';
import { Backend, storage } from '../support/client-fixture';
import type { Obj } from '../../src/client/domain';

const stall = { detectedAt: '2026-10-20T07:50:37.633Z', durationMs: 12_422, loadAverage: 219.4, cpuCount: 18 };

describe('hostFreezeNotice', () => {
  test('blames an overloaded CPU when the load is above the core count', () => {
    expect(hostFreezeNotice('leftbook', stall)).toEqual({
      title: 'leftbook froze for 12s',
      description: 'Its CPU is overloaded (load 219 on 18 cores).',
    });
  });

  test('blames T3 Code when the CPU had room, or the host reports no load', () => {
    const busy = 'T3 Code on leftbook was too busy to answer.';
    expect(hostFreezeNotice('leftbook', { ...stall, loadAverage: 4 })?.description).toBe(busy);
    const { loadAverage: _load, ...windows } = stall;
    expect(hostFreezeNotice('leftbook', windows)?.description).toBe(busy);
  });

  test('reads only a stall of the reference shape', () => {
    expect(eventLoopStall({ lastEventLoopStall: stall })).toEqual(stall);
    expect(eventLoopStall({})).toBeUndefined();
    expect(eventLoopStall({ lastEventLoopStall: { ...stall, cpuCount: 0 } })).toBeUndefined();
    expect(eventLoopStall({ lastEventLoopStall: { ...stall, durationMs: -1 } })).toBeUndefined();
  });
});

const freezes = (client: T3Client) => toasts(client).filter(toast => toast.title.includes('froze'));

describe('the reconnect after a freeze', () => {
  test('warns once per stall, naming the host', async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    native.config = { ...native.config, environment: { ...(native.config.environment as Obj), label: 'leftbook' }, lastEventLoopStall: { ...stall, loadAverage: 3.2 } };
    await client.refresh(native, disk.files);
    expect(freezes(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([
      ['warning', 'leftbook froze for 12s', 'T3 Code on leftbook was too busy to answer.'],
    ]);
    // The same stall, read again after another reconnect, is not news.
    native.generation++;
    await client.refresh(native, disk.files);
    expect(freezes(client)).toHaveLength(1);
    // A new one is.
    native.config = { ...native.config, lastEventLoopStall: { ...stall, detectedAt: '2026-10-20T07:58:00.000Z', durationMs: 7_000 } };
    native.generation++;
    await client.refresh(native, disk.files);
    expect(freezes(client).map(toast => toast.title)).toEqual(['leftbook froze for 12s', 'leftbook froze for 7s']);
  });

  test('a server without the field shows nothing', async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    await client.refresh(native, disk.files);
    expect(freezes(client)).toEqual([]);
  });
});

describe('a read whose session went away', () => {
  async function refreshFailing(error: Obj) {
    const client = new T3Client(), native = new Backend(), disk = storage();
    const original = native.later.bind(native);
    native.later = async (input: unknown) => (input as Obj).method === 'server.getConfig'
      ? { ok: false, generation: native.generation, error } : original(input);
    await client.refresh(native, disk.files);
    return client;
  }

  test('shows no failure: the connection says it is reconnecting, and the read is asked again', async () => {
    const client = await refreshFailing({ kind: 'Network', message: 'Server env1 stopped responding for 15s. It may be overloaded, asleep, or off the network.', uncertain: true, unavailable: true });
    expect(client.error).toBe('');
  });

  test('any other failure of the read still shows', async () => {
    const client = await refreshFailing({ kind: 'Network', message: 'The configuration could not be read.', uncertain: false });
    expect(client.error).toBe('The configuration could not be read.');
  });
});
