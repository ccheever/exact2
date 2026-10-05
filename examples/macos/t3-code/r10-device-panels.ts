// Lane r10-device: the right panel's surfaces survive a relaunch (MIT reference, see LICENSE-T3:
// apps/web/src/rightPanelStore.ts persists `byThreadKey` — each thread's surfaces, active surface and
// whether the panel is open — under "t3code:right-panel-state:v2", and migratePersistedRightPanelState
// reads it back). The reference restores the panel with its file, so the Files breadcrumbs settle at
// the trail's end after a relaunch (r10-device-crumbs.ts). Here the panels are kept in the client's one
// preference file under `rightPanels`, keyed as the panel store keys them (the thread's draft key).
// The Files explorer, file tabs and the linked pull requests list are restored; lane r11-device adds the
// Diff, the Device surface with its device, pull request details and attachments (r11-device-panels.ts).
// As the migration does, an active surface that was not kept falls back to the first kept one while
// the panel is open, and a panel with nothing kept stays closed.
import { arr, obj, str, type Obj } from './domain';
import type { PanelState, Surface } from './r4-surfaces-panel';
import { keepR11, readR11, R11_KINDS, type R11SavedSurface, type R11Surface } from './r11-device-panels';

export type SavedSurface = { id: string; kind: 'files' | 'file' | 'pull-requests'; path: string; line: number } | R11SavedSurface;
export type SavedPanel = { visible: boolean; active: string; surfaces: SavedSurface[] };
type Holder = { local: object };
const KEPT = new Set(['files', 'file', 'pull-requests']);
const MAX_PANELS = 200;

function keep(surface: R11Surface): SavedSurface | null {
  if (R11_KINDS.has(surface.kind)) return keepR11(surface);
  if (!KEPT.has(surface.kind)) return null;
  if (surface.kind === 'file') {
    if (!surface.path || surface.id !== `file:${surface.path}`) return null;
    return { id: surface.id, kind: 'file', path: surface.path, line: surface.line > 0 ? Math.trunc(surface.line) : 0 };
  }
  return surface.id === surface.kind ? { id: surface.id, kind: surface.kind as 'files' | 'pull-requests', path: '', line: 0 } : null;
}

/** One panel as saved: its kept surfaces, the active one (or the first while open), open only with something kept. */
export function savedPanel(state: { surfaces: readonly R11Surface[]; active: string; visible: boolean }): SavedPanel | null {
  const surfaces = state.surfaces.map(keep).filter((entry): entry is SavedSurface => !!entry);
  if (!surfaces.length) return null;
  const kept = surfaces.some(entry => entry.id === state.active);
  const visible = state.visible;
  return { visible, active: kept ? state.active : visible ? surfaces[0]!.id : '', surfaces };
}

const local = (owner: Holder) => owner.local as { rightPanels?: Record<string, SavedPanel> };

/** load(): the saved panels, each re-validated; anything malformed is dropped. */
export function adoptRightPanels(next: object, saved: Obj): void {
  const panels: Record<string, SavedPanel> = {};
  for (const [key, value] of Object.entries(obj(saved.rightPanels)).slice(0, MAX_PANELS)) {
    const entry = obj(value);
    const panel = savedPanel({
      surfaces: arr(entry.surfaces).map(readR11),
      active: str(entry.active), visible: entry.visible === true,
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
    if (!state.surfaces.length && !state.userRevision) continue;
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

/** A panel the session has not touched takes its saved state, once, after the preferences were read and the
 *  client is live (a request made before the connection settles can be abandoned with its generation). */
export function restoreRightPanel(owner: Holder & { preferencesLoaded: boolean; ready: boolean }, key: string, state: PanelState): boolean {
  if (!owner.preferencesLoaded || !owner.ready) return false;
  let seen = restored.get(owner);
  if (!seen) { seen = new Set(); restored.set(owner, seen); }
  if (seen.has(key)) return false;
  seen.add(key);
  const saved = local(owner).rightPanels?.[key];
  if (!saved || state.surfaces.length || state.userRevision) return false;
  state.surfaces = saved.surfaces.map((entry): Surface => ({
    id: entry.id, kind: entry.kind, path: entry.path, line: entry.line, reveal: 0,
    ...('pr' in entry ? { pr: entry.pr } : {}), ...('attachment' in entry ? { attachment: entry.attachment } : {}), ...('device' in entry && entry.device ? { device: entry.device } : {}),
  }));
  state.active = saved.active;
  state.visible = saved.visible && !!saved.active;
  return true;
}
