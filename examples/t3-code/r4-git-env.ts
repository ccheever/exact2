// "Run on" (lane r4-git), adapted from T3 Code (MIT; see LICENSE-T3):
// components/BranchToolbarEnvironmentSelector.tsx (the panel Select and its static
// label), ChatView.tsx logicalProjectEnvironments / onEnvironmentChange, and
// packages/client-runtime/src/state/projectGrouping.ts (deriveLogicalProjectKey).
// A draft can run on any connected environment that holds the same logical
// project (its repository identity, or its path on one machine). The focused
// environment is T3Client's connection and the others are background transports
// (settings-b-fleet.ts), so choosing another one moves the draft there and
// refocuses the client on it, as opening one of its threads does.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { fleet, isLoopback, type EnvironmentFleet } from './settings-b-fleet';
import { machineKind } from './connections';
import { environmentIndicator } from './shell-details';
import { runOnMenuWidth, workspaceLabels } from './r5-composer-menus';
import { isScratch, openRemoteScratch, scratchChoices, scratchRootOf } from './r12-threads-scratch'; // r12-threads: No project drafts switch machine (c47f4263f9)
import { pushToast } from './toast';
import { letGo } from './let-go';

const normalize = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
/** deriveLogicalProjectKey for one grouping mode. */
export function logicalProjectKey(project: Obj, environmentId: string, mode: string): string {
  const physical = `${environmentId}:${normalize(project.workspaceRoot)}`;
  if (mode === 'separate') return physical;
  const identity = obj(project.repositoryIdentity), canonical = str(identity.canonicalKey);
  if (!canonical) return physical;
  if (mode === 'repository') return canonical;
  const root = normalize(identity.rootPath), path = normalize(project.workspaceRoot);
  const relative = root && path.startsWith(`${root}/`) ? path.slice(root.length + 1) : '';
  return relative ? `${canonical}::${relative}` : canonical;
}
const groupingMode = (client: T3Client, project: Obj, environmentId: string): string =>
  client.local.groupingOverrides?.[`${environmentId}:${normalize(project.workspaceRoot)}`] || client.local.groupingMode || 'repository';

export type EnvironmentOption = { id: string; label: string; machine: string; primary: boolean; projectId: string; selected: boolean };
/** logicalProjectEnvironments: the focused environment first among the primaries, then by label. */
export function environmentOptions(client: T3Client, source: EnvironmentFleet = fleet): EnvironmentOption[] {
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  if (!project || !client.environmentId) return [];
  const key = logicalProjectKey(project, client.environmentId, groupingMode(client, project, client.environmentId));
  const focused = environmentIndicator({ isPrimary: isLoopback(client.origin), available: 1, environmentId: client.environmentId,
    runtimeLabel: str(obj(client.config.environment).label), savedLabel: '', machine: machineKind(client.config) });
  const options: EnvironmentOption[] = [{ id: client.environmentId, label: focused.envLabel, machine: focused.envKind, primary: isLoopback(client.origin), projectId: client.projectId, selected: true }];
  // r12-threads (c47f4263f9): a No project draft offers every machine with a "No project" folder, created when picked.
  const scratch = !client.threadId && isScratch(project, scratchRootOf(client.connection === 'connected', client.config))
    ? new Map(scratchChoices(client, source.entries.values()).map(choice => [choice.environmentId, choice.projectId])) : null;
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    const match = scratch ? (scratch.has(entry.environmentId) ? { id: scratch.get(entry.environmentId) } : undefined)
      : entry.shell.projects.find(candidate => logicalProjectKey(candidate, entry.environmentId, groupingMode(client, candidate, entry.environmentId)) === key);
    if (!match) continue;
    const label = environmentIndicator({ isPrimary: isLoopback(entry.origin), available: 2, environmentId: entry.environmentId,
      runtimeLabel: str(obj(entry.config.environment).label), savedLabel: '', machine: machineKind(entry.config) });
    options.push({ id: entry.environmentId, label: label.envLabel, machine: label.envKind, primary: isLoopback(entry.origin), projectId: str(match.id), selected: false });
  }
  return options.sort((left, right) => left.primary !== right.primary ? (left.primary ? -1 : 1) : left.label.localeCompare(right.label));
}

/** The environment row: a picker for an unstarted draft with somewhere else to run, else the static label. */
export function environmentView(client: T3Client) {
  const options = environmentOptions(client);
  // envLocked: a started thread (messages or a runtime) keeps its machine; the Select needs onEnvironmentChange (a draft).
  const pick = !client.threadId && options.length > 1;
  return { envPick: pick, envOptions: pick ? options : [], envCount: Math.max(1, options.length) };
}

/**
 * onEnvironmentChange for a draft: the same draft (text and workspace choices)
 * moves to the target environment's copy of the project, and the client focuses
 * that environment. Returns the connection status to adopt.
 */
export async function runOnEnvironment(client: T3Client, native: Native, environmentId: string, source: EnvironmentFleet = fleet): Promise<{ status: Obj | null; generation: number }> {
  if (client.threadId) throw new ClientError('A started thread keeps its environment.');
  if (environmentId === client.environmentId) return { status: null, generation: -1 };
  const target = environmentOptions(client, source).find(option => option.id === environmentId);
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!target || !entry) throw new ClientError('That environment is no longer connected.');
  let projectId = target.projectId;
  if (!projectId) {
    const original = client.draftKey;
    // r12-threads (845ddd9354): the machine's "No project" folder is created now; a failure keeps the draft where it is.
    try { projectId = await openRemoteScratch(client, native, entry); }
    catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Could not switch machine', description: error instanceof Error && error.message ? error.message : 'An error occurred.' });
      return { status: null, generation: -1 };
    }
    if (client.threadId || client.draftKey !== original) return { status: null, generation: -1 };
  }
  const from = client.draftKey, to = `${environmentId}:new:${projectId}`;
  const text = client.local.drafts[from];
  if (text) { client.local.drafts[to] = text; delete client.local.drafts[from]; }
  const contexts = (client.local.composerControls as { contexts?: Record<string, Obj> }).contexts ??= {};
  if (contexts[from]) { contexts[to] = { ...contexts[from], worktreePath: '' }; delete contexts[from]; }
  client.local.selections[environmentId] = { projectId, threadId: '' };
  source.forget(entry.key);
  await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => {});
  const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, credential: '' });
  if (!reply.ok) throw new ClientError(reply.error!.message);
  return { status: obj(reply.value), generation: reply.generation };
}

/**
 * MobileRunContextSelector: the strip's workspace control names the machine when
 * there is a machine to pick (or the only one is remote), and its menu leads with
 * the "Run on" group. The menu sizes to its longest label, as the reference's does.
 */
export const NO_RUN_ON = { envShow: false, envMachine: 'server', envMachineLabel: '', envOptions: [] as EnvironmentOption[], envMenuWidth: 160, originShown: false, originOn: false };
export function stripRunOn(client: T3Client, worktree = false): typeof NO_RUN_ON {
  const options = environmentOptions(client), focused = options.find(option => option.selected);
  const pick = !client.threadId && options.length > 1;
  if (!focused || !(pick || !focused.primary)) return NO_RUN_ON;
  // r5-composer: the menu's width from its measured labels (r5-composer-menus.ts); the window caps it in the view.
  return { ...NO_RUN_ON, envShow: true, envMachine: focused.machine, envMachineLabel: focused.label, envOptions: pick ? options : [],
    envMenuWidth: runOnMenuWidth(client.presentation, options.map(option => option.label), workspaceLabels(worktree)) };
}
