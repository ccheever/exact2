// Pull request routing through the clone's servers (pr-links-previews-and-routing): T3Client.rpc hands every
// `pullRequests.*` request to the router (pages-pr-routing.ts) over the focused connection and the background
// transports (pages-pr-environments.ts), with each server's GitHub sharing read from Settings › Connections' stored
// preference under its current saved key. The router's own decisions are ported in pages-pr-routing.test.ts; these
// drive the clone's wiring: which transport each request went to, with which fields.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { noteBalancePrefs } from './auto-balance';
import { singleRouteKey } from './connection-routes';
import { prEnvironments, routedPullRequestRequest } from './pages-pr-environments';
import { pullRequestRouter } from './pages-pr-routing';

const B = 'http://127.0.0.1:16361';
const KEY_B = `${B}\nenv-b`;
const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 };
const identity = { accountId: 'acct-1', host: 'github.com', provider: 'github', viewer: 'lane-primary', projectTitle: 'sandbox', workspaceRoot: '/a/sandbox' };
type Call = { at: string; method: string; payload: Obj; timeout?: number };

function setup(originA: string, sharing: { a: string; b: string }, answers: { a?: (method: string, payload: Obj) => Obj; b?: (method: string, payload: Obj) => Obj } = {}) {
  const entry: FleetEntry = { key: KEY_B, origin: B, environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {},
    config: { environment: { label: 'Lane B', capabilities: { pullRequests: true } } }, scopes: [], error: '', requested: true, shell: initialShell() };
  fleet.entries.set(KEY_B, entry);
  fleet.saved = [{ origin: originA, environmentId: 'env-a', enabled: true }, { origin: B, environmentId: 'env-b', enabled: true }];
  const calls: Call[] = [];
  const answerA = answers.a ?? ((method: string) => (method === 'pullRequests.routing' ? identity : { projectId: 'p1', title: 'from A' }));
  const answerB = answers.b ?? ((method: string) => (method === 'pullRequests.routingIdentity' ? { accountId: 'acct-1', host: 'github.com', provider: 'github', viewer: 'lane-primary' } : { projectId: 'b1', title: 'from B' }));
  const client = {
    environmentId: 'env-a', origin: originA, connection: 'connected', statusMessage: '', scopes: [], ready: true, generation: 1, config: { environment: { label: 'Lane A', capabilities: { pullRequests: true } } },
    shell: { projects: [], threads: [] },
    async request(_native: unknown, method: string, payload: Obj) { calls.push({ at: 'A', method, payload }); return answerA(method, payload); },
    async call(_native: unknown, request: Obj) { calls.push({ at: 'A', method: String(request.method), payload: request.payload as Obj, timeout: Number(request.timeout) }); return answerA(String(request.method), request.payload as Obj); },
  } as unknown as T3Client;
  noteBalancePrefs(client, { loadBalancingEnabled: false, loadBalancingWeights: {}, githubRouting: {
    ...(sharing.a !== 'off' ? { [singleRouteKey(originA, 'env-a')]: sharing.a } : {}), ...(sharing.b !== 'off' ? { [singleRouteKey(B, 'env-b')]: sharing.b } : {}) } });
  const native = { available: true, watch: () => {}, later: async (request: Obj) => {
    if (request.fleet !== KEY_B) return { ok: true, generation: 1, value: {} };
    calls.push({ at: 'B', method: String(request.method), payload: request.payload as Obj, timeout: Number(request.timeout) });
    try { return { ok: true, generation: 4, value: answerB(String(request.method), request.payload as Obj) }; }
    catch (error) { return { ok: false, generation: 4, error: { kind: error instanceof ClientError ? error.kind : 'transport', message: (error as Error).message, uncertain: false } }; }
  } } as unknown as Native;
  return { client, native, calls };
}
beforeEach(() => pullRequestRouter.reset());
afterEach(() => { fleet.entries.delete(KEY_B); fleet.saved = []; });
const at = (calls: Call[], method: string) => calls.filter(call => call.method === method).map(call => call.at);

