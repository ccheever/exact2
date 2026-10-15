// The "Sidebar (legacy)" gestures (LegacySidebar.tsx, T3 Code 1e2ecbd975; MIT,
// see LICENSE-T3): project expansion, Show more / Show less, Sidebar options,
// manual project order, the project and thread context menus, the
// multi-selection menu, the project New thread button, inline archive, and the
// Rename project / Project grouping / remove project flows with the
// reference's toasts. Local ops arrive as `sidebarlocal:legacy-*`, server ops
// as `sidebar:legacy-*` (sidebar-commands.ts routes both here).
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { canArchive, nativeTemplate, type MenuItem } from './sidebar-menu';
import { sidebarPrefs, sidebarSession } from './sidebar-state';
import { menuAnchor, isMenuKey, withMenuAnchor } from './r12-sidebar-keys';
import { applyDeviceSetting } from './settings-core';
import { archive, deleteThreads, markUnread, remember, rename } from './sidebar-commands';
import { buildMultiSelectThreadContextMenuItems, clampSidebarThreadPreviewCount, legacyProjectMenuItems, legacyThreadMenuItems, memberActionLabel, resolveProjectExpanded } from './legacy-sidebar-model';
import { emptyDialog, expansionKeys, legacyGroups, legacySession, previewCount, sortedGroups, type LegacyGroup } from './legacy-sidebar-view';
import { openBranchDraft, openRowPullRequest, rowThread } from './legacy-sidebar-links';
import { openRowPort } from './legacy-sidebar-ports';
import { archiveBackground, backgroundProjectThreads, backgroundRef, backgroundThreadPath, focusBackground, forgetBackgroundDrafts, isBackground, markUnreadBackground,
  openBackgroundDraft, renameBackground, writeBackgroundProject } from './legacy-sidebar-fleet';
import { letGo } from './let-go';

type Local = Parameters<typeof applyDeviceSetting>[0];
interface Navigator { openSelected?(native: Native, id: string): Promise<void>; openDraft?(native: Native, projectId: string): Promise<void> }
const failure = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';
/** The failure toast; a let-go request is rethrown instead (let-go.ts), so nothing after it runs. */
const toast = (client: T3Client, title: string, error: unknown) => { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title, description: failure(error), stacked: true }); };
/** A row's thread: the focused environment's, or a background environment's (`fleet:<environment>:<id>`). */
const threadOf = (client: T3Client, id: string): Obj | undefined => rowThread(client, id)?.thread;
const groupOf = (client: T3Client, key: string): LegacyGroup | undefined => legacyGroups(client).find(group => group.key === key);
function memberOf(client: T3Client, memberId: string) {
  for (const group of legacyGroups(client)) { const member = group.members.find(candidate => candidate.id === memberId); if (member) return { group, member }; }
  return null;
}

