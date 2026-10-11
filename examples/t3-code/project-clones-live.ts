// Tracked project clones on the client (MIT reference, see LICENSE-T3; T3 Code 1e2ecbd975:
// components/ProjectCloneToastCoordinator.tsx, hooks/useRemoveClonedProject.ts,
// state/projectClones.ts useProjectClone, components/ChatView.tsx runProjectCloneAction,
// components/CommandPalette.tsx submitAddProjectCloneFlow). The clone list of every
// environment comes from live-streams.ts; this module runs the toast coordinator from the
// shell's view build (as providerUpdates does), answers the toast and banner buttons
// (`shell:clone-*`), and starts a tracked clone from the palette.
import type { T3Client } from './client';
import { applyShell, initialShell, str, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { cloneTracking, liveEnvironment, liveEnvironments, watchLive } from './live-streams';
import { syncCloneToasts, projectCloneBanner, projectCloneSendBlockReason, type TrackedToast, type ToastPort } from './project-clones';
import { dismissToast, pushToast, toasts, updateToast } from './toast';
import { composerNow } from './composer-controls';
import { letGo } from './let-go';

const trackedByClient = new WeakMap<T3Client, Map<string, Map<string, TrackedToast>>>();
function trackedFor(client: T3Client, environmentId: string): Map<string, TrackedToast> {
  let byEnvironment = trackedByClient.get(client);
  if (!byEnvironment) { byEnvironment = new Map(); trackedByClient.set(client, byEnvironment); }
  let tracked = byEnvironment.get(environmentId);
  if (!tracked) { tracked = new Map(); byEnvironment.set(environmentId, tracked); }
  return tracked;
}
function port(client: T3Client): ToastPort {
  return {
    add: (key, toast, onClose) => pushToast(client, { ...toast, action: toast.action ?? undefined, secondary: toast.secondary ?? undefined, stacked: true, key, onClose }),
    update: (id, toast) => updateToast(client, id, { kind: toast.kind, title: toast.title, description: toast.description, timeoutMs: toast.timeoutMs, action: toast.action, secondary: toast.secondary, hideCopy: toast.hideCopy }),
    close: id => dismissToast(client, id),
    live: id => toasts(client).some(toast => toast.id === id),
  };
}

/** The focused environment's tracked clone for the viewed project (useProjectClone), or null. */
export function activeProjectClone(client: T3Client): Obj | null {
  if (!client.projectId || !cloneTracking(client.config)) return null;
  const clones = liveEnvironment(client, null, client.environmentId)?.clones.value ?? [];
  return clones.find(clone => clone.projectId === client.projectId) ?? null;
}
/** The send status and block for the viewed project (composer-controls-view.ts, client-ops-composer.ts send). */
export const projectCloneBlock = (client: T3Client) => projectCloneSendBlockReason(activeProjectClone(client));
export const projectCloneNotice = (client: T3Client) => projectCloneBanner(activeProjectClone(client));

/** ProjectCloneToastCoordinator over every environment, once per shell answer. */
export async function cloneToasts(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  await watchLive(client, native);
  for (const environment of liveEnvironments(client, native)) {
    if (!environment.connected || !cloneTracking(environment.config) || !environment.clones.value) continue;
    // The composer banner shows the same progress while that project's draft is open.
    const viewing = environment.focused && !client.threadId ? client.projectId : '';
    syncCloneToasts(trackedFor(client, environment.environmentId), environment.clones.value, environment.environmentId, viewing, port(client), environment.focused);
  }
}

/**
 * `shell:clone-cancel` / `clone-retry` / `clone-remove` (id: project id, value: environment id;
 * the composer banner sends neither and means the viewed project). Failures toast as the
 * reference's runCloneAction does; the clone's own state comes back on the stream.
 */
export async function cloneCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  const projectId = id || client.projectId, environment = liveEnvironment(client, native, value || client.environmentId);
  const title = op === 'clone-cancel' ? 'Failed to cancel clone' : op === 'clone-retry' ? 'Failed to retry clone' : 'Failed to remove project';
  try {
    if (!environment?.connected) throw new ClientError('That environment is not connected.');
    if (op === 'clone-cancel' || op === 'clone-retry') {
      await environment.request(op === 'clone-cancel' ? 'projectClone.cancel' : 'projectClone.retry', { projectId }, true);
      return '';
    }
    if (op !== 'clone-remove') throw new ClientError(`Unknown clone action: ${op}`);
    // Not forced: a project whose clone never landed has no threads, and if one appeared
    // in the meantime the server refuses rather than silently deleting it.
    if (environment.focused) {
      const access = client.restAccess(native);
      const [commandId] = await access.ids(1);
      await access.write(storage, { method: 'projects.mutate', description: 'Remove project', threadId: '', text: '', uncertain: false,
        payload: { type: 'project.delete', commandId, projectId } });
      client.shell = applyShell(initialShell(), await access.http('/api/orchestration/shell'));
      delete client.local.drafts[`${client.environmentId}:new:${projectId}`];
      if (client.projectId === projectId && !client.threadId) await client.openProjectDraft(native, str(client.shell.projects[0]?.id));
    } else {
      const [commandId] = await environment.ids(1);
      await environment.request('projects.mutate', { type: 'project.delete', commandId, projectId }, true);
    }
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title, description: error instanceof Error ? error.message : 'An error occurred.', stacked: true });
  }
  return '';
}

/**
 * submitAddProjectCloneFlow on a server with clone tracking: `projectClone.start` returns once
 * the project exists and git runs; the palette then closes and the project's draft opens
 * (after up to three shell reads standing in for waitForProject's 3 s, as data sources have no
 * timers). An error before git runs is the "Clone failed" toast and the palette stays.
 */
export async function startTrackedClone(client: T3Client, native: Native, remoteUrl: string, destinationPath: string, title: string): Promise<{ ok: boolean; projectId: string; message: string }> {
  const access = client.restAccess(native);
  const [projectId] = await access.ids(1);
  // Data sources cannot read the clock: the newest wall time a snapshot saw.
  const now = composerNow(client);
  try {
    await access.request('projectClone.start', { projectId, title, createdAt: new Date(now).toISOString(), remoteUrl, destinationPath }, true);
  } catch (error) {
    if (letGo(error)) throw error;
    const text = error instanceof Error ? error.message : 'An error occurred.';
    pushToast(client, { kind: 'error', title: 'Clone failed', description: text, stacked: true });
    return { ok: false, projectId: '', message: text };
  }
  await watchLive(client, native);
  for (let attempt = 0; attempt < 3 && !client.shell.projects.some(project => project.id === projectId); attempt++) {
    try { client.shell = applyShell(initialShell(), await access.http('/api/orchestration/shell')); } catch { break; }
  }
  if (!client.shell.projects.some(project => project.id === projectId)) {
    pushToast(client, { kind: 'error', title: 'Failed to open project', description: 'The clone started. The project will appear in the sidebar once this client catches up.', stacked: true });
    return { ok: true, projectId: '', message: '' };
  }
  return { ok: true, projectId: projectId!, message: '' };
}
