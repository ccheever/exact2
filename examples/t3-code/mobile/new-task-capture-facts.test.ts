import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind, mobileNewTaskDraftPresentation, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskCaptureFacts } from './new-task-capture-facts';
import { mobileCaptureNewTaskOutbox } from './mobile-outbox-capture';
import { mobileDraftAttachmentRecord } from './draft-attachment-order';
import { mobileComposerAttachmentAction } from './composer-attachments';
import { queuedEditRefreshOrigin } from './queued-edit-origin';
import { type Native, type Files, ClientError } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
import { branchState } from './shared/r4-git-branch';
import { vcsStatusEvent } from './shared/shell-vcs';
import { contextLink, contextId } from './shared/composer-editor-menu';
import { setDraftFiles } from './shared/composer-editor-files';
import { saveTerminalContext, terminalContextReference } from './shared/terminal-integrations';

const now = Date.parse('2026-10-08T05:00:00.000Z'), key = 'new-task:one';
const metadata = { threadId: 't', messageId: 'm', commandId: 'c', createdAt: new Date(now).toISOString() };
const imageId = '11111111-1111-4111-a111-111111111111', fileId = '22222222-2222-4222-a222-222222222222';
async function fixture() {
  const client = new MobileDraftClient();
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  const loader: Native = { available: true, watch() {}, async later(request) { return { ok: true, generation: 4,
    value: obj(request).op === 'status' ? { state: 'disconnected', origin: 'https://server.test', environmentId: 'env', message: '' } : {} }; } };
  const handles = mobileDraftRecoveryHandles(client, loader, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin: 'https://server.test', environmentId: 'env', generation: 4, projectId: 'p',
    connection: 'connected', configLive: true, shellLoaded: true, shellLive: true });
  client.config = { settings: { defaultModelSelection: { instanceId: 'codex', model: 'model' },
    providerInstances: { codex: { driver: 'codex', enabled: true } } }, environment: { capabilities: { attachmentUploads: true,
      fileAttachments: { maxUploadBytes: 1000000 } } }, providers: [{ instanceId: 'codex', driver: 'codex',
        enabled: true, installed: true, auth: { status: 'authenticated' }, models: [{ slug: 'model', isDefault: true }] }] };
  client.shell.projects = [{ id: 'p', title: 'Project', workspaceRoot: '/repo', archivedAt: null }];
  mobileNewTaskDraftCreate(client, { id: 'one', origin: client.origin, environmentId: 'env', projectId: 'p', createdAt: new Date(now - 1000).toISOString() });
  mobileNewTaskDraftBind(client, key, 'flow'); client.local.drafts[key] = 'Task';
  let active = true, preferences: unknown = { planModeEnabled: true };
  let session: Obj = { authenticated: true, permissions: ['orchestration:operate', 'filesystem:read'] };
  let file: Obj = { relativePath: 't3.json', contents: '{}', truncated: false, byteLength: 2 };
  let status: Obj = { origin: client.origin, environmentId: 'env', homeOrigin: client.origin };
  const calls: Obj[] = [], hooks = new Map<string, () => Promise<void>>(), errors = new Map<string, Error>();
  const native: Native = { available: true, watch() {}, async later(request) {
    const input = obj(request), op = String(input.method || input.op); calls.push(input);
    if (hooks.has(op)) await hooks.get(op)!();
    if (errors.has(op)) throw errors.get(op)!;
    let value: unknown;
    if (op === 'status') value = status;
    else if (op === 'mobilePreferences') value = preferences;
    else if (op === 'http' && input.path === '/api/auth/session') value = session;
    else if (op === 'projects.readFile') value = file;
    else if (op === 'subscribeVcsStatus') value = { id: '4-10' };
    else throw new Error(`Unexpected request ${op}`);
    return { ok: true, generation: 4, value };
  } };
  const adapter = () => mobileNewTaskCaptureFacts(client, key, () => active, now);
  const capture = (facts: ReturnType<ReturnType<typeof adapter>['facts']>) => mobileCaptureNewTaskOutbox(mobileNewTaskDraftPresentation(client, key)!, facts, metadata);
  return { client, calls, hooks, errors, native, adapter, capture, setActive(value: boolean) { active = value; },
    setSession(value: Obj) { session = value; }, setPreferences(value: unknown) { preferences = value; }, setFile(value: Obj) { file = value; }, setStatus(value: Obj) { status = value; } };
}

