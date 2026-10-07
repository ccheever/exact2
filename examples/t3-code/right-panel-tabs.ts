// MIT T3 Code 1e2ecbd975: rightPanelStore / RightPanelTabs / ChatView.
// Browser's toggle-mute slot is reserved in TabAction; Browser remains excluded (X1).
import type { T3Client } from './client';
import type { PanelState, Surface, SurfaceKind } from './r4-surfaces-panel';
import type { DeviceTarget } from './r6-media-device';
import type { Native } from './protocol';
import { obj, str } from './domain';
import { pushToast } from './toast';
import { letGo } from './let-go';
export type TabAction = 'rename' | 'copy-path' | 'toggle-mute' | 'close' | 'close-others' | 'close-to-right' | 'close-all';
export type TabMenuItem = { id: TabAction; label: string; disabled?: boolean };
/** A tab's menu row as the tab strip's `contextPopover` shows it (R4Tab.menu): every flag spelled out. */
export type TabMenuRow = { id: TabAction; label: string; disabled: boolean };
export function tabContextMenuItems(surface: Surface, surfaces: readonly Surface[]): TabMenuItem[] {
  const index = surfaces.findIndex(entry => entry.id === surface.id);
  if (index < 0) return [];
  return [
    ...(surface.kind === 'device' ? [{ id: 'rename' as const, label: 'Rename' }] : []),
    ...(surface.kind === 'file' && !surface.attachment ? [{ id: 'copy-path' as const, label: 'Copy path' }] : []),
    { id: 'close', label: 'Close' },
    { id: 'close-others', label: 'Close others', disabled: surfaces.length <= 1 },
    { id: 'close-to-right', label: 'Close to the right', disabled: index >= surfaces.length - 1 },
    { id: 'close-all', label: 'Close all', disabled: surfaces.length === 0 },
  ];
}
/**
 * The rows of the strip's one context popover (r4-surfaces.contract `r4-tab-menu`). A
 * right-click opens it through the host (macOS: an NSMenu at the pointer; the agent: the
 * painted popover), so no native request waits on menu tracking; each row presses
 * `surface-<id>` for its tab, the same op the menu's choice ran before.
 */
export const tabMenuRows = (surface: Surface, surfaces: readonly Surface[]): TabMenuRow[] =>
  tabContextMenuItems(surface, surfaces).map(item => ({ id: item.id, label: item.label, disabled: item.disabled === true }));
