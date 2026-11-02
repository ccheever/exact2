// One way to open a machine's "No project" folder (upstream
// 845ddd9354; T3 Code, MIT, see LICENSE-T3: client-runtime projectCommands.ts
// openScratch / ScratchProjectNotLoadedError, operations/projects.ts
// availableScratchWorkspaceRoot, web useScratchProject.ts at v0.1.5). projects.ensureScratch
// finds or creates the Scratch project; it is usable once it is in this client's
// shell, since drafts key off its stored path. The palette's New thread without a
// project and the new-thread heading's No project both open it here, on the machine
// scratchEnvironmentId picks: the primary (this Mac) whenever it offers the folder.
import { obj, str } from '../client/domain';
import type { T3Client } from '../client/client';
import { ClientError, type Native } from '../client/protocol';
import { awaitFocusedScratch, openRemoteScratch } from './scratch-machine';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { primaryEnvironmentId } from '../connections/local-primary';

/** The Scratch project was created, but it never reached this client. */
export const SCRATCH_NOT_LOADED = 'The folder for threads without a project has not reached this device yet. Try again.';

/** availableScratchWorkspaceRoot: the folder a connected environment offers threads without a project, else null. */
export function availableScratchWorkspaceRoot(connection: string | null | undefined, config: unknown): string | null {
  if (connection !== 'connected') return null;
  const root = str(obj(config).scratchWorkspaceRoot).trim();
  return root || null;
}

/** openScratch: the Scratch project's id once it is in the shell (rereads for up to 10 s when its event has not arrived). */
export async function openScratchProject(client: T3Client, native: Native): Promise<string> {
  const result = await client.rpc(native, 'projects.ensureScratch', {}, true);
  const projectId = str(result.projectId);
  if (!projectId) throw new ClientError(SCRATCH_NOT_LOADED);
  await awaitFocusedScratch(client, native, projectId); // openScratch waits up to 10 s for the project
  return projectId;
}

/** A machine as scratchEnvironmentId weighs it: its connection phase and server config. */
export type ScratchEnvironment = { environmentId: string; connection: string; config: unknown };

/**
 * useScratchProject scratchEnvironmentId: the primary environment wins whenever it offers a folder for threads
 * without a project; `fallback` (the machine on screen) only when it does not; else the one machine that offers
 * one, if exactly one does.
 */
export function scratchEnvironmentId(environments: readonly ScratchEnvironment[], primary: string | null, fallback: string | null): string | null {
  const rootFor = (environmentId: string | null) => {
    const environment = [...environments].reverse().find(entry => entry.environmentId === environmentId);
    return availableScratchWorkspaceRoot(environment?.connection, environment?.config);
  };
  if (primary !== null && rootFor(primary) !== null) return primary;
  if (fallback !== null && rootFor(fallback) !== null) return fallback;
  const offering = environments.filter(entry => rootFor(entry.environmentId) !== null);
  return offering.length === 1 ? offering[0]!.environmentId : null;
}

/** The machines this client can open a Scratch project on: the focused one and every synchronized machine of the fleet. */
export function scratchEnvironments(client: T3Client, source: EnvironmentFleet = fleet): ScratchEnvironment[] {
  const others = [...source.entries.values()].filter(entry => entry.environmentId !== client.environmentId && entry.synchronized === entry.generation)
    .map(entry => ({ environmentId: entry.environmentId, connection: entry.phase, config: entry.config }));
  return [{ environmentId: client.environmentId, connection: client.connection, config: client.config }, ...others];
}

/**
 * The Scratch project for a new thread without a project, on the machine scratchEnvironmentId picks (the machine on
 * screen as the fallback): `environmentId` and `projectId`, or null when no machine offers one.
 */
export async function openPrimaryScratch(client: T3Client, native: Native, source: EnvironmentFleet = fleet): Promise<{ environmentId: string; projectId: string } | null> {
  const environmentId = scratchEnvironmentId(scratchEnvironments(client, source), primaryEnvironmentId(), client.environmentId || null);
  if (environmentId === null) return null;
  if (environmentId === client.environmentId) return { environmentId, projectId: await openScratchProject(client, native) };
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry) throw new ClientError('That environment is no longer connected.');
  return { environmentId, projectId: await openRemoteScratch(client, native, entry) };
}