test('actual read preparation uses auth, JSONC project defaults and current VCS only', async () => {
  const f = await fixture(); f.setFile({ contents: '{/* source */"defaultThreadEnvMode":"worktree",}', truncated: false, byteLength: 50 });
  const adapter = f.adapter(); expect(adapter.now).toBe(now); expect(adapter.facts().blockReason).toContain('load');
  await adapter.prepareFacts(f.native);
  expect(f.calls.map(call => call.method || call.op)).toEqual(['status', 'mobilePreferences', 'http', 'projects.readFile', 'subscribeVcsStatus']);
  expect(f.calls[3]).toMatchObject({ generation: 4, payload: { cwd: '/repo', relativePath: 't3.json' } });
  expect(adapter.facts()).toMatchObject({ key, connected: true, canOperate: true, defaultRuntimeMode: 'full-access',
    planPreferenceLoaded: true, planModeEnabled: true, workspace: { mode: 'worktree', currentCheckoutBranch: null } });
  expect(f.capture(adapter.facts())).toMatchObject({ status: 'blocked', reason: 'Select a base branch before creating a worktree.' });
  expect(clientSelections(f.client)).toEqual({ choices: null, workspace: undefined });
});
function clientSelections(client: MobileDraftClient) {
  return { choices: mobileNewTaskDraftStore(client).records[key]!.choices, workspace: client.local.composerControls.contexts[key] };
}

test('legacy and folded project defaults use real scoped precedence including null-to-file', async () => {
  for (const [folded, override, expected] of [[false, undefined, 'worktree'], [true, undefined, 'local'], [false, 'local', 'local'], [false, null, 'worktree']] as const) {
    const f = await fixture(); f.client.shell.projects[0]!.defaultThreadEnvMode = 'worktree';
    Object.assign(obj(f.client.config.settings), { projectSettingsFolded: folded, defaultThreadEnvMode: 'local',
      projectSettingsOverrides: { p: override === undefined ? {} : { defaultThreadEnvMode: override } } });
    f.setFile({ contents: '{"defaultThreadEnvMode":"worktree"}', truncated: false, byteLength: 40 });
    const a = f.adapter(); await a.prepareFacts(f.native); expect(a.facts().workspace.mode).toBe(expected);
  }
});

test('explicit workspace and source full-access runtime/default model do not freeze sparse choices', async () => {
  const f = await fixture(); f.client.local.composerControls.contexts[key] = { envMode: 'worktree', branch: 'base', worktreePath: '' };
  mobileNewTaskDraftStore(f.client).records[key]!.branchChoice = { kind: 'automatic', envMode: 'worktree', branch: 'base', worktreePath: '' };
  Object.assign(obj(f.client.config.settings), { newWorktreesStartFromOrigin: true, projectSettingsOverrides: { p: { defaultRuntimeMode: 'auto' } } });
  branchState(f.client).origin.set(key, false);
  const a = f.adapter(); await a.prepareFacts(f.native);
  expect(a.facts()).toMatchObject({ defaultRuntimeMode: 'auto', selectedModel: { instanceId: 'codex', model: 'model' },
    workspace: { mode: 'worktree', explicitBranch: 'base', startFromOrigin: false } });
  expect(f.capture(a.facts())).toMatchObject({ status: 'ready', record: { runtimeMode: 'auto', creation: { workspaceMode: 'worktree', branch: 'base' } } });
  expect(mobileNewTaskDraftStore(f.client).records[key]!.choices).toBeNull();
});

test('scratch always captures local without stale branch/worktree but retains source resolved origin flag', async () => {
  const f = await fixture(); f.client.config.scratchWorkspaceRoot = '/repo';
  f.client.local.composerControls.contexts[key] = { envMode: 'worktree', branch: 'old', worktreePath: '/old' };
  const a = f.adapter(); await a.prepareFacts(f.native);
  expect(a.facts().workspace).toEqual({ canChoose: false, mode: 'local', worktreePath: null, explicitBranch: null, currentCheckoutBranch: null, startFromOrigin: true });
  expect(f.capture(a.facts())).toMatchObject({ status: 'ready', record: { creation: { workspaceMode: 'local', branch: null, worktreePath: null } } });
});

test('offline retained config queues without auth/file/stream requests; connection is not client.ready', async () => {
  const f = await fixture(); f.client.connection = 'disconnected'; f.client.configLive = false;
  const a = f.adapter(); await a.prepareFacts(f.native);
  expect(f.calls.map(call => call.op)).toEqual(['status', 'mobilePreferences']);
  expect(f.capture(a.facts())).toMatchObject({ status: 'ready', presentation: { queuesInsteadOfStarting: true } });
  const online = await fixture(); online.client.shellLive = false; online.setSession({ authenticated: true, permissions: [] });
  const b = online.adapter(); await b.prepareFacts(online.native);
  expect(b.facts()).toMatchObject({ connected: true, canOperate: false });
  expect(online.capture(b.facts())).toMatchObject({ status: 'blocked', reason: 'This connection cannot start tasks.' });
});

