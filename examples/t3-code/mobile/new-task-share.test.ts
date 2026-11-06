import { expect, test } from 'bun:test';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileIncomingShareRead } from './incoming-share-inbox';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as action, mobileNewTaskFlowOwns as owns } from './new-task-flow';
import { mobileNewTaskDraftCreate } from './mobile-new-task-drafts';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { noteNow } from './shared/composer-controls';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
const id = `share-${'a'.repeat(64)}`, adoptionId = '00000000-0000-4000-8000-000000000010';
const entry = { schemaVersion: 1, id, instanceId: '00000000-0000-4000-8000-000000000020', createdAt: '2026-10-08T10:00:00Z', text: 'Shared task', attachments: [], warnings: [] };
async function fixture() {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  let disk: Obj = { version: 1 }, reservations: Obj[] = [], consumed = false, fail = false, choice = 'retry';
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(JSON.stringify(disk)).buffer; }, async atomicWriteFile(_path, value) { disk = JSON.parse(new TextDecoder().decode(value)); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); let value: unknown = {};
    if (request.op === 'mobileOutbox') value = request.action === 'read' ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] } : { complete: true, fingerprint: null, claims: [] };
    if (request.op === 'ids') value = ['shared-draft'];
    if (request.op === 'mobileAlert') value = { choice };
    if (request.op === 'mobileIncomingShares') {
      if (request.action === 'read') value = { available: false, entries: consumed ? [] : [entry], reservations };
      else {
        if (request.action === 'reserve') {
          if (fail) return { ok: false, generation: 1, error: { kind: 'Share', message: 'Disk unavailable' } };
          reservations = [{ shareId: id, adoptionId, destination: request.destination, attachmentIds: [], phase: 'reserved' }];
        }
        if (request.action === 'consume') { expect(obj(obj(disk.mobileIncomingShareImports)[client.draftKey])[adoptionId]).toBeDefined(); consumed = true; reservations = []; }
        value = { adoptionId, consumed, entry: consumed ? null : entry, attachments: [] };
      }
    }
    return { ok: true, generation: 1, value };
  } };
  await client.command('dismiss-error', '', '', 0, native, storage);
  Object.assign(client, { environmentId: 'env', origin: 'https://share.test', projectId: 'a', generation: 1, connection: 'connected', configLive: true, shellLoaded: true, shellLive: true, config: { providers: [] } });
  client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  noteNow(client, Date.parse(entry.createdAt)); await mobileIncomingShareRead(client, native); calls.length = 0;
  const snapshot = (location: string, visit = 'visit', session = 'flow') => view(session, visit, location, true, true, client, fleet);
  const act = (owner: string, kind: string, arg = '', visit = 'visit') => action(owner, visit, kind, arg, '', native, storage, client, fleet);
  return { client, native, storage, calls, snapshot, act, setReservation(value: Obj) { reservations = [value]; }, failReserve() { fail = true; }, chooseCancel() { choice = 'cancel-import'; } };
}
test('share project choice keeps identity, imports once and admits composer only after saved consumption', async () => {
  const f = await fixture(), chooser = f.snapshot(`/new?incomingShareId=${id}`);
  expect(chooser.title).toBe('Start a task'); expect(chooser.subtitle).toBe('Choose a project for what you shared');
  const chosen = await f.act(chooser.owner, 'project', '["env","b"]'); expect(chosen.nextLocation).toBe(`/new/draft?incomingShareId=${id}`);
  const before = f.snapshot(chosen.nextLocation, 'draft'); expect(before.needsPrepare).toBe(true); expect(owns(before.owner, 'draft', f.client)).toBe(false);
  expect((await f.act(before.owner, 'prepare', '', 'draft')).message).toBe('');
  expect(f.snapshot(chosen.nextLocation, 'draft').ready).toBe(true); expect(f.client.draft).toBe('Shared task');
  expect(f.snapshot('/new/draft/settings', 'settings').needsPrepare).toBe(true);
  expect(f.calls.filter(call => call.action === 'consume')).toHaveLength(1);
});
test('reserved share resumes exact saved draft without allocating another or changing project', async () => {
  const f = await fixture(), saved = mobileNewTaskDraftCreate(f.client, { id: 'reserved', environmentId: 'env', projectId: 'b', origin: f.client.origin, createdAt: entry.createdAt });
  f.setReservation({ shareId: id, adoptionId, destination: { draftKey: saved.key, environmentId: 'env', projectId: 'b', origin: f.client.origin }, attachmentIds: [], phase: 'staged' });
  await mobileIncomingShareRead(f.client, f.native);
  const chooser = f.snapshot(`/new?incomingShareId=${id}`);
  expect(chooser.nextLocation).toContain('draftId=new-task%3Areserved'); expect(chooser.reservedProject).toBe('["env","b"]');
  expect((await f.act(chooser.owner, 'project', '["env","a"]')).message).toContain('reserved');
  expect((await f.act(chooser.owner, 'scratch')).message).toContain('reserved');
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(0); expect(f.client.projectId).toBe('a');
});
test('failed import cancel before reservation leaves composer editable without repeated auto import', async () => {
  const f = await fixture(); f.failReserve(); f.chooseCancel();
  const chooser = f.snapshot(`/new?incomingShareId=${id}`), chosen = await f.act(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot(chosen.nextLocation, 'draft');
  await f.act(draft.owner, 'prepare', '', 'draft'); expect(f.snapshot(chosen.nextLocation, 'draft').needsPrepare).toBe(true);
  await f.act(draft.owner, 'prepare', '', 'draft'); expect(f.snapshot(chosen.nextLocation, 'draft').ready).toBe(true);
  expect(f.client.draft).toBe(''); expect(f.calls.filter(call => call.op === 'mobileAlert').map(call => call.kind)).toEqual(['share-import']);
  expect(f.calls.filter(call => call.action === 'reserve')).toHaveLength(1);
});
test('direct conflicting saved draft redirects before project or branch mutation', async () => {
  const f = await fixture(), saved = mobileNewTaskDraftCreate(f.client, { id: 'reserved', environmentId: 'env', projectId: 'b', origin: f.client.origin, createdAt: entry.createdAt });
  f.setReservation({ shareId: id, adoptionId, destination: { draftKey: saved.key, environmentId: 'env', projectId: 'b', origin: f.client.origin }, attachmentIds: [], phase: 'staged' });
  await mobileIncomingShareRead(f.client, f.native); f.calls.length = 0;
  const route = `/new/draft?environmentId=env&projectId=a&draftId=new-task:wrong&branch=main&incomingShareId=${id}`;
  const initial = f.snapshot(route); expect(initial.nextLocation).toContain('draftId=new-task%3Areserved'); expect(initial.needsPrepare).toBe(false);
  expect((await f.act(initial.owner, 'prepare')).nextLocation).toBe(initial.nextLocation);
  expect(f.client.projectId).toBe('a'); expect(f.calls).toEqual([]);
});
