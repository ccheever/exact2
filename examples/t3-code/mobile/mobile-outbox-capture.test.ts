import { expect, test } from 'bun:test';
import { mobileCaptureNewTaskOutbox, type MobileOutboxCaptureFacts, type MobileOutboxDraftPresentation } from './mobile-outbox-capture';
import { T3Client } from './shared/client';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileDraftAttachmentRecord } from './draft-attachment-order';
import { setDraftFiles } from './shared/composer-editor-files';
import { contextId, contextLink } from './shared/composer-editor-menu';

const imageId = '11111111-1111-4111-a111-111111111111', fileId = '22222222-2222-4222-a222-222222222222';
const metadata = { threadId: 'thread', messageId: 'message', commandId: 'command', createdAt: '2026-10-08T02:00:00.000Z' };
const base = (): MobileOutboxDraftPresentation => ({ key: 'new-task:one', environmentId: 'env', projectId: 'project', origin: 'https://home.test',
  createdAt: '2026-10-08T01:00:00.000Z', revision: 4, choices: null, attachmentIds: [], text: '  task\ntext  ', images: [], files: [], workspace: null });
const facts = (): MobileOutboxCaptureFacts => ({ key: 'new-task:one', environmentId: 'env', projectId: 'project', origin: 'https://home.test',
  config: { environment: { capabilities: { attachmentUploads: true, fileAttachments: { maxUploadBytes: 50000000 } } },
    providers: [{ instanceId: 'p', driver: 'codex', enabled: true, installed: true, auth: { status: 'authenticated' }, models: [] }] },
  selectedModel: { instanceId: 'p', model: 'model', options: [] }, defaultRuntimeMode: 'auto', connected: true, canOperate: true,
  planPreferenceLoaded: true, planModeEnabled: false,
  workspace: { canChoose: true, mode: 'local', explicitBranch: null, currentCheckoutBranch: 'main', worktreePath: null, startFromOrigin: true } });