test('auth failure remains connected and denied; file denial skips read; preferences failure gives unloaded Build', async () => {
  const failed = await fixture(); failed.errors.set('http', new Error('auth unavailable'));
  const a = failed.adapter(); await a.prepareFacts(failed.native);
  expect(a.facts()).toMatchObject({ connected: true, canOperate: false }); expect(a.facts().blockReason).toContain('permissions');
  expect(failed.calls.some(call => call.method === 'projects.readFile')).toBe(false);
  const f = await fixture(); f.setSession({ authenticated: true, permissions: ['orchestration:operate'] });
  f.errors.set('mobilePreferences', new Error('preferences unavailable'));
  mobileNewTaskDraftStore(f.client).records[key]!.choices = { interactionMode: 'plan' };
  const b = f.adapter(); await b.prepareFacts(f.native);
  expect(b.facts()).toMatchObject({ planPreferenceLoaded: false, planModeEnabled: false });
  expect(f.calls.some(call => call.method === 'projects.readFile')).toBe(false);
  expect(f.capture(b.facts())).toMatchObject({ status: 'ready', record: { interactionMode: 'default' } });
  expect(mobileNewTaskDraftStore(f.client).records[key]!.choices?.interactionMode).toBe('plan');
});

test('missing, malformed, oversized and truncated project files are absent source defaults', async () => {
  for (const file of [ { contents: '{"defaultThreadEnvMode":"worktree"}', byteLength: 40, truncated: true },
    { contents: '{"defaultThreadEnvMode":"worktree","scripts":[{}]}', byteLength: 60, truncated: false },
    { contents: '{"defaultThreadEnvMode":"worktree"}', byteLength: 1048577, truncated: false },
    { contents: '{}', byteLength: -1, truncated: false }, { contents: '{}', byteLength: 2 } ]) {
    const f = await fixture(); f.setFile(file); const a = f.adapter(); await a.prepareFacts(f.native); expect(a.facts().workspace.mode).toBe('local');
  }
  const f = await fixture(); f.errors.set('projects.readFile', new ClientError('Missing file', 'WorkspacePathNotFoundError'));
  const a = f.adapter(); await a.prepareFacts(f.native); expect(a.facts().workspace.mode).toBe('local');
});

test('facts observe new exact local VCS data after prepare but never listRefs or remote defaults', async () => {
  const f = await fixture(), a = f.adapter(); await a.prepareFacts(f.native);
  event('remoteUpdated'); expect(a.facts().workspace.currentCheckoutBranch).toBeNull();
  event('snapshot', 'fresh'); expect(a.facts().workspace.currentCheckoutBranch).toBe('fresh');
  event('localUpdated', 'external-switch'); expect(a.facts().workspace.currentCheckoutBranch).toBe('external-switch');
  event('localUpdated', null); expect(a.facts().workspace.currentCheckoutBranch).toBeNull();
  function event(tag: string, refName?: string | null) { vcsStatusEvent(f.client, { subscriptionId: '4-10', value: { _tag: tag,
    local: { isRepo: true, refName }, remote: { aheadCount: 2 } } }); }
});

test('unknown upload references remain blocked while staged image/file order and context survive', async () => {
  const f = await fixture(); f.client.local.snapshotDrafts[key] = [{ id: imageId, name: 'photo.png', mimeType: 'image/png', sizeBytes: 4 }];
  setDraftFiles(f.client.local, [{ id: fileId, contextId: 'file_one', draftKey: key, environmentId: 'env', name: 'clip.mp4',
    mimeType: 'video/mp4', sizeBytes: 4, source: 'attached', attachmentId: '', status: 'staged' }]);
  mobileDraftAttachmentRecord(f.client, key, [fileId, imageId]);
  f.client.local.drafts[key] = `Task ${contextLink('file', 'file_one', 'clip')} ${contextLink('image', contextId('image', imageId), 'photo')}`;
  const a = f.adapter(); await a.prepareFacts(f.native);
  const ready = f.capture(a.facts()); expect(ready.status).toBe('ready');
  if (ready.status === 'ready') { expect(ready.record.attachments.map(file => file.id)).toEqual([fileId, imageId]);
    expect((ready.record.context!.records as Obj[]).map(record => record.attachmentId)).toEqual([fileId, imageId]); }
  f.client.local.snapshotDrafts[key]![0]!.uploadId = 'old-unowned';
  const b = f.adapter(); await b.prepareFacts(f.native);
  expect(b.facts().uploadOwners).toEqual({}); expect(b.facts().uploadStates).toEqual({});
  expect(f.capture(b.facts())).toMatchObject({ status: 'blocked', reason: expect.stringContaining('upload owner') });
  expect(f.client.local.snapshotDrafts[key]![0]!.uploadId).toBe('old-unowned');
});

