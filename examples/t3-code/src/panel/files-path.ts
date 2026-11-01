// The Files breadcrumbs' paths (MIT reference, see LICENSE-T3: T3 Code 2359643cff
// components/files/filePath.ts and packages/shared/src/path.ts): the crumbs of a workspace or host
// path, the trail the crumbs keep after the browser walks up a folder, and host paths inside a
// host folder the browser lists.

export type FileBreadcrumb = { label: string; path: string; kind: 'project' | 'directory' | 'file' };

const isWindowsDrivePath = (value: string) => /^[a-zA-Z]:([/\\]|$)/.test(value);
export const isWindowsAbsolutePath = (value: string) => value.startsWith('\\\\') || isWindowsDrivePath(value);
export const isAbsolutePath = (value: string) => value.startsWith('/') || isWindowsAbsolutePath(value);
/** isHostPath: a host path outside the workspace; the server expands a home-relative one (`~/`). */
export const isHostPath = (value: string) => isAbsolutePath(value) || value.startsWith('~/');

export function fileBreadcrumbs(projectName: string, relativePath: string): FileBreadcrumb[] {
  const hostPath = isHostPath(relativePath);
  const separator = isWindowsAbsolutePath(relativePath) ? '\\' : '/';
  const parts = relativePath.split(/[\\/]/).filter(Boolean);
  const root = relativePath.startsWith('\\\\') ? '\\\\' : relativePath.startsWith('/') ? '/' : '';
  return [
    ...(hostPath ? [] : [{ label: projectName, path: '', kind: 'project' as const }]),
    ...parts.map((part, index) => {
      const path = root + parts.slice(0, index + 1).join(separator);
      // A bare drive (`C:`) means the drive's current directory; its root is `C:\`. A bare `~` is not a host path;
      // the home folder is `~/`.
      return { label: part, path: /^[A-Za-z]:$/.test(path) ? `${path}\\` : hostPath && path === '~' ? '~/' : path, kind: index === parts.length - 1 ? 'file' as const : 'directory' as const };
    }),
  ];
}

/** `relativePath` written the way its own crumb names it, so a link with a trailing or repeated separator still finds its crumb. */
export function breadcrumbPathOf(relativePath: string): string {
  return fileBreadcrumbs('', relativePath).at(-1)?.path ?? relativePath;
}

/** Whether a host folder crumb names a directory the server can list. A UNC server (`\\server`) is not one; its shares are. */
export function isListableHostFolder(path: string): boolean {
  return isHostPath(path) && !/^\\\\[^\\/]+$/.test(path);
}

/** Index of `path`'s crumb among `trail`'s crumbs; -1 when the trail does not pass through it. */
function trailCrumbIndex(trail: string, path: string): number {
  const crumbPath = breadcrumbPathOf(path);
  return fileBreadcrumbs('', trail).findIndex(crumb => crumb.path === crumbPath);
}

/**
 * The deepest path the breadcrumbs keep showing after the browser moves to `path`. Going up keeps the folders
 * below visible so the way back down is one click; moving anywhere else starts over at `path`.
 */
export function retainedBreadcrumbTrail(trail: string | undefined, path: string): string {
  return trail !== undefined && trailCrumbIndex(trail, path) >= 0 ? trail : path;
}

/** The entry of `folder` that leads toward `trail`; null when the trail ends at the folder. */
export function trailChildOf(folder: string, trail: string): string | null {
  const index = trailCrumbIndex(trail, folder);
  return index < 0 ? null : (fileBreadcrumbs('', trail)[index + 1]?.label ?? null);
}

/** A host path for `relativePath` (as a host folder's listing names it, with `/`) inside host folder `folder`, in the folder's own separator. */
export function joinHostPath(folder: string, relativePath: string): string {
  const separator = isWindowsAbsolutePath(folder) ? '\\' : '/';
  const base = folder.endsWith(separator) ? folder : `${folder}${separator}`;
  return base + relativePath.split('/').join(separator);
}
