// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r11-upstream-scratch.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r11-upstream: one way to open a machine's "No project" folder (upstream
// 845ddd9354; T3 Code, MIT, see LICENSE-T3: client-runtime projectCommands.ts
// openScratch / ScratchProjectNotLoadedError, operations/projects.ts
// availableScratchWorkspaceRoot, web useScratchProject.ts). projects.ensureScratch
// finds or creates the Scratch project; it is usable once it is in this client's
// shell, since drafts key off its stored path. The palette's New thread without a
// project and the new-thread heading's No project both open it here.
import { obj, str } from './domain';
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { awaitFocusedScratch } from './r12-threads-scratch';

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
  await awaitFocusedScratch(client, native, projectId); // r12-threads: openScratch waits up to 10 s for the project
  return projectId;
}