test('durable selected terminal content resolves while thread chips cannot borrow current-shell ownership', async () => {
  const f = await fixture(), terminal = { id: 'selection', threadId: 'old-thread', createdAt: new Date(now).toISOString(), terminalId: 'term', terminalLabel: 'Shell', lineStart: 1, lineEnd: 2, text: 'captured output' };
  saveTerminalContext(f.client, terminal); const ref = terminalContextReference(terminal);
  f.client.local.drafts[key] = `Task ${contextLink(ref.kind, ref.contextId, ref.label)}`;
  const a = f.adapter(); await a.prepareFacts(f.native);
  expect(f.capture(a.facts())).toMatchObject({ status: 'ready', record: { context: { records: [{ kind: 'terminal', text: 'captured output' }] } } });
  f.client.local.drafts[key] = `Task ${contextLink('thread', 'thread_equal-id', 'Other environment')}`;
  f.client.shell.threads = [{ id: 'equal-id', projectId: 'p', title: 'Current environment' }];
  const b = f.adapter(); await b.prepareFacts(f.native); expect(b.facts().blockReason).toContain('saved context');
});

test('owner changes at each native preparation await cannot publish facts', async () => {
  for (const op of ['status', 'mobilePreferences', 'http', 'projects.readFile', 'subscribeVcsStatus']) {
    const f = await fixture(), a = f.adapter(); f.hooks.set(op, async () => { f.client.threadEpoch++; });
    await expect(a.prepareFacts(f.native)).rejects.toMatchObject({ kind: 'superseded' });
    expect(a.current()).toBe(false); expect(() => a.facts()).toThrow();
  }
  for (const change of [(f: Awaited<ReturnType<typeof fixture>>) => { f.client.origin = 'https://replacement.test'; },
    (f: Awaited<ReturnType<typeof fixture>>) => { f.client.generation++; }, (f: Awaited<ReturnType<typeof fixture>>) => { f.client.connection = 'disconnected'; },
    (f: Awaited<ReturnType<typeof fixture>>) => { obj(f.client.config.settings).defaultRuntimeMode = 'auto'; },
    (f: Awaited<ReturnType<typeof fixture>>) => { f.client.shell.projects[0]!.workspaceRoot = '/other'; },
    (f: Awaited<ReturnType<typeof fixture>>) => { f.client.local.drafts[key] = 'Changed'; }, (f: Awaited<ReturnType<typeof fixture>>) => f.setActive(false)]) {
    const f = await fixture(), a = f.adapter(); f.hooks.set('http', async () => change(f));
    await expect(a.prepareFacts(f.native)).rejects.toMatchObject({ kind: 'superseded' }); expect(a.current()).toBe(false);
  }
});

test('let-go in tolerated preference/file/VCS reads is never swallowed or followed by another read', async () => {
  for (const op of ['mobilePreferences', 'projects.readFile', 'subscribeVcsStatus']) {
    const f = await fixture(), a = f.adapter(); f.errors.set(op, Object.assign(new Error('gone'), { name: 'FetchError', kind: 'Aborted' }));
    await expect(a.prepareFacts(f.native)).rejects.toMatchObject({ kind: 'superseded' }); expect(a.current()).toBe(false);
    expect(String(f.calls.at(-1)!.method || f.calls.at(-1)!.op)).toBe(op);
  }
});

test('canonical home origin is verified without replacing transport selection', async () => {
  const f = await fixture(); f.setStatus({ origin: f.client.origin, environmentId: 'env', homeOrigin: 'https://home.test' });
  await queuedEditRefreshOrigin(f.native, f.client); mobileNewTaskDraftStore(f.client).records[key]!.origin = 'https://home.test';
  const a = f.adapter(); await a.prepareFacts(f.native); expect(a.facts().origin).toBe('https://home.test'); expect(f.client.origin).toBe('https://server.test');
  const g = await fixture(), b = g.adapter(); g.setStatus({ origin: g.client.origin, environmentId: 'env', homeOrigin: 'https://wrong.test' });
  await expect(b.prepareFacts(g.native)).rejects.toMatchObject({ kind: 'superseded' });
});

