// Lane r10-connect: a first connection made from Add environment toasts and leaves Settings alone,
// a failed one leaves no trace of the failed origin, and PullRequestThreadDialog debounces its
// lookup by 450 ms with "Resolving pull request..." drawn while it waits.
import { describe, expect, test } from 'bun:test';
import { runConnectionOp } from './connections';
import { fleet } from './settings-b-fleet';
import { toasts } from './toast';
import { obj, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { checkoutLocal, checkoutView, checkoutState } from './r9-connect-checkout';

class Transport implements Native {
  available = true; calls: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const ok = (value: unknown) => ({ ok: true, generation: 1, value });
    if (request.fleet) return ok({});
    if (request.op === 'environments') return ok({ saved: [] });
    if (request.op === 'connect') {
      if (request.credential === 'WRONGCODE') return { ok: false, generation: 1, error: { kind: 'Authentication', message: 'The environment credential is invalid.', uncertain: false } };
      return ok({ state: 'connected', origin: request.origin, environmentId: 'env-b', message: '' });
    }
    if (request.op === 'disconnect') return ok({ state: 'disconnected', origin: request.abandon ? '' : 'http://127.0.0.1:14987', environmentId: '', message: 'Disconnected.' });
    return { ok: false, generation: 1, error: { kind: 'Arguments', message: 'Unknown native operation.', uncertain: false } };
  }
}
const disconnected = () => ({ origin: 'http://127.0.0.1:3773', environmentId: '', connection: 'disconnected', local: {} } as unknown as T3Client);

describe('Add environment with nothing connected', () => {
  test('a successful pairing toasts "Backend added", as handleAddSavedBackend does beside a connection', async () => {
    const native = new Transport(), client = disconnected();
    const result = await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14987', 'GOODCODE', false, client);
    expect(obj(result.status).state).toBe('connected');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([
      ['success', 'Backend added', 'The environment is saved and will reconnect on app startup.']]);
    fleet.entries.clear();
  });
  test('a failed pairing drops the failed origin from the transport and the client', async () => {
    const native = new Transport(), client = disconnected();
    // The transport's status named the attempted origin while it tried (t3.status adoption).
    const attempt = runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14987', 'WRONGCODE', false, client);
    client.origin = 'http://127.0.0.1:14987';
    await expect(attempt).rejects.toThrow('The environment credential is invalid.');
    expect(native.calls.find(call => call.op === 'disconnect')).toMatchObject({ forget: false, abandon: true });
    expect(client.origin).toBe('http://127.0.0.1:3773');
    expect(JSON.stringify(client)).not.toContain('14987');
    expect(toasts(client).map(toast => toast.title)).toEqual(['Could not add backend']);
    fleet.entries.clear();
  });
  // settings-rows-and-labels S2-8: the reference's renderer escapes the spaces (https://not%20a%20url/) and its request
  // fails at the transport; nothing reaches the native transport here.
  test('a host with spaces fails as the reference request does, after the pairing code check', async () => {
    const message = 'Failed to fetch remote environment endpoint https://not%20a%20url/.well-known/t3/environment (HttpClientError: Transport error (GET https://not%20a%20url/.well-known/t3/environment)).';
    for (const connected of [true, false]) {
      const native = new Transport(), client = disconnected();
      await expect(runConnectionOp(native, 'environment-add', 'not a url', 'ABC', connected, client)).rejects.toThrow(message);
      expect(native.calls).toEqual([]);
      expect(toasts(client).map(toast => [toast.title, toast.description])).toEqual([['Could not add backend', message]]);
    }
    await expect(runConnectionOp(new Transport(), 'environment-add', 'not a url', '', true, disconnected())).rejects.toThrow('Enter a pairing code.');
    fleet.entries.clear();
  });
});

// A module whose sleeps end when the test says so; wakes are counted.
class Clock implements Native {
  available = true; ops: string[] = []; sleeps: (() => void)[] = [];
  watch() {}
  later(input: unknown): Promise<unknown> {
    const request = obj(input); this.ops.push(String(request.op));
    if (request.op === 'timelineSleep') return new Promise(done => this.sleeps.push(() => done({ ok: true, generation: 0, value: {} })));
    return Promise.resolve({ ok: true, generation: 0, value: {} });
  }
  async elapse() { const due = this.sleeps.splice(0); due.forEach(done => done()); await new Promise(done => setTimeout(done, 0)); }
}
function dialogClient() {
  const lookups: string[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, draftKey: 'env:new:p1',
    local: { composerControls: { contexts: {}, draftThreads: {} } },
    shell: { projects: [{ id: 'p1', title: 'pr-demo', workspaceRoot: '/repos/pr-demo' }], threads: [] },
    restAccess: () => ({ request: async (method: string, payload: Obj = {}) => {
      lookups.push(String(payload.reference));
      if (payload.reference === '999') throw new ClientError('Pull request not found.', 'Network');
      return { pullRequest: { number: Number(payload.reference), title: `PR ${payload.reference}`, baseBranch: 'main', headBranch: 'feature', state: 'open' } };
    } }),
  } as unknown as T3Client;
  return { client, lookups };
}

