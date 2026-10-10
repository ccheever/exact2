// @ref llp/1109.005-composer-and-transcript.decision.md#terminal-draft-publication-and-recovery-coordinator
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { mobileQueuedEditOrigin } from './queued-edit-origin';

/** Saved project identity supports local editing, never a fabricated server project,
 * a remote operation grant, or a replacement connection's credentials. */
export function mobileNewTaskRestoredProject(client: T3Client, key: string) {
  const draft = mobileNewTaskDraftLookup(client, key);
  if (!draft || !key.startsWith('new-task:restored-')) return null;
  const matches = Object.entries(obj(obj(client.local).mobileRecoveredDrafts)).filter(([owner, raw]) => {
    const marker = obj(raw);
    return (owner.startsWith('outbox:') || owner.startsWith('outbox-editor:')) && marker.key === key && marker.origin === draft.origin && marker.environmentId === draft.environmentId
      && obj(marker.creation).projectId === draft.projectId;
  });
  if (matches.length !== 1) return null;
  const creation = obj(obj(matches[0][1]).creation);
  return { draft, projectId: draft.projectId, title: str(creation.projectTitle) || 'Unknown project', cwd: str(creation.projectCwd) };
}

export function mobileNewTaskRestoredContext(client: T3Client, key = client.draftKey) {
  const value = mobileNewTaskRestoredProject(client, key);
  return value?.draft.origin === mobileQueuedEditOrigin(client) && value.draft.environmentId === client.environmentId ? value : null;
}
