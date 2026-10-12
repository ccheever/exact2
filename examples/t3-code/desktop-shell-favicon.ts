// The project icon picker's "Open in <Finder>" (task desktop-shell-details). Reference
// ProjectFaviconPickerDialog.tsx, ProjectSettingsPanel.tsx pickProjectFavicon and
// apps/desktop/src/ipc/methods/window.ts pickProjectFavicon (1e2ecbd975; MIT, see LICENSE-T3):
// the footer's trailing action opens the native picker for one image in the project's workspace
// root; a pick closes the dialog and selects that absolute path, a failure toasts "Could not open
// image picker" and keeps the dialog open. Offered only when every member is a project of the
// primary environment (local-primary.ts, the embedded server on this Mac) whose path the Mac
// can open (canPickExternalProjectFavicon).
import { obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { isPrimaryOrigin } from './local-primary';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { letGo } from './let-go';

const isWindowsPlatform = (platform: string) => /^win(dows)?/i.test(platform);
const isWindowsAbsolutePath = (value: string) => /^\\\\/.test(value) || /^[a-zA-Z]:[\\/]/.test(value);
/** canPickExternalProjectFavicon: a Windows host cannot pick for a WSL path. */
export function canPickExternalProjectFavicon(cwd: string, platform: string): boolean {
  return !isWindowsPlatform(platform) || isWindowsAbsolutePath(cwd);
}
/** getLocalFileManagerName. */
export function localFileManagerName(platform: string): string {
  if (/mac|iphone|ipad|ipod/i.test(platform)) return 'Finder';
  if (isWindowsPlatform(platform)) return 'File Explorer';
  return 'Files';
}
/** This client is the macOS desktop: navigator.platform reads MacIntel there. */
export const PLATFORM = 'MacIntel';

/** The footer action's label, or '' when it is not offered for these members. */
export function faviconPickLabel(origin: string, members: Obj[]): string {
  const offered = members.length > 0 && isPrimaryOrigin(origin) && members.every(member => canPickExternalProjectFavicon(str(member.workspaceRoot), PLATFORM));
  return offered ? `Open in ${localFileManagerName(PLATFORM)}` : '';
}

export const FAVICON_PICK_OP = 'project-favicon-pick';
/** sb:project-favicon-pick: the native picker (T3Menus.pickProjectFavicon); answers project-favicon-set's value. */
export async function pickProjectFavicon(client: T3Client, native: Native, value: string): Promise<string> {
  const cwd = str(Object.fromEntries(new URLSearchParams(value)).cwd);
  let path = '';
  try {
    path = str(obj(await client.restAccess(native).call({ op: 'pickProjectFavicon', path: cwd })).path);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not open image picker', description: error instanceof Error ? error.message : 'An error occurred.' });
    throw new ClientError('toasted:');
  }
  // Cancelled: the dialog stays open.
  if (!path) throw new ClientError('toasted:');
  return `path=${encodeURIComponent(path)}`;
}