function ready(draft = base(), current = facts()) {
  const result = mobileCaptureNewTaskOutbox(draft, current, metadata);
  if (result.status !== 'ready') throw new Error(result.reason);
  return result;
}
function blocked(draft = base(), current = facts()) {
  const result = mobileCaptureNewTaskOutbox(draft, current, metadata);
  expect(result.status).toBe('blocked');
  return result.status === 'blocked' ? result.reason : '';
}
function mixed(): MobileOutboxDraftPresentation {
  return { ...base(), attachmentIds: [fileId, imageId], text: ` task ${contextLink('file', 'file_one', 'notes')} ${contextLink('image', contextId('image', imageId), 'photo')}`,
    images: [{ id: imageId, name: 'photo.webp', mimeType: 'image/webp', sizeBytes: 40, source: { app: 'camera' } }],
    files: [{ id: fileId, draftKey: 'new-task:one', environmentId: 'env', name: 'notes.mp4', mimeType: 'video/mp4', sizeBytes: 50,
      contextId: 'file_one', source: 'attached', attachmentId: '', status: 'staged', videoWidth: 640, videoHeight: 480 }] };
}
test('actual draft presentation maps mixed accepted order and preserves complete original snapshot', () => {
  const client = new T3Client(), draft = mixed();
  mobileNewTaskDraftStore(client).records[draft.key] = { key: draft.key, environmentId: draft.environmentId, projectId: draft.projectId,
    origin: draft.origin, createdAt: draft.createdAt, revision: draft.revision, choices: draft.choices };
  client.local.drafts[draft.key] = draft.text; client.local.snapshotDrafts[draft.key] = draft.images; setDraftFiles(client.local, draft.files);
  mobileDraftAttachmentRecord(client, draft.key, draft.attachmentIds);
  const actual = mobileNewTaskDraftPresentation(client, draft.key)!;
  const captured = ready(actual), before = JSON.stringify(actual);
  expect(captured.record.attachments.map(file => file.id)).toEqual([fileId, imageId]);
  expect(captured.record.attachments[0]).toMatchObject({ contextId: 'file_one', source: 'attached', videoWidth: 640, videoHeight: 480 });
  expect(captured.record.attachments[1]).toMatchObject({ source: { app: 'camera' }, mimeType: 'image/webp' });
  expect(captured.draftCapture).toEqual(actual); expect(captured.record.text).toBe(actual.text.trim());
  captured.record.attachments[0]!.name = 'changed'; captured.draftCapture.text = 'changed';
  expect(JSON.stringify(actual)).toBe(before); expect(client.local.drafts[draft.key]).toBe(actual.text);
});
test('supplied submit IDs/time differ from draft creation and are never allocated by capture', () => {
  expect(ready().record).toMatchObject(metadata); expect(ready().draftCapture.createdAt).not.toBe(metadata.createdAt);
  expect(ready().record.creation).toEqual({ projectId: 'project', workspaceMode: 'local', branch: 'main', worktreePath: null, startFromOrigin: true });
  expect(ready().record).not.toHaveProperty('title'); expect(ready().presentation.title).toBe('task text');
});
test('extra metadata cannot overwrite the captured destination or schema', () => {
  const extra = { ...metadata, origin: 'https://other.test', environmentId: 'other', schemaVersion: 999 };
  const result = mobileCaptureNewTaskOutbox(base(), facts(), extra);
  expect(result.status).toBe('ready');
  if (result.status === 'ready') expect(result.record).toMatchObject({ origin: 'https://home.test', environmentId: 'env', schemaVersion: 1 });
});
test('raw citation text and original whitespace survive capture independently of title formatting', () => {
  const draft = { ...base(), text: '  [Assistant quote](t3-citation://v1/e/t/m?text=hello)  ' };
  const result = ready(draft); expect(result.record.text).toBe(draft.text.trim()); expect(result.draftCapture.text).toBe(draft.text);
});
test('blank, changed destination, and explicit operation blockers retain input without side effects', () => {
  expect(blocked({ ...mixed(), text: '  ' })).toContain('Enter a task');
  expect(blocked(base(), { ...facts(), key: 'new-task:two' })).toContain('destination changed');
  for (const blockReason of ['Cloning repository', 'Voice recording', 'Context import pending', 'Paste pending'])
    expect(blocked(base(), { ...facts(), blockReason })).toBe(blockReason);
});
test('offline retains captured project/model without a live operate grant or config', () => {
  const draft = { ...base(), choices: { providerId: 'saved', modelId: 'old', modelOptions: [] } };
  const result = ready(draft, { ...facts(), config: null, selectedModel: null, connected: false, canOperate: false });
  expect(result.record.modelSelection).toEqual({ instanceId: 'saved', model: 'old', options: [] });
  expect(result.record.creation?.branch).toBeNull(); expect(result.presentation.queuesInsteadOfStarting).toBe(true);
  expect(blocked(base(), { ...facts(), canOperate: false })).toContain('cannot start');
});
test('ordinary unavailable explicit provider falls back while absent catalog model is retained', () => {
  const draft = { ...base(), choices: { providerId: 'missing', modelId: 'old', modelOptions: [] } };
  expect(ready(draft).record.modelSelection?.instanceId).toBe('p');
  draft.choices.providerId = 'p'; expect(ready(draft).record.modelSelection?.model).toBe('old');
  expect(blocked(draft, { ...facts(), config: { providers: [] }, selectedModel: null })).toContain('Choose a model');
});
test('unavailable Antigravity remains captured offline and blocks only connected submission', () => {
  const draft = { ...base(), choices: { providerId: 'ag', modelId: 'retained', modelOptions: [] } };
  const current = { ...facts(), config: { providers: [], settings: { providerInstances: { ag: { driver: 'antigravity' } } } } };
  expect(blocked(draft, current)).toContain('Antigravity');
  expect(ready(draft, { ...current, connected: false }).record.modelSelection?.instanceId).toBe('ag');
});
test('effective Plan/Build and runtime capture never erase sparse draft choices', () => {
  const draft = { ...base(), choices: { interactionMode: 'plan', runtimeMode: 'full-access' } };
  expect(ready(draft).record).toMatchObject({ interactionMode: 'default', runtimeMode: 'full-access' });
  expect(ready(draft, { ...facts(), planModeEnabled: true }).record.interactionMode).toBe('plan');
  expect(ready(draft, { ...facts(), planPreferenceLoaded: false, planModeEnabled: true }).record.interactionMode).toBe('default');
  const current = facts(); (current.config!.providers as Record<string, unknown>[])[0]!.showInteractionModeToggle = false; current.planModeEnabled = true;
  expect(ready(draft, current).record.interactionMode).toBe('default'); expect(draft.choices.interactionMode).toBe('plan');
  expect(ready().record.runtimeMode).toBe('auto');
});
test('empty options stay distinct from absent options', () => {
  expect(ready().record.modelSelection).toHaveProperty('options', []);
  expect(ready(base(), { ...facts(), selectedModel: { instanceId: 'p', model: 'm' } }).record.modelSelection).not.toHaveProperty('options');
});
test('online upload state controls queue navigation and checkout capture, not local IDs', () => {
  const draft = mixed(), current = facts();
  expect(ready(draft, current).presentation.queuesInsteadOfStarting).toBe(true);
  expect(ready(draft, current).record.creation?.branch).toBeNull();
  current.uploadStates = { [fileId]: 'ready', [imageId]: 'ready' };
  expect(ready(draft, current).presentation.queuesInsteadOfStarting).toBe(false); expect(ready(draft, current).record.creation?.branch).toBe('main');
  current.uploadStates[fileId] = 'failed'; expect(blocked(draft, current)).toBe('Retry or remove the failed attachment');
  expect(ready(draft, { ...current, connected: false }).presentation.queuesInsteadOfStarting).toBe(true);
  current.config = { providers: [], environment: { capabilities: {} } };
  expect(ready(draft, current).presentation.queuesInsteadOfStarting).toBe(false);
});
test('existing uploads require exact captured home/environment and preserve metadata', () => {
  const draft = mixed(); draft.images[0]!.uploadId = 'remote';
  expect(blocked(draft)).toContain('upload owner');
  expect(blocked(draft, { ...facts(), uploadOwners: { [imageId]: { origin: 'https://other.test', environmentId: 'env' } } })).toContain('upload owner');
  const result = ready(draft, { ...facts(), uploadOwners: { [imageId]: { origin: draft.origin, environmentId: draft.environmentId } } });
  expect(result.record.attachments[1]).toMatchObject({ uploadId: 'remote', status: 'ready', uploadEnvironmentId: 'env' });
});
test('ambiguous branch blocks only queued local capture; explicit same-name branch survives offline', () => {
  const draft = { ...base(), workspace: { envMode: 'local', branch: 'main', worktreePath: '' } }, current = facts();
  current.workspace.explicitBranch = undefined;
  expect(ready(draft, current).record.creation?.branch).toBe('main');
  expect(blocked(draft, { ...current, connected: false })).toContain('Confirm the selected branch');
  current.workspace.explicitBranch = 'main'; expect(ready(draft, { ...current, connected: false }).record.creation?.branch).toBe('main');
  current.workspace.explicitBranch = null; expect(ready(draft, { ...current, connected: false }).record.creation?.branch).toBeNull();
});
test('worktree requires explicit base and drops existing path; scratch ignores old workspace', () => {
  const current = facts(); current.workspace = { ...current.workspace, mode: 'worktree', worktreePath: '/old' };
  expect(blocked(base(), current)).toContain('base branch'); current.workspace.explicitBranch = 'base';
  expect(ready(base(), current).record.creation).toMatchObject({ workspaceMode: 'worktree', branch: 'base', worktreePath: null });
  current.workspace.canChoose = false;
  expect(ready(base(), current).record.creation).toEqual({ projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null, startFromOrigin: true });
});
test('local existing worktree and optional project snapshots are preserved without placeholders', () => {
  const current = facts(); current.projectTitle = ''; current.projectCwd = '/repo';
  current.workspace = { ...current.workspace, worktreePath: '/worktree', explicitBranch: 'feature', startFromOrigin: false };
  expect(ready(base(), current).record.creation).toEqual({ projectId: 'project', projectTitle: '', projectCwd: '/repo',
    workspaceMode: 'local', branch: 'feature', worktreePath: '/worktree' });
});
test('local attachment context is generated before upload and repeated references do not duplicate records', () => {
  const draft = mixed(); draft.text += ` ${contextLink('file', 'file_one', 'again')}`;
  const records = ready(draft).record.context!.records as Record<string, unknown>[];
  expect(records).toHaveLength(2); expect(records.map(record => record.attachmentId)).toEqual([fileId, imageId]);
  expect(records[0]).toMatchObject({ name: 'notes.mp4', mimeType: 'video/mp4', kind: 'file' });
});
test('supplied attachment context must match its actual local kind and metadata', () => {
  const draft = mixed(), generated = ready(draft).record.context!;
  const records = generated.records as Record<string, unknown>[];
  for (const patch of [{ kind: 'image' }, { name: 'other' }, { mimeType: 'image/png' }, { sizeBytes: 51 }])
    expect(blocked({ ...draft, context: { version: 1, records: [{ ...records[0], ...patch }, records[1]] } })).toContain('local owner');
  expect(ready({ ...draft, context: generated }).record.context).toEqual(generated);
});
test('actual known context payloads preserved; unresolved/unsupported references remain blocked', () => {
  const record = { version: 1, contextId: 'term', label: 'Terminal', kind: 'terminal', terminalId: 't', terminalLabel: 'Shell', lineStart: 0, lineEnd: 2, text: 'output' };
  const draft = { ...base(), text: contextLink('terminal', 'term', 'Terminal') };
  expect(blocked(draft)).toContain('cannot be resolved');
  expect(ready({ ...draft, context: { version: 1, records: [record] } }).record.context).toEqual({ version: 1, records: [record] });
  expect(blocked({ ...draft, context: { version: 1, records: [{ ...record, lineEnd: -1 }] } })).toContain('invalid context');
  expect(blocked({ ...draft, context: { version: 1, records: [{ ...record, kind: 'element' }] } })).toContain('unsupported');
});
test('context count and known record boundaries are enforced independently of storage decoder', () => {
  const records = Array.from({ length: 200 }, (_, i) => ({ version: 1, contextId: `t${i}`, label: '', kind: 'thread', environmentId: 'env', threadId: `thread${i}`, title: '' }));
  expect((ready({ ...base(), text: records.map(record => contextLink('thread', record.contextId, '')).join(' '), context: { version: 1, records } }).record.context!.records as unknown[]).length).toBe(200);
  expect(blocked({ ...base(), text: [...records.map(record => contextLink('thread', record.contextId, '')), contextLink('thread', 'extra', '')].join(' '), context: { version: 1, records: [...records, { ...records[0], contextId: 'extra' }] } })).toContain('too much context');
  expect(blocked({ ...base(), context: { version: 1, records: [records[0], records[0]] } })).toContain('invalid context');
});
test('unsupported memory-only files, invalid identities and broken order do not produce queue records', () => {
  const draft = mixed(); draft.files[0]!.source = 'pasted-text'; expect(blocked(draft)).toContain('Save the local file bytes');
  draft.files[0]!.source = 'attached'; draft.attachmentIds.reverse(); expect(ready(draft).record.attachments[0]!.id).toBe(imageId);
  draft.attachmentIds.push(imageId); expect(blocked(draft)).toContain('order');
  expect(mobileCaptureNewTaskOutbox(base(), facts(), { ...metadata, createdAt: 'bad' }).status).toBe('blocked');
  expect(blocked({ ...base(), origin: 'https://home.test/path' }, { ...facts(), origin: 'https://home.test/path' })).toContain('owner');
});
