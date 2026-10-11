// realinput-1010e-followups: what did not pass in the real-input session realinput-1010e. RE-3 (the chip press) is in
// macos/tests/composer (chippress.swift), RE-1 and RE-2 (the permission helper) in macos/tests/snapshot, RE-5's error text
// in macos/tests/transport; these check RE-4's focus return in the Contract sources (as realinput-1010c-fixes.test.ts does)
// and RE-5's routing through the Code tab's resource, with a focused server and a paired one: the diff, the Viewed read,
// a hidden range's contents, the Viewed write (detached, held while the server is away) and its routing with GitHub shared.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { prCodeLocalFor, pullRequestDetail } from './pages-pr-detail';
import { pullRequestRouter } from './pages-pr-routing';
import { usageFleetEvent } from './usage-replies';
import { composerReplyEvent } from './composer-replies';
import { noteBalancePrefs } from './auto-balance';
import { singleRouteKey } from './connection-routes';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

describe('RE-4: the import wizard gives the focus back to Add profile', () => {
  test('however it closes, the focus goes to the Add profile button (Base UI Dialog\'s finalFocus)', async () => {
    const app = await source('app.contract'), profiles = await source('browser-profiles.contract');
    expect(app).toContain('  task importWizardFocus when integrations.browserProfiles.wizard.open != importWizardShown\n    after(1, importWizardFlip)\n  action importWizardFlip\n    if importWizardShown and not integrations.browserProfiles.wizard.open\n      focus("browser-profiles-add")\n    importWizardShown = integrations.browserProfiles.wizard.open\n');
    // `focus` names an element by its `id`: the trigger whose menu opened the wizard.
    expect(profiles).toContain('button id="browser-profiles-add" press=local("browser-profiles-sources", "") popovertarget="browser-profiles-add-menu"');
    expect(profiles.match(/id="browser-profiles-add"/g)).toHaveLength(1);
  });
});

// RE-5. The session's lane copy ran its embedded server (focused, "This machine", no GitHub project) and was paired with
// the GitHub lane's server, where #168 was listed. The detail came from the lane server; the diff and the Viewed reads
// went to the embedded one, which answered 503 PullRequestUnavailableError (provider-unsupported).
const B = 'http://127.0.0.1:16643';
const KEY_B = `${B}\nenv-lane`;
const NOW = Date.parse('2026-10-10T12:00:00Z');
const selected = JSON.stringify({ projectId: 'p-lane', host: 'github.com', repository: 'acme/playground', number: 168, environmentId: 'env-lane' });
const detail = { provider: 'github', projectId: 'p-lane', repository: 'acme/playground', number: 168, title: 'Build the catalog', body: '', url: 'https://github.com/acme/playground/pull/168',
  author: { login: 'primary', name: null, avatarUrl: null }, state: 'open', isDraft: false, changedFiles: 1, additions: 1, deletions: 1, headBranch: 'feature/catalog', baseBranch: 'main',
  createdAt: '2026-10-09T12:00:00Z', updatedAt: '2026-10-09T12:00:00Z', checks: [], labels: [], reviewers: [],
  capabilities: { diff: true, comment: true, reactions: true, viewedFiles: 'host', review: { inlineComment: true, reply: true, resolve: true, verdicts: ['comment'] }, actions: [] },
  viewerPermissions: { comment: true, resolve: true, verdicts: ['comment'], actions: [] } };
const patch = 'diff --git a/src/catalog.js b/src/catalog.js\n--- a/src/catalog.js\n+++ b/src/catalog.js\n@@ -1,2 +1,2 @@\n export const items = [];\n-export const size = 0;\n+export const size = 1;\n';
const contents = (size: number) => `export const items = [];\nexport const size = ${size};\nexport const name = "catalog";\nexport default items;\n`;

