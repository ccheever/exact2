import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftLookup as lookup,
  mobileNewTaskDraftRetarget as retarget, mobileNewTaskDraftChoicesUpdate as updateChoices } from './mobile-new-task-drafts';
import { mobileSend } from './composer-behavior';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

const A = 'new-task:A', B = 'new-task:B', origin = 'https://draft-settings.test';
async function fixture(saved: Obj = { version: 1 }) {
  const client = new MobileDraftClient(), calls: Obj[] = [], writes: Obj[] = []; let disk = JSON.stringify(saved), serial = 0;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(disk))); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const call = obj(input); calls.push(call);
    return { ok: true, generation: client.generation, value: call.op === 'ids'
      ? Array.from({ length: Number(call.count) }, () => `id-${++serial}`)
      : call.op === 'mobilePreferences' ? { planModeEnabled: false, followUpBehavior: 'queue' } : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.command('dismiss-error', '', '', 0, handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', projectId: 'p', origin, generation: 1, connection: 'connected',
    configLive: true, shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'] });
  const optionDescriptors = [{ id: 'effort', type: 'select', label: 'Effort', options: [{ id: 'low', label: 'Low' }, { id: 'high', label: 'High' }] }];
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, settings: { defaultRuntimeMode: 'approval-required' }, providers: [
    { instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', supportedRuntimeModes: ['approval-required', 'full-access'],
      models: [{ slug: 'a', name: 'A', isDefault: true }, { slug: 'b', name: 'B', capabilities: { optionDescriptors } }] },
    { instanceId: 'hidden', driver: 'pi', enabled: true, installed: true, status: 'ready', showInteractionModeToggle: false,
      models: [{ slug: 'h', name: 'Hidden Plan' }] }] };
  client.shell.projects = [{ id: 'p', title: 'P', workspaceRoot: '/p' }, { id: 'q', title: 'Q', workspaceRoot: '/q' }];
  for (const id of ['A', 'B']) if (!lookup(client, `new-task:${id}`)) {
    create(client, { id, environmentId: 'env', projectId: 'p', origin, createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts[`new-task:${id}`] = 'keep content';
  }
  bind(client, A, 'flow-A'); client.chooseDefaults(); calls.length = 0; writes.length = 0;
  return { client, native, storage, calls, writes, disk: () => obj(JSON.parse(disk)) };
}
const command = (f: Awaited<ReturnType<typeof fixture>>, op: string, id = '', value = '') => f.client.command(op, id, value, 0, f.native, f.storage);
const savedChoices = (document: Obj, key = A) => obj(obj(obj(document.mobileNewTaskDrafts).records)[key]).choices;

for (const [op, id, value, expected] of [
  ['model', 'b', 'provider', { providerId: 'provider', modelId: 'b', modelOptions: [] }],
  ['runtime', '', 'full-access', { runtimeMode: 'full-access' }],
  ['interaction', '', 'plan', { interactionMode: 'plan' }],
] as const) test(`${op} stores only the explicit choice in its first durable command save`, async () => {
  const f = await fixture(); expect((await command(f, op, id, value)).message).toBe('');
  expect(f.writes.length).toBeGreaterThan(0); expect(savedChoices(f.writes[0]!)).toEqual(expected);
  expect(lookup(f.client, B)?.choices).toBeNull();
  const restarted = await fixture(f.disk()); expect(lookup(restarted.client, A)?.choices).toEqual(expected);
});

test('model options survive defaults and retarget without freezing an implicit runtime', async () => {
  const f = await fixture(); await command(f, 'model', 'b', 'provider'); await command(f, 'model-option', 'effort', 'high');
  expect(lookup(f.client, A)?.choices).toEqual({ providerId: 'provider', modelId: 'b', modelOptions: [{ id: 'effort', value: 'high' }] });
  obj(f.client.config.settings).defaultRuntimeMode = 'full-access'; f.client.chooseDefaults();
  expect(f.client.modelId).toBe('b'); expect(f.client.modelOptions).toEqual([{ id: 'effort', value: 'high' }]); expect(f.client.runtimeMode).toBe('full-access');
  expect(retarget(f.client, A, { environmentId: 'env', projectId: 'q', origin })).toBe(true);
  f.client.projectId = 'q'; f.client.chooseDefaults(); expect(f.client.modelId).toBe('b'); expect(f.client.runtimeMode).toBe('full-access');
  await command(f, 'runtime', '', 'approval-required'); obj(f.client.config.settings).defaultRuntimeMode = 'full-access';
  f.client.chooseDefaults(); expect(f.client.runtimeMode).toBe('approval-required');
});

test('model switch to a provider hiding Plan clears only that draft stored Plan', async () => {
  const f = await fixture(); await command(f, 'interaction', '', 'plan'); f.writes.length = 0;
  expect((await command(f, 'model', 'h', 'hidden')).message).toBe('');
  expect(savedChoices(f.writes[0]!)).toEqual({ providerId: 'hidden', modelId: 'h', modelOptions: [], interactionMode: 'default' });
  expect(f.client.interactionMode).toBe('default'); expect(lookup(f.client, B)?.choices).toBeNull();
});

test('hydrated choice whitelist cannot assign unrelated fields or client methods', async () => {
  const f = await fixture(); await f.client.persist(f.storage); const document = f.disk();
  obj(obj(obj(document.mobileNewTaskDrafts).records)[A]).choices = { runtimeMode: 'full-access', origin: 'https://wrong.test',
    threadId: 'wrong-thread', busy: true, error: 'injected', chooseDefaults: 'replace method', local: {} };
  const r = await fixture(document); expect(lookup(r.client, A)?.choices).toEqual({ runtimeMode: 'full-access' });
  expect(r.client.runtimeMode).toBe('full-access'); expect(r.client.origin).toBe(origin); expect(r.client.threadId).toBe('');
  expect(r.client.busy).toBe(false); expect(r.client.error).not.toBe('injected'); expect(typeof r.client.chooseDefaults).toBe('function');
});

for (const at of ['devicePresentation', 'composerSendIntent', 'storage']) test(`same-project A to B during ${at} cannot apply A's model to B`, async () => {
  const f = await fixture(); obj(obj(f.client.config.environment).capabilities).requiredWorktreeBootstrap = true;
  bind(f.client, B, 'prepare-B'); updateChoices(f.client, B, { providerId: 'provider', modelId: 'a', modelOptions: [] });
  bind(f.client, A, 'flow-A'); f.client.chooseDefaults();
  const switchDraft = () => { expect(bind(f.client, B, 'flow-B')).toBe(true); f.client.chooseDefaults(); };
  if (at === 'storage') {
    const original = f.storage.fs.atomicWriteFile; f.storage.fs.atomicWriteFile = async (path, bytes) => { await original(path, bytes); switchDraft(); };
  } else {
    const original = f.native.later; f.native.later = async input => { const reply = await original(input); if (obj(input).op === at) switchDraft(); return reply; };
  }
  await command(f, 'model', 'b', 'provider');
  expect(f.client.draftKey).toBe(B); expect(f.client.modelId).toBe('a');
  expect(lookup(f.client, B)?.choices).toEqual({ providerId: 'provider', modelId: 'a', modelOptions: [] });
  if (at !== 'storage') { expect(f.writes).toEqual([]); expect(lookup(f.client, A)?.choices).toBeNull(); }
});

for (const outcome of ['refused', 'uncertain']) test(`${outcome} Send captures Build without replacing stored Plan or pending payload`, async () => {
  const f = await fixture(); await command(f, 'interaction', '', 'plan'); f.writes.length = 0;
  const original = f.native.later; f.native.later = async input => {
    const call = obj(input);
    if (call.method === 'orchestration.launchThread') {
      f.calls.push(call);
      if (outcome === 'uncertain') throw new Error('lost reply');
      return { ok: false, generation: f.client.generation, error: { kind: 'remote', message: 'refused', uncertain: false } };
    }
    return original(input);
  };
  const result = await mobileSend(f.client, false, f.native, f.storage); expect(result.message).not.toBe('');
  const sent = obj(f.calls.find(call => call.method === 'orchestration.launchThread')?.payload);
  expect(sent.interactionMode).toBe('default'); expect(f.client.interactionMode).toBe('plan');
  expect(lookup(f.client, A)?.choices).toEqual({ interactionMode: 'plan' });
  expect(f.writes.every(document => obj(savedChoices(document)).interactionMode === 'plan')).toBe(true);
  if (outcome === 'uncertain') {
    expect(f.client.pending?.uncertain).toBe(true); expect(f.client.pending?.payload).toEqual(sent);
    expect(obj(obj(obj(f.disk().pending).env).payload).interactionMode).toBe('default');
  } else expect(f.client.pending).toBeUndefined();
});

test('temporary Build cleanup never restores Plan into a newer owner', async () => {
  const f = await fixture(); await command(f, 'interaction', '', 'plan');
  const original = f.native.later; f.native.later = async input => {
    const reply = await original(input);
    if (obj(input).op === 'devicePresentation') { bind(f.client, B, 'flow-B'); f.client.chooseDefaults(); }
    return reply;
  };
  await mobileSend(f.client, false, f.native, f.storage);
  expect(f.client.draftKey).toBe(B); expect(f.client.interactionMode).toBe('default');
  expect(lookup(f.client, A)?.choices).toEqual({ interactionMode: 'plan' }); expect(lookup(f.client, B)?.choices).toBeNull();
  expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
});

test('temporary Build cleanup preserves a newer explicit choice on the same draft', async () => {
  const f = await fixture(); await command(f, 'interaction', '', 'plan');
  const original = f.native.later; f.native.later = async input => {
    if (obj(input).op === 'ids') { updateChoices(f.client, A, { interactionMode: 'default' }); throw new Error('ids failed'); }
    return original(input);
  };
  await mobileSend(f.client, false, f.native, f.storage);
  expect(f.client.interactionMode).toBe('default'); expect(lookup(f.client, A)?.choices).toEqual({ interactionMode: 'default' });
});
