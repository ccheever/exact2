// The sidebar's client.command() ops (client-ops.ts): thread search, its open
// state and width, project grouping, and the project and project-group writes,
// which re-read the server's projects before and after each change.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { groupingModes, message, projectPath } from './client-shared';
import { startThreadSearch } from './sidebar-presentation';
import { storeSidebarWidth } from './r4-polish-sidebar-width';
import { str, initialShell, applyShell } from './domain';
import { ClientError, type Native, type Files } from './protocol';

/** Thread search, the sidebar's open state and width, and project grouping. */
export async function sidebarOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'grouping-mode') {
      if (!groupingModes.includes(value)) throw new ClientError('Unsupported grouping mode.');
      this.local.groupingMode = value;
    } else if (op === 'grouping-override') {
      const project = this.shell.projects.find(project => project.id === id);
      if (!project) throw new ClientError('That project is no longer available.');
      if (!groupingModes.includes(value) && value !== 'inherit') throw new ClientError('Unsupported grouping mode.');
      const key = `${this.environmentId}:${projectPath(project.workspaceRoot)}`;
      if (value === 'inherit') delete this.local.groupingOverrides[key];
      else this.local.groupingOverrides[key] = value;
    } else if (op === 'search') { this.query = value; startThreadSearch(this, native, value); }
    else if (op === 'sidebar') {
      if (n) { this.local.sidebarWidth = Math.min(4096, Math.max(208, n)); storeSidebarWidth(this); } // a drag ended (the toggle sends 0)
      if (value === 'open' || value === 'closed') this.local.sidebarOpen = value === 'open';
      else if (!n) this.local.sidebarOpen = !this.local.sidebarOpen;
    } else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** Add, rename and remove a project; rename and remove a project group or one of its checkouts. */
export async function sidebarWrites(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'add-project') await addProject.call(this, native, storage, id, value);
    else if (op === 'rename-project' || op === 'remove-project') await manageProject.call(this, native, storage, op, id, value);
    else if (op === 'rename-group' || op === 'remove-group') await manageGroup.call(this, native, storage, op, id, value);
    else if (op === 'remove-group-member') {
      this.shell = applyShell(this.shell, await this.http(native, '/api/orchestration/shell'));
      if (!this.projectGroups().some(group => group.key === value && group.members.some(member => member.id === id))) throw new ClientError('Project group membership changed. Reopen its settings.');
      await manageProject.call(this, native, storage, 'remove-project', id, '');
    }
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
async function addProject(this: T3Client, native: Native, storage: Files, path: string, title: string): Promise<void> {
  if (!path.trim()) throw new ClientError('Enter the project folder on the T3 server.');
  const [commandId, projectId] = await this.ids(native, 2);
  await this.write(native, storage, { method: 'projects.mutate', description: 'Add project', threadId: '', text: '', uncertain: false,
    payload: { type: 'project.create', commandId, projectId, title: title.trim() || path.trim().replace(/\/$/, '').split('/').pop() || 'Project', workspaceRoot: path.trim(), createWorkspaceRootIfMissing: false } });
  this.shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (!this.shell.projects.some(project => project.id === projectId)) throw new ClientError('The project was created but is not yet available. Refresh before selecting it.');
  this.projectId = projectId; this.threadId = ''; this.thread = null; this.threadLive = true; this.chooseDefaults();
}
async function manageGroup(this: T3Client, native: Native, storage: Files, op: string, key: string, title: string): Promise<void> {
  // Resolve the current server members, never a cached list from a dialog.
  this.shell = applyShell(this.shell, await this.http(native, '/api/orchestration/shell'));
  const group = this.projectGroups().find(group => group.key === key);
  if (!group) throw new ClientError('That project group is no longer available.');
  if (op === 'rename-group' && !title.trim()) throw new ClientError('Project title cannot be empty');
  for (const member of group.members) {
    if (!this.projectGroups().some(group => group.key === key && group.members.some(current => current.id === member.id))) {
      throw new ClientError('Project group membership changed. Reopen its settings.');
    }
    try { await manageProject.call(this, native, storage, op === 'rename-group' ? 'rename-project' : 'remove-project', str(member.id), title); }
    catch (error) { throw new ClientError(`Could not ${op === 'rename-group' ? 'rename' : 'remove'} checkout ${str(member.workspaceRoot)}: ${message(error)}`); }
  }
}
async function manageProject(this: T3Client, native: Native, storage: Files, op: string, id: string, title: string): Promise<void> {
  const project = this.shell.projects.find(project => project.id === id);
  if (!project) throw new ClientError('That project is no longer available.');
  if (op === 'rename-project' && !title.trim()) throw new ClientError('Project title cannot be empty');
  const removedThreads = this.shell.threads.filter(thread => thread.projectId === id);
  const [commandId] = await this.ids(native, 1);
  await this.write(native, storage, { method: 'projects.mutate', description: op === 'rename-project' ? 'Rename project' : 'Remove project',
    threadId: '', text: '', uncertain: false, payload: op === 'rename-project'
      ? { type: 'project.update', commandId, projectId: id, title: title.trim() }
      : { type: 'project.delete', commandId, projectId: id, force: true } });
  // Refresh canonical records before selection/default resolution. The live
  // event can arrive after the mutation acknowledgment.
  this.shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (op === 'remove-project') {
    for (const [key, selection] of Object.entries(this.local.selections)) {
      if (key === this.environmentId && selection.projectId === id) delete this.local.selections[key];
    }
    for (const thread of removedThreads) {
      if (thread.projectId === id) delete this.local.drafts[`${this.environmentId}:${thread.id}`];
    }
    delete this.local.drafts[`${this.environmentId}:new:${id}`];
    if (this.projectId === id) {
      this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
      delete this.subscriptions.thread;
      await this.call(native, { op: 'unsubscribe', key: 'thread' });
      this.projectId = str(this.shell.projects[0]?.id); this.threadLive = true; this.chooseDefaults();
    }
  }
  this.ensureSelection(); this.error = '';
}
