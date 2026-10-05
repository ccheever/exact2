// Pure SnapShot destination rules. The client owns effects; these decide only
// which draft a capture may enter and how a failed association is undone.
import { arr, str, type Obj } from './domain';

export type SnapshotIdentity = [origin: string, environmentId: string, projectId: string, threadId: string];

/** A capture's owner is the draft identity at capture time, never the current route. */
export function snapshotIdentity(owner: string, origin: string, environmentId: string): SnapshotIdentity | null {
  let parsed: unknown;
  try { parsed = JSON.parse(owner); } catch { return null; }
  if (!Array.isArray(parsed) || parsed.length !== 4 || !parsed.every(part => typeof part === 'string')) return null;
  return parsed[0] === origin && parsed[1] === environmentId ? parsed as SnapshotIdentity : null;
}

const comparablePath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');

/**
 * Reference useHandleNewThread: defaultProjectRef = orderedProjects[0], where
 * orderedProjects = orderItemsByPreferredIds(projects, projectOrder) keyed by
 * the physical project key or its legacy cwd key. An empty order keeps the
 * shell's source order; no project is no destination.
 */
export function snapshotDefaultProject(projects: Obj[], environmentId: string, projectOrder: readonly string[] = []): string {
  if (!projectOrder.length) return str(projects[0]?.id);
  const keys = (project: Obj) => {
    const path = comparablePath(project.workspaceRoot);
    return [`${environmentId}:${path}`, `legacy-project-cwd:${path}`];
  };
  for (const preferred of projectOrder) {
    const found = projects.find(project => keys(project).includes(preferred));
    if (found) return str(found.id);
  }
  return str(projects[0]?.id);
}

/** A thread destination must still belong to the same project in the fresh catalog. */
export function snapshotDestinationExists(catalog: Obj, projectId: string, threadId: string): boolean {
  return Boolean(projectId) && arr(catalog.projects).some(project => project.id === projectId)
    && (!threadId || arr(catalog.threads).some(thread => thread.id === threadId && thread.projectId === projectId));
}

/** Undo exactly one capture's association against the CURRENT array; never restore a stale copy. */
export function withoutSnapshot(drafts: Record<string, Obj[]>, key: string, id: string): boolean {
  const current = drafts[key];
  if (!current?.some(image => image.id === id)) return false;
  drafts[key] = current.filter(image => image.id !== id);
  return true;
}

export const snapshotNoProjectMessage = 'Snapshot taken, but no project is available. Add a project, then capture the window again.';
export const snapshotFailureMessage = (id: string, message: string) => `Snapshot failed. Capture ${id}: ${message}`;
