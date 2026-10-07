// r6-polish: the workspace card's project script row, adapted from T3 Code (MIT;
// see LICENSE-T3): ProjectScriptsControl displayMode="panel" (the primary script
// as a split row: its icon and name, a hairline and the chevron that opens every
// script plus "Add project script") and projectScripts.ts primaryProjectScript
// (the first script that is not a worktree setup script, else the first). Running
// a script opens the terminal drawer; "Add project script" opens project Actions.
import { obj, str, type Obj } from './domain';
import { resolveScripts } from './settings-b-actions';
import type { T3Client } from './client';
import { ClientError, type Native, type Files } from './protocol';
import { runTerminalCommand } from './terminal-drawer-view';
import { pushToast } from './toast';
import { letGo } from './let-go';

export type CardScript = { id: string; label: string; icon: string };
export type CardScripts = { scriptName: string; scriptIcon: string; scripts: CardScript[] };
export const NO_SCRIPTS: CardScripts = { scriptName: '', scriptIcon: 'play', scripts: [] };

/** The card's script row for `project` (its resolved scripts under the environment's settings). */
export function cardScripts(settings: Obj, project: Obj | undefined, lastInvoked = ''): CardScripts {
  if (!project) return NO_SCRIPTS;
  const scripts = resolveScripts(obj(settings), project);
  const primary = scripts.find(script => script.id === lastInvoked) ?? scripts.find(script => !script.runOnWorktreeCreate) ?? scripts[0];
  if (!primary) return NO_SCRIPTS;
  return { scriptName: primary.name, scriptIcon: primary.icon || 'play',
    scripts: scripts.map(script => ({ id: script.id, label: script.runOnWorktreeCreate ? `${script.name} (setup)` : script.name, icon: script.icon || 'play' })) };
}


const lastScripts = new WeakMap<T3Client, Map<string, string>>();
export function lastInvokedProjectScript(client: T3Client): string { return lastScripts.get(client)?.get(client.projectId) ?? ''; }
export async function runProjectTerminalScript(client: T3Client, native: Native, storage: Files, scriptId: string): Promise<void> {
  const project = client.shell.projects.find(project => project.id === client.projectId);
  if (!project || !client.threadId) return;
  const scripts = resolveScripts(obj(client.config.settings), project);
  const id = scriptId || lastInvokedProjectScript(client);
  const script = scripts.find(script => script.id === id) ?? (!scriptId ? scripts.find(script => !script.runOnWorktreeCreate) ?? scripts[0] : undefined);
  if (!script) throw new ClientError('That project script is no longer available.');
  let remembered = lastScripts.get(client);
  if (!remembered) { remembered = new Map(); lastScripts.set(client, remembered); }
  remembered.set(client.projectId, script.id);
  try { await runTerminalCommand(client, native, storage, script.command, script.name); }
  catch (error) { if (letGo(error)) throw error; throw new ClientError(error instanceof Error ? error.message : `Failed to run script "${script.name}".`); }
  if (script.autoOpenPreview && str(script.previewUrl)) pushToast(client, { kind: 'error', title: 'Could not open preview',
    description: 'The in-app Browser is not available in this build.' });
}
