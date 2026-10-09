import { expect, test } from 'bun:test';
import { withMobileFaviconIO } from './mobile-favicon-io';
import { type MobileFaviconIO } from './mobile-favicon-cache';
import { type Native } from './shared/protocol';

const dataUrl = 'data:image/png;base64,YQ==';
const ok = (value: unknown = {}) => ({ ok: true, generation: 0, value });
const key = { environmentId: 'environment', kind: 'project-favicon' as const, key: 'resource' };
function mock(perform: (request: any) => unknown) {
  const calls: any[] = [];
  const native: Native = { available: true, watch() {}, async later(request) { calls.push(request); return perform(request); } };
  return { native, calls };
}
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
test('favicon IO delegates bounded global cache operations and scoped clearing', async () => {
  const { native, calls } = mock(request => ok(request.action === 'list' ? { rows: [] }
    : request.action === 'read' ? { record: null } : request.action === 'ticket' ? { ticket: 'ticket' }
    : request.action === 'write' ? { written: true, stale: false } : { removed: 0 }));
  await withMobileFaviconIO(native, async io => {
    expect(await io.list(null, 128)).toEqual([]);
    expect(await io.read(key)).toBeNull();
    expect(await io.ticket(key)).toBe('ticket');
    expect(await io.write(key, 'ticket', '{}')).toBe(true);
    await io.remove(key, '{}'); await io.clear('environment'); await io.clear();
  });
  expect(calls.map(row => row.action)).toEqual(['list', 'read', 'ticket', 'write', 'remove', 'clear', 'clearKind']);
  expect(calls[0]).toEqual({ op: 'mobileClientCache', action: 'list', kind: 'project-favicon', after: null, limit: 128 });
  expect(calls[4].expectedPayload).toBe('{}');
  expect(calls[5]).toEqual({ op: 'mobileClientCache', action: 'clear', kind: 'project-favicon', environmentId: 'environment' });
  expect(calls[6]).toEqual({ op: 'mobileClientCache', action: 'clearKind', kind: 'project-favicon' });
});
test('loads use unique request IDs and return only bounded supported image data', async () => {
  const { native, calls } = mock(() => ok({ dataUrl }));
  for (let index = 0; index < 2; index++) await withMobileFaviconIO(native, async io => {
    expect(await io.load('https://image.invalid/a.png?token=private', new AbortController().signal)).toBe(dataUrl);
  });
  expect(calls[0].requestId).not.toBe(calls[1].requestId);
  expect(Object.keys(calls[0]).sort()).toEqual(['action', 'op', 'requestId', 'url']);
  for (const invalid of ['', 'https://remote.invalid/a.png', 'data:text/html;base64,YQ==', dataUrl + 'A'.repeat(32768)]) {
    const fixture = mock(() => ok({ dataUrl: invalid }));
    await expect(withMobileFaviconIO(fixture.native, io => io.load('https://image.invalid', new AbortController().signal)))
      .rejects.toThrow('invalid image data');
  }
});
test('abort before admission performs no native work', async () => {
  const { native, calls } = mock(() => ok({ dataUrl })), controller = new AbortController(); controller.abort();
  await expect(withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal))).rejects.toMatchObject({ kind: 'Cancelled' });
  expect(calls).toHaveLength(0);
});
test('abort cancels the exact request, awaits its acknowledgment, and removes the listener', async () => {
  const load = deferred<unknown>(), cancel = deferred<unknown>(), controller = new AbortController();
  const { native, calls } = mock(request => request.action === 'load' ? load.promise : cancel.promise);
  let finished = false;
  const answer = withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal));
  const outcome = answer.then(() => { finished = true; }, error => { finished = true; return error; });
  controller.abort(); controller.abort();
  expect(calls.map(row => row.action)).toEqual(['load', 'cancel']);
  expect(calls[0].requestId).toBe(calls[1].requestId);
  load.resolve(ok({ dataUrl })); await Promise.resolve(); await Promise.resolve();
  expect(finished).toBe(false);
  cancel.resolve(ok()); expect(await outcome).toMatchObject({ kind: 'Cancelled' });
  expect(calls).toHaveLength(2);
});
test('lost answer propagates and refuses later cache or abort calls', async () => {
  const controller = new AbortController();
  const { native, calls } = mock(() => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await withMobileFaviconIO(native, async io => {
    await expect(io.load('https://image.invalid', controller.signal)).rejects.toMatchObject({ kind: 'superseded' });
    controller.abort();
    await expect(io.ticket(key)).rejects.toMatchObject({ kind: 'superseded' });
  });
  expect(calls).toHaveLength(1);
});
test('a lost cancellation acknowledgment propagates even if image finished successfully', async () => {
  const load = deferred<unknown>(), controller = new AbortController();
  const { native } = mock(request => {
    if (request.action === 'load') return load.promise;
    throw { name: 'FetchError', kind: 'Aborted' };
  });
  const answer = withMobileFaviconIO(native, io => io.load('https://image.invalid', controller.signal));
  controller.abort(); load.resolve(ok({ dataUrl }));
  await expect(answer).rejects.toMatchObject({ kind: 'superseded' });
});
test('completed and failed brackets refuse an escaped adapter and detach abort listeners', async () => {
  for (const fail of [false, true]) {
    const { native, calls } = mock(() => ok({ dataUrl })), controller = new AbortController();
    let escaped!: MobileFaviconIO;
    const answer = withMobileFaviconIO(native, async io => {
      escaped = io; await io.load('https://image.invalid', controller.signal);
      if (fail) throw new Error('caller failed');
    });
    if (fail) await expect(answer).rejects.toThrow('caller failed'); else await answer;
    controller.abort();
    expect(() => escaped.ticket(key)).toThrow('superseded');
    expect(calls).toHaveLength(1);
  }
});
