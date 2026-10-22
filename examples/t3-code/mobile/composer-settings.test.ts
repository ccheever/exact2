import { describe, expect, spyOn, test } from 'bun:test';
import { T3Client } from './shared/client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { arr, obj, type Obj } from './shared/domain';
import { type Native, type Files } from './shared/protocol';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTask, mobileNewTaskAction, mobileNewTaskPrepare } from './new-task';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { draftContext, patchDraftContext } from './shared/composer-controls-branch';
import { mobileCacheAdoptThreadPresentation } from './mobile-client-cache-sync';
import type { ThreadState } from './shared/domain';

const options = [{ id: 'effort', label: 'Reasoning effort', type: 'select', currentValue: 'medium', promptInjectedValues: ['ultrathink'],
  options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High' }, { id: 'ultracode', label: 'Ultracode' }, { id: 'ultrathink', label: 'Ultrathink' }] }];
function fixture(client: T3Client = new T3Client()) {
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
  test('native search menu projects catalog filters without query or selection side effects', async () => {
    const f = fixture(); await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    const first = mobileComposerSettings('', 'other', true, f.client);
    const menu = JSON.parse(first.searchConfiguration);
    expect(menu).toEqual({ filter: 'other', showLegacy: true, hasLegacy: true,
      filters: first.filters.map(({ id, label, selected, disabled }) => ({ id, label, selected, disabled })) });
    expect(menu.filters.filter((row: { selected: boolean }) => row.selected).map((row: { id: string }) => row.id)).toEqual(['other']);
    expect(mobileComposerSettings('Typed query', 'other', true, f.client).searchConfiguration).toBe(first.searchConfiguration);
    expect(mobileComposerSettings('', '', false, f.client)).toMatchObject({ pending: true, saveLabel: 'Save' });
    expect(f.client.modelId).toBe('a'); expect(f.calls).toHaveLength(0);
  });
  test('native search menu carries escaped labels and removes departed catalog filters', () => {
    const f = fixture();
    const providers = arr(f.client.config.providers);
    providers[0]!.displayName = 'Provider "name"\nnext line';
    const before = JSON.parse(mobileComposerSettings('', '', false, f.client).searchConfiguration);
    expect(before.filters.find((row: { id: string }) => row.id === 'provider').label).toBe('Provider "name"\nnext line');
    f.client.config.providers = providers.slice(0, 1);
    const after = JSON.parse(mobileComposerSettings('', '', false, f.client).searchConfiguration);
    expect(after.filters.map((row: { id: string }) => row.id)).toEqual(['', '@favorites', 'provider']);
    expect(after.showLegacy).toBe(false); expect(after.hasLegacy).toBe(true);
  });
  for (const connection of ['read-only', 'disconnected']) {
    test(`unchanged existing-thread Done closes without writes while ${connection}`, async () => {
      const f = fixture(); f.client.threadId = 'thread';
      f.client.local.drafts[f.client.draftKey] = 'Keep this draft';
      await settings(f, 'open');
      if (connection === 'read-only') f.client.scopes = [];
      else f.client.connection = 'disconnected';
      let fileCalls = 0;
      f.storage.fs.mkdir = async () => { fileCalls++; };
      f.storage.fs.readFile = async () => { fileCalls++; return new ArrayBuffer(0); };
      f.storage.fs.atomicWriteFile = async () => { fileCalls++; };
      const before = mobileComposerSettings('', '', false, f.client);
      const result = await settings(f, 'save');
      expect({ canSave: before.canSave, canEdit: before.canEdit, label: before.saveLabel, result })
        .toMatchObject({ canSave: true, canEdit: false, label: 'Done', result: { closed: true, message: '' } });
      expect(mobileComposerSettings('', '', false, f.client).open).toBe(false);
      expect(f.client.modelId).toBe('a'); expect(f.client.draft).toBe('Keep this draft');
      expect(f.calls).toHaveLength(0); expect(fileCalls).toBe(0);
    });
    test(`pending Save stays staged without writes after becoming ${connection}`, async () => {
      const f = fixture(); await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
      if (connection === 'read-only') f.client.scopes = [];
      else f.client.connection = 'disconnected';
      expect(mobileComposerSettings('', '', false, f.client)).toMatchObject({ canSave: false, pending: true, saveLabel: 'Save' });
      expect(await settings(f, 'save')).toMatchObject({ closed: false, message: expect.stringContaining('writable') });
      expect(mobileComposerSettings('', '', false, f.client)).toMatchObject({ open: true, pending: true, saveLabel: 'Save' });
      expect(f.client.modelId).toBe('a'); expect(f.calls).toHaveLength(0);
    });
  }
  for (const blocked of ['unopened', 'generation', 'draft', 'applied', 'busy', 'pending']) {
    test(`unchanged Done preserves the ${blocked} fence`, async () => {
      const f = fixture();
      if (blocked !== 'unopened') await settings(f, 'open');
      if (blocked === 'generation') f.client.generation++;
      if (blocked === 'draft') f.client.projectId = 'p2';
      if (blocked === 'applied') f.client.modelId = 'b';
      if (blocked === 'busy') f.client.busy = true;
      if (blocked === 'pending') f.client.local.pending[f.client.environmentId] = { method: 'orchestration.dispatchCommand', payload: {}, description: 'Pending', threadId: '', text: '', uncertain: true };
      f.client.scopes = [];
      expect(mobileComposerSettings('', '', false, f.client).canSave).toBe(false);
      expect(await settings(f, 'save')).toMatchObject({ closed: false, message: expect.stringContaining(
        blocked === 'applied' ? 'model changed' : blocked === 'busy' || blocked === 'pending' ? 'current settings change' : 'session changed') });
      expect(f.calls).toHaveLength(0);
    });
  }
  test('search, disclosure and Cancel do not change applied selection; repeated pick preserves pending options', async () => {
    const f = fixture(); await settings(f, 'open');
    expect(mobileComposerSettings('Model B', '', false, f.client).models.filter(row => row.kind === 'model')).toHaveLength(1);
    await settings(f, 'pick', 'provider', 'b'); await settings(f, 'option', 'effort', 'high'); await settings(f, 'pick', 'provider', 'b');
    expect(f.client.modelId).toBe('a');
    expect(mobileComposerSettings('', '', false, f.client).options[0]?.value).toBe('High');
    await settings(f, 'cancel'); expect(f.client.modelId).toBe('a'); expect(f.calls.some(call => call.op === 'request')).toBe(false);
  });
  test('idle existing-thread Save keeps the model and all options as the thread draft setting without a command', async () => {
    const f = fixture(); f.client.threadId = 'thread'; f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { thread: { id: 'thread', projectId: 'p', modelSelection: { instanceId: 'provider', model: 'a' } }, runs: [], turnItems: [] } };
    const sticky = JSON.stringify([f.client.local.composerControls.stickyProvider, f.client.local.composerControls.stickyByProvider]);
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b'); await settings(f, 'option', 'effort', 'high');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect(f.calls.filter(call => String(call.method).startsWith('orchestration.'))).toEqual([]);
    expect(f.client.local.composerControls.staged['env:thread']).toEqual({ providerId: 'provider', modelId: 'b', options: [{ id: 'effort', value: 'high' }] });
    expect([f.client.modelId, f.client.modelOptions]).toEqual(['b', [{ id: 'effort', value: 'high' }]]);
    expect(JSON.stringify([f.client.local.composerControls.stickyProvider, f.client.local.composerControls.stickyByProvider])).toBe(sticky);
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


describe('configured models absent from the catalog', () => {
  test('authenticated current fallback can cancel a pending pick and close without a write', async () => {
    const f = fixture(); f.client.modelId = 'configured'; f.client.modelOptions = [{ id: 'effort', value: 'high' }];
    const fallback = () => mobileComposerSettings('', '', false, f.client).models.find(row => row.key === 'provider:configured');
    expect(fallback()).toMatchObject({ selected: true, disabled: false, label: 'configured', reason: '' });
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    expect(mobileComposerSettings('', '', false, f.client).pending).toBe(true);
    expect((await settings(f, 'pick', 'provider', 'configured')).message).toBe('');
    expect(mobileComposerSettings('', '', false, f.client).pending).toBe(false);
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect(f.client.modelId).toBe('configured'); expect(f.client.modelOptions).toEqual([{ id: 'effort', value: 'high' }]);
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    expect(await settings(f, 'cancel')).toMatchObject({ closed: true, message: '' });
    expect(f.client.modelId).toBe('configured');
    expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
  });
  test('fallback display never bypasses provider readiness or write permissions', async () => {
    for (const condition of ['missing', 'signed-out', 'disabled', 'uninstalled', 'not-ready', 'read-only', 'pending']) {
      const f = fixture(); f.client.modelId = 'configured';
      const provider = obj((f.client.config.providers as Obj[])[0]);
      if (condition === 'missing') f.client.config.providers = [];
      if (condition === 'signed-out') provider.auth = { status: 'unauthenticated' };
      if (condition === 'disabled') provider.enabled = false;
      if (condition === 'uninstalled') provider.installed = false;
      if (condition === 'not-ready') provider.status = 'error';
      if (condition === 'read-only') f.client.scopes = [];
      if (condition === 'pending') f.client.busy = true;
      await settings(f, 'open');
      expect(mobileComposerSettings('', '', false, f.client).models.find(row => row.key === 'provider:configured')?.disabled).toBe(true);
      expect((await settings(f, 'pick', 'provider', 'configured')).message).not.toBe('');
      expect(f.client.modelId).toBe('configured'); expect(f.calls).toHaveLength(0);
    }
  });
  test('Antigravity unavailable fallback remains visible but cannot be picked', async () => {
    const f = fixture(); f.client.modelId = 'configured';
    obj((f.client.config.providers as Obj[])[0]).driver = 'antigravity';
    await settings(f, 'open');
    expect(mobileComposerSettings('', '', false, f.client).models.find(row => row.key === 'provider:configured')).toMatchObject({ selected: true, disabled: true });
    expect((await settings(f, 'pick', 'provider', 'configured')).message).toContain('another model');
    expect(await settings(f, 'cancel')).toMatchObject({ closed: true }); expect(f.client.modelId).toBe('configured');
  });
});


describe('staged mobile model option memory', () => {
  const high = [{ id: 'effort', value: 'high' }];
  function memoryFixture(client?: T3Client) {
    const f = fixture(client), writes: Obj[] = [];
    f.client.local.drafts[f.client.draftKey] = 'Keep the draft';
    const controls = f.client.local.composerControls;
    controls.stickyProvider = 'provider';
    controls.stickyByProvider = { provider: { model: 'a', options: [] }, other: { model: 'remote', options: [] } };
    controls.stickyOptionsByModel = { provider: { a: [{ id: 'keep', value: true }] }, other: { b: [{ id: 'other', value: 'untouched' }] } };
    const sticky = JSON.stringify({ provider: controls.stickyProvider, choices: controls.stickyByProvider });
    f.storage.fs.atomicWriteFile = async (_path, bytes) => { writes.push(obj(JSON.parse(new TextDecoder().decode(bytes)))); };
    return { ...f, writes, sticky };
  }
  test('a staged explicit option survives Cancel in independent memory and persisted preferences', async () => {
    const f = memoryFixture();
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    await settings(f, 'option', 'effort', 'high'); await settings(f, 'cancel');
    const controls = f.client.local.composerControls;
    expect({ model: f.client.modelId, provider: f.client.providerId, draft: f.client.draft,
      sticky: JSON.stringify({ provider: controls.stickyProvider, choices: controls.stickyByProvider }), native: f.calls,
      remembered: controls.stickyOptionsByModel.provider?.b,
      persisted: obj(obj(f.writes.at(-1)?.composerControls).stickyOptionsByModel).provider })
      .toEqual({ model: 'a', provider: 'provider', draft: 'Keep the draft', sticky: f.sticky, native: [],
        remembered: high, persisted: { a: [{ id: 'keep', value: true }], b: high } });
    const restored = new T3Client(), bytes = new TextEncoder().encode(JSON.stringify(f.writes.at(-1)));
    await restored.load({ fs: { ...f.storage.fs, readFile: async () => bytes.buffer } });
    expect(restored.local.composerControls.stickyOptionsByModel.provider?.b).toEqual(high);
  });
  test('a later Save stores exactly the option remembered before Cancel as the thread draft setting', async () => {
    const f = memoryFixture(); f.client.threadId = 'thread';
    f.client.local.drafts[f.client.draftKey] = 'Keep the draft';
    f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { thread: { id: 'thread', projectId: 'p', modelSelection: { instanceId: 'provider', model: 'a' } }, runs: [], turnItems: [] } };
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    await settings(f, 'option', 'effort', 'high'); await settings(f, 'cancel');
    expect(f.calls).toEqual([]);
    expect(f.client.local.composerControls.stickyOptionsByModel.other?.b).toEqual([{ id: 'other', value: 'untouched' }]);
    expect(f.client.local.composerControls.stickyOptionsByModel.provider?.a).toEqual([{ id: 'keep', value: true }]);
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect(f.calls.filter(call => String(call.method).startsWith('orchestration.'))).toEqual([]);
    expect(f.client.local.composerControls.staged['env:thread']).toEqual({ providerId: 'provider', modelId: 'b', options: high });
    expect(f.client.draft).toBe('Keep the draft');
    const controls = f.client.local.composerControls;
    expect({ sticky: JSON.stringify({ provider: controls.stickyProvider, choices: controls.stickyByProvider }), memory: controls.stickyOptionsByModel.provider?.b })
      .toEqual({ sticky: f.sticky, memory: high });
  });
  test('invalid options, permission loss and changed owners do not remember or persist a staged choice', async () => {
    for (const refusal of ['unknown', 'unoffered', 'injected', 'read-only', 'owner']) {
      const f = memoryFixture(); await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
      const before = JSON.stringify(f.client.local.composerControls);
      if (refusal === 'read-only') f.client.scopes = [];
      if (refusal === 'owner') f.client.generation++;
      const response = refusal === 'owner' ? await settings(f, 'save') : await settings(f, 'option', refusal === 'unknown' ? 'missing' : 'effort',
        refusal === 'unoffered' ? 'missing' : refusal === 'injected' ? 'ultrathink' : 'high');
      expect(response.message).not.toBe('');
      expect(JSON.stringify(f.client.local.composerControls)).toBe(before);
      expect(f.writes).toEqual([]); expect(f.calls).toEqual([]);
      expect(f.client.modelId).toBe('a'); expect(f.client.draft).toBe('Keep the draft');
    }
  });
  for (const [name, client] of [['shared', () => new T3Client()], ['mobile', () => new MobileDraftClient()]] as const) {
    function heldMemory() {
      const f = memoryFixture(client()), writes: { document: Obj; resolve(): void; reject(error: Error): void }[] = [];
      let saved: Obj | undefined;
      f.storage.fs.atomicWriteFile = (_path, bytes) => new Promise<void>((resolve, reject) => {
        const document = obj(JSON.parse(new TextDecoder().decode(bytes)));
        writes.push({ document, resolve() { saved = document; resolve(); }, reject });
      });
      return { ...f, heldWrites: writes, saved: () => saved };
    }
    test(`${name} local writes leave option acceptance, another edit and Cancel available`, async () => {
      const f = heldMemory(); await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
      let response: Awaited<ReturnType<typeof settings>> | null = null;
      const first = settings(f, 'option', 'effort', 'high').then(value => { response = value; return value; });
      let observed: unknown, second: Awaited<ReturnType<typeof settings>> | null = null;
      let secondAction: ReturnType<typeof settings> | null = null;
      try {
        await new Promise<void>(resolve => setImmediate(resolve));
        const before = mobileComposerSettings('', '', false, f.client);
        secondAction = settings(f, 'option', 'effort', 'medium').then(value => { second = value; return value; });
        await new Promise<void>(resolve => setImmediate(resolve));
        const cancelled = await settings(f, 'cancel');
        const controls = f.client.local.composerControls;
        observed = { response, busy: before.busy, canEdit: before.canEdit, second, cancelled,
          memory: controls.stickyOptionsByModel.provider?.b, native: f.calls, model: f.client.modelId, draft: f.client.draft,
          sticky: JSON.stringify({ provider: controls.stickyProvider, choices: controls.stickyByProvider }),
          issued: f.heldWrites.map(write => obj(obj(obj(write.document.composerControls).stickyOptionsByModel).provider).b) };
      } finally {
        for (const write of f.heldWrites) write.resolve();
        await first; await secondAction;
      }
      expect(observed).toEqual({ response: expect.objectContaining({ message: '', closed: false }), busy: false, canEdit: true,
        second: expect.objectContaining({ message: '', closed: false }), cancelled: expect.objectContaining({ message: '', closed: true }),
        memory: [{ id: 'effort', value: 'medium' }], native: [], model: 'a', draft: 'Keep the draft', sticky: f.sticky,
        issued: [high, [{ id: 'effort', value: 'medium' }]] });
      expect(mobileComposerSettings('', '', false, f.client)).toMatchObject({ pending: false, open: false, busy: false, error: '' });
      expect(obj(obj(obj(f.saved()?.composerControls).stickyOptionsByModel).provider).b).toEqual([{ id: 'effort', value: 'medium' }]);
    });
    test(`${name} failed background persistence keeps editing and Cancel local without an action error`, async () => {
      const f = heldMemory(), warnings: unknown[][] = [];
      const warning = spyOn(console, 'warn').mockImplementation((...values: unknown[]) => { warnings.push(values); });
      await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b');
      let response: Awaited<ReturnType<typeof settings>> | null = null;
      const first = settings(f, 'option', 'effort', 'high').then(value => { response = value; return value; });
      let beforeFailure: unknown, later: unknown, second: Awaited<ReturnType<typeof settings>> | null = null;
      let secondAction: ReturnType<typeof settings> | null = null;
      try {
        await new Promise<void>(resolve => setImmediate(resolve));
        beforeFailure = response;
        f.heldWrites[0]!.reject(new Error('Memory save refused'));
        await first;
        await new Promise<void>(resolve => setImmediate(resolve));
        secondAction = settings(f, 'option', 'effort', 'medium').then(value => { second = value; return value; });
        await new Promise<void>(resolve => setImmediate(resolve));
        const cancelled = await settings(f, 'cancel');
        const view = mobileComposerSettings('', '', false, f.client), controls = f.client.local.composerControls;
        later = { second, cancelled, error: view.error, busy: view.busy, open: view.open, native: f.calls,
          memory: controls.stickyOptionsByModel.provider?.b, model: f.client.modelId, draft: f.client.draft,
          sticky: JSON.stringify({ provider: controls.stickyProvider, choices: controls.stickyByProvider }),
          issued: f.heldWrites.map(write => obj(obj(obj(write.document.composerControls).stickyOptionsByModel).provider).b),
          warnings: warnings.map(values => values.map(value => value instanceof Error ? value.message : value)) };
      } finally {
        for (const write of f.heldWrites) write.resolve();
        await first; await secondAction; warning.mockRestore();
      }
      expect(beforeFailure).toMatchObject({ message: '', closed: false });
      expect(later).toEqual({ second: expect.objectContaining({ message: '', closed: false }), cancelled: expect.objectContaining({ message: '', closed: true }),
        error: '', busy: false, open: false, native: [], memory: [{ id: 'effort', value: 'medium' }], model: 'a', draft: 'Keep the draft', sticky: f.sticky,
        issued: [high, [{ id: 'effort', value: 'medium' }]], warnings: [['Could not save model option memory.', 'Memory save refused']] });
      expect(obj(obj(obj(f.saved()?.composerControls).stickyOptionsByModel).provider).b).toEqual([{ id: 'effort', value: 'medium' }]);
    });
  }
});

describe('idle existing-thread settings stay local thread draft settings until Send', () => {
  const high = [{ id: 'effort', value: 'high' }];
  const orchestration = (f: ReturnType<typeof fixture>) => f.calls.filter(call => String(call.method).startsWith('orchestration.'));
  const state = (model: string, thread: Obj = {}): ThreadState => ({ sequence: 2, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
    projection: { thread: { id: 'thread', projectId: 'p', modelSelection: { instanceId: 'provider', model }, runtimeMode: 'approval-required',
      interactionMode: 'default', ...thread }, runs: [], turnItems: [] } });
  function threadFixture(model = 'a') {
    const f = fixture(), writes: Obj[] = [];
    f.client.threadId = 'thread'; f.client.thread = state(model); f.client.modelId = model;
    f.storage.fs.atomicWriteFile = async (_path, bytes) => { writes.push(obj(JSON.parse(new TextDecoder().decode(bytes)))); };
    return { ...f, writes };
  }
  const staged = (f: ReturnType<typeof threadFixture>) => f.client.local.composerControls.staged['env:thread'];
  const remote = (f: ReturnType<typeof threadFixture>, next: ThreadState) => { f.client.thread = next; mobileCacheAdoptThreadPresentation(f.client, next); };
  async function cold(f: ReturnType<typeof threadFixture>, document: Obj = obj(f.writes.at(-1))) {
    const client = new T3Client(), bytes = new TextEncoder().encode(JSON.stringify(document));
    await client.load({ fs: { ...f.storage.fs, readFile: async () => bytes.buffer } });
    Object.assign(client, { environmentId: 'env', projectId: 'p', threadId: 'thread' });
    return client;
  }
  test('Save, an applied option and each mode issue no orchestration command and store only the edited fields', async () => {
    const save = threadFixture();
    await settings(save, 'open'); await settings(save, 'pick', 'provider', 'b'); await settings(save, 'option', 'effort', 'high');
    expect(await settings(save, 'save')).toMatchObject({ closed: true, message: '' });
    expect(staged(save)).toEqual({ providerId: 'provider', modelId: 'b', options: high });
    const option = threadFixture('b');
    expect((await settings(option, 'option', 'effort', 'high')).message).toBe('');
    expect(staged(option)).toEqual({ providerId: 'provider', modelId: 'b', options: high });
    const runtime = threadFixture();
    expect((await settings(runtime, 'runtime', '', 'full-access')).message).toBe('');
    expect(staged(runtime)).toEqual({ runtimeMode: 'full-access' });
    const interaction = threadFixture();
    expect((await interaction.client.command('interaction', '', 'plan', 0, interaction.native, interaction.storage)).message).toBe('');
    expect(staged(interaction)).toEqual({ interactionMode: 'plan' });
    expect([save.client.modelId, option.client.modelOptions, runtime.client.runtimeMode, interaction.client.interactionMode]).toEqual(['b', high, 'full-access', 'plan']);
    for (const f of [save, option, runtime, interaction]) {
      expect(orchestration(f)).toEqual([]);
      expect(obj(obj(f.writes.at(-1)?.composerControls).staged)['env:thread']).toEqual(staged(f));
    }
  });
  test('a saved thread model survives a cold load through the production cache presentation', async () => {
    const f = threadFixture();
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b'); await settings(f, 'option', 'effort', 'high'); await settings(f, 'save');
    const restored = await cold(f); mobileCacheAdoptThreadPresentation(restored, state('a'));
    expect([restored.providerId, restored.modelId, restored.modelOptions, restored.runtimeMode, restored.interactionMode])
      .toEqual(['provider', 'b', high, 'approval-required', 'default']);
  });
  test('a runtime-only choice follows a remote model change, while an explicit model does not', async () => {
    const runtime = threadFixture(); await settings(runtime, 'runtime', '', 'full-access');
    remote(runtime, state('c'));
    expect([runtime.client.modelId, runtime.client.runtimeMode]).toEqual(['c', 'full-access']);
    const model = threadFixture();
    await settings(model, 'open'); await settings(model, 'pick', 'provider', 'b'); await settings(model, 'save');
    remote(model, state('c', { runtimeMode: 'full-access' }));
    expect([model.client.modelId, model.client.runtimeMode]).toEqual(['b', 'full-access']);
    expect([orchestration(runtime), orchestration(model)]).toEqual([[], []]);
  });
  test('an explicit pick equal to the server model stays the thread choice after a remote change', async () => {
    const f = threadFixture();
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'b'); await settings(f, 'save');
    await settings(f, 'open'); await settings(f, 'pick', 'provider', 'a');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect(staged(f)).toEqual({ providerId: 'provider', modelId: 'a', options: [] });
    remote(f, state('c')); expect(f.client.modelId).toBe('a'); expect(orchestration(f)).toEqual([]);
  });
  test('Cancel, read-only, a provider that requires a new thread and a locked provider leave settings untouched', async () => {
    const cancel = threadFixture();
    await settings(cancel, 'open'); await settings(cancel, 'pick', 'provider', 'b'); await settings(cancel, 'option', 'effort', 'high');
    expect(await settings(cancel, 'cancel')).toMatchObject({ closed: true });
    const readOnly = threadFixture(); readOnly.client.scopes = [];
    expect((await settings(readOnly, 'runtime', '', 'full-access')).message).toContain('writable');
    expect((await readOnly.client.command('model', 'b', 'provider', 0, readOnly.native, readOnly.storage)).message).not.toBe('');
    expect((await readOnly.client.command('interaction', '', 'plan', 0, readOnly.native, readOnly.storage)).message).not.toBe('');
    const fresh = threadFixture('b'); obj((fresh.client.config.providers as Obj[])[0]).requiresNewThreadForModelChange = true;
    await settings(fresh, 'open'); expect((await settings(fresh, 'pick', 'provider', 'a')).message).toContain('new thread');
    expect((await fresh.client.command('model', 'a', 'provider', 0, fresh.native, fresh.storage)).message).toContain('new thread');
    expect((await fresh.client.command('model-option', 'effort', 'high', 0, fresh.native, fresh.storage)).message).toContain('new thread');
    const locked = threadFixture(), started = state('a');
    locked.client.thread = { ...started, projection: { ...started.projection, runs: [{ id: 'run', status: 'completed', ordinal: 1, completedAt: '2026-10-18T00:00:00Z' }] } };
    await settings(locked, 'open'); expect((await settings(locked, 'pick', 'other', 'remote')).message).toContain('current provider');
    expect((await locked.client.command('provider', 'other', '', 0, locked.native, locked.storage)).message).toContain('current provider');
    for (const [f, model] of [[cancel, 'a'], [readOnly, 'a'], [fresh, 'b'], [locked, 'a']] as const) {
      expect(f.client.local.composerControls.staged).toEqual({}); expect(orchestration(f)).toEqual([]);
      expect([f.client.providerId, f.client.modelId, f.client.modelOptions, f.client.runtimeMode]).toEqual(['provider', model, [], 'approval-required']);
    }
  });
  test('a provider switch keeps the runtime choice absent and resets only interaction for a provider without Plan', async () => {
    const f = threadFixture(); f.client.thread = state('a', { interactionMode: 'plan' }); f.client.interactionMode = 'plan';
    Object.assign(obj((f.client.config.providers as Obj[])[1]), { supportedRuntimeModes: ['full-access'], showInteractionModeToggle: false });
    await settings(f, 'open'); await settings(f, 'pick', 'other', 'remote');
    expect(mobileComposerSettings('', '', false, f.client).runtimes.find(row => row.selected)?.id).toBe('full-access');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect(staged(f)).toEqual({ providerId: 'other', modelId: 'remote', options: [], interactionMode: 'default' });
    expect([f.client.providerId, f.client.runtimeMode, f.client.interactionMode]).toEqual(['other', 'approval-required', 'default']);
    expect(orchestration(f)).toEqual([]);
  });
  test('New Task Save still moves an unsupported runtime to the new provider first supported mode', async () => {
    const f = fixture(); Object.assign(obj((f.client.config.providers as Obj[])[1]), { supportedRuntimeModes: ['full-access'] });
    await settings(f, 'open'); await settings(f, 'pick', 'other', 'remote');
    expect(await settings(f, 'save')).toMatchObject({ closed: true, message: '' });
    expect([f.client.providerId, f.client.modelId, f.client.runtimeMode, f.client.local.composerControls.stickyProvider]).toEqual(['other', 'remote', 'full-access', 'other']);
    expect(f.client.local.composerControls.staged).toEqual({}); expect(f.calls.filter(call => String(call.method).startsWith('orchestration.'))).toEqual([]);
  });
  test('more than 200 explicit thread settings survive real persistence and decoding', async () => {
    const f = threadFixture(), entries: Record<string, Obj> = {};
    for (let n = 0; n < 250; n++) entries[`env:thread-${n}`] = n % 2 ? { runtimeMode: 'full-access' } : { providerId: 'provider', modelId: 'b', options: high };
    Object.assign(f.client.local.composerControls.staged, entries);
    expect((await settings(f, 'runtime', '', 'full-access')).message).toBe('');
    const document = obj(f.writes.at(-1)), saved = obj(obj(document.composerControls).staged);
    Object.assign(saved, { 'env:half': { providerId: 'provider', runtimeMode: 'auto' }, 'env:no-options': { providerId: 'provider', modelId: 'b' },
      'env:empty': {}, 'env:invalid': { interactionMode: 7 } });
    const restored = await cold(f, document);
    expect(restored.local.composerControls.staged).toEqual({ ...entries, 'env:thread': { runtimeMode: 'full-access' }, 'env:half': { runtimeMode: 'auto' } });
    await restored.persist(f.storage);
    expect(Object.keys(obj(obj(f.writes.at(-1)?.composerControls).staged))).toHaveLength(252);
  });
});
