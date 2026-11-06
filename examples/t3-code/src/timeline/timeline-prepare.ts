import { fileMetadataNeeded, refreshFileMetadata } from '../shared/file-metadata';
// The timeline's native work before each snapshot: the worktree setup stream (timeline-worktree.ts), command output, a
// workflow thread's parent detail, and the saved SSH aliases a file link's menu rows read (file-link-menu.ts).
import { refreshNextOpenTurnItemDetail, turnItemDetailsNeeded } from './timeline-item-fetch';
import { syncToolActivityIcons, toolActivityIconsNeeded } from './timeline-tool-icons';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { syncWorktreeSetup } from './timeline-worktree';
import { syncCommandOutputs } from './command-output';
import { refreshSecretAnswer, secretAnswersNeeded } from './secret-answers';
import { syncWorkflowOpening } from './timeline-workflow-opening';
import { loadSshAliases } from '../connections/remote-open';

export async function prepareTimeline(client: T3Client, native: Native | null | undefined): Promise<void> {
  await syncWorktreeSetup(client, native);
  await syncCommandOutputs(client, native);
  await syncWorkflowOpening(client, native); // a workflow thread's opening reads its parent's coordinator row
  if (native?.available) await loadSshAliases(native); // read once, again after an SSH environment is added
}

/** Snapshot exposes readiness; a distinct root mutation owns every pending native call. */
export function timelineReadsNeeded(client: T3Client): boolean {
  return secretAnswersNeeded(client) || turnItemDetailsNeeded(client) || toolActivityIconsNeeded(client) || fileMetadataNeeded(client);
}
export async function refreshTimelineReads(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  if (!await refreshSecretAnswer(client, native) && !await refreshNextOpenTurnItemDetail(client, native) && !await refreshFileMetadata(client, native)) await syncToolActivityIcons(client, native);
}
