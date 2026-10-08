// Pinned365aa87982 NewTaskFile uses ThreadFileScreen with explicit draft cwd.
// @ref llp/1107.006-review-and-files.decision.md#draft-file-ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { draftContext } from './shared/composer-controls-branch';
import { mobileNewTaskFlowOwns, mobileNewTaskFlowCurrent } from './new-task-flow';
import { reviewFileAccess } from './review-owner';
import { mobileFilePresentation } from './file-data';

interface Read { owner: string; serial: number; checking: boolean; error: string; contents?: string; truncated: boolean }
const reads = new WeakMap<T3Client, Read>();
function context(path: string, location: string, visit: string, client: T3Client) {
  const query = new URLSearchParams(location.split('?')[1] ?? ''), project = client.shell.projects.find(value => value.id === client.projectId);
  const environment = query.get('environmentId')?.trim() || client.environmentId;
  const cwd = query.get('cwd')?.trim() || draftContext(client).worktreePath || str(project?.workspaceRoot);
  const projectName = query.get('projectName')?.trim() || str(project?.title, 'Files');
  const line = Number(query.get('line')); const initialLine = Number.isInteger(line) && line > 0 ? line : 0;
  const owner = JSON.stringify([path, location, visit, client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch, cwd]);
  return { environment, cwd, projectName, initialLine, owner };
}
function state(owner: string, client: T3Client) {
  let read = reads.get(client);
  if (!read || read.owner !== owner) { read = { owner, serial: 0, checking: false, error: '', truncated: false }; reads.set(client, read); }
  return read;
}
export function mobileNewTaskFileSnapshot(path: string, location: string, visit: string, flowOwner: string, dark: boolean, client: T3Client = mobileClient) {
  const scope = context(path, location, visit, client);
  const allowed = mobileNewTaskFlowCurrent(flowOwner, visit, location, client) && mobileNewTaskFlowOwns(flowOwner, visit, client) && scope.environment === client.environmentId;
  const read = allowed ? state(scope.owner, client) : undefined;
  return mobileFilePresentation({ owner: scope.owner, revision: client.revision, projectName: scope.projectName, loading: read?.checking === true,
    error: read?.error ?? '', read: read?.contents !== undefined ? { contents: read.contents, truncated: read.truncated } : undefined }, path, dark, scope.initialLine);
}
export async function mobileNewTaskFileRead(path: string, location: string, visit: string, flowOwner: string, dark: boolean,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  if (!mobileNewTaskFlowCurrent(flowOwner, visit, location, client) || !mobileNewTaskFlowOwns(flowOwner, visit, client))
    throw new ClientError('The draft file route changed.', 'superseded');
  const scope = context(path, location, visit, client), read = state(scope.owner, client), serial = ++read.serial;
  const current = () => reads.get(client) === read && read.serial === serial && context(path, location, visit, client).owner === scope.owner
    && mobileNewTaskFlowCurrent(flowOwner, visit, location, client) && mobileNewTaskFlowOwns(flowOwner, visit, client);
  const check = () => { if (!current()) throw new ClientError('The draft file route changed.', 'superseded'); };
  read.checking = true; read.error = ''; read.contents = undefined;
  try {
    check();
    if (!nativeInput?.available || !client.ready || !scope.cwd || scope.environment !== client.environmentId)
      throw new ClientError('Connect to this draft workspace to view its files.');
    if (!path.trim() || path.includes('\0') || scope.cwd.includes('\0')) throw new ClientError('Choose a file to view.');
    const base = letGoAware(mobileNative(nativeInput));
    const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
      check(); const reply = await base.later(input); check(); return reply;
    } };
    if (!await reviewFileAccess(client, native)) throw new ClientError('This connection cannot read host files.');
    check();
    const result = await client.restAccess(native).request('projects.readFile', { cwd: scope.cwd, relativePath: path });
    check(); read.contents = str(obj(result).contents); read.truncated = obj(result).truncated === true;
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) read.error = error instanceof Error ? error.message : 'File unavailable';
  } finally { if (current()) { read.checking = false; client.revision++; } }
  return mobileNewTaskFileSnapshot(path, location, visit, flowOwner, dark, client);
}