/** The sidebar's local ops: view state and client settings that never reach the server. */
export function legacyLocal(client: T3Client, op: string, id: string, value: string): string {
  const prefs = sidebarPrefs(client), session = legacySession(client);
  if (op === 'toggle') {
    const group = groupOf(client, id);
    if (!group) return '';
    sidebarSession(client).selection = [];
    const next = !resolveProjectExpanded(prefs.projectExpanded, expansionKeys(group));
    for (const key of expansionKeys(group)) prefs.projectExpanded[key] = next;
  } else if (op === 'more') session.expandedLists.add(id);
  else if (op === 'less') session.expandedLists.delete(id);
  else if (op === 'sort-projects' || op === 'sort-threads') {
    applyDeviceSetting(client.local as unknown as Local, op === 'sort-projects' ? 'sidebarProjectSortOrder' : 'sidebarThreadSortOrder', value);
  } else if (op === 'preview' || op === 'preview-step') {
    // NumberField onValueChange: an empty or unparsable field changes nothing; a number clamps to 1-15.
    const typed = Number(value.trim());
    const next = op === 'preview-step' ? previewCount(client) + (value === '-1' ? -1 : 1) : value.trim() === '' || !Number.isFinite(typed) ? NaN : typed;
    if (Number.isFinite(next)) applyDeviceSetting(client.local as unknown as Local, 'sidebarThreadPreviewCount', String(clampSidebarThreadPreviewCount(next)));
  } else if (op === 'reorder') reorderGroups(client, id, value);
  else if (op === 'dialog-close') session.dialog = emptyDialog();
  else if (op === 'dialog-grouping') { if (['inherit', 'repository', 'repository_path', 'separate'].includes(value)) session.dialog = { ...session.dialog, grouping: value }; }
  else if (op === 'dialog-title') session.dialog = { ...session.dialog, title: value };
  else if (op === 'rename-start') {
    // startThreadRename on any row, a background environment's included (sidebar-commands' own reads the focused shell).
    const thread = threadOf(client, id), rename = sidebarSession(client);
    if (thread && rename.renameId !== id) { rename.renameId = id; rename.renameTitle = str(thread.title); }
  }
  else throw new ClientError(`Unknown sidebar action: legacy-${op}`);
  return '';
}

/**
 * Manual order: the dragged project goes before `before` ('' for the end). The persisted
 * projectOrder lists every member's physical key in the new group order (reorderProjects).
 */
export function reorderGroups(client: T3Client, dragged: string, before: string): void {
  const groups = sortedGroups(client).map(entry => entry.group);
  const moving = groups.find(group => group.key === dragged);
  if (!moving || dragged === before) return;
  const rest = groups.filter(group => group.key !== dragged);
  const at = before ? rest.findIndex(group => group.key === before) : rest.length;
  rest.splice(at < 0 ? rest.length : at, 0, moving);
  sidebarPrefs(client).projectOrder = rest.flatMap(group => group.members.map(member => member.physicalKey));
}

async function showMenu(client: T3Client, native: Native, items: MenuItem[]): Promise<string> {
  const reply = obj(await client.restAccess(native).call({ op: 'sidebarMenu', items: nativeTemplate(items), ...menuAnchor(client) }));
  return str(reply.id);
}
async function copy(client: T3Client, native: Native, text: string, title: string): Promise<void> {
  try { await client.restAccess(native).call({ op: 'copyText', text }); pushToast(client, { kind: 'success', title, description: text }); }
  catch (error) { toast(client, title === 'Path copied' ? 'Failed to copy path' : 'Failed to copy thread ID', error); }
}
async function attempt(client: T3Client, title: string, run: () => Promise<void>): Promise<boolean> {
  try { await run(); return true; } catch (error) { toast(client, title, error); return false; }
}
async function projectWrite(client: T3Client, native: Native, storage: Files, payload: Obj, description: string): Promise<void> {
  if (isBackground(client, str(payload.projectId))) return writeBackgroundProject(client, native, str(payload.projectId), payload); // the member's own environment
  if (!client.writable) throw new ClientError(client.connection !== 'connected' ? 'Reconnect before making changes.' : 'Wait for synchronization and check your connection permissions.');
  const access = client.restAccess(native), [commandId] = await access.ids(1);
  await access.write(storage, { method: 'projects.mutate', description, threadId: '', text: '', uncertain: false, payload: { ...payload, commandId } });
}

