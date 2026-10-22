// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r9-input-panel.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r9-input: ⌘W (rightPanel.close) closes the active right-panel surface tab, as the
// reference's ChatView does (MIT, see LICENSE-T3: components/ChatView.tsx `rightPanel.close` →
// closeRightPanelSurface(activeRightPanelSurface) → rightPanelStore.closeSurface): the
// neighbour at the closed index becomes active, the last tab closes the panel, and with no
// active surface the chord keeps its native meaning (Close Window). The Pull Requests page
// closes its open pull request (routes/_chat.pull-requests.tsx closeActiveSurfaceFromShortcut).
import type { T3Client } from './client';
import { panelState, syncDiff } from './r4-surfaces-panel';

/** The surface ⌘W closes: the open panel's active tab id, or '' when no surface is active. */
export function surfaceToClose(client: T3Client, panelOpen: boolean): string {
  if (!panelOpen) return '';
  const state = panelState(client);
  syncDiff(client, state);
  if (!state.visible) return '';
  return state.surfaces.some(surface => surface.id === state.active) ? state.active : '';
}

/** The dispatch row for rightPanel.close in this context ([kind, target]), or null for the window's Close. */
export function closeChordTarget(client: T3Client, panelOpen: boolean, prNumber: string): [string, string] | null {
  if (prNumber) return ['pr-close', ''];
  const surface = surfaceToClose(client, panelOpen);
  if (surface) return ['close-surface', surface];
  // An open panel whose state names no tab (a diff sheet the store has not seen): hide it, as before.
  return panelOpen ? ['close-diff', ''] : null;
}
