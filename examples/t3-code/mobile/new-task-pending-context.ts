// @ref llp/1109.005-composer-and-transcript.decision.md#pending-task-editor-save-and-restart-recovery
import type { T3Client } from './shared/client';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey } from './mobile-new-task-drafts';
import { mobilePendingTaskEditorsSnapshot } from './mobile-pending-task-state';
import { mobileQueuedEditOrigin } from './queued-edit-origin';

/** Captured metadata is presentation context, never an entry in the server shell. */
export function mobileNewTaskPendingContext(client: T3Client) {
  const draft = mobileNewTaskDraftCurrent(client);
  if (!draft || !mobileNewTaskDraftIsPendingKey(draft.key)) return null;
  const markers = mobilePendingTaskEditorsSnapshot(client);
  if (!markers.ready) return null;
  const matching = markers.markers.filter(marker => marker.draftKey === draft.key
    && marker.owner.origin === mobileQueuedEditOrigin(client) && marker.owner.environmentId === client.environmentId
    && marker.baseline.record.creation?.projectId === client.projectId);
  if (matching.length !== 1) return null;
  const marker = matching[0], creation = marker.baseline.record.creation!;
  return { marker, projectId: creation.projectId, title: creation.projectTitle ?? 'Unknown project', cwd: creation.projectCwd ?? '' };
}