export function closeSurface(state: PanelState, id: string): void {
  const index = state.surfaces.findIndex(entry => entry.id === id);
  if (index < 0) return;
  state.surfaces.splice(index, 1);
  if (state.active === id) state.active = state.surfaces[Math.min(index, state.surfaces.length - 1)]?.id ?? '';
  if (!state.surfaces.length) { state.visible = false; state.active = ''; }
}
export function closeOtherSurfaces(state: PanelState, id: string): void {
  const surface = state.surfaces.find(entry => entry.id === id);
  if (!surface || state.surfaces.length <= 1) return;
  state.surfaces = [surface]; state.active = id; state.visible = true;
}
export function closeSurfacesToRight(state: PanelState, id: string): void {
  const index = state.surfaces.findIndex(entry => entry.id === id);
  if (index < 0 || index === state.surfaces.length - 1) return;
  state.surfaces = state.surfaces.slice(0, index + 1);
  if (!state.surfaces.some(entry => entry.id === state.active)) state.active = id;
}
export function closeAllSurfaces(state: PanelState): void { state.surfaces = []; state.active = ''; state.visible = false; }
export const deviceSurfaceId = (target: DeviceTarget) => `device:${encodeURIComponent(target.hostId)}:${encodeURIComponent(target.deviceId)}`;
export function openDeviceSurface(state: PanelState, target: DeviceTarget): Surface {
  const id = deviceSurfaceId(target), existing = state.surfaces.find(entry => entry.id === id);
  const surface: Surface = existing ?? { id, kind: 'device', path: '', line: 0, reveal: 0, device: target };
  const placeholder = state.surfaces.findIndex(entry => entry.id === 'device');
  if (existing) state.surfaces = state.surfaces.filter(entry => entry.id !== 'device');
  else if (placeholder >= 0) state.surfaces[placeholder] = surface;
  else state.surfaces.push(surface);
  state.active = id; state.visible = true;
  return surface;
}
export function renameDevice(state: PanelState, id: string, name: string): void {
  const surface = state.surfaces.find(entry => entry.id === id && entry.kind === 'device');
  if (surface) surface.title = name.trim() || surface.device?.name || 'Device';
}
type CloseHook = { guard?: (surface: Surface) => boolean | Promise<boolean>; cleanup: (surface: Surface) => void | Promise<void> };
const hooks = new WeakMap<object, Map<SurfaceKind, CloseHook>>();
export function registerSurfaceClose(owner: object, kind: SurfaceKind, hook: CloseHook): () => void {
  let map = hooks.get(owner); if (!map) { map = new Map(); hooks.set(owner, map); }
  map.set(kind, hook);
  return () => { if (map.get(kind) === hook) map.delete(kind); };
}
export async function closePanelSurfaces(owner: object, state: PanelState, action: string, id: string): Promise<void> {
  const surface = state.surfaces.find(entry => entry.id === id);
  if (action !== 'close-all' && !surface) return;
  if (action === 'close' && surface && await hooks.get(owner)?.get(surface.kind)?.guard?.(surface) === false) return;
  const next = { ...state, surfaces: [...state.surfaces] };
  if (action === 'close') closeSurface(next, id);
  else if (action === 'close-others') closeOtherSurfaces(next, id);
  else if (action === 'close-to-right') closeSurfacesToRight(next, id);
  else if (action === 'close-all') closeAllSurfaces(next);
  else return;
  const removed = state.surfaces.filter(entry => !next.surfaces.some(kept => kept.id === entry.id));
  for (const entry of removed) await hooks.get(owner)?.get(entry.kind)?.cleanup(entry);
  // A cleanup may wait for a native surface to stop. Preserve tabs opened or
  // selected during that wait; remove only the identities this operation owns.
  const activeWasRemoved = removed.some(entry => entry.id === state.active);
  for (const entry of removed) closeSurface(state, entry.id);
  if (removed.length && state.surfaces.some(entry => entry.id === id)) {
    if (action === 'close-others') { state.active = id; state.visible = true; }
    else if (action === 'close-to-right' && activeWasRemoved) state.active = id;
  }
}
type Rename = { id: string; value: string };
const editors = new WeakMap<PanelState, Rename>();
export const tabRename = (state: PanelState): Rename => editors.get(state) ?? { id: '', value: '' };
export function editTabName(state: PanelState, op: string, id: string, value: string): void {
  const surface = state.surfaces.find(entry => entry.id === id && entry.kind === 'device');
  if (!surface) return;
  if (op === 'rename') editors.set(state, { id, value: surface.title || surface.device?.name || 'Device' });
  // The editor's field owns the typed name and commits it as `value` (r4-surfaces.contract
  // R4TabNameField): Enter and blur, after Escape's cancel the commit finds no editor.
  else if (op === 'rename-commit' && editors.get(state)?.id === id) { renameDevice(state, id, value); editors.delete(state); }
  else if (op === 'rename-cancel') editors.delete(state);
}
export async function copyTabPath(client: T3Client, native: Native, surface: Surface): Promise<void> {
  if (surface.kind !== 'file' || surface.attachment) return;
  try {
    if (!native.available) throw new Error('Clipboard API unavailable.');
    const result = obj(await client.restAccess(native).call({ op: 'copyText', text: surface.path }));
    if (result.copied === false) throw new Error('Clipboard API unavailable.');
    pushToast(client, { kind: 'success', title: 'Path copied', description: surface.path });
  } catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Failed to copy path', description: error instanceof Error && error.message ? error.message : 'Clipboard API unavailable.' }); }
}
export async function showTabMenu(client: T3Client, native: Native, state: PanelState, id: string, keyboard = false): Promise<string> {
  const surface = state.surfaces.find(entry => entry.id === id);
  if (!surface) return '';
  const items = tabContextMenuItems(surface, state.surfaces);
  const result = obj(await client.restAccess(native).call({ op: 'contextMenu', items, ...(keyboard ? { anchor: 'focus' } : {}) }));
  const action = str(result.clicked);
  return items.some(item => item.id === action && !item.disabled) ? action : '';
}
