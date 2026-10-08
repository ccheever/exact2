import { expect, test } from 'bun:test';
import { T3Client } from './client';
import { type Obj, obj } from './domain';
import type { Native } from './protocol';
import { peekCurrentVcsStatus as current, peekVcsStatus, restartVcsStatus, vcsStatusEvent as event, watchVcsStatus } from './shell-vcs';

function fixture() {
  const client = new T3Client();
  Object.assign(client, { generation: 1, origin: 'https://a.test', environmentId: 'a', connection: 'connected' });
  const pending: { generation: number; answer(value: Obj): void; fail(error: Error): void }[] = [];
  const requests: Obj[] = [];
  const native: Native = { available: true, watch() {}, later(request) {
    const input = obj(request); requests.push(input);
    if (input.op !== 'subscribe') return Promise.resolve({ ok: true, generation: client.generation, value: {} });
    const generation = client.generation;
    return new Promise((resolve, reject) => pending.push({ generation,
      answer: value => resolve({ ok: true, generation, value }), fail: reject }));
  } };
  const local = (id: string, refName: string | null, tag = 'snapshot') => event(client, {
    subscriptionId: id, value: { _tag: tag, local: { isRepo: true, refName, workingTree: { files: [] } }, remote: null },
  });
  const watch = (cwd = '/repo', now = 1) => watchVcsStatus(client, native, cwd, now);
  return { client, native, requests, pending, local, watch };
}

test('local evidence waits for its exact subscribe reply, then snapshots are detached', async () => {
  const f = fixture(), watching = f.watch(); f.local('1-10', 'main');
  expect(current(f.client, '/repo')).toBeNull();
  expect(peekVcsStatus(f.client, '/repo')?.refName).toBe('main');
  f.pending[0]!.answer({ id: '1-10' }); await watching;
  expect(current(f.client, '/repo')).toEqual({ origin: 'https://a.test', environmentId: 'a', generation: 1,
    cwd: '/repo', subscriptionId: '1-10', status: { isRepo: true, refName: 'main', workingTree: { files: [] } } });
  const status = current(f.client, '/repo')!.status; obj(status.workingTree).files = ['changed'];
  expect(obj(current(f.client, '/repo')!.status.workingTree).files).toEqual([]);
  f.local('1-10', 'switched', 'localUpdated'); expect(current(f.client, '/repo')?.status.refName).toBe('switched');
  f.local('1-10', null); expect(current(f.client, '/repo')?.status.refName).toBeNull();
});

test('remote-only defaults and wrong-generation local events never establish evidence', async () => {
  const f = fixture(), watching = f.watch(); f.pending[0]!.answer({ id: '1-10' }); await watching;
  event(f.client, { subscriptionId: '1-10', value: { _tag: 'remoteUpdated', remote: { aheadCount: 4 } } });
  expect(peekVcsStatus(f.client, '/repo')?.isRepo).toBe(true); expect(current(f.client, '/repo')).toBeNull();
  f.local('2-11', 'wrong'); expect(current(f.client, '/repo')).toBeNull();
  event(f.client, { generation: 2, subscriptionId: '1-10', value: { _tag: 'snapshot', local: { isRepo: true, refName: 'wrong' } } });
  expect(current(f.client, '/repo')).toBeNull();
  f.local('1-10', 'right'); expect(current(f.client, '/repo')?.status.refName).toBe('right');
});

test('cwd away/back keeps warm display but requires new subscription local data', async () => {
  const f = fixture(); let watching = f.watch('/a'); f.pending[0]!.answer({ id: '1-10' }); await watching; f.local('1-10', 'old-a');
  watching = f.watch('/b'); f.pending[1]!.answer({ id: '1-20' }); await watching; f.local('1-20', 'b');
  expect(current(f.client, '/a')).toBeNull(); expect(current(f.client, '/b')?.status.refName).toBe('b');
  watching = f.watch('/a'); expect(peekVcsStatus(f.client, '/a')?.refName).toBe('old-a');
  f.pending[2]!.answer({ id: '1-30' }); await watching;
  expect(current(f.client, '/a')).toBeNull(); f.local('1-10', 'stale-a'); expect(current(f.client, '/a')).toBeNull();
  f.local('1-30', 'new-a'); expect(current(f.client, '/a')?.status.refName).toBe('new-a');
});

test('origin, environment, generation and disconnect boundaries retire prior evidence', async () => {
  for (const change of [{ origin: 'https://b.test' }, { environmentId: 'b' }, { generation: 2 }, { connection: 'disconnected' }]) {
    const f = fixture(), watching = f.watch(); f.pending[0]!.answer({ id: '1-10' }); await watching; f.local('1-10', 'a');
    Object.assign(f.client, change); expect(current(f.client, '/repo')).toBeNull();
    Object.assign(f.client, { origin: 'https://a.test', environmentId: 'a', generation: 1, connection: 'connected' });
    expect(current(f.client, '/repo')).toBeNull();
  }
});

