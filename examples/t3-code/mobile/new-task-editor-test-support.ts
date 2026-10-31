import { expect } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorRefresh as refresh, mobileNewTaskEditorView as view,
  type NewTaskEditorView } from './new-task-editor';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction } from './new-task-flow';
import { mobileNewTaskContextGuard, mobileNewTaskContextWrite } from './mobile-new-task-context';
import type { ComposerEditorEvent, ComposerMountedIdentity } from './composer-editor-state';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { noteNow } from './shared/composer-controls';
import { obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { draftFiles } from './shared/composer-editor-files';
import type { PickerIntakeFile } from './composer-picker-intake-io';

export const origin = 'https://new-task-editor.test', now = 1791420000000;
export const receiptId = '11111111-1111-4111-8111-111111111111';
export function deferred<T>() {
  let resolve: (value: T) => void = () => { throw Error('uninitialized'); };
  let reject: (error: unknown) => void = () => { throw Error('uninitialized'); };
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
export function mounted(projection: NewTaskEditorView): ComposerMountedIdentity {
  if (!projection.node) throw Error(projection.message);
  const configuration = obj(JSON.parse(projection.node.configuration));
  return { owner: str(configuration.owner), editorId: str(configuration.editorId), routeVisit: str(configuration.routeVisit),
    renderEpoch: str(configuration.renderEpoch), mountId: str(configuration.mountId) || 'native-mount' };
}
export function event(identity: ComposerMountedIdentity, value: string, eventCount = 1, kind: ComposerEditorEvent['kind'] = 'ready'): ComposerEditorEvent {
  return { ...identity, kind, eventCount, value, selection: { start: value.length, end: value.length }, composing: false, focused: true };
}
export function receipt(identity: ComposerMountedIdentity, value = '🙂 final source', eventCount = 3) {
  return { receiptId, identity, event: { ...event(identity, value, eventCount, 'text'), kind: 'retired' }, retired: true };
}
export class CountingClient extends MobileDraftClient {
  refreshes = 0;
  override async refresh(native: Native | null | undefined, storage: Files): Promise<void> {
    this.refreshes++; await super.refresh(native, storage);
  }
}
export async function fixture(seed?: (client: MobileDraftClient, key: string) => void, document = JSON.stringify({ version: 1 })) {
  const client = new CountingClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [], order: string[] = [], writes: Obj[] = [];
  let disk = document, receipts: unknown[] = [], allocated = 0;
  let reserved: ComposerMountedIdentity[] = [];
  let writeHook: (() => Promise<void>) | null = null;
  let readHook: (() => Promise<unknown>) | null = null;
  let ackHook: (() => Promise<unknown>) | null = null;
  let idsHook: (() => Promise<string[]>) | null = null;
  let handoffHook: ((request: Obj) => Promise<unknown>) | null = null;
  let resumeHook: ((request: Obj) => Promise<unknown>) | null = null;
  let intakeHook: ((request: Obj) => Promise<unknown>) | null = null;
  let holdHook: ((request: Obj) => Promise<unknown>) | null = null;
  const storage: Files = { fs: {
    async mkdir() {}, async readFile() { order.push('load'); return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) {
      const output = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(output))); order.push('save');
      if (writeHook) await writeHook();
      disk = output;
    },
  } };
  const native: Native = { available: true, watch(topic) { order.push(`watch:${topic}`); }, async later(input) {
    const request = obj(input); calls.push(request); order.push(`${request.op}:${request.action ?? ''}`);
    const generation = client.generation;
    if (request.op === 'ids' && idsHook) return { ok: true, generation, value: await idsHook() };
    if (request.op === 'composerEditorHandoff' && request.action === 'resume' && !resumeHook) throw Error('Mock native resume reply unavailable');
    if (request.op === 'composerEditorHandoff' && request.action === 'resume' && resumeHook) return resumeHook(request);
    if (request.op === 'composerEditorHandoff' && handoffHook) return handoffHook(request);
    if ((request.op === 'composerFileHold' || request.op === 'composerPickerIntake' && request.action === 'hold') && holdHook)
      return holdHook(request);
    if (request.op === 'composerPickerIntake' && intakeHook) return intakeHook(request);
    if (request.op === 'composerEditorFinalizations') {
      if (request.action === 'read') return readHook ? readHook() : { ok: true, generation, value: { receipts, reserved } };
      if (ackHook) return ackHook();
      const source = receipts.find(value => obj(value).receiptId === request.receiptId);
      if (obj(source).retired === true)
        reserved = reserved.filter(identity => identity.mountId !== obj(obj(source).identity).mountId);
      receipts = receipts.filter(value => obj(value).receiptId !== request.receiptId);
      return { ok: true, generation, value: { acknowledged: true } };
    }
    if (request.op === 'mobileOutbox') return { ok: true, generation, value: request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] } };
    return { ok: true, generation, value: request.op === 'status' ? { phase: 'disconnected' }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `draft-${++allocated}`)
      : request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] } : {} };
  } };
  const handles = () => mobileDraftRecoveryHandles(client, native, storage);
  const initial = handles(); await client.refresh(initial.native, initial.storage);
  const readyClient = () => {
    Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 3, connection: 'connected',
      configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
    client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
    client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/project' }]; noteNow(client, now);
  };
  readyClient();
  const checked = handles(); await refresh(client, checked.native, checked.storage); readyClient();
  const open = async (session = 'flow', visit = 'draft') => {
    readyClient();
    const chooser = mobileNewTaskFlowView(session, 'choose', '/new', true, true, client, fleet);
    const selected = await mobileNewTaskFlowAction(chooser.owner, 'choose', 'project', '["env","project"]', '', native, storage, client, fleet, now);
    expect(selected.message).toBe('');
    const draft = mobileNewTaskFlowView(session, visit, '/new/draft', true, true, client, fleet);
    expect(draft.ready).toBe(true);
    return { owner: draft.owner, visit, location: '/new/draft', editorId: 'new-task-editor' };
  };
  const route = await open(), key = client.draftKey;
  const resume = async (session = 'resume', generation = 3, draftKey = key) => {
    readyClient(); client.generation = generation;
    const location = `/new/draft?draftId=${encodeURIComponent(draftKey)}`;
    const initial = mobileNewTaskFlowView(session, 'resume', location, true, true, client, fleet);
    const prepared = await mobileNewTaskFlowAction(initial.owner, 'resume', 'prepare', '', '', native, storage, client, fleet, now);
    expect(prepared.message).toBe('');
    const current = mobileNewTaskFlowView(session, 'resume', location, true, true, client, fleet);
    expect(current.ready).toBe(true);
    return { owner: current.owner, visit: 'resume', location, editorId: 'new-task-editor' };
  };
  seed?.(client, key);
  const offer = view(client, route), identity = mounted(offer);
  expect(offer.kind).toBe('mounting');
  expect(view(client, route, event(identity, client.local.drafts[key] ?? '')).kind).toBe(
    draftFiles(client.local).some(file => file.draftKey === key && file.source === 'attached') ? 'mounting' : 'editing');
  reserved.push(identity);
  writes.length = 0; calls.length = 0; order.length = 0;
  const reread = async () => { const current = handles(); await refresh(client, current.native, current.storage); };
  const detach = () => {
    mobileNewTaskFlowView('flow', 'away', '/', false, true, client, fleet);
    Object.assign(client, { environmentId: 'other', projectId: 'other-project', threadId: 'other-thread', generation: 4 });
    client.local.drafts['other:other-thread'] = 'unrelated';
  };
  const writeText = (text: string) => {
    const guard = mobileNewTaskContextGuard(client, key);
    if (!guard || !mobileNewTaskContextWrite(client, guard, text)) throw Error('write refused');
  };
  return { client, native, storage, route, key, identity, order, writes, calls, open, resume, detach, reread, writeText,
    disk: () => obj(JSON.parse(disk)), receipts: (next: unknown[]) => {
      receipts = next;
      for (const source of next) {
        const identity = obj(source).identity;
        if (typeof identity === 'object' && identity !== null && !Array.isArray(identity)) {
          const mount = { owner: str(identity.owner), editorId: str(identity.editorId), routeVisit: str(identity.routeVisit),
            renderEpoch: str(identity.renderEpoch), mountId: str(identity.mountId) };
          if (!reserved.some(previous => previous.mountId === mount.mountId)) reserved.push(mount);
        }
      }
    }, reserved: (next: ComposerMountedIdentity[]) => { reserved = next; },
    writeHook: (next: typeof writeHook) => { writeHook = next; }, readHook: (next: typeof readHook) => { readHook = next; },
    ackHook: (next: typeof ackHook) => { ackHook = next; }, idsHook: (next: typeof idsHook) => { idsHook = next; },
    resumeHook: (next: typeof resumeHook) => { resumeHook = next; },
    handoffHook: (next: typeof handoffHook) => { handoffHook = next; },
    intakeHook: (next: typeof intakeHook) => { intakeHook = next; }, holdHook: (next: typeof holdHook) => { holdHook = next; } };
}