function setup() {
  const sent: { at: string; op: string; method: string }[] = [];
  // What the paired server was sent, with its payload and, for a detached write, the key its reply is filed under.
  const lane: { method: string; payload: Obj; deliver: string }[] = [];
  const entry: FleetEntry = { key: KEY_B, origin: B, environmentId: 'env-lane', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {},
    config: { environment: { label: 'GitHub lane', capabilities: { pullRequests: true } } }, scopes: [], error: '', requested: true,
    shell: { ...initialShell(), projects: [{ id: 'p-lane', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }] } };
  fleet.entries.set(KEY_B, entry);
  const answer = (op: string, method: string): unknown => {
    if (op === 'prDiff') return { patch, truncated: false, nextCursor: null, omittedFileStats: [] };
    if (method === 'pullRequests.filesViewed') return { files: [], truncated: false };
    if (method === 'pullRequests.diffFileContents') return { oldContents: contents(0), newContents: contents(1) };
    if (method === 'pullRequests.detail') return detail;
    if (method === 'pullRequests.activity') return { author: detail.author, comments: [], commentCount: 0, commits: [], reviewThreads: [] };
    return {};
  };
  // The focused server: the embedded one, which holds no GitHub project and refuses a pull request read.
  const refuse = () => { throw Object.assign(new Error('Change requests cannot be browsed for this project\'s host yet.'), { kind: 'PullRequestUnavailableError' }); };
  const client = {
    environmentId: 'env-local', origin: 'http://127.0.0.1:16642', connection: 'connected', ready: true, generation: 1, revision: 0, local: {} as Obj, diffState: {}, scopes: [],
    config: { environment: { label: 'This machine', capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p-work', title: 'work' }], threads: [] },
    // The panel's own reads (pullRequests.detail, .activity) already go to the listing server (T3Client.rpc's router).
    rpc: async (_native: unknown, method: string) => { sent.push({ at: 'lane', op: 'request', method }); return answer('request', method); },
    request: async (_native: unknown, method: string) => { sent.push({ at: 'local', op: 'request', method }); return refuse(); },
    call: async (_native: unknown, request: Obj) => { sent.push({ at: 'local', op: String(request.op), method: String(request.method ?? '') }); return refuse(); },
    ids: async () => ['r1'], savePreferences: async () => {},
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => {
    if (request.fleet !== KEY_B) return { ok: true, generation: 1, value: { at: NOW } };
    sent.push({ at: 'lane', op: String(request.op), method: String(request.method ?? '') });
    lane.push({ method: String(request.method ?? request.op), payload: (request.payload ?? {}) as Obj, deliver: String(request.deliver ?? '') });
    // A detached request (a `deliver` key) is only taken here; its reply is filed in the inbox for the fleet's drain.
    return { ok: true, generation: 4, value: request.deliver ? { id: 'sent' } : answer(String(request.op), String(request.method ?? '')) };
  } } as unknown as Native;
  /** The panel's resource asked again until it has nothing left to do (each wake is shown, then asked again). */
  const settle = async () => { let view = await pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code' }); for (let i = 0; i < 12; i++) view = await pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code' }); return view; };
  const press = (op: string, value: string) => prCodeLocalFor(client, native, op, selected, value);
  /** The fleet's drain: the paired server's reply to a detached write lands (usage-replies.ts usageFleetEvent). */
  const land = (deliver: string) => usageFleetEvent({ key: KEY_B, generation: 4 }, { key: deliver, value: { _reply: {} } });
  return { client, native, sent, lane, entry, settle, press, land };
}
beforeEach(() => pullRequestRouter.reset());
afterEach(() => { fleet.entries.delete(KEY_B); fleet.saved = []; });