const tick = () => new Promise(done => setTimeout(done, 0));
describe('PullRequestThreadDialog shows its lookup after a 450 ms debounce (HEAD build, measured)', () => {
  test('an edit is looked up at once, but "Resolving pull request..." stays until the 450 ms pass', async () => {
    const { client, lookups } = dialogClient(), clock = new Clock();
    await checkoutLocal(client, clock, 'open', '101');
    expect(lookups).toEqual(['101']); // the opening reference resolves at once (debounced value starts there)
    const typing = checkoutLocal(client, clock, 'text', '#102');
    await tick();
    expect(clock.ops).toEqual(['r10Wake', 'timelineSleep']);
    expect(lookups).toEqual(['101', '102']); // the answer is in the cache already...
    expect(checkoutView(client)).toMatchObject({ resolving: true, resolvingLabel: 'Resolving pull request...', prTitle: '', canConfirm: false, settled: '101' });
    await clock.elapse(); await typing; // ...and shows when the wait ends
    expect(clock.ops).toEqual(['r10Wake', 'timelineSleep', 'r10Wake']);
    expect(checkoutView(client)).toMatchObject({ resolving: false, prTitle: 'PR 102', canConfirm: true, settled: '102' });
  });
  test('edits inside the wait each look up; only the last one settles', async () => {
    const { client, lookups } = dialogClient(), clock = new Clock();
    await checkoutLocal(client, clock, 'open', '101');
    const edits = ['#1', '#10', '#103'].map(text => checkoutLocal(client, clock, 'text', text));
    await tick();
    expect(checkoutState(client).edits).toBe(3);
    expect(lookups).toEqual(['101', '1', '10', '103']);
    expect(checkoutView(client)).toMatchObject({ resolving: true, prTitle: '' });
    await clock.elapse(); await Promise.all(edits);
    expect(checkoutView(client)).toMatchObject({ prTitle: 'PR 103', resolving: false, settled: '103' });
  });
  test('a pull request cached before the edit shows at once; a failed lookup’s error stays beside "Resolving" until the next one settles', async () => {
    const { client, lookups } = dialogClient(), clock = new Clock();
    await checkoutLocal(client, clock, 'open', '999');
    expect(checkoutView(client)).toMatchObject({ error: 'Pull request not found.', resolving: false });
    const back = checkoutLocal(client, clock, 'text', '#101');
    await tick();
    expect(checkoutView(client)).toMatchObject({ resolving: true, error: 'Pull request not found.', prTitle: '' });
    await clock.elapse(); await back;
    expect(checkoutView(client)).toMatchObject({ resolving: false, error: '', prTitle: 'PR 101' });
    const again = checkoutLocal(client, clock, 'text', '101');
    await tick();
    expect(checkoutView(client)).toMatchObject({ resolving: false, prTitle: 'PR 101', canConfirm: true });
    await clock.elapse(); await again;
    expect(lookups).toEqual(['999', '101']);
  });
  test('closing during the wait leaves the dialog closed', async () => {
    const { client } = dialogClient(), clock = new Clock();
    await checkoutLocal(client, clock, 'open', '101');
    const typing = checkoutLocal(client, clock, 'text', '#105');
    await tick();
    await checkoutLocal(client, clock, 'close', '');
    await clock.elapse(); await typing;
    expect(checkoutView(client).open).toBe(false);
    expect(checkoutState(client).debounced).toBe('101');
  });
});

describe('Add Environment fills both fields from a pasted pairing URL (parsePairingUrlFields)', () => {
  test('a link with a token splits into its origin and the token; anything else is left alone', async () => {
    const { pairingFields } = await import('./r10-connect-pairing');
    expect(pairingFields('http://127.0.0.1:14987/pair#token=SAMPLE42')).toEqual({ source: 'http://127.0.0.1:14987/pair#token=SAMPLE42', host: 'http://127.0.0.1:14987', code: 'SAMPLE42' });
    expect(pairingFields(' backend.example.com/pair?token=ABC ')).toMatchObject({ host: 'https://backend.example.com', code: 'ABC' });
    expect(pairingFields('https://app.t3.codes/pair?host=https%3A%2F%2Fbox.example.com&token=XYZ#')).toMatchObject({ host: 'https://box.example.com', code: 'XYZ' });
    expect(pairingFields('backend.example.com')).toMatchObject({ host: '', code: '' });
    expect(pairingFields('http://127.0.0.1:14987/pair#token=')).toMatchObject({ host: '' });
    expect(pairingFields('')).toEqual({ source: '', host: '', code: '' });
    expect(pairingFields('http://[bad')).toMatchObject({ host: '' });
  });
});
