// The timeline's native work before each snapshot: Mermaid layouts
// (timeline-mermaid.ts) and the worktree setup stream (timeline-worktree.ts).
import type { T3Client } from './client';
import type { Native } from './protocol';
import { prepareMermaid } from './timeline-mermaid';
import { syncWorktreeSetup } from './timeline-worktree';

export async function prepareTimeline(client: T3Client, native: Native | null | undefined): Promise<void> {
  await syncWorktreeSetup(client, native);
  await prepareMermaid(client, native);
}
