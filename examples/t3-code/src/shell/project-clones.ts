// Tracked project clones (MIT reference, see LICENSE-T3; T3 Code 7c3b3b3605ef:
// packages/contracts/src/projectClone.ts projectCloneDisplayName / projectCloneProgressSummary,
// apps/web/src/components/ProjectCloneToastCoordinator.tsx; T3 Code 74541bab17: the clone and
// file-copy banner and send block of components/ChatView.tsx). The toast coordinator and the banner are pure functions of the
// environment's clone list and the viewed draft; the toast queue is reached through a port so
// the state machine runs in tests. Changes: an action button closes its toast here (toast.ts),
// so a toast the user did not dismiss with × comes back when its clone changes.
import { num, obj, str, type Obj } from '../client/domain';
import type { ToastAction, ToastKind } from './toast';

const STAGE_LABELS: Record<string, string> = {
  connecting: 'Connecting', counting: 'Counting objects', receiving: 'Receiving objects', resolving: 'Resolving deltas', checkout: 'Checking out files',
};
export const projectCloneStageLabel = (stage: string) => STAGE_LABELS[stage] ?? 'Connecting';

/** Display name for a clone: the looked-up `owner/repo`, else the folder being cloned into. */
export function projectCloneDisplayName(snapshot: Obj): string {
  const repository = obj(snapshot.repository);
  if (snapshot.repository && str(repository.nameWithOwner)) return str(repository.nameWithOwner);
  const destination = str(snapshot.destinationPath);
  const segments = destination.split(/[/\\]/).filter(segment => segment.length > 0);
  return segments[segments.length - 1] ?? destination;
}

/** One-line progress summary: `Receiving objects · 45% · 12.3 MiB | 5.0 MiB/s`, or `… · Stalled` once git has printed
 *  nothing for a while (`stalled`, which older servers omit): the detail's transfer speed is then stale. */
export function projectCloneProgressSummary(snapshot: Obj): string {
  const parts = [projectCloneStageLabel(str(snapshot.stage))];
  if (snapshot.percent !== null && snapshot.percent !== undefined) parts.push(`${num(snapshot.percent)}%`);
  if (snapshot.stalled === true) parts.push('Stalled');
  else if (snapshot.detail) parts.push(str(snapshot.detail));
  return parts.join(' · ');
}

/** What the composer knows of an ignored-file copy owed to the project (add-project-machine.ts ProjectFileCopy). */
export type FileCopyState = { phase: 'copying' } | { phase: 'failed'; title: string; failures: string[] };

/** ChatView projectCloneSendBlockReason: the clone first, then the ignored files copied after it. */
export function projectCloneSendBlockReason(clone: Obj | null | undefined, fileCopy: FileCopyState | null = null): string {
  if (clone?.phase === 'running') return 'Cloning repository';
  if (clone && clone.phase !== 'done') return 'Repository not cloned';
  return fileCopy?.phase === 'copying' ? 'Copying files' : '';
}

export type CloneBanner = { id: string; variant: string; title: string; description: string; action: string; actionLabel: string; action2: string; action2Label: string; dismiss: string; dismissLabel: string };
/**
 * ChatView projectCloneBannerItem: a failed file copy (dismissable) leads; then running (Cancel), cancelled /
 * failed (Remove project, Retry).
 */
export function projectCloneBanner(clone: Obj | null | undefined, fileCopy: FileCopyState | null = null, projectId = ''): CloneBanner | null {
  const none = { action: '', actionLabel: '', action2: '', action2Label: '', dismiss: '', dismissLabel: '' };
  if (fileCopy?.phase === 'failed') return { ...none, id: `project-file-copy:${projectId}`, variant: 'error', title: fileCopy.title, description: fileCopy.failures.join('\n'),
    dismiss: 'shelllocal:add-machine-copy-dismiss', dismissLabel: 'Dismiss' };
  if (!clone || clone.phase === 'done') return null;
  const name = projectCloneDisplayName(clone), id = `project-clone:${str(clone.projectId)}`;
  if (clone.phase === 'running') return { ...none, id, variant: 'info', title: `Cloning ${name}`, description: projectCloneProgressSummary(clone), action: 'shell:clone-cancel', actionLabel: 'Cancel' };
  const cancelled = clone.phase === 'cancelled';
  return { ...none, id, variant: cancelled ? 'warning' : 'error', title: cancelled ? `Cancelled cloning ${name}` : `Failed to clone ${name}`,
    description: cancelled ? 'Retry to bring in the repository.' : str(clone.error), action: 'shell:clone-remove', actionLabel: 'Remove project', action2: 'shell:clone-retry', action2Label: 'Retry' };
}

