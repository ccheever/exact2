// New-thread workspace defaults from shared/projectSettings.ts at T3 Code 9c161462c9
// (MIT, see LICENSE-T3), including the first thread in a newly created project.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { effectiveSetting, parseProjectFile } from '../settings/settings-core';
import { letGo } from '../client/let-go';
import { normalizeProjectPathForComparison as normalize } from '../shared/project-paths';

const files = new WeakMap<T3Client, { key: string; file: Obj | null }>();
export function resolveNewThreadEnvMode(input: { configured: string; source: string; workspaceRoot: string; newProjectsRoot: string; projectHasThreads: boolean }): string {
  if (input.configured !== 'worktree' || input.projectHasThreads || input.source !== 'environment' || !input.workspaceRoot || !input.newProjectsRoot) return input.configured;
  const root = normalize(input.newProjectsRoot), folder = normalize(input.workspaceRoot), separator = root.includes('\\') ? '\\' : '/';
  const prefix = root.endsWith(separator) ? root : `${root}${separator}`;
  return folder.startsWith(prefix) && folder.length > prefix.length && !folder.slice(prefix.length).includes(separator) ? 'local' : input.configured;
}
export function newThreadEnvMode(client: T3Client): string {
  const project = client.shell.projects.find(project => project.id === client.projectId), settings = obj(client.config.settings);
  const key = `${client.environmentId}:${client.generation}:${str(project?.workspaceRoot)}`;
  const resolved = effectiveSetting(settings, client.projectId, 'defaultThreadEnvMode', files.get(client)?.key === key ? files.get(client)!.file : undefined);
  return resolveNewThreadEnvMode({ configured: resolved.value === 'worktree' ? 'worktree' : 'local', source: resolved.source,
    workspaceRoot: str(project?.workspaceRoot), newProjectsRoot: str(client.config.newProjectsRoot), projectHasThreads: client.shell.threads.some(thread => thread.projectId === client.projectId) });
}
/** Read t3.json only when the configured value delegates to it. */
export async function refreshNewThreadContext(client: T3Client, native: Native): Promise<void> {
  if (client.threadId) return;
  const root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);
  const settings = obj(client.config.settings), override = obj(obj(settings.projectSettingsOverrides)[client.projectId]);
  const configured = override.defaultThreadEnvMode ?? settings.defaultThreadEnvMode;
  if (!root || configured === 'worktree' || configured === 'local') return;
  const key = `${client.environmentId}:${client.generation}:${root}`;
  if (files.get(client)?.key === key) return;
  let file: Obj | null = null;
  try { const read = await client.restAccess(native).request('projects.readFile', { cwd: root, relativePath: 't3.json' }); if (read.truncated !== true) file = parseProjectFile(str(read.contents)); }
  catch (error) { if (letGo(error)) throw error; }
  files.set(client, { key, file });
}
