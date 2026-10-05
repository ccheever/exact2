// Lane r12-threads (driven in lane r13-threads): a No project draft can switch machine (upstream c47f4263f9 and 845ddd9354;
// T3 Code, MIT, see LICENSE-T3: ChatView.tsx logicalProjectEnvironments for an active Scratch
// project, onEnvironmentChange's openScratchProject(target, "Could not switch machine"),
// isEnvironmentChanging's "Preparing machine"; client-runtime projectCommands.ts openScratch, which
// waits up to 10 seconds for the created project to reach the client store before it fails with
// ScratchProjectNotLoadedError). Each machine keeps its own "No project" folder at its own path,
// so they never group as one logical project: every connected machine that offers one is a
// choice, and the folder is created on the machine when it is picked.
import { applyShell, obj, str, type Obj, type Shell } from './domain';
import type { T3Client } from './client';
import { ClientError, bridgeReply, type Native } from './protocol';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { settle, wakeShell } from './r10-connect-timing';

/** The Scratch project was created, but it never reached this client. */
export const SCRATCH_NOT_LOADED = 'The folder for threads without a project has not reached this device yet. Try again.';
/** openScratch's Effect.timeoutOption("10 seconds"), polled every half second. */
export const SCRATCH_WAIT_MS = 10_000, SCRATCH_POLL_MS = 500;

const normalize = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
/** availableScratchWorkspaceRoot: a connected environment's Scratch folder, else ''. */
export const scratchRootOf = (connected: boolean, config: Obj): string => connected ? str(config.scratchWorkspaceRoot).trim() : '';
/** isScratchProject: the project whose workspace root is the environment's Scratch folder. */
export const isScratch = (project: Obj | undefined, root: string): boolean => !!project && !!root && normalize(project.workspaceRoot) === normalize(root);

/**
 * Waits until `has` sees the project, rereading through `reread` between polls, for up to
 * 10 seconds (the store wait in openScratch). Throws SCRATCH_NOT_LOADED when it never arrives.
 */
export async function awaitScratchProject(native: Native, has: () => boolean, reread: () => Promise<void>, wait = SCRATCH_WAIT_MS): Promise<void> {
  for (let waited = 0; !has(); waited += SCRATCH_POLL_MS) {
    if (waited >= wait) throw new ClientError(SCRATCH_NOT_LOADED);
    await settle(native, SCRATCH_POLL_MS);
    await reread().catch(() => undefined);
  }
}

/** The environment option a Scratch draft offers: the machine's own Scratch project, or '' until it is created. */
export type ScratchChoice = { environmentId: string; projectId: string };
/** logicalProjectEnvironments for an active Scratch project on a draft: the focused machine, then every connected one with a Scratch folder. */
export function scratchChoices(client: Pick<T3Client, 'environmentId' | 'projectId'>, entries: Iterable<FleetEntry>): ScratchChoice[] {
  const choices: ScratchChoice[] = [{ environmentId: client.environmentId, projectId: client.projectId }];
  for (const entry of entries) {
    if (entry.environmentId === client.environmentId || entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    const root = scratchRootOf(true, entry.config);
    if (!root) continue;
    choices.push({ environmentId: entry.environmentId, projectId: str(entry.shell.projects.find(project => isScratch(project, root))?.id) });
  }
  return choices;
}

/** Machines a switch is preparing (isEnvironmentChanging): the composer reads "Preparing machine". */
const changing = new WeakMap<T3Client, number>();
export const machineChanging = (client: T3Client): boolean => (changing.get(client) ?? 0) > 0;

/**
 * openScratch on another machine: projects.ensureScratch there, then its shell until the project
 * is in it (at most 10 s). Returns the machine's Scratch project id; a failure is the toast
 * "Could not switch machine" in the caller.
 */
export async function openRemoteScratch(client: T3Client, native: Native, entry: FleetEntry): Promise<string> {
  const remote = EnvironmentFleet.native(native, entry.key), generation = entry.generation;
  const call = async (request: Obj): Promise<Obj> => {
    const reply = await bridgeReply(remote, { ...request, generation });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    if (reply.generation !== generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    return obj(reply.value);
  };
  changing.set(client, (changing.get(client) ?? 0) + 1);
  // Lane r13-threads: a command's state is drawn only when a source reads again. Wake the readers now, so Send
  // reads "Preparing machine" while the machine is prepared, and again when it is done. The composer's status is
  // the snapshot's: it redraws on this wake only once the snapshot watches t3.notify (client.ts refresh).
  await wakeShell(native);
  try {
    const projectId = str((await call({ op: 'request', method: 'projects.ensureScratch', payload: {} })).projectId);
    if (!projectId) throw new ClientError(SCRATCH_NOT_LOADED);
    const has = () => entry.shell.projects.some(project => project.id === projectId);
    await awaitScratchProject(native, has, async () => { entry.shell = applyShell(entry.shell, await call({ op: 'http', path: '/api/orchestration/shell' })); });
    return projectId;
  } finally { changing.set(client, (changing.get(client) ?? 1) - 1); await wakeShell(native); }
}

/** The focused machine's Scratch project after projects.ensureScratch (openScratch's wait, at most 10 s). */
export async function awaitFocusedScratch(client: T3Client, native: Native, projectId: string): Promise<void> {
  const has = () => client.shell.projects.some(project => project.id === projectId);
  await awaitScratchProject(native, has, async () => {
    client.shell = applyShell(client.shell, await client.restAccess(native).http('/api/orchestration/shell'));
  });
}

/** Test seam: whether a shell lists a Scratch project for `root`. */
export const shellHasScratch = (shell: Shell, root: string): boolean => shell.projects.some(project => isScratch(project, root));