/** The sidebar's server ops and menus. Failures are toasts. */
export async function legacyCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  const session = legacySession(client), selection = sidebarSession(client);
  if (op === 'row-key') return isMenuKey(value) && id ? withMenuAnchor(client, 'center', () => threadMenu(client, native, storage, id)) : '';
  if (op === 'thread-menu') return threadMenu(client, native, storage, id);
  if (op === 'project-menu') return projectMenu(client, native, id);
  if (op === 'new-thread') return newThread(client, native, id);
  if (op === 'pr') return openRowPullRequest(client, native, id, value);
  if (op === 'port') return openRowPort(client, native, id);
  if (op === 'archive') { await attempt(client, 'Failed to archive thread', () => archiveRow(client, native, id)); return ''; }
  if (op === 'rename') {
    // commitRename on any row: a background environment's goes to its own server (rename() reads the focused shell).
    if (!isBackground(client, id)) { await rename(client, native, id, value); return ''; }
    if (selection.renameId !== id) return '';
    selection.renameId = ''; selection.renameTitle = '';
    const thread = threadOf(client, id), trimmed = value.trim();
    if (!trimmed) { pushToast(client, { kind: 'warning', title: 'Thread title cannot be empty' }); return ''; }
    if (!thread || trimmed === str(thread.title)) return '';
    await attempt(client, 'Failed to rename thread', () => renameBackground(client, native, id, trimmed));
    return '';
  }
  if (op === 'rename-project') {
    const found = memberOf(client, id), trimmed = value.trim();
    if (!found) { session.dialog = emptyDialog(); return ''; }
    if (!trimmed) { pushToast(client, { kind: 'warning', title: 'Project title cannot be empty' }); return ''; }
    if (trimmed === found.member.title) { session.dialog = emptyDialog(); return ''; }
    if (await attempt(client, 'Failed to rename project', () => projectWrite(client, native, storage, { type: 'project.update', projectId: id, title: trimmed }, 'Rename project'))) session.dialog = emptyDialog();
    return '';
  }
  if (op === 'grouping-save') {
    const found = memberOf(client, session.dialog.memberId), grouping = session.dialog.grouping;
    session.dialog = emptyDialog();
    if (!found) return '';
    if (grouping === 'inherit') delete client.local.groupingOverrides[found.member.physicalKey];
    else client.local.groupingOverrides[found.member.physicalKey] = grouping;
    return '';
  }
  if (op === 'remove-anyway') { openRemoval(client, id); return ''; }
  if (op === 'dialog-confirm') {
    const dialog = session.dialog;
    session.dialog = emptyDialog();
    if (dialog.kind === 'remove-project') await removeProject(client, native, storage, dialog.memberId, dialog.title);
    if (dialog.kind === 'archive-many') return legacyCommand(client, native, storage, 'archive-many', '', dialog.ids.join(','));
    return '';
  }
  if (op === 'archive-many') {
    const ids = value.split(',').filter(Boolean);
    let failed: unknown = null;
    for (const threadId of ids) { try { await archiveRow(client, native, threadId); } catch (error) { failed = error; break; } }
    if (failed) toast(client, 'Failed to archive threads', failed);
    // removeFromSelection: the archived threads leave the selection (all of them when every archive landed).
    selection.selection = failed ? selection.selection.filter(key => threadOf(client, key)?.archivedAt == null) : selection.selection.filter(key => !ids.includes(key));
    return '';
  }
  throw new ClientError(`Unknown sidebar action: legacy-${op}`);
}

