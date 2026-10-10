// Pinned365aa87982 NewTaskFile uses ThreadFileScreen with explicit draft cwd.
// @ref llp/1109.006-review-and-files.decision.md#draft-file-ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { draftContext } from './shared/composer-controls-branch';
import { mobileNewTaskFileRouteCurrent, mobileNewTaskRoute } from './new-task-flow';
import { newTaskFileEndpointIdentity, readStandaloneNewTaskFile } from './new-task-file-source';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { reviewFileAccess } from './review-owner';
import { mobileFilePresentation } from './file-data';

interface Read { owner: string; serial: number; checking: boolean; error: string; contents?: string; truncated: boolean }
const reads = new WeakMap<T3Client, Read>();
function context(path: string, location: string, visit: string, flowOwner: string, client: T3Client, background: EnvironmentFleet) {
  const queryAt = location.indexOf('?'), query = new URLSearchParams(queryAt < 0 ? '' : location.slice(queryAt + 1));
  const project = client.shell.projects.find(value => value.id === client.projectId);
  // Upstream firstRouteParam treats blank as missing but preserves valid path bytes.
  const param = (key: string) => { const value = query.get(key); return value?.trim() ? value : ''; };
  const standalone = mobileNewTaskRoute(location).standaloneFile;
  const environment = param('environmentId') || client.environmentId;
  const cwd = param('cwd') || draftContext(client).worktreePath || str(project?.workspaceRoot);
  const projectName = param('projectName') || str(project?.title, 'Files');
  const line = Number(query.get('line')); const initialLine = Number.isInteger(line) && line > 0 ? line : 0;
  const owner = standalone
    ? JSON.stringify(['file', flowOwner, path, location, visit, environment, cwd, newTaskFileEndpointIdentity(environment, client, background)])
    : JSON.stringify([flowOwner, path, location, visit, client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch, cwd]);
  return { environment, cwd, projectName, initialLine, owner, standalone };
}
function state(owner: string, client: T3Client) {
  let read = reads.get(client);
  if (!read || read.owner !== owner) { read = { owner, serial: 0, checking: false, error: '', truncated: false }; reads.set(client, read); }
  return read;
}
export function mobileNewTaskFileSnapshot(path: string, location: string, visit: string, flowOwner: string, dark: boolean, client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  const scope = context(path, location, visit, flowOwner, client, background);
  const allowed = mobileNewTaskFileRouteCurrent(flowOwner, visit, location, client) && (scope.standalone || scope.environment === client.environmentId);
  const read = allowed ? state(scope.owner, client) : undefined;
  return mobileFilePresentation({ owner: scope.owner, revision: client.revision, projectName: scope.projectName, loading: read?.checking === true,
    error: read?.error ?? '', read: read?.contents !== undefined ? { contents: read.contents, truncated: read.truncated } : undefined }, path, dark, scope.initialLine);
}
export async function mobileNewTaskFileRead(path: string, location: string, visit: string, flowOwner: string, dark: boolean,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  if (!mobileNewTaskFileRouteCurrent(flowOwner, visit, location, client))
    throw new ClientError('The draft file route changed.', 'superseded');
  const scope = context(path, location, visit, flowOwner, client, background), read = state(scope.owner, client), serial = ++read.serial;
  const current = () => reads.get(client) === read && read.serial === serial && context(path, location, visit, flowOwner, client, background).owner === scope.owner
    && mobileNewTaskFileRouteCurrent(flowOwner, visit, location, client);
  const check = () => { if (!current()) throw new ClientError('The draft file route changed.', 'superseded'); };
  read.checking = true; read.error = ''; read.contents = undefined;
  try {
    check();
    if (!nativeInput?.available || !scope.cwd || !scope.standalone && (!client.ready || scope.environment !== client.environmentId))
      throw new ClientError('Connect to this draft workspace to view its files.');
    if (!path.trim() || path.includes('\0') || scope.cwd.includes('\0')) throw new ClientError('Choose a file to view.');
    const base = letGoAware(mobileNative(nativeInput));
    const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
      check(); const reply = await base.later(input); check(); return reply;
    } };
    let result;
    if (scope.standalone) result = await readStandaloneNewTaskFile(scope.environment, scope.cwd, path, native, client, background, check);
    else {
      if (!await reviewFileAccess(client, native)) throw new ClientError('This connection cannot read host files.');
      check();
      result = await client.restAccess(native).request('projects.readFile', { cwd: scope.cwd, relativePath: path });
    }
    check(); read.contents = str(obj(result).contents); read.truncated = obj(result).truncated === true;
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) read.error = error instanceof Error ? error.message : 'File unavailable';
  } finally { if (current()) { read.checking = false; client.revision++; } }
  return mobileNewTaskFileSnapshot(path, location, visit, flowOwner, dark, client, background);
}
