// The client.command() ops by area. Each client-ops-<area>.ts holds one area's
// branches as they stood in client.ts: a read group runs before the write
// check and a write group after `requireWrite()`. A group answers false for an
// op that is not its own, and hands its message (and the id/value a branch may
// rewrite) back through `out`. No two groups share an op, so their order here
// decides nothing. A feature adds its ops to its area's group, or adds a file
// and one line below.
import type { T3Client } from './client';
import type { Files, Native } from './protocol';
import { connectionOps } from './client-ops-connection';
import { snapshotOps } from './client-ops-snapshot';
import { settingsOps, settingsWrites } from './client-ops-settings';
import { composerOps, composerWrites } from './client-ops-composer';
import { threadOps, threadWrites } from './client-ops-threads';
import { sidebarOps, sidebarWrites } from './client-ops-sidebar';

export type OpOut = { message: string; id: string; value: string };
export type OpGroup = (this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut) => Promise<boolean>;

export const READ_OPS: OpGroup[] = [connectionOps, snapshotOps, settingsOps, composerOps, threadOps, sidebarOps];
export const WRITE_OPS: OpGroup[] = [settingsWrites, composerWrites, threadWrites, sidebarWrites];

/** Runs `op` in the first group that owns it; false when none does. */
export async function runOps(client: T3Client, groups: OpGroup[], op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  for (const group of groups) if (await group.call(client, op, id, value, n, native, storage, out)) return true;
  return false;
}
