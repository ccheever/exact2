// @ref llp/1109.005-composer-and-transcript.decision.md#pending-task-editor-save-and-restart-recovery
import { expect, test } from 'bun:test';
import { projectMobileHome, mobileHome, type HomeSource } from './home';
import { mobileHomeView } from './home-state';
import type { HomeDraftInput } from './home-drafts';
import { mobileOutboxOwner, type MobilePendingTask } from './mobile-outbox-presentation';
import { mobilePendingTaskEditorsHydrate, mobilePendingTaskEditorsCreate, type MobilePendingTaskMarker } from './mobile-pending-task-state';
import { mobileNewTaskDraftCreate } from './mobile-new-task-drafts';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { mobileHomeAction } from './home-actions';
const now = Date.parse('2026-10-08T12:00:00.000Z');
function fixture() {
  const owner = { origin: 'https://home.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
  const marker: MobilePendingTaskMarker = { version: 1, owner, session: 'editor', revision: 1, draftKey: 'new-task:pending-message', contentRevision: 0, pending: null,
    baseline: { token: 'epoch:1', revision: 1, record: { schemaVersion: 1, ...owner, text: 'Original queued content', attachments: [],
      createdAt: new Date(now).toISOString(), creation: { projectId: 'project', projectTitle: 'Saved project', workspaceMode: 'local', branch: null, worktreePath: null } } } };
  const draft: HomeDraftInput = { key: marker.draftKey, origin: owner.origin, environmentId: 'env', projectId: 'project',
    createdAt: marker.baseline.record.createdAt, text: 'Newer preserved edit', images: [], files: [], workspace: null };
  const task: MobilePendingTask = { owner: mobileOutboxOwner(marker.baseline.record), record: marker.baseline.record, status: 'queued', reason: '', canRetry: false };
  return { marker, draft, task };
}
test('missing-row editor replaces only its duplicate generic draft and presents current saved text', () => {
  const f = fixture(), before = JSON.stringify(f);
  const result = projectMobileHome([], now, { drafts: [f.draft], pendingEditors: [f.marker] });
  expect(result.items).toHaveLength(1);
  expect(result.items[0]).toMatchObject({ kind: 'pending', queuedOwner: f.task.owner, title: 'Newer preserved edit',
    environmentId: 'env', threadId: 'thread', status: 'Saved changes need attention', projectTitle: 'Saved project' });
  expect(result.items[0].menuItems).toEqual([]); expect(JSON.stringify(f)).toBe(before);
  expect(projectMobileHome([], now, { drafts: [f.draft], pendingEditors: [f.marker], query: 'newer' }).items).toHaveLength(1);
  expect(projectMobileHome([], now, { drafts: [f.draft], pendingEditors: [f.marker], query: 'original' }).items).toHaveLength(0);
});
test('queue and retained editor share one row while synchronized threads never conceal saved edits', () => {
  const f = fixture();
  expect(projectMobileHome([], now, { drafts: [f.draft], pendingTasks: [f.task], pendingEditors: [f.marker] }).items).toHaveLength(1);
  const source: HomeSource = { environmentId: 'env', label: 'Home', machine: '', config: {}, focused: true,
    shell: { sequence: 1, projects: [{ id: 'project', title: 'P' }], threads: [{ id: 'thread', projectId: 'project', title: 'Synchronized thread' }] } };
  const result = projectMobileHome([source], now, { drafts: [f.draft], pendingTasks: [{ ...f.task, status: 'delivered' }], pendingEditors: [f.marker] });
  expect(result.items.filter(item => item.kind === 'pending')).toHaveLength(1);
  expect(result.items.some(item => item.kind === 'draft')).toBe(false);
});
test('unowned internal drafts stay visible and a mismatched stamp cannot hide unrelated content', () => {
  const f = fixture();
  expect(projectMobileHome([], now, { drafts: [f.draft] }).items[0].kind).toBe('draft');
  const other = { ...f.draft, environmentId: 'other-env', text: 'Other owner content' };
  const result = projectMobileHome([], now, { drafts: [other], pendingEditors: [f.marker], environmentId: 'other-env' });
  expect(result.items).toHaveLength(1); expect(result.items[0]).toMatchObject({ kind: 'draft', title: 'Other owner content' });
});
test('actual Home wrapper exposes validated marker recovery without queue or draft metadata', async () => {
  const f = fixture(), client = new MobileDraftClient(), background = new EnvironmentFleet();
  const native: Native = { available: true, watch() {}, async later(input) { return { ok: true, generation: client.generation,
    value: obj(input).op === 'status' ? { phase: 'disconnected' } : {} }; } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}'); }, async atomicWriteFile() {} } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  mobilePendingTaskEditorsHydrate(client, {}); expect(mobilePendingTaskEditorsCreate(client, f.marker)).not.toBeNull();
  const args = [0, now, '', 10, true, false, false, false, false, '', '', '', 'repository', 'home', true, false, []];
  const view = mobileHomeView(args, client, background), row = view.items.find(item => item.kind === 'pending')!;
  expect(row.queuedOwner).toBe(f.task.owner); expect(view.emptyTitle).toBe(''); expect(view.addEnvironment).toBe(false);
  const opened = await mobileHomeAction('home', row.environmentId, row.threadId, 'pending-open', row.queuedOwner, now, null, client, background);
  expect(opened.nextLocation).toBe('/new/draft?environmentId=env&projectId=project&pendingTaskId=message');
  const draft = mobileNewTaskDraftCreate(client, { id: 'pending-message', origin: 'https://home.test', environmentId: 'env', projectId: 'project', createdAt: new Date(now).toISOString() });
  client.local.drafts[draft.key] = 'Recovered actual draft';
  const withDraft = mobileHomeView(args, client, background);
  expect(withDraft.items.filter(item => ['pending', 'draft'].includes(item.kind))).toHaveLength(1);
  expect(withDraft.items.find(item => item.kind === 'pending')?.title).toBe('Recovered actual draft');
  expect(mobileHome(now, {}, client, background).items.some(item => item.kind === 'pending')).toBe(true);
});
