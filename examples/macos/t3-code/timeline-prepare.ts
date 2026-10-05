// The timeline's native work before each snapshot: Mermaid layouts
// (timeline-mermaid.ts) and the worktree setup stream (timeline-worktree.ts).
import { refreshOpenTurnItemDetails, turnItemDetailsNeeded } from './timeline-item-fetch';
import { syncToolActivityIcons, toolActivityIconsNeeded } from './timeline-tool-icons';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { prepareMermaid } from './timeline-mermaid';
import { syncWorktreeSetup } from './timeline-worktree';

export async function prepareTimeline(client: T3Client, native: Native | null | undefined): Promise<void> {
  await syncWorktreeSetup(client, native);
  await prepareMermaid(client, native);
}

/** Snapshot exposes readiness; a distinct root mutation owns every pending native call. */
export function timelineReadsNeeded(client: T3Client): boolean {
  return turnItemDetailsNeeded(client) || toolActivityIconsNeeded(client);
}
export async function refreshTimelineReads(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  await Promise.all([refreshOpenTurnItemDetails(client, native), syncToolActivityIcons(client, native)]);
}
