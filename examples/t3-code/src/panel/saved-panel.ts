// The right panel's surfaces survive a relaunch (MIT reference 9c161462c9, see LICENSE-T3:
// apps/web/src/rightPanelStore.ts persists `byThreadKey` — each thread's surfaces, active surface and
// whether the panel is open — under "t3code:right-panel-state:v2", and migratePersistedRightPanelState
// reads it back). The reference restores the panel with its file, so the Files breadcrumbs settle at
// the trail's end after a relaunch (files-crumbs-mount.ts). Here the panels are kept in the client's one
// preference file under `rightPanels`, keyed as the panel store keys them (the thread's draft key).
// The Files explorer, file tabs and the linked pull requests list are restored, and so are the
// Diff, the Device surface with its device, pull request details and attachments (saved-surfaces.ts).
// As the migration does, an active surface that was not kept falls back to the first kept one while
// the panel is open, and a panel with nothing kept stays closed. The turn whose diff was last offered
// proactively (`proactiveDiffTurnId`, 74541bab17) is kept with the panel, also when it keeps no surface,
// so a closed proactive diff stays closed after a relaunch; so are the device tabs the person closed
// (`dismissedDeviceSurfaceIds`), so an agent opening that device again after a relaunch adds no tab.
import { arr, obj, str, type Obj } from '../client/domain';
import type { PanelState, Surface } from './panel-surfaces';
import { keepRestored, readRestored, RESTORED_KINDS, type RestoredSurface, type SurfaceEntry } from './saved-surfaces';

export type SavedSurface = { id: string; kind: 'files' | 'file' | 'pull-requests'; path: string; line: number; folderTrail?: string } | RestoredSurface;
export type SavedPanel = { visible: boolean; active: string; surfaces: SavedSurface[]; maximized?: true; proactiveDiffTurnId?: string; dismissedDeviceSurfaceIds?: string[] };
type Holder = { local: object };
const KEPT = new Set(['files', 'file', 'pull-requests']);
const MAX_PANELS = 200;

function keep(surface: SurfaceEntry): SavedSurface | null {
  if (RESTORED_KINDS.has(surface.kind)) return keepRestored(surface);
  if (!KEPT.has(surface.kind)) return null;
  // The migration drops a folder trail that is not a string and keeps the surface.
  const trail = typeof surface.folderTrail === 'string' && surface.kind !== 'pull-requests' ? { folderTrail: surface.folderTrail } : {};
  if (surface.kind === 'file') {
    if (!surface.path || surface.id !== `file:${surface.path}`) return null;
    return { id: surface.id, kind: 'file', path: surface.path, line: surface.line > 0 ? Math.trunc(surface.line) : 0, ...trail };
  }
  return surface.id === surface.kind ? { id: surface.id, kind: surface.kind as 'files' | 'pull-requests', path: '', line: 0, ...trail } : null;
}

/** One panel as saved: its kept surfaces, the active one (or the first while open), open only with something kept. */
export function savedPanel(state: { surfaces: readonly SurfaceEntry[]; active: string; visible: boolean; maximized?: boolean; proactiveDiffTurnId?: string; dismissedDevices?: readonly string[] }): SavedPanel | null {
  const surfaces = state.surfaces.map(keep).filter((entry): entry is SavedSurface => !!entry);
  const dismissed = [...new Set((state.dismissedDevices ?? []).filter(id => typeof id === 'string' && id))];
  const offered = { ...(typeof state.proactiveDiffTurnId === 'string' && state.proactiveDiffTurnId ? { proactiveDiffTurnId: state.proactiveDiffTurnId } : {}),
    ...(dismissed.length ? { dismissedDeviceSurfaceIds: dismissed } : {}) };
  if (!surfaces.length) return Object.keys(offered).length ? { visible: false, active: '', surfaces, ...offered } : null;
  const kept = surfaces.some(entry => entry.id === state.active);
  const visible = state.visible;
  return { visible, active: kept ? state.active : visible ? surfaces[0]!.id : '', surfaces, ...(state.maximized === true ? { maximized: true } : {}), ...offered };
}

const local = (owner: Holder) => owner.local as { rightPanels?: Record<string, SavedPanel> };

