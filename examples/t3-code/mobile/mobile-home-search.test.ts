import { expect, test } from 'bun:test';
import { mobileHomeSearchMatches, mobileHomeSearchPending, runMobileHomeSearch } from './mobile-home-search';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';

function fixture() {
  const client = new T3Client(), background = new EnvironmentFleet();
  Object.assign(client, { environmentId: 'one', origin: 'https://one.test', generation: 3,
    connection: 'connected', configLive: true, shellLive: true, shellLoaded: true });
  const remote: FleetEntry = { key: 'remote', environmentId: 'two', origin: 'https://two.test', generation: 8,
    synchronized: 8, phase: 'connected', message: '', traceId: '', lastEvent: 0, subscriptions: {}, config: {},
    shell: initialShell(), scopes: [], error: '', requested: true };
  background.entries.set(remote.key, remote);
  return { client, background };
}

function response(request: Obj, matches: Obj[]): Obj {
  return { ok: true, generation: request.fleet === 'remote' ? 8 : 3, value: { matches } };
}

test('Home search publishes each environment as it answers and keeps the query pending', async () => {
  const f = fixture(); f.client.query = 'needle'; let releaseRemote!: () => void;
  const remoteReady = new Promise<void>(resolve => { releaseRemote = resolve; });
  let notifications = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    if (request.op !== 'request' || request.method !== 'orchestration.searchThreads') return response(request, []);
    if (request.fleet === 'remote') await remoteReady;
    return response(request, request.fleet === 'remote'
      ? [{ threadId: 'shared', snippet: 'Remote needle', source: 'assistant' }]
      : [{ threadId: 'local', snippet: 'Local needle', source: 'user' }]);
  } };
  const pending = runMobileHomeSearch(f.client, native, 'needle', f.background, () => { notifications++; });
  await new Promise(resolve => setTimeout(resolve, 0));
  expect([...mobileHomeSearchMatches(f.client, true).keys()]).toEqual(['one:local']);
  expect(mobileHomeSearchPending(f.client)).toBe(true);
  releaseRemote(); await pending;
  expect([...mobileHomeSearchMatches(f.client, true).keys()]).toEqual(['one:local', 'two:shared']);
  expect(mobileHomeSearchPending(f.client)).toBe(false);
  expect(notifications).toBe(2);
});

test('a related Home query keeps the prior matches while its refined answer is pending', async () => {
  const f = fixture(); f.background.entries.clear(); f.client.query = 'needle';
  let queryMatches = true, release!: () => void;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    if (request.op !== 'request' || request.method !== 'orchestration.searchThreads') return response(request, []);
    if (!queryMatches) await new Promise<void>(resolve => { release = resolve; });
    return response(request, queryMatches ? [{ threadId: 'thread', snippet: 'Needle result', source: 'assistant' }] : []);
  } };
  await runMobileHomeSearch(f.client, native, 'needle');
  expect([...mobileHomeSearchMatches(f.client, true).keys()]).toEqual(['one:thread']);
  queryMatches = false; f.client.query = 'needle extra';
  const pending = runMobileHomeSearch(f.client, native, 'needle extra');
  await Promise.resolve(); await Promise.resolve();
  expect([...mobileHomeSearchMatches(f.client, true).keys()]).toEqual(['one:thread']);
  expect(mobileHomeSearchPending(f.client)).toBe(true);
  release(); await pending;
  expect(mobileHomeSearchMatches(f.client, true).size).toBe(0);
  expect(mobileHomeSearchPending(f.client)).toBe(false);
});

test('an unrelated Home query does not display stale search rows', async () => {
  const f = fixture(); f.background.entries.clear(); f.client.query = 'needle';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    return response(request, [{ threadId: 'thread', snippet: 'Needle result', source: 'assistant' }]);
  } };
  await runMobileHomeSearch(f.client, native, 'needle');
  f.client.query = 'other';
  let release!: () => void;
  const pending = runMobileHomeSearch(f.client, { ...native, later: async input => {
    await new Promise<void>(resolve => { release = resolve; }); return response(obj(input), []);
  } }, 'other');
  await Promise.resolve(); await Promise.resolve();
  expect(mobileHomeSearchMatches(f.client, true).size).toBe(0);
  expect(mobileHomeSearchPending(f.client)).toBe(true);
  release(); await pending;
});