/** handleRowContextMenu: a selected row opens the multi-selection menu; any other clears the selection first. */
async function threadMenu(client: T3Client, native: Native, storage: Files, id: string): Promise<string> {
  const selection = sidebarSession(client), settings = client.local.clientSettings;
  if (selection.selection.length > 0 && selection.selection.includes(id)) {
    // readThreadShell per selected key: a background environment's rows keep their `fleet:` keys.
    const ids = selection.selection.filter(key => !!threadOf(client, key)), threads = ids.map(key => threadOf(client, key)!);
    const choice = await showMenu(client, native, buildMultiSelectThreadContextMenuItems({ count: selection.selection.length, hasRunningThread: threads.some(thread => !canArchive(thread)) }));
    if (choice === 'mark-unread') {
      for (const threadId of ids) await markUnreadRow(client, native, threadId);
      selection.selection = [];
    } else if (choice === 'archive') {
      if (settings?.confirmThreadArchive) { legacySession(client).dialog = { ...emptyDialog(), kind: 'archive-many', ids }; return ''; }
      return legacyCommand(client, native, storage, 'archive-many', '', ids.join(','));
    } else if (choice === 'delete') {
      if (settings?.confirmThreadDelete !== false) { selection.dialog = { kind: 'delete-many', threadIds: ids, title: '' }; return ''; }
      await deleteThreads(client, native, storage, ids, true); // the worktree question
    }
    return '';
  }
  if (selection.selection.length) selection.selection = [];
  const thread = threadOf(client, id);
  if (!thread) return '';
  const choice = await showMenu(client, native, legacyThreadMenuItems(str(thread.branch)));
  const background = backgroundRef(client, id);
  switch (choice) {
    case 'new-thread-on-branch': {
      // The explicit branch carry-over (its worktree, or its branch on the local checkout); a background thread's on its own environment.
      if (!background) return openBranchDraft(client, native, thread, str(thread.projectId));
      const worktreePath = str(thread.worktreePath);
      try { await openBackgroundDraft(client, native, `fleet:${background.environmentId}:${str(thread.projectId)}`, { branch: str(thread.branch), worktreePath, envMode: worktreePath ? 'worktree' : 'local' }); }
      catch (error) { toast(client, 'Could not create thread', error); return ''; }
      return 'sidebar:new-thread';
    }
    case 'rename': selection.renameId = id; selection.renameTitle = str(thread.title); return '';
    case 'mark-unread': await markUnreadRow(client, native, id); return '';
    case 'copy-path': {
      const path = background ? backgroundThreadPath(client, id) : str(thread.worktreePath) || str(client.shell.projects.find(project => project.id === thread.projectId)?.workspaceRoot);
      if (!path) { pushToast(client, { kind: 'error', title: 'Path unavailable', description: 'This thread does not have a workspace path to copy.', stacked: true }); return ''; }
      await copy(client, native, path, 'Path copied'); return '';
    }
    case 'copy-thread-id': await copy(client, native, background?.id ?? id, 'Thread ID copied'); return '';
    case 'project-settings': return projectSettings(client, native, background ? `fleet:${background.environmentId}:${str(thread.projectId)}` : str(thread.projectId));
    case 'delete':
      if (settings?.confirmThreadDelete !== false) { selection.dialog = { kind: 'delete', threadIds: [id], title: str(thread.title) }; return ''; }
      await deleteThreads(client, native, storage, [id], false); return '';
    default: return '';
  }
}

/** handleProjectButtonContextMenu: Rename, Group into..., Copy Path, Project settings, Remove. */
async function projectMenu(client: T3Client, native: Native, key: string): Promise<string> {
  const group = groupOf(client, key);
  if (!group) return '';
  const choice = await showMenu(client, native, legacyProjectMenuItems(group.members));
  if (!choice) return '';
  if (choice === 'project-settings') return projectSettings(client, native, (group.members.find(candidate => !candidate.remote) ?? group.members[0]!).id);
  const split = choice.indexOf(':'), action = choice.slice(0, split), physical = choice.slice(split + 1);
  const member = group.members.find(candidate => candidate.physicalKey === physical);
  if (!member) return '';
  const session = legacySession(client);
  // Every action targets the member's own environment, a background one's included.
  if (action === 'rename') session.dialog = { ...emptyDialog(), kind: 'rename-project', memberId: member.id, title: member.title, root: member.workspaceRoot, environment: member.environmentLabel };
  else if (action === 'grouping') session.dialog = { ...emptyDialog(), kind: 'grouping', memberId: member.id, title: member.title, root: member.workspaceRoot, grouping: client.local.groupingOverrides[member.physicalKey] ?? 'inherit' };
  else if (action === 'copy-path') await copy(client, native, member.workspaceRoot, 'Path copied');
  else if (action === 'delete') {
    // handleRemoveProject: a project with threads asks for "Delete anyway" first.
    const count = memberThreads(client, member.id).length;
    if (count > 0) pushToast(client, { kind: 'warning', title: 'Project is not empty', description: 'Delete all threads in this project before removing it.', stacked: true, actionVariant: 'destructive',
      action: { label: 'Delete anyway', op: 'sidebar:legacy-remove-anyway', id: member.id }, closeOnAction: true });
    else openRemoval(client, member.id);
  }
  return '';
}
/** A member's threads on its own environment (memberThreadCountByPhysicalKey). */
const memberThreads = (client: T3Client, memberId: string): Obj[] =>
  isBackground(client, memberId) ? backgroundProjectThreads(client, memberId) : client.shell.threads.filter(thread => thread.projectId === memberId);