describe('RE-5: the Code tab reads the pull request on the server it was listed on', () => {
  test('the diff (POST /api/pull-requests/diff) and the Viewed read go to the paired server, none to the focused one', async () => {
    const { client, native, sent } = setup();
    let view = await pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code' });
    for (let i = 0; i < 12 && view.codeTab.phase !== 'files'; i++) view = await pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code' });
    expect(view.codeTab.phase).toBe('files');
    expect(view.codeTab.filesLabel).toBe('1 file');
    expect(sent.filter(call => call.op === 'prDiff')).toEqual([{ at: 'lane', op: 'prDiff', method: '' }]);
    expect(sent.filter(call => call.method === 'pullRequests.filesViewed').map(call => call.at)).toEqual(['lane']);
    expect(sent.filter(call => call.at === 'local')).toEqual([]);
  });
  test('a hidden range\'s file contents (pullRequests.diffFileContents) are read through the paired server', async () => {
    const { sent, lane, settle, press } = setup();
    expect((await settle()).codeTab.phase).toBe('files');
    await press('fold', 'src/catalog.js'); await press('expand', 'src/catalog.js|1');
    const view = await settle();
    expect(lane.filter(call => call.method === 'pullRequests.diffFileContents').map(call => call.payload)).toEqual([
      { projectId: 'p-lane', host: 'github.com', repository: 'acme/playground', number: 168, changeType: 'change', oldPath: 'src/catalog.js', newPath: 'src/catalog.js' }]);
    // Lines 3 and 4, after the hunk, opened from the paired server's contents.
    expect(view.codeTab.items.filter(item => item.path === 'src/catalog.js' && item.kind === 'line').map(item => item.number)).toEqual(['1', '2', '2', '3', '4']);
    expect(sent.filter(call => call.at === 'local')).toEqual([]);
  });
  test('a Viewed tick (pullRequests.setFilesViewed) is sent detached to the paired server, and the marks are read there again once it landed', async () => {
    const { sent, lane, settle, press, land } = setup();
    expect((await settle()).codeTab.phase).toBe('files');
    await press('viewed', 'src/catalog.js|true');
    let view = await settle();
    const writes = lane.filter(call => call.method === 'pullRequests.setFilesViewed');
    expect(writes.map(call => call.payload)).toEqual([{ projectId: 'p-lane', host: 'github.com', repository: 'acme/playground', number: 168, files: [{ path: 'src/catalog.js', viewed: true }] }]);
    expect(writes[0]!.deliver).toStartWith('usage-reply:');
    expect([view.codeTab.viewedQueued, view.codeTab.viewedCount]).toEqual([0, '1 / 1']);
    expect(land(writes[0]!.deliver)).toBe(true);
    await Bun.sleep(0);
    view = await settle();
    expect(lane.filter(call => call.method === 'pullRequests.filesViewed')).toHaveLength(2); // read again after the write
    expect(sent.filter(call => call.at === 'local')).toEqual([]);
  });
  test('a Viewed tick waits while the paired server is not connected, then goes to it', async () => {
    const { sent, lane, entry, settle, press } = setup();
    expect((await settle()).codeTab.phase).toBe('files');
    entry.phase = 'reconnecting';
    await press('viewed', 'src/catalog.js|true');
    let view = await settle();
    // Nothing left: not to the paired server, and not to the focused one in its place.
    expect(lane.filter(call => call.method === 'pullRequests.setFilesViewed')).toEqual([]);
    expect(sent.filter(call => call.at === 'local')).toEqual([]);
    expect([view.codeTab.viewedQueued, view.codeTab.viewedCount]).toEqual([1, '1 / 1']);
    entry.phase = 'connected';
    view = await settle();
    expect(lane.filter(call => call.method === 'pullRequests.setFilesViewed').map(call => call.payload.files)).toEqual([[{ path: 'src/catalog.js', viewed: true }]]);
    expect(view.codeTab.viewedQueued).toBe(0);
  });
});

