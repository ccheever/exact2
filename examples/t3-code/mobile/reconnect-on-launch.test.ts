import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { launchFocusKey, reconnectOnLaunch } from './shared/r8-pointer-reconnect';

const origin = 'http://127.0.0.1:53461';
const saved = [{ origin, environmentId: 'env', enabled: true }];
const good = (value: unknown) => ({ ok: true, generation: 0, value });
const status = { state: 'disconnected', origin, environmentId: '' };
const focus = () => ({ connection: 'disconnected', origin, environmentId: '' });
function fixture(read: () => Promise<unknown> = async () => good({ saved }), connect: () => Promise<unknown> = async () => good({})) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later(request) {
    const input = obj(request); calls.push(input);
    return input.op === 'environments' ? read() : connect();
  } };
  return { native, calls, connects: () => calls.filter(call => call.op === 'connect') };
}
function deferred() {
  let resolve!: (value: unknown) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe('mobile launch reconnect ownership', () => {
  for (const failure of [
    { name: 'FetchError', kind: 'Aborted', message: 'Read answer ended.' },
    new ClientError('Newer refresh owns the read.', 'superseded'),
  ]) test(`fresh answer retries ${obj(failure).kind} catalog read`, async () => {
    const client = focus();
    const lost = fixture(async () => { throw failure; });
    expect(await reconnectOnLaunch(client, letGoAware(lost.native), status)).toBe(false);
    expect(launchFocusKey(client)).toBe('');
    const fresh = fixture();
    expect(await reconnectOnLaunch(client, fresh.native, status)).toBe(true);
    expect(await reconnectOnLaunch(client, fresh.native, status)).toBe(false);
    expect(lost.connects()).toHaveLength(0);
    expect(fresh.connects()).toEqual([{ op: 'connect', origin, credential: '' }]);
    expect(launchFocusKey(client)).toBe(`${origin}\nenv`);
  });

  test('a rejected catalog reply remains retryable', async () => {
    const client = focus();
    const denied = fixture(async () => ({ ok: false, generation: 0, error: { kind: 'Aborted', message: 'Read unavailable', uncertain: false } }));
    expect(await reconnectOnLaunch(client, denied.native, status)).toBe(false);
    const fresh = fixture();
    expect(await reconnectOnLaunch(client, fresh.native, status)).toBe(true);
    expect(fresh.connects()).toHaveLength(1);
  });

  for (const firstFinishesFirst of [true, false]) test(`latest concurrent read owns focus, older finishes ${firstFinishesFirst ? 'first' : 'last'}`, async () => {
    const client = focus(), first = deferred(), latest = deferred();
    const a = fixture(() => first.promise), b = fixture(() => latest.promise);
    const older = reconnectOnLaunch(client, a.native, status);
    const newer = reconnectOnLaunch(client, b.native, status);
    const finishOlder = async () => { first.resolve(good({ saved: [{ origin: 'https://obsolete.invalid', environmentId: 'old' }] })); expect(await older).toBe(false); };
    const finishNewer = async () => { latest.resolve(good({ saved })); expect(await newer).toBe(true); };
    if (firstFinishesFirst) { await finishOlder(); await finishNewer(); }
    else { await finishNewer(); await finishOlder(); }
    expect(a.connects()).toHaveLength(0);
    expect(b.connects()).toHaveLength(1);
    expect(launchFocusKey(client)).toBe(`${origin}\nenv`);
  });

  test('an older rejected read cannot release a newer pending connection', async () => {
    const client = focus(), first = deferred();
    const a = fixture(() => first.promise), b = fixture();
    const older = reconnectOnLaunch(client, a.native, status);
    expect(await reconnectOnLaunch(client, b.native, status)).toBe(true);
    first.reject(new ClientError('Superseded', 'superseded'));
    expect(await older).toBe(false);
    expect(await reconnectOnLaunch(client, b.native, status)).toBe(false);
    expect(b.connects()).toHaveLength(1);
    expect(launchFocusKey(client)).toBe(`${origin}\nenv`);
  });

  test('a successful empty catalog is a terminal launch decision', async () => {
    const client = focus(), empty = fixture(async () => good({ saved: [] })), later = fixture();
    expect(await reconnectOnLaunch(client, empty.native, status)).toBe(false);
    expect(await reconnectOnLaunch(client, later.native, status)).toBe(false);
    expect(later.calls).toHaveLength(0);
    expect(launchFocusKey(client)).toBe('');
  });

  for (const state of ['connected', 'connecting', 'error']) test(`${state} focus remains a terminal launch decision`, async () => {
    const client = focus(), f = fixture();
    expect(await reconnectOnLaunch(client, f.native, { ...status, state })).toBe(false);
    expect(await reconnectOnLaunch(client, f.native, status)).toBe(false);
    expect(f.calls).toHaveLength(0);
  });

  test('an identified focus does not reconnect and invalidates an older catalog', async () => {
    const client = focus(), waiting = deferred(), old = fixture(() => waiting.promise), f = fixture();
    const earlier = reconnectOnLaunch(client, old.native, status);
    expect(await reconnectOnLaunch(client, f.native, { ...status, environmentId: 'env' })).toBe(false);
    waiting.resolve(good({ saved }));
    expect(await earlier).toBe(false);
    expect(old.connects()).toHaveLength(0);
  });

  test('unknown dispatched connect rejection retains once-only ownership until status resolves', async () => {
    const client = focus(), f = fixture(undefined, async () => { throw { name: 'FetchError', kind: 'Aborted', message: 'Request may have been sent.' }; });
    expect(await reconnectOnLaunch(client, f.native, status)).toBe(true);
    await Promise.resolve();
    expect(await reconnectOnLaunch(client, f.native, status)).toBe(false);
    expect(f.connects()).toHaveLength(1);
    expect(launchFocusKey(client)).toBe(`${origin}\nenv`);
    client.connection = 'error';
    expect(launchFocusKey(client)).toBe('');
  });
});