test('missing config/project and actual busy/pending state stay blockers', async () => {
  for (const change of [(f: Awaited<ReturnType<typeof fixture>>) => { f.client.config = {}; },
    (f: Awaited<ReturnType<typeof fixture>>) => { f.client.shell.projects = []; }, (f: Awaited<ReturnType<typeof fixture>>) => { f.client.busy = true; },
    (f: Awaited<ReturnType<typeof fixture>>) => { f.client.local.pending.env = { method: 'orchestration.launchThread', payload: {}, description: 'pending', threadId: 'other', text: 'A', uncertain: true }; }]) {
    const f = await fixture(); change(f); const a = f.adapter(); await a.prepareFacts(f.native);
    expect(a.facts().blockReason).toBeTruthy(); expect(f.capture(a.facts()).status).toBe('blocked');
  }
});


test('source NewTask keeps project-root VCS and drops an inconsistent saved worktree path in new-worktree mode', async () => {
  const f = await fixture();
  f.client.local.composerControls.contexts[key] = { envMode: 'worktree', branch: 'base', worktreePath: '/existing' };
  mobileNewTaskDraftStore(f.client).records[key]!.branchChoice = { kind: 'explicit', envMode: 'worktree', branch: 'base', worktreePath: '/existing' };
  const a = f.adapter(); await a.prepareFacts(f.native);
  expect(f.calls.find(call => call.method === 'subscribeVcsStatus')?.payload).toEqual({ cwd: '/repo' });
  expect(f.capture(a.facts())).toMatchObject({ status: 'ready', record: { creation: { workspaceMode: 'worktree', worktreePath: null, branch: 'base', startFromOrigin: true } } });
});

test('stale bound key cannot derive a different project or environment facts', async () => {
  for (const field of ['projectId', 'environmentId'] as const) {
    const f = await fixture(); f.client[field] = 'other'; const a = f.adapter();
    expect(a.current()).toBe(false); await expect(a.prepareFacts(f.native)).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.calls).toHaveLength(0);
  }
});

test('real attachment picker state blocks capture until the picker completes', async () => {
  const f = await fixture(); let release!: (value: unknown) => void;
  const picker: Native = { available: true, watch() {}, later() { return new Promise(resolve => { release = resolve; }); } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new Uint8Array(); }, async atomicWriteFile() {} } };
  const picking = mobileComposerAttachmentAction('photos', '', picker, files, f.client);
  const a = f.adapter(); await a.prepareFacts(f.native); expect(a.facts().blockReason).toContain('composer operation');
  release({ ok: true, generation: 4, value: { files: [] } }); await picking;
  expect(a.facts().blockReason).toBeUndefined();
});


test('native generation replacement cannot become an absent file or tolerated stream error', async () => {
  for (const op of ['status', 'http', 'projects.readFile', 'subscribeVcsStatus']) {
    const f = await fixture(), a = f.adapter();
    const native: Native = { ...f.native, async later(request) {
      const response = obj(await f.native.later(request)), input = obj(request);
      return String(input.method || input.op) === op ? { ...response, generation: 5 } : response;
    } };
    await expect(a.prepareFacts(native)).rejects.toMatchObject({ kind: 'superseded' });
    expect(a.current()).toBe(false); expect(String(f.calls.at(-1)!.method || f.calls.at(-1)!.op)).toBe(op);
  }
});

// Server bounds raw file bytes before replacing malformed UTF-8 on decode.
test('project file decoded replacement bytes preserve source workspace defaults', async () => {
  const prefix = new TextEncoder().encode('{"defaultThreadEnvMode":"worktree","unknown":"');
  const suffix = new TextEncoder().encode('"}');
  const bytes = new Uint8Array(prefix.length + 400000 + suffix.length);
  bytes.set(prefix); bytes.fill(0xff, prefix.length, bytes.length - suffix.length); bytes.set(suffix, bytes.length - suffix.length);
  const contents = new TextDecoder().decode(bytes);
  expect(bytes.length).toBeLessThan(1024 * 1024);
  expect(new TextEncoder().encode(contents).length).toBeGreaterThan(1024 * 1024);
  const f = await fixture(); f.setFile({ contents, byteLength: bytes.length, truncated: false });
  const adapter = f.adapter(); await adapter.prepareFacts(f.native);
  expect(adapter.facts().workspace.mode).toBe('worktree');
});
