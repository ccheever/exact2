// r6-polish: the workspace card's project script row, adapted from T3 Code (MIT;
// see LICENSE-T3): ProjectScriptsControl displayMode="panel" (the primary script
// as a split row: its icon and name, a hairline and the chevron that opens every
// script plus "Add project script") and projectScripts.ts primaryProjectScript
// (the first script that is not a worktree setup script, else the first). Running
// a script opens the terminal drawer, which this client excludes (EX01): the run
// controls are shown disabled; "Add project script" opens the project's Actions.
import { obj, type Obj } from './domain';
import { resolveScripts } from './settings-b-actions';

export type CardScript = { id: string; label: string; icon: string };
export type CardScripts = { scriptName: string; scriptIcon: string; scripts: CardScript[] };
export const NO_SCRIPTS: CardScripts = { scriptName: '', scriptIcon: 'play', scripts: [] };

/** The card's script row for `project` (its resolved scripts under the environment's settings). */
export function cardScripts(settings: Obj, project: Obj | undefined): CardScripts {
  if (!project) return NO_SCRIPTS;
  const scripts = resolveScripts(obj(settings), project);
  const primary = scripts.find(script => !script.runOnWorktreeCreate) ?? scripts[0];
  if (!primary) return NO_SCRIPTS;
  return { scriptName: primary.name, scriptIcon: primary.icon || 'play',
    scripts: scripts.map(script => ({ id: script.id, label: script.runOnWorktreeCreate ? `${script.name} (setup)` : script.name, icon: script.icon || 'play' })) };
}