/** load(): the saved panels, each re-validated; anything malformed is dropped. */
export function adoptRightPanels(next: object, saved: Obj): void {
  const panels: Record<string, SavedPanel> = {};
  for (const [key, value] of Object.entries(obj(saved.rightPanels)).slice(0, MAX_PANELS)) {
    const entry = obj(value);
    const panel = savedPanel({
      surfaces: arr(entry.surfaces).map(readRestored),
      active: str(entry.active), visible: entry.visible === true, maximized: entry.maximized === true,
      ...(typeof entry.proactiveDiffTurnId === 'string' ? { proactiveDiffTurnId: entry.proactiveDiffTurnId } : {}),
      dismissedDevices: Array.isArray(entry.dismissedDeviceSurfaceIds) ? entry.dismissedDeviceSurfaceIds.filter((id): id is string => typeof id === 'string') : [],
    });
    if (key && panel) panels[key] = panel;
  }
  if (Object.keys(panels).length) (next as { rightPanels?: Record<string, SavedPanel> }).rightPanels = panels;
}

/** The panels as they should be saved now; true when that differs from the preference record (save it). */
export function syncRightPanels(owner: Holder, panels: ReadonlyMap<string, PanelState>): boolean {
  const record = local(owner), next: Record<string, SavedPanel> = { ...(record.rightPanels ?? {}) };
  for (const [key, state] of panels) {
    // An untouched panel (not restored yet, never used this session) leaves its saved state alone.
    if (!state.surfaces.length && !state.userRevision && state.proactiveDiffTurnId === undefined && state.dismissedDevices === undefined) continue;
    const panel = savedPanel(state);
    if (panel) next[key] = panel; else delete next[key];
  }
  const keys = Object.keys(next);
  for (const key of keys.slice(0, Math.max(0, keys.length - MAX_PANELS))) delete next[key];
  if (JSON.stringify(next) === JSON.stringify(record.rightPanels ?? {})) return false;
  record.rightPanels = next;
  return true;
}

const restored = new WeakMap<object, Set<string>>();

/**
 * The device tabs a panel closed in an earlier launch, for a panel not restored yet that the session has not given them:
 * the store's loaded thread state keeps them (rightPanelStore `dismissedDeviceSurfaceIds`), so an agent's opening in a
 * background thread finds them, and the panels' save writes them back with the tab it opened.
 */
export function adoptSavedDismissals(owner: Holder, key: string, state: { dismissedDevices?: string[] }): void {
  if (state.dismissedDevices !== undefined || restored.get(owner)?.has(key)) return;
  const saved = local(owner).rightPanels?.[key]?.dismissedDeviceSurfaceIds;
  if (saved?.length) state.dismissedDevices = [...saved];
}

/** A panel the session has not touched takes its saved state, once, after the preferences were read and the
 *  client is live (a request made before the connection settles can be abandoned with its generation). */
export function restoreRightPanel(owner: Holder & { preferencesLoaded: boolean; ready: boolean }, key: string, state: PanelState): boolean {
  if (!owner.preferencesLoaded || !owner.ready) return false;
  let seen = restored.get(owner);
  if (!seen) { seen = new Set(); restored.set(owner, seen); }
  if (seen.has(key)) return false;
  seen.add(key);
  const saved = local(owner).rightPanels?.[key];
  if (saved?.proactiveDiffTurnId && state.proactiveDiffTurnId === undefined) state.proactiveDiffTurnId = saved.proactiveDiffTurnId;
  if (saved?.dismissedDeviceSurfaceIds?.length && state.dismissedDevices === undefined) state.dismissedDevices = [...saved.dismissedDeviceSurfaceIds];
  if (!saved || !saved.surfaces.length || state.surfaces.length || state.userRevision) return false;
  state.surfaces = saved.surfaces.map((entry): Surface => ({
    id: entry.id, kind: entry.kind, path: entry.path, line: entry.line, reveal: 0,
    ...('folderTrail' in entry && entry.folderTrail !== undefined ? { folderTrail: entry.folderTrail } : {}),
    ...('terminal' in entry ? { terminal: entry.terminal } : {}),
    ...('title' in entry ? { title: entry.title } : {}), ...('pr' in entry ? { pr: entry.pr } : {}), ...('detailTab' in entry && entry.detailTab ? { detailTab: entry.detailTab } : {}), ...('attachment' in entry ? { attachment: entry.attachment } : {}), ...('device' in entry && entry.device ? { device: entry.device } : {}),
  }));
  if (saved.maximized === true) state.maximized = true; else delete state.maximized;
  state.active = saved.active;
  state.visible = saved.visible && !!saved.active;
  return true;
}
