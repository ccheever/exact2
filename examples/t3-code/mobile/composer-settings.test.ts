import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { type Native, type Files } from './shared/protocol';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTask, mobileNewTaskAction, mobileNewTaskPrepare } from './new-task';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { draftContext, patchDraftContext } from './shared/composer-controls-branch';

const options = [{ id: 'effort', label: 'Reasoning effort', type: 'select', currentValue: 'medium', promptInjectedValues: ['ultrathink'],
  options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High' }, { id: 'ultracode', label: 'Ultracode' }, { id: 'ultrathink', label: 'Ultrathink' }] }];
function fixture() {
  const client = new T3Client();
  client.environmentId = 'env'; client.projectId = 'p'; client.origin = 'http://example.test'; client.connection = 'connected';
  client.configLive = true; client.shellLive = true; client.threadLive = true; client.shellLoaded = true;
  client.scopes = ['orchestration:operate']; client.providerId = 'provider'; client.modelId = 'a';
  client.config = { environment: { label: 'Machine', capabilities: { serverResolvedCommandContext: true } }, providers: [
    { instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', supportedRuntimeModes: ['approval-required', 'full-access'],
      models: [{ slug: 'a', name: 'Model A', isDefault: true }, { slug: 'b', name: 'Model B', capabilities: { optionDescriptors: options } }, { slug: 'old', name: 'Old model', isLegacy: true }] },
    { instanceId: 'other', driver: 'pi', enabled: true, installed: true, status: 'ready', models: [{ slug: 'remote', name: 'Other model' }] }] };
  client.shell.projects = [{ id: 'p', title: 'Repo', workspaceRoot: '/repo' }, { id: 'p2', title: 'Second', workspaceRoot: '/second' }];
  const calls: Obj[] = []; let next = 0;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value: unknown = request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++next}`) : {};
    return { ok: true, generation: client.generation, value };
  } };
  return { client, calls, native, storage };
}
const settings = (f: ReturnType<typeof fixture>, kind: string, id = '', value = '') => mobileComposerSettingsAction(kind, id, value, f.native, f.storage, f.client);
const task = (f: ReturnType<typeof fixture>, kind: string, id = '', value = '') => mobileNewTaskAction(kind, id, value, f.native, f.storage, f.client);

describe('mobile staged model/settings ownership', () => {
  test('search, disclosure and Cancel do not change applied selection; repeated pick preserves pending options', async () => {
    const f = fixture(); await settings(f, 'open');
    expect(mobileComposerSettings('Model B', '', false, f.client).models.filter(row => row.kind === 'model')).toHaveLength(1);
    await settings(f, 'pick', 'provider', 'b'); await settings(f, 'option', 'effort', 'high'); await settings(f, 'pick', 'provider', 'b');
    expect(f.client.modelId).toBe('a');
    expect(mobileComposerSettings('', '', false, f.client).options[0]?.value).toBe('High');
    await settings(f, 'cancel'); expect(f.client.modelId).toBe('a'); expect(f.calls.some(call => call.op === 'request')).toBe(false);
  });
  test('Save applies model and all options through one shared selection command', async () => {
    const f = fixture(); f.client.threadId = 'thread'; f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { thread: { id: 'thread', projectId: 'p', modelSelection: { instanceId: 'provider', model: 'a' } }, runs: [], turnItems: [] } };
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b'); await settings(f, 'option', 'effort', 'high');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    const writes = f.calls.filter(call => call.method === 'orchestration.dispatchCommand');
    expect(writes).toHaveLength(1); expect(obj(writes[0]?.payload)).toMatchObject({ type: 'thread.model-selection.set', modelSelection: { instanceId: 'provider', model: 'b', options: [{ id: 'effort', value: 'high' }] } });
  });
  test('catalog removal, permission loss and external selection changes refuse Save', async () => {
    const f = fixture(); await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    f.client.scopes = []; expect((await settings(f, 'save')).message).toContain('writable'); expect(f.client.modelId).toBe('a');
    f.client.scopes = ['orchestration:operate']; f.client.modelId = 'old';
    expect((await settings(f, 'save')).message).toContain('changed');
    f.client.modelId = 'a'; obj((f.client.config.providers as Obj[])[0]).models = [{ slug: 'a', name: 'Model A' }];
    expect((await settings(f, 'save')).message).toContain('another model'); expect(f.client.modelId).toBe('a');
  });
  test('mobile filtering keeps provider order, favorites first and hides desktop injected efforts', async () => {
    const f = fixture(); f.client.local.favoriteModels = [JSON.stringify(['provider', 'b'])];
    expect(mobileComposerSettings('', '', false, f.client).models.filter(row => row.kind === 'model').map(row => row.id)).toEqual(['b', 'a']);
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    expect(mobileComposerSettings('', '', false, f.client).options[0]?.choices.map(row => row.id)).toEqual(['medium', 'high']);
    expect((await settings(f, 'option', 'effort', 'ultracode')).message).toContain('not offered');
  });
});

describe('new task shared selections and submissions', () => {
  test('scoped project ids distinguish identical remote ids; project switches preserve each draft', async () => {
    const f = fixture(), fleet = new EnvironmentFleet();
    const remote: FleetEntry = { key: 'remote', environmentId: 'remote', origin: 'http://remote.test', phase: 'connected', generation: 1, synchronized: 1,
      message: '', traceId: '', lastEvent: 0, subscriptions: {}, scopes: [], config: { environment: { label: 'Remote' } }, shell: { sequence: 1, threads: [], projects: [{ id: 'p', title: 'Remote Repo', workspaceRoot: '/repo' }] }, error: '', requested: true };
    fleet.entries.set('remote', remote);
    expect(mobileNewTask('', f.client, fleet).projects.map(project => project.id)).toEqual(['["env","p"]', '["env","p2"]', '["remote","p"]']);
    f.client.local.drafts['env:new:p'] = 'Original'; f.client.local.drafts['env:new:p2'] = 'Other';
    await task(f, 'project', '["env","p2"]'); expect(f.client.projectId).toBe('p2'); expect(f.client.draft).toBe('Other');
    expect(f.client.local.drafts['env:new:p']).toBe('Original');
  });
  test('overlapping draft input preserves latest text while an earlier persistence is pending', async () => {
    const f = fixture(); let release!: () => void, entered!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; });
    const gate = new Promise<void>(resolve => { release = resolve; }); let writes = 0;
    f.storage.fs.atomicWriteFile = async () => { if (++writes === 1) { entered(); await gate; } };
    const first = task(f, 'draft', '', 'first'); await started;
    const second = task(f, 'draft', '', 'latest text');
    expect(f.client.draft).toBe('latest text');
    expect(await second).toMatchObject({ message: '' }); release();
    expect(await first).toMatchObject({ message: '' }); expect(f.client.draft).toBe('latest text');
  });
  test('an edit applies to project A before a synchronous switch to project B', async () => {
    const f = fixture(); f.client.local.drafts['env:new:p2'] = 'B original';
    const edit = task(f, 'draft', '', 'A changed');
    f.client.projectId = 'p2';
    expect(await edit).toMatchObject({ message: '' });
    expect(f.client.local.drafts['env:new:p']).toBe('A changed');
    expect(f.client.draft).toBe('B original');
    expect(f.calls.some(call => call.op === 'devicePresentation' || call.op === 'readPreferences')).toBe(false);
  });
  test('workspace picks are shared contexts and cannot move a started thread', async () => {
    const f = fixture(); await task(f, 'workspace', '', 'worktree'); expect(draftContext(f.client).envMode).toBe('worktree');
    expect(mobileNewTask('', f.client).composer.canSend).toBe(false);
    f.client.threadId = 'started'; expect((await task(f, 'workspace', '', 'local')).message).toContain('keeps its workspace');
  });
  test('permissionless branch checkout is disabled while a new worktree base is selectable', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => {
      const request = obj(input);
      if (request.op === 'http') return { ok: true, generation: f.client.generation, value: { authenticated: true, permissions: [] } };
      if (request.method === 'vcs.refreshStatus') return { ok: true, generation: f.client.generation, value: { isRepo: true, refName: 'main' } };
      if (request.method === 'vcs.listRefs') return { ok: true, generation: f.client.generation, value: { refs: [{ name: 'main', current: true }, { name: 'feature', current: false }], total: 2, nextCursor: null } };
      return original(input);
    };
    await mobileNewTaskPrepare('', f.native, f.client);
    expect(mobileNewTask('', f.client).branches.map(row => [row.id, row.disabled])).toEqual([['main', false], ['feature', true]]);
    expect((await task(f, 'branch', 'feature')).message).toContain('cannot check out');
    patchDraftContext(f.client, { envMode: 'worktree' }); await mobileNewTaskPrepare('', f.native, f.client);
    expect((await task(f, 'branch', 'feature')).message).toBe(''); expect(draftContext(f.client).branch).toBe('feature');
    expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
  });
  test('a changed draft owner before ids return cannot launch and original text survives', async () => {
    const f = fixture(), original = f.native.later; f.client.local.drafts[f.client.draftKey] = 'Create safely';
    f.native.later = async input => { const reply = await original(input); if (obj(input).op === 'ids') f.client.projectId = 'p2'; return reply; };
    const response = await task(f, 'send');
    expect(response.submitted).toBe(false); expect(response.message).toContain('changed before sending');
    expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false); expect(f.client.local.drafts['env:new:p']).toBe('Create safely');
  });
  test('confirmed launch returns real thread identity, clears sent draft and keeps other drafts', async () => {
    const f = fixture(), original = f.native.later; f.client.local.drafts[f.client.draftKey] = 'Ship this'; f.client.local.drafts['env:new:p2'] = 'Keep this';
    f.native.later = async input => {
      const request = obj(input);
      if (request.method === 'orchestration.launchThread') return { ok: true, generation: f.client.generation, value: { threadId: 'server-thread' } };
      if (request.op === 'http' && String(request.path).endsWith('/bounded')) {
        const projection: Obj = { thread: { id: 'server-thread', projectId: 'p', title: 'Ship this', modelSelection: { instanceId: 'provider', model: 'a' } } };
        for (const key of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests', 'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[key] = [];
        return { ok: true, generation: f.client.generation, value: { projection, snapshotSequence: 1, hasMoreHistory: false } };
      }
      return original(input);
    };
    expect(await task(f, 'send')).toMatchObject({ submitted: true, message: '', environmentId: 'env', projectId: 'p', threadId: 'server-thread' });
    expect(f.client.local.drafts['env:new:p']).toBeUndefined(); expect(f.client.local.drafts['env:new:p2']).toBe('Keep this');
  });
  test('uncertain launch belongs to shared pending journal, retains text and refuses a second submission', async () => {
    const f = fixture(), original = f.native.later; f.client.local.drafts[f.client.draftKey] = 'Create safely';
    f.native.later = async input => obj(input).method === 'orchestration.launchThread'
      ? { ok: false, generation: f.client.generation, error: { kind: 'transport', message: 'Lost response', uncertain: true } } : original(input);
    expect(await task(f, 'send')).toMatchObject({ submitted: false, message: 'Lost response' });
    expect(f.client.pending?.uncertain).toBe(true); expect(f.client.draft).toBe('Create safely');
    expect((await task(f, 'send')).message).toContain('current submission');
  });
});