function openRemoval(client: T3Client, memberId: string): void {
  const found = memberOf(client, memberId);
  if (!found) return;
  const threads = memberThreads(client, memberId).length;
  legacySession(client).dialog = { ...emptyDialog(), kind: 'remove-project', memberId, title: found.member.title, root: found.member.workspaceRoot, environment: found.member.environmentLabel, threads };
}
async function removeProject(client: T3Client, native: Native, storage: Files, memberId: string, title: string): Promise<void> {
  const threads = memberThreads(client, memberId);
  const ok = await attempt(client, `Failed to remove "${title}"`, () => projectWrite(client, native, storage, { type: 'project.delete', projectId: memberId, force: true }, 'Remove project'));
  if (!ok) return;
  if (isBackground(client, memberId)) { forgetBackgroundDrafts(client, memberId, threads); return; }
  for (const thread of client.shell.threads) if (thread.projectId === memberId) delete client.local.drafts[`${client.environmentId}:${str(thread.id)}`];
  delete client.local.drafts[`${client.environmentId}:new:${memberId}`];
}

/** handleCreateThreadClick: one member creates at once; several ask which (a native menu of members). */
async function newThread(client: T3Client, native: Native, key: string): Promise<string> {
  const group = groupOf(client, key);
  if (!group) return '';
  let member = group.members.length === 1 ? group.members[0] : undefined;
  if (!member) {
    const choice = await showMenu(client, native, group.members.map(candidate => ({ id: candidate.physicalKey, label: memberActionLabel(candidate, group.members.length) })));
    member = group.members.find(candidate => candidate.physicalKey === choice);
  }
  if (!member) return '';
  // createThreadForProjectMember: a background member's draft on its own environment.
  try { if (member.remote) await openBackgroundDraft(client, native, member.id); else await (client as unknown as Navigator).openDraft?.(native, member.id); }
  catch (error) { toast(client, 'Could not create thread', error); return ''; }
  return 'sidebar:new-thread';
}

/** Archive, mark unread and settings on any row: the focused environment's through sidebar-commands, a background one's through its own transport. */
const archiveRow = (client: T3Client, native: Native, id: string): Promise<void> => isBackground(client, id) ? archiveBackground(client, native, id, remember) : archive(client, native, id);
/** useMarkThreadUnread uses reportFailure:false; a thread without a completed run can be refused silently. */
async function markUnreadRow(client: T3Client, native: Native, id: string): Promise<void> {
  try { await (isBackground(client, id) ? markUnreadBackground(client, native, id) : markUnread(client, native, id)); }
  catch (error) { if (letGo(error)) throw error; }
}
/**
 * "Project settings" (/projects/$projectKey → Settings › Projects scoped to the project): a background member's
 * environment becomes the focus first, as Settings › Projects lists the focused environment's projects (a declared
 * difference: the reference's page spans every environment's project groups, and this page reads the focused one's).
 */
async function projectSettings(client: T3Client, native: Native, memberId: string): Promise<string> {
  const background = backgroundRef(client, memberId);
  if (background) {
    try { await focusBackground(client, native, background.environmentId); }
    catch (error) { toast(client, 'Could not open project settings', error); return ''; }
  }
  return `sidebar:project-settings:${background?.id ?? memberId}`;
}
