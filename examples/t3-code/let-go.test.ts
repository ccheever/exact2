import { describe, test, expect } from 'bun:test';
import { letGo, letGoAware } from './let-go';
import { ClientError, type Native } from './protocol';
import { connected } from './composer-controls-fixture';
import { obj } from './domain';
import { toasts } from './toast';
import { copyTabPath } from './right-panel-tabs';
import { loadFilePatches, type DiffSource, type LazyPatches } from './diff-lazy';

// The prelude's FetchError (js/src/prelude.js): an Error with own name, kind and message.
class FetchError extends Error {
  readonly kind: string;
  constructor(kind: string, text: string) { super(text); this.kind = kind; this.name = 'FetchError'; }
}
const LET_GO = 'the answer was let go before this reply; the request may already have been sent';
const aborted = () => new FetchError('Aborted', LET_GO);
/** `native` whose next call matching `when` rejects with `error`. */
function failOnce(native: Native, when: (request: Record<string, unknown>) => boolean, error: unknown): Native {
  const later = native.later.bind(native);
  let armed = true;
  native.later = async request => {
    if (armed && when(obj(request))) { armed = false; throw error; }
    return later(request);
  };
  return native;
}

describe('let go', () => {
  test('a let-go rejection and a superseded call are let go; real failures are not', () => {
    expect(letGo(aborted())).toBe(true);
    expect(letGo(new ClientError('This operation was superseded.', 'superseded'))).toBe(true);
    expect(letGo(new FetchError('Network', 'The network connection was lost.'))).toBe(false);
    expect(letGo(new FetchError('Timeout', 'timed out'))).toBe(false);
    expect(letGo(new ClientError('lost connection', 'transport', true))).toBe(false);
    expect(letGo(new ClientError('Aborted', 'Aborted'))).toBe(false);
    expect(letGo(new Error('Aborted'))).toBe(false);
    expect(letGo(undefined)).toBe(false);
  });

  test('the seam turns the runtime rejection into a superseded ClientError and passes the rest', async () => {
    const reject = (error: unknown): Native => ({ available: true, watch() {}, later: async () => { throw error; } });
    const caught = await letGoAware(reject(aborted())).later({}).catch(error => error);
    expect(caught).toBeInstanceOf(ClientError);
    expect(caught.kind).toBe('superseded');
    const network = new FetchError('Network', 'offline');
    expect(await letGoAware(reject(network)).later({}).catch(error => error)).toBe(network);
    expect(await letGoAware({ available: true, watch() {}, later: async () => 7 }).later({})).toBe(7);
    // A watch is passed through until the answer is let go, and refused as superseded after.
    const watched: string[] = [];
    const seam = letGoAware({ available: true, watch: topic => { watched.push(topic); }, later: async () => { throw aborted(); } });
    seam.watch('t3.status');
    await seam.later({}).catch(() => {});
    let refused: unknown;
    try { seam.watch('t3.local'); } catch (error) { refused = error; }
    expect(refused).toBeInstanceOf(ClientError);
    expect((refused as ClientError).kind).toBe('superseded');
    expect(watched).toEqual(['t3.status']);
  });

  test('a refresh let go mid-flight shows no banner (the regression)', async () => {
    const { client, native, disk } = await connected();
    expect(client.error).toBe('');
    failOnce(native, request => request.op === 'events', aborted());
    await client.refresh(letGoAware(native), disk);
    expect(client.error).toBe('');
    expect(client.ready).toBe(true);
  });

  test('a refresh let go inside a read it tolerates watches nothing outside its answer (round 5 follow-up)', async () => {
    // fleet.sync swallows a failed `environments` read; the refresh then reads the embedded
    // server, whose `native.watch` the prelude refuses once the answer was let go
    // (js/src/prelude.js: "native.watch outside an answer"), which became the transcript banner.
    const { client, native, disk } = await connected();
    let gone = false, watchedAfter = 0;
    const later = native.later.bind(native);
    native.later = async request => {
      if (!gone && obj(request).op === 'environments') { gone = true; throw aborted(); }
      return later(request);
    };
    native.watch = () => { if (gone) { watchedAfter++; throw new Error('native.watch outside an answer'); } };
    await client.refresh(letGoAware(native), disk);
    expect(gone).toBe(true);
    expect(watchedAfter).toBe(0);
    expect(client.error).toBe('');
  });

  test('a refresh that really fails still shows the error', async () => {
    const { client, native, disk } = await connected();
    failOnce(native, request => request.op === 'events', new FetchError('Network', 'The network connection was lost.'));
    await client.refresh(letGoAware(native), disk);
    expect(client.error).toBe('The network connection was lost.');
  });

  test('a send let go mid-flight is neither an error nor a failed operation', async () => {
    const { client, native, disk } = await connected();
    failOnce(native, request => request.op === 'request', aborted());
    const result = await client.command('send', '', 'Hello', 0, letGoAware(native), disk);
    expect(result.message).toBe('');
    expect(client.error).toBe('');
    expect(toasts(client)).toEqual([]);
  });

  test('a send that really fails still shows the error', async () => {
    const { client, native, disk } = await connected();
    failOnce(native, request => request.op === 'request', new FetchError('Network', 'The network connection was lost.'));
    await client.command('send', '', 'Hello', 0, letGoAware(native), disk);
    expect(client.error).not.toBe('');
  });

  test('a copy let go mid-flight posts no failure toast; a refused one still does', async () => {
    const surface = { id: 'src/app.ts', kind: 'file' as const, path: 'src/app.ts', line: 0, reveal: 0 };
    const letGoCopy = await connected();
    failOnce(letGoCopy.native, request => request.op === 'copyText', aborted());
    const caught = await copyTabPath(letGoCopy.client, letGoAware(letGoCopy.native), surface).catch(error => error);
    expect(caught).toBeInstanceOf(ClientError);
    expect(toasts(letGoCopy.client)).toEqual([]);
    const refused = await connected();
    failOnce(refused.native, request => request.op === 'copyText', new Error('Denied'));
    await copyTabPath(refused.client, letGoAware(refused.native), surface);
    expect(toasts(refused.client)[0]).toMatchObject({ kind: 'error', title: 'Failed to copy path', description: 'Denied' });
  });

  test('a diff file request let go is asked again, not marked as an error', async () => {
    const file = { path: 'a.ts', previousPath: null, additions: 1, deletions: 0 };
    const source: DiffSource = { kind: 'branch-range', cwd: '/repo', baseRef: 'main', headRef: null, diffHash: 'h', truncated: false, files: [file] };
    const lazy = (): LazyPatches => ({ scope: JSON.stringify(['/repo', 'branch-range']), files: [file], requested: [0], patches: new Map() });
    const letGoLazy = lazy();
    await loadFilePatches(letGoLazy, source, false, async () => { throw new ClientError(LET_GO, 'superseded'); });
    expect(letGoLazy.patches.has('a.ts')).toBe(false);
    const failedLazy = lazy();
    await loadFilePatches(failedLazy, source, false, async () => { throw new ClientError('The diff is unavailable.', 'transport'); });
    expect(failedLazy.patches.get('a.ts')?.state).toBe('error');
  });
});
