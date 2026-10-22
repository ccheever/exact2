// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r11-upstream-drafts.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r11-upstream: the draft rows' context menu and discarding a draft behind
// the sidebar's undo notice (upstream 95edeb753b; T3 Code, MIT, see LICENSE-T3:
// threadActionMenu.logic.ts buildDraftActionMenuItems, Sidebar.tsx
// handleDraftContextMenu, lib/discardComposerDraft.ts, hooks/showThreadUndoNotice.ts).
// A new-thread draft (`env:new:project`) loses its whole session, workspace choice
// included; a thread's draft only loses its composer content. Undo brings back the
// text and attachments unless the draft has new content; the uploads are released
// once the notice can no longer undo it.
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { nativeTemplate, type MenuItem } from './sidebar-menu';
import { remember } from './sidebar-commands';
import { sidebarSession } from './sidebar-state';
import { draftContext, type DraftContext } from './composer-controls-branch';
import { menuAnchor } from './r12-sidebar-keys';
import { letGo } from './let-go';

/** buildDraftActionMenuItems: Copy (Path, Branch), Project settings, then the destructive Discard draft. */
export function draftMenuItems(options: { hasPath: boolean; hasBranch: boolean; hasProject: boolean }): MenuItem[] {
  return [
    { id: 'copy', label: 'Copy', disabled: !options.hasPath && !options.hasBranch, children: [
      ...(options.hasPath ? [{ id: 'copy-path', label: 'Path' }] : []),
      ...(options.hasBranch ? [{ id: 'copy-branch', label: 'Branch' }] : []),
    ] },
    ...(options.hasProject ? [{ id: 'project-settings', label: 'Project settings' }] : []),
    { id: 'discard', label: 'Discard draft', destructive: true, separatorBefore: true },
  ];
}

const contexts = (client: T3Client): Record<string, DraftContext> => {
  const controls = client.local.composerControls as { contexts?: Record<string, DraftContext> } | undefined;
  return controls ? controls.contexts ??= {} : {};
};
const hasContent = (client: T3Client, key: string) => !!(client.local.drafts[key] ?? '').trim() || (client.local.snapshotDrafts[key] ?? []).length > 0;
const release = (client: T3Client, images: Obj[]) => { for (const image of images) client.local.snapshotReleases.push(str(image.id)); };

/** discardComposerDraft: clears the draft now and offers Undo for five seconds. */
export function discardDraft(client: T3Client, native: Native, key: string, wholeSession: boolean): void {
  const text = client.local.drafts[key], images = client.local.snapshotDrafts[key] ?? [];
  const context = wholeSession ? contexts(client)[key] : undefined;
  if (text === undefined && !images.length) return;
  delete client.local.drafts[key];
  delete client.local.snapshotDrafts[key];
  if (wholeSession) delete contexts(client)[key];
  if (!(text ?? '').trim() && !images.length) return;
  remember(client, native, 'Discarded', `draft:${key}`, 'Failed to restore draft', async () => {
    if (hasContent(client, key)) { release(client, images); throw new ClientError('The draft has new content.'); }
    if (text !== undefined) client.local.drafts[key] = text;
    if (images.length) client.local.snapshotDrafts[key] = images;
    if (context && !contexts(client)[key]) contexts(client)[key] = context;
  }, () => release(client, images));
}

async function copy(client: T3Client, native: Native, text: string, title: string, failure: string): Promise<void> {
  try { await client.restAccess(native).call({ op: 'copyText', text }); pushToast(client, { kind: 'success', title, description: text }); }
  catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: failure, description: error instanceof Error ? error.message : 'An error occurred.', stacked: true }); }
}

/** handleDraftContextMenu for a new-thread draft row (`projectId`). */
export async function draftMenu(client: T3Client, native: Native, projectId: string): Promise<string> {
  const key = `${client.environmentId}:new:${projectId}`;
  const project = client.shell.projects.find(entry => entry.id === projectId);
  if (!hasContent(client, key)) return '';
  const context = draftContext(client, key);
  const workspacePath = context.worktreePath || str(project?.workspaceRoot);
  const hasProject = !!project && client.projectGroups().some(group => group.members.some(member => member.id === projectId));
  const reply = obj(await client.restAccess(native).call({ op: 'sidebarMenu', items: nativeTemplate(draftMenuItems({ hasPath: !!workspacePath, hasBranch: !!context.branch, hasProject })), ...menuAnchor(client) }));
  switch (str(reply.id)) {
    case 'project-settings':
      if (!hasProject) return '';
      sidebarSession(client).navigate = { kind: 'project-settings', projectId };
      return 'sidebar:navigate';
    case 'copy-path': if (workspacePath) await copy(client, native, workspacePath, 'Path copied', 'Failed to copy path'); return '';
    case 'copy-branch': if (context.branch) await copy(client, native, context.branch, 'Branch copied', 'Failed to copy branch'); return '';
    case 'discard': if (hasContent(client, key)) discardDraft(client, native, key, true); return '';
    default: return '';
  }
}
