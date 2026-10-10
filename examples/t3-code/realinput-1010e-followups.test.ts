// realinput-1010e-followups: what did not pass in the real-input session realinput-1010e. RE-3 (the chip press) is in
// macos/tests/composer (chippress.swift), RE-1 and RE-2 (the permission helper) in macos/tests/snapshot, RE-5's error text
// in macos/tests/transport; these check RE-4's focus return in the Contract sources (as realinput-1010c-fixes.test.ts does)
// and RE-5's routing through the Code tab's resource, with a focused server and a paired one.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { pullRequestDetail } from './pages-pr-detail';
import { pullRequestRouter } from './pages-pr-routing';

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

function setup() {
  const sent: { at: string; op: string; method: string }[] = [];
  const entry: FleetEntry = { key: KEY_B, origin: B, environmentId: 'env-lane', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {},
    config: { environment: { label: 'GitHub lane', capabilities: { pullRequests: true } } }, scopes: [], error: '', requested: true,
    shell: { ...initialShell(), projects: [{ id: 'p-lane', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }] } };
  fleet.entries.set(KEY_B, entry);
  const answer = (op: string, method: string): unknown => {
    if (op === 'prDiff') return { patch, truncated: false, nextCursor: null, omittedFileStats: [] };
    if (method === 'pullRequests.filesViewed') return { files: [], truncated: false };
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
    return { ok: true, generation: 4, value: answer(String(request.op), String(request.method ?? '')) };
  } } as unknown as Native;
  return { client, native, sent };
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
});