test('new owner with the same cwd cannot inherit warm status before its local event', async () => {
  const f = fixture(); let watching = f.watch(); f.pending[0]!.answer({ id: '1-10' }); await watching; f.local('1-10', 'a');
  Object.assign(f.client, { origin: 'https://b.test', environmentId: 'b', generation: 2 });
  watching = f.watch(); f.local('1-19', 'old'); expect(current(f.client, '/repo')).toBeNull();
  f.pending[1]!.answer({ id: '2-20' }); await watching;
  expect(current(f.client, '/repo')).toBeNull(); f.local('2-20', 'b');
  expect(current(f.client, '/repo')).toMatchObject({ origin: 'https://b.test', environmentId: 'b', generation: 2, status: { refName: 'b' } });
});

test('an unseen old stream above the floor is not authenticated by a different reply', async () => {
  const f = fixture(), watching = f.watch(); f.local('1-19', 'old');
  f.pending[0]!.answer({ id: '1-20' }); await watching;
  expect(current(f.client, '/repo')).toBeNull(); f.local('1-19', 'late-old'); expect(current(f.client, '/repo')).toBeNull();
  f.local('1-20', 'fresh'); expect(current(f.client, '/repo')?.status.refName).toBe('fresh');
});

test('delayed same-path replies cannot publish after owner or cwd ABA', async () => {
  const f = fixture(), a1 = f.watch('/a'), b = f.watch('/b'), a2 = f.watch('/a');
  f.local('1-30', 'fresh-a'); f.pending[2]!.answer({ id: '1-30' }); await a2;
  f.pending[0]!.answer({ id: '1-10' }); await a1; f.pending[1]!.answer({ id: '1-20' }); await b;
  expect(current(f.client, '/a')?.subscriptionId).toBe('1-30');
  const g = fixture(), old = g.watch(); Object.assign(g.client, { origin: 'https://b.test', environmentId: 'b', generation: 2 });
  const next = g.watch(); g.pending[1]!.answer({ id: '2-20' }); await next; g.local('2-20', 'b');
  g.pending[0]!.answer({ id: '1-10' }); await old;
  expect(current(g.client, '/repo')?.status.refName).toBe('b');
});

test('restart, end and transport errors require new local evidence without changing display retention', async () => {
  for (const marker of ['_retryDue', '_streamEnded', '_transportError']) {
    const f = fixture(), watching = f.watch(); f.pending[0]!.answer({ id: '1-10' }); await watching; f.local('1-10', 'main');
    event(f.client, { subscriptionId: '1-10', value: { [marker]: { message: 'lost' } } });
    expect(current(f.client, '/repo')).toBeNull(); expect(peekVcsStatus(f.client, '/repo')?.refName).toBe('main');
    f.local('1-10', 'delayed-after-end'); expect(current(f.client, '/repo')).toBeNull();
    restartVcsStatus(f.client, '/repo'); const again = f.watch(); f.pending[1]!.answer({ id: '1-20' }); await again;
    expect(current(f.client, '/repo')).toBeNull(); f.local('1-20', 'new'); expect(current(f.client, '/repo')?.status.refName).toBe('new');
  }
});

test('failed subscribe and newer unacknowledged streams cannot lend local authority', async () => {
  const f = fixture(), watching = f.watch(); f.local('1-10', 'provisional'); f.pending[0]!.fail(new Error('gone')); await watching;
  expect(current(f.client, '/repo')).toBeNull(); expect(peekVcsStatus(f.client, '/repo')?.refName).toBe('provisional');
  const g = fixture(), subscribed = g.watch(); g.pending[0]!.answer({ id: '1-10' }); await subscribed; g.local('1-10', 'known');
  g.local('1-20', 'replacement'); expect(current(g.client, '/repo')).toBeNull();
  g.local('1-10', 'old'); expect(current(g.client, '/repo')).toBeNull();
});

test('closing the stream and client isolation do not leak evidence or add native requests', async () => {
  const f = fixture(), g = fixture(), watching = f.watch(); f.pending[0]!.answer({ id: '1-10' }); await watching; f.local('1-10', 'main');
  expect(current(g.client, '/repo')).toBeNull();
  for (let i = 0; i < 5; i++) current(f.client, '/repo'); expect(f.requests).toHaveLength(1);
  await f.watch(''); expect(current(f.client, '/repo')).toBeNull(); expect(peekVcsStatus(f.client, '/repo')?.refName).toBe('main');
  expect(f.requests.map(request => request.op)).toEqual(['subscribe', 'unsubscribe']);
});