export const operationId = '22222222-2222-4222-8222-222222222222';
export type Fixture = Awaited<ReturnType<typeof fixture>>;
export function frozenSource(f: Fixture, request: Obj, value = '🙂 picked here') {
  return { receiptId, identity: f.identity, retired: false, event: {
    ...event(f.identity, value, 4, 'text'), kind: 'frozen', selection: { start: 3, end: 9 },
    operationId: request.operationId, operationRevision: request.operationRevision,
  } };
}
export function picker(f: Fixture) {
  f.idsHook(async () => [operationId]);
  f.handoffHook(async request => {
    const source = frozenSource(f, request); f.receipts([source]);
    return { ok: true, generation: request.generation, value: { receipt: source } };
  });
  return () => action(f.client, f.route, { kind: 'pick', source: 'files' }, f.native, f.storage);
}
export function config(projection: NewTaskEditorView) {
  if (!projection.node) throw Error(projection.message);
  return obj(JSON.parse(projection.node.configuration));
}

export const pickedId = '33333333-3333-4333-8333-333333333333';
export const oldId = '44444444-4444-4444-8444-444444444444';
export const pickedFile: PickerIntakeFile = { kind: 'file', id: pickedId, name: 'note.md', mimeType: 'text/markdown', sizeBytes: 123 };
export const pickedImage: PickerIntakeFile = { kind: 'image', id: pickedId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 234 };
export function intakeReply(request: Obj, status: string, files: readonly PickerIntakeFile[] = []) {
  return { ok: true, generation: request.generation, value: { operationId: request.operationId,
    identity: request.identity, status, files, error: '' } };
}
export function heldReply(input: Obj) {
  const request = input.op === 'composerPickerIntake' ? obj(input.request) : input;
  return { ok: true, generation: request.generation, value: request.action === 'release'
    ? { status: 'released', requestId: request.requestId } : { status: 'held', receipt: {
      identity: request.identity, requestId: request.requestId, holdId: '55555555-5555-4555-8555-555555555555',
      fileIdentity: '66666666-6666-4666-8666-666666666666', id: obj(request.file).id, sizeBytes: obj(request.file).sizeBytes,
    } } };
}
export function continuation(f: Fixture, files: readonly PickerIntakeFile[] = [pickedFile], source?: string, selection?: { start: number; end: number }) {
  picker(f);
  f.client.config = { ...f.client.config, environment: { capabilities: {
    attachmentUploads: true, fileAttachments: { maxUploadBytes: 50 * 1024 * 1024 },
  } } };
  if (source !== undefined) f.handoffHook(async request => {
    const frozen = frozenSource(f, request, source);
    frozen.event.selection = selection ?? { start: source.length, end: source.length };
    f.receipts([frozen]); return { ok: true, generation: request.generation, value: { receipt: frozen } };
  });
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished'
    : request.action === 'cancel' ? 'cancelled' : 'staged', files));
  f.holdHook(async request => heldReply(request));
  return (kind: 'photos' | 'files' = 'files') => action(f.client, f.route, { kind: 'pick', source: kind }, f.native, f.storage);
}
export const intakeCalls = (f: Fixture, action: string) => f.calls.filter(call => call.op === 'composerPickerIntake' && call.action === action);
export const savedRecord = (f: Fixture) => obj(obj(obj(f.disk().mobileNewTaskDrafts).records)[f.key]);