describe('the servers the router sees (prEnvironments)', () => {
  test('each server\'s GitHub sharing is the stored preference under its saved key; a loopback origin is local', () => {
    const { client } = setup('http://10.0.0.5:3773', { a: 'read', b: 'read-write' });
    expect(prEnvironments(client).map(environment => [environment.id, environment.permission, environment.local, environment.connected])).toEqual([
      ['env-a', 'read', false, true], ['env-b', 'read-write', true, true]]);
  });
  test('an address that changed since sharing was set reads "off" (trust belongs to the saved endpoint)', () => {
    const { client } = setup('http://10.0.0.5:3773', { a: 'read', b: 'read-write' });
    fleet.saved = [{ origin: 'http://10.0.0.5:3773', environmentId: 'env-a' }, { origin: 'http://127.0.0.1:16999', environmentId: 'env-b' }];
    expect(prEnvironments(client).find(environment => environment.id === 'env-b')?.permission).toBe('off');
  });
});

describe('reads and writes land on the server the reader shares GitHub with', () => {
  test('a remote origin reads through the local server that holds the same account, with host and expectedAccountId', async () => {
    const { client, native, calls } = setup('http://10.0.0.5:3773', { a: 'read', b: 'read' });
    const answer = await routedPullRequestRequest(client, native, 'pullRequests.detail', ref, false);
    expect(answer).toMatchObject({ title: 'from B', projectId: 'p1', projectTitle: 'sandbox', workspaceRoot: '/a/sandbox' });
    expect(calls.find(call => call.method === 'pullRequests.routing')).toMatchObject({ at: 'A', timeout: 2 });
    expect(calls.find(call => call.method === 'pullRequests.routingIdentity')).toMatchObject({ at: 'B', payload: { host: 'github.com' }, timeout: 2 });
    expect(calls.find(call => call.method === 'pullRequests.detail')).toMatchObject({ at: 'B', payload: { ...ref, allowStale: false, host: 'github.com', expectedAccountId: 'acct-1' } });
  });
  test('with sharing off on either side nothing is probed and the origin answers', async () => {
    for (const sharing of [{ a: 'off', b: 'read-write' }, { a: 'read-write', b: 'off' }]) {
      pullRequestRouter.reset();
      const { client, native, calls } = setup('http://10.0.0.5:3773', sharing);
      expect(await routedPullRequestRequest(client, native, 'pullRequests.detail', ref, false)).toMatchObject({ title: 'from A' });
      expect(calls.map(call => `${call.at} ${call.method}`)).toEqual(['A pullRequests.detail']);
      fleet.entries.delete(KEY_B);
    }
  });
  test('a write with one side only "read" stays on the origin', async () => {
    const { client, native, calls } = setup('http://10.0.0.5:3773', { a: 'read-write', b: 'read' });
    await routedPullRequestRequest(client, native, 'pullRequests.comment', { ...ref, body: 'Looks good.' }, true);
    expect(at(calls, 'pullRequests.comment')).toEqual(['A']);
    expect(calls.some(call => call.at === 'B')).toBe(false);
  });
  test('with both sides "read-write" a write goes to the local server first and invalidates the origin after', async () => {
    const { client, native, calls } = setup('http://10.0.0.5:3773', { a: 'read-write', b: 'read-write' });
    await routedPullRequestRequest(client, native, 'pullRequests.comment', { ...ref, body: 'Looks good.' }, true);
    expect(calls.find(call => call.method === 'pullRequests.comment')).toMatchObject({ at: 'B', payload: { body: 'Looks good.', host: 'github.com', expectedAccountId: 'acct-1' } });
    expect(at(calls, 'pullRequests.invalidate')).toContain('A');
  });
  test('an identity mismatch is refused before dispatch: the other account\'s server is never written to', async () => {
    const { client, native, calls } = setup('http://10.0.0.5:3773', { a: 'read-write', b: 'read-write' }, {
      b: method => (method === 'pullRequests.routingIdentity' ? { accountId: 'acct-2', host: 'github.com', provider: 'github', viewer: 'someone-else' } : { title: 'from B' }) });
    await routedPullRequestRequest(client, native, 'pullRequests.comment', { ...ref, body: 'Looks good.' }, true);
    expect(at(calls, 'pullRequests.comment')).toEqual(['A']);
  });
  test('a row a background server listed is read there, and its marker never reaches the wire', async () => {
    const { client, native, calls } = setup('http://10.0.0.5:3773', { a: 'off', b: 'off' });
    await routedPullRequestRequest(client, native, 'pullRequests.activity', { ...ref, projectId: 'b1', environmentId: 'env-b' }, false);
    expect(calls.map(({ at, method, payload }) => ({ at, method, payload }))).toEqual([{ at: 'B', method: 'pullRequests.activity', payload: { ...ref, projectId: 'b1' } }]);
  });
});
