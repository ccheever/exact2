import { expect, test } from 'bun:test';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftUnbind as unbind,
  mobileNewTaskDraftDiscard as discard, mobileNewTaskDraftRetarget as retarget, mobileNewTaskDraftLookup as lookup } from './mobile-new-task-drafts';
import { mobileComposerTarget as target, mobileComposerTargetExists as exists, mobileComposerTargetCurrent as current,
  mobileComposerTargetRevision as revision, mobileComposerTargetWriteText as writeText } from './composer-target';
import { mobileDraftChanged } from './draft';
import { mobileSend } from './composer-behavior';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

async function fixture() {
  const client = new MobileDraftClient(), calls: Obj[] = []; let serial = 0;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const call = obj(input); calls.push(call);
    return { ok: true, generation: client.generation, value: call.op === 'ids'
      ? Array.from({ length: Number(call.count) }, () => `id-${++serial}`) : {} };
  } };
  await client.command('dismiss-error', '', '', 0, native, storage);
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin: 'https://draft.test', connection: 'connected',
    configLive: true, shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'],
    providerId: 'provider', modelId: 'model' });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [
    { instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', models: [{ slug: 'model', name: 'Model' }] }] };
  client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/project' }];
  for (const id of ['A', 'B']) {
    create(client, { id, environmentId: 'env', projectId: 'project', origin: client.origin, createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts[`new-task:${id}`] = 'same';
  }
  bind(client, 'new-task:A', 'flow-A'); calls.length = 0;
  return { client, native, storage, calls };
}

test('keyboard and late voice writes keep independent content revisions through text ABA', async () => {
  const f = await fixture(), a = target(f.client);
  expect(revision(f.client, a)).toBe(0);
  expect((await mobileDraftChanged(f.client, 'changed', f.native, f.storage, a.owner)).message).toBe('');
  expect((await mobileDraftChanged(f.client, 'same', f.native, f.storage, a.owner)).message).toBe('');
  expect(revision(f.client, a)).toBe(2);
  bind(f.client, 'new-task:B', 'flow-B');
  expect(current(f.client, a)).toBe(false); expect(exists(f.client, a)).toBe(true);
  expect(writeText(f.client, a, 'late voice')).toBe(true);
  expect(revision(f.client, a)).toBe(3); expect(f.client.draft).toBe('same');
  expect(f.client.local.drafts[a.key]).toBe('late voice');
});
test('discard and retarget reject captured composer completions without resurrecting content', async () => {
  for (const change of ['discard', 'retarget', 'origin', 'generation']) {
    const f = await fixture(), a = target(f.client);
    if (change === 'discard') discard(f.client, a.key);
    if (change === 'retarget') retarget(f.client, a.key, { environmentId: 'env', projectId: 'other', origin: f.client.origin });
    if (change === 'origin') f.client.origin = 'https://replacement.test';
    if (change === 'generation') f.client.generation++;
    expect(exists(f.client, a)).toBe(false); expect(revision(f.client, a)).toBeNull();
    expect(writeText(f.client, a, 'resurrected')).toBe(false);
    expect(f.client.local.drafts[a.key]).not.toBe('resurrected');
  }
});
test('thread drafts retain ordinary completion behavior without independent metadata', async () => {
  const f = await fixture(); unbind(f.client); f.client.threadId = 'thread';
  const t = target(f.client); expect(t.key).toBe('env:thread');
  expect(writeText(f.client, t, 'thread text')).toBe(true); expect(revision(f.client, t)).toBe(0);
  expect(lookup(f.client, t.key)).toBeNull(); expect(f.client.draft).toBe('thread text');
});
for (const point of ['mobilePreferences', 'devicePresentation', 'ids']) {
  test(`full shared Send refuses equal-text A-to-B switch during ${point}`, async () => {
    const f = await fixture(), original = f.native.later; let switched = false;
    f.native.later = async input => {
      if (obj(input).op === point) { switched = true; bind(f.client, 'new-task:B', 'flow-B'); }
      return original(input);
    };
    try { await mobileSend(f.client, false, f.native, f.storage); }
    catch (error) { expect(obj(error).kind).toBe('superseded'); }
    expect(switched).toBe(true);
    expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
    expect(f.client.local.drafts['new-task:A']).toBe('same'); expect(f.client.local.drafts['new-task:B']).toBe('same');
    expect(f.client.pending).toBeUndefined();
  });
}