export type CloneToast = { kind: ToastKind; title: string; description: string; timeoutMs: number; action: ToastAction | null; secondary: ToastAction | null; hideCopy: boolean };
/** The toast queue as the coordinator uses it (toastManager.add / update / close). */
export type ToastPort = { add(key: string, toast: CloneToast, onClose: () => void): number; update(id: number, toast: CloneToast): void; close(id: number): void; live(id: number): boolean };
export type TrackedToast = { toastId: number; renderedKey: string; phase: string; dismissed: boolean };

const renderKey = (clone: Obj) => `${str(clone.phase)}:${str(clone.stage)}:${clone.percent ?? ''}:${str(clone.detail)}:${clone.stalled === true}:${str(clone.error)}`;

/**
 * The toast for one snapshot. Open project is the reference's openProject on every environment (handleNewThread with
 * scopeProjectRef(environmentId, projectId)): `new-thread` with the project named on its environment, the sidebar's
 * `fleet:<environmentId>:<id>` form (settings-b-fleet.ts parseFleetThreadId), so it opens that environment's draft even
 * after the window moved to another environment (client-ops-threads.ts).
 */
export function cloneToast(clone: Obj, environmentId: string): CloneToast {
  const name = projectCloneDisplayName(clone), projectId = str(clone.projectId);
  const act = (label: string, op: string): ToastAction => ({ label, op, id: projectId, value: environmentId });
  if (clone.phase === 'running') return { kind: 'loading', title: `Cloning ${name}`, description: projectCloneProgressSummary(clone), timeoutMs: 0, action: act('Cancel', 'shell:clone-cancel'), secondary: null, hideCopy: true };
  if (clone.phase === 'done') return { kind: 'success', title: `Cloned ${name}`, description: str(clone.destinationPath), timeoutMs: 8000,
    action: { label: 'Open project', op: 'new-thread', id: `fleet:${environmentId}:${projectId}`, value: '' }, secondary: null, hideCopy: true };
  // Failed or cancelled: the project stays, pointing at an empty folder.
  const cancelled = clone.phase === 'cancelled';
  return { kind: cancelled ? 'info' : 'error', title: cancelled ? `Cancelled cloning ${name}` : `Failed to clone ${name}`,
    description: cancelled ? str(clone.destinationPath) : str(clone.error, 'The clone failed.'), timeoutMs: 0,
    action: act('Retry', 'shell:clone-retry'), secondary: act('Remove project', 'shell:clone-remove'), hideCopy: cancelled };
}

/** The clones whose progress stays out of toasts (add-project-machine.ts silenceProjectCloneToasts). */
export type Silenced = { has(projectId: string): boolean; add(projectId: string): void };
const NONE_SILENCED: Silenced = { has: () => false, add: () => {} };

/**
 * EnvironmentCloneToasts' effect, one pass: a toast per clone keyed by project, an identical
 * redraw skipped, the toast stepping aside while that project's draft is viewed (which silences
 * its progress for good), a silenced clone's progress and success kept out of toasts, and a clone
 * that leaves the list taking its toast with it unless it settled into the timed success toast.
 */
export function syncCloneToasts(tracked: Map<string, TrackedToast>, clones: Obj[], environmentId: string, viewingProjectId: string, port: ToastPort, silenced: Silenced = NONE_SILENCED): void {
  const seen = new Set<string>();
  for (const clone of clones) {
    const projectId = str(clone.projectId);
    seen.add(projectId);
    const key = renderKey(clone), current = tracked.get(projectId);
    const closeToast = () => { if (current) { port.close(current.toastId); tracked.delete(projectId); } };
    if (projectId === viewingProjectId) {
      silenced.add(projectId);
      closeToast();
      continue;
    }
    if (current?.renderedKey === key) continue;
    if (silenced.has(projectId) && (clone.phase === 'running' || clone.phase === 'done')) { closeToast(); continue; }
    const toast = cloneToast(clone, environmentId);
    // The entry is the one the toast's onClose holds, so it changes in place.
    if (current && (current.dismissed || port.live(current.toastId))) {
      if (!current.dismissed) port.update(current.toastId, toast);
      current.renderedKey = key; current.phase = str(clone.phase);
      continue;
    }
    const entry: TrackedToast = { toastId: 0, renderedKey: key, phase: str(clone.phase), dismissed: false };
    entry.toastId = port.add(`clone:${environmentId}:${projectId}`, toast, () => { entry.dismissed = true; });
    tracked.set(projectId, entry);
  }
  for (const [projectId, entry] of tracked) {
    if (seen.has(projectId)) continue;
    if (entry.phase !== 'done') port.close(entry.toastId);
    tracked.delete(projectId);
  }
}
