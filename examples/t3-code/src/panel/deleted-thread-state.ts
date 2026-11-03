// What a deleted thread leaves behind on this device (T3 Code 5d3d78ab93, MIT, see LICENSE-T3:
// apps/web/src/hooks/useThreadActions.ts clearDeletedThreadPanelState): its right panel, saved or live, its Changes
// selection and comparison target, and its floating device player, so a deleted thread's entry does not hold one of
// the saved panels' 200 places.
import type { T3Client } from '../client/client';
import { closeMiniDevice } from '../device/device-session';
import { surfaceStore } from './panel-surfaces';

export function clearDeletedThreadPanelState(client: T3Client, environmentId: string, threadId: string): void {
  const key = `${environmentId}:${threadId}`;
  surfaceStore(client).panels.delete(key);
  const saved = (client.local as { rightPanels?: Record<string, unknown> }).rightPanels;
  if (saved) delete saved[key];
  delete client.diffState.selections[key];
  delete client.diffState.branchBaseRefs[key];
  closeMiniDevice(client, threadId);
}