// RE-5's review: `setFilesViewed` is one of the router's writes (pullRequestRouting.ts `writes`), so it is routed from the
// listing server as the reference routes it, not sent to that server's transport as it is. Here the listing server is a
// remote one and both servers share GitHub ("read and act"): the reference writes through the local server that holds
// the same account (its guard `expectedAccountId`), and tells the readers it routed through once GitHub took the write.
describe('RE-5: the Viewed write is routed as the reference routes it', () => {
  const REMOTE = 'http://10.0.0.7:16643', KEY_R = `${REMOTE}\nenv-lane`, LOCAL = 'http://127.0.0.1:16642';
  const identity = { host: 'github.com', provider: 'github', accountId: 'acct-1', viewer: 'primary' };
  afterEach(() => { fleet.entries.delete(KEY_R); });
  function shared() {
    const calls: { at: string; method: string; payload: Obj }[] = [], delivered: string[] = [];
    fleet.entries.set(KEY_R, { key: KEY_R, origin: REMOTE, environmentId: 'env-lane', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {},
      config: { environment: { label: 'GitHub lane', capabilities: { pullRequests: true } } }, scopes: [], error: '', requested: true,
      shell: { ...initialShell(), projects: [{ id: 'p-lane', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }] } });
    fleet.saved = [{ origin: LOCAL, environmentId: 'env-local', enabled: true }, { origin: REMOTE, environmentId: 'env-lane', enabled: true }];
    const answer = (method: string): unknown => method === 'pullRequests.routing' || method === 'pullRequests.routingIdentity' ? identity
      : method === 'pullRequests.filesViewed' ? { files: [], truncated: false } : method === 'pullRequests.detail' ? detail
      : method === 'pullRequests.activity' ? { author: detail.author, comments: [], commentCount: 0, commits: [], reviewThreads: [] } : {};
    const client = {
      environmentId: 'env-local', origin: LOCAL, connection: 'connected', statusMessage: '', ready: true, generation: 1, revision: 0, local: {} as Obj, diffState: {}, scopes: [],
      config: { environment: { label: 'This machine', capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p-work', title: 'work' }], threads: [] },
      rpc: async (_native: unknown, method: string) => answer(method),
      request: async (_native: unknown, method: string, payload: Obj) => { calls.push({ at: 'local', method, payload }); return answer(method); },
      call: async (_native: unknown, request: Obj) => {
        calls.push({ at: 'local', method: String(request.method ?? request.op), payload: (request.payload ?? {}) as Obj });
        if (request.deliver) { delivered.push(String(request.deliver)); return { id: 'sent' }; }
        return answer(String(request.method));
      },
      ids: async () => [`r${delivered.length + 1}`], savePreferences: async () => {},
    } as unknown as T3Client;
    noteBalancePrefs(client, { loadBalancingEnabled: false, loadBalancingWeights: {}, githubRouting: { [singleRouteKey(LOCAL, 'env-local')]: 'read-write', [singleRouteKey(REMOTE, 'env-lane')]: 'read-write' } });
    const native = { available: true, watch: () => {}, later: async (request: Obj) => {
      if (request.fleet !== KEY_R) return { ok: true, generation: 1, value: { at: NOW } };
      calls.push({ at: 'lane', method: String(request.method ?? request.op), payload: (request.payload ?? {}) as Obj });
      return { ok: true, generation: 4, value: request.op === 'prDiff' ? { patch, truncated: false, nextCursor: null, omittedFileStats: [] } : answer(String(request.method)) };
    } } as unknown as Native;
    const ask = () => pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code' });
    const settle = async () => { let view = await ask(); for (let i = 0; i < 12; i++) view = await ask(); return view; };
    return { client, native, calls, delivered, settle };
  }
  test('through the local server with the same GitHub account, guarded; the readers are told after GitHub took it', async () => {
    const { client, native, calls, delivered, settle } = shared();
    expect((await settle()).codeTab.phase).toBe('files');
    await prCodeLocalFor(client, native, 'viewed', selected, 'src/catalog.js|true');
    await settle();
    const writes = calls.filter(call => call.method === 'pullRequests.setFilesViewed');
    expect(writes).toEqual([{ at: 'local', method: 'pullRequests.setFilesViewed', payload: {
      projectId: 'p-lane', host: 'github.com', repository: 'acme/playground', number: 168, files: [{ path: 'src/catalog.js', viewed: true }], expectedAccountId: 'acct-1' } }]);
    // The probes that chose it: the listing server's identity, then the local server's for that host.
    expect(calls.filter(call => call.method.endsWith('.routing') || call.method.endsWith('.routingIdentity')).map(call => `${call.at} ${call.method}`)).toContain('local pullRequests.routingIdentity');
    expect(calls.filter(call => call.method === 'pullRequests.invalidate')).toEqual([]); // not before the reply
    composerReplyEvent(client, { key: delivered[0]!, value: { _reply: {} } });
    await Bun.sleep(0);
    await settle();
    const told = calls.filter(call => call.method === 'pullRequests.invalidate');
    expect(told.map(call => call.at).sort()).toEqual(['lane', 'local']);
    for (const call of told) expect(call.payload).toEqual({ reference: expect.objectContaining({ projectId: 'p-lane', repository: 'acme/playground', number: 168 }), filesViewedOnly: true });
    // Then the marks are read again (through the same local server).
    expect(calls.filter(call => call.method === 'pullRequests.filesViewed').map(call => call.at)).toEqual(['local', 'local']);
  });
});
