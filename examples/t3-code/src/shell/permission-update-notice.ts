// PermissionUpdateNotice (MIT reference, see LICENSE-T3, T3 Code 7c3b3b3605ef:
// apps/web/src/components/PermissionUpdateNotice.ts): once per environment whose session the
// server kept from before the permission split (remote-scopes.ts sessionHasLegacyPermissions), a
// toast that stays until it is closed. Open Sharing & access and Dismiss close it; any close hides it
// for that environment on this client for good (`t3code:permission-update:v1:<environment>`,
// kept in the shell preferences as `permissionUpdateDismissals`).
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import { fleet } from '../connections/settings-b-fleet';
import { pushToast } from './toast';
import { shellPrefs } from './shell-prefs';

// Keeps an active toast across shell reads (the reference's module-level `shown`).
const shown = new WeakMap<object, Set<string>>();

export const permissionUpdateCopy = (label: string) => ({
  title: `Permissions have changed for ${label}`,
  description: 'This connection still uses the old permissions, so some actions may no longer be available. Pair again using a new link with the permissions you need.',
});

/** The environments to warn about: the focused one and each synchronized background one. */
function legacyEnvironments(client: T3Client): { environmentId: string; label: string }[] {
  const found: { environmentId: string; label: string }[] = [];
  if (client.environmentId && client.ready && client.legacyPermissions) {
    found.push({ environmentId: client.environmentId, label: str(obj(client.config.environment).label, 'this environment') });
  }
  for (const entry of fleet.entries.values()) {
    if (!entry.environmentId || entry.environmentId === client.environmentId || entry.synchronized !== entry.generation || !entry.legacyPermissions) continue;
    found.push({ environmentId: entry.environmentId, label: str(obj(entry.config.environment).label, 'this environment') });
  }
  return found;
}

/** Posts the notice for each environment not warned about yet; returns how many it posted. */
export function permissionUpdateNotices(client: T3Client, preferencesLoaded: boolean): number {
  if (!preferencesLoaded) return 0;
  let seen = shown.get(client);
  if (!seen) { seen = new Set(); shown.set(client, seen); }
  let posted = 0;
  for (const { environmentId, label } of legacyEnvironments(client)) {
    if (seen.has(environmentId) || shellPrefs(client).permissionUpdateDismissals.includes(environmentId)) continue;
    seen.add(environmentId);
    pushToast(client, {
      kind: 'info', ...permissionUpdateCopy(label), timeoutMs: 0, stacked: true, closeOnAction: true,
      action: { label: 'Open Sharing & access', op: 'ui:settings', id: 'access' },
      secondary: { label: 'Dismiss', op: 'dismiss' }, secondaryVariant: 'ghost',
      onClose: () => {
        const prefs = shellPrefs(client);
        if (!prefs.permissionUpdateDismissals.includes(environmentId)) prefs.permissionUpdateDismissals = [...prefs.permissionUpdateDismissals, environmentId].slice(-200);
      },
    });
    posted++;
  }
  return posted;
}
