// The thread action menu (threadActionMenu.logic.ts buildThreadActionMenuItems)
// and the multi-select bulk menu (Sidebar.tsx handleMultiSelectContextMenu;
// Sidebar.logic.ts buildBulk*). Items are the contracts' ContextMenuItem: the
// desktop shell turns them into a native menu (ElectronMenu.ts). A row's
// right-click shows them as the sidebar's context popover (`menuRows`,
// sidebar-row.contract RowMenu), which macOS presents as an NSMenu with its
// submenus (exact2 #223); a menu the keyboard opens at the focused row is the
// module's (T3Sidebar.swift, `nativeTemplate`), since a context popover opens
// only from the pointer.
import { arr, str, type Obj } from './domain';
import type { T3Client } from './client';
import { canSnooze, capabilities, effectiveSnoozed, sectionOf, type Caps } from './sidebar-model';
import { sidebarPrefs, sidebarSession } from './sidebar-state';
import { snoozePresets } from './sidebar-presentation';

export interface MenuItem {
  id: string; label: string;
  disabled?: boolean; checked?: boolean; destructive?: boolean; separatorBefore?: boolean;
  children?: MenuItem[];
}
export interface MenuPreset { id: string; label: string; wakeLabel: string }
export interface ThreadMenuState {
  branch: string;
  /** Null on surfaces without a scoped list (the chat header). */
  projectFilter: { label: string; isActive: boolean } | null;
  isPinned: boolean; isSettled: boolean; autoSettleEnabled: boolean; isSnoozed: boolean; canSnoozeNow: boolean;
  isRegeneratingTitle: boolean; isRunning: boolean; caps: Caps; presets: MenuPreset[];
}

type Scope = { key: string; name: string; ids: Set<string> };
/** handleThreadContextMenu's state for one thread; `scopes` are the sidebar's project groups (projectScopes). */
export function threadMenuState(client: T3Client, thread: Obj, header: boolean, scopes: Scope[], now: number): ThreadMenuState & { scope: Scope | undefined } {
  const caps = capabilities(client.config), prefs = sidebarPrefs(client);
  const scope = scopes.find(group => group.ids.has(str(thread.projectId)));
  const section = sectionOf(thread, caps, now, client.local.clientSettings?.sidebarWorkingShelfEnabled === true);
  return {
    branch: str(thread.branch), projectFilter: header || !scope ? null : { label: scope.name, isActive: prefs.scope === scope.key },
    isPinned: thread.pinnedAt != null, isSettled: caps.settlement && thread.settledOverride === 'settled' && section === 'settled',
    autoSettleEnabled: thread.autoSettleDisabledAt == null, isSnoozed: caps.snooze && effectiveSnoozed(thread, now),
    canSnoozeNow: canSnooze(thread, now), isRegeneratingTitle: thread.titleRegeneration != null || sidebarSession(client).regenerating.has(str(thread.id)),
    isRunning: !canArchive(thread), caps, presets: snoozePresets(now, client.local.deviceSettings.timestampFormat), scope,
  };
}

export function threadMenuItems(state: ThreadMenuState): MenuItem[] {
  const items: MenuItem[] = [];
  if (state.branch) items.push({ id: 'new-thread-on-branch', label: `New thread on ${state.branch}` });
  if (state.caps.pinning) items.push(state.isPinned ? { id: 'unpin', label: 'Unpin thread' } : { id: 'pin', label: 'Pin thread' });
  if (state.caps.settlement) items.push(state.isSettled ? { id: 'unsettle', label: 'Un-settle thread' } : { id: 'settle', label: 'Settle thread' });
  if (state.caps.snooze) {
    items.push(state.isSnoozed ? { id: 'unsnooze', label: 'Wake thread' } : {
      id: 'snooze', label: 'Snooze', disabled: !state.canSnoozeNow,
      children: [...state.presets.map(preset => ({ id: `snooze:${preset.id}`, label: `${preset.label} (${preset.wakeLabel})` })),
        { id: 'snooze:custom', label: 'Custom…', separatorBefore: true }],
    });
  }
  items.push({ id: 'rename', label: 'Rename thread', separatorBefore: true });
  if (state.caps.titleRegeneration) {
    items.push({ id: 'regenerate-title', label: state.isRegeneratingTitle ? 'Regenerating…' : 'Regenerate title', disabled: state.isRegeneratingTitle });
  }
  items.push({ id: 'mark-unread', label: 'Mark unread' });
  if (state.projectFilter) {
    items.push({ id: 'filter-by-project', label: state.projectFilter.isActive ? 'Show all projects' : `Filter by ${state.projectFilter.label}` });
  }
  if (state.caps.autoSettleOptOut) {
    items.push({ id: 'auto-settle', label: 'Auto-settle behavior', children: [
      { id: 'auto-settle:enabled', label: 'Enabled', checked: state.autoSettleEnabled },
      { id: 'auto-settle:disabled', label: 'Disabled', checked: !state.autoSettleEnabled },
    ] });
  }
  items.push({ id: 'copy', label: 'Copy', separatorBefore: true, children: [
    { id: 'copy-path', label: 'Path' },
    ...(state.branch ? [{ id: 'copy-branch', label: 'Branch' }] : []),
    { id: 'copy-thread-id', label: 'Thread ID' },
  ] });
  items.push({ id: 'project-settings', label: 'Project settings' });
  items.push({ id: 'archive', label: 'Archive thread', disabled: state.isRunning, separatorBefore: true });
  items.push({ id: 'delete', label: 'Delete', destructive: true });
  return items;
}

export interface BulkMenuState {
  count: number; pinnedCount: number; canSnooze: boolean; regeneratable: number; regenerationSupported: number; presets: MenuPreset[];
}
export function bulkMenuItems(state: BulkMenuState): MenuItem[] {
  const items: MenuItem[] = [];
  if (state.pinnedCount > 0) items.push({ id: 'unpin', label: `Unpin (${state.pinnedCount})` });
  items.push({ id: 'settle', label: `Settle (${state.count})` });
  if (state.canSnooze) {
    items.push({ id: 'snooze', label: `Snooze (${state.count})`, children: [
      ...state.presets.map(preset => ({ id: `snooze:${preset.id}`, label: `${preset.label} (${preset.wakeLabel})` })),
      { id: 'snooze:custom', label: 'Custom…', separatorBefore: true },
    ] });
  }
  if (state.regenerationSupported > 0) {
    items.push(state.regeneratable === 0
      ? { id: 'regenerate-title', label: `Regenerating… (${state.regenerationSupported})`, disabled: true }
      : { id: 'regenerate-title', label: `Regenerate titles (${state.regeneratable})` });
  }
  items.push({ id: 'mark-unread', label: `Mark unread (${state.count})` });
  items.push({ id: 'delete', label: `Delete (${state.count})`, destructive: true });
  return items;
}

/** handleMultiSelectContextMenu over the selected rows painted now (`threads`). */
export function bulkMenuState(client: T3Client, threads: Obj[], now: number) {
  const caps = capabilities(client.config), session = sidebarSession(client);
  const presets = snoozePresets(now, client.local.deviceSettings.timestampFormat);
  const pinned = caps.pinning ? threads.filter(thread => thread.pinnedAt != null) : [];
  const supported = caps.titleRegeneration ? threads : [];
  const regeneratable = supported.filter(thread => thread.titleRegeneration == null && !session.regenerating.has(str(thread.id)));
  const items = bulkMenuItems({ count: threads.length, pinnedCount: pinned.length, canSnooze: caps.snooze && threads.every(thread => canSnooze(thread, now)),
    regeneratable: regeneratable.length, regenerationSupported: supported.length, presets });
  return { items, presets, pinned, regeneratable, ids: threads.map(thread => str(thread.id)) };
}

/** models.ts threadRuntimeCanArchive: never detach a provider mid-turn. */
export function canArchive(thread: Obj): boolean {
  const status = str(thread.activityRunStatus ?? thread.status);
  if (status === 'queued') return thread.activeRunId == null;
  if (thread.latestRunId == null && thread.activeProviderThreadId == null) return true;
  return !['preparing', 'starting', 'running'].includes(status);
}

/** ElectronMenu.ts buildTemplate: explicit separators, then one before the first destructive row. */
export function nativeTemplate(items: MenuItem[]): Obj[] {
  const out: Obj[] = [];
  let destructiveSeparator = false, explicit = false;
  const separator = () => { if (out.length && out[out.length - 1]!.type !== 'separator') out.push({ type: 'separator' }); };
  for (const item of items) {
    if (item.separatorBefore) { separator(); explicit = true; }
    if (item.destructive && !destructiveSeparator && !explicit && out.length) { separator(); destructiveSeparator = true; }
    out.push({ type: item.children?.length ? 'submenu' : 'item', id: item.id, label: item.label, enabled: !item.disabled,
      ...(typeof item.checked === 'boolean' ? { checked: item.checked } : {}), destructive: item.destructive === true && !item.children?.length,
      ...(item.children?.length ? { children: nativeTemplate(item.children) } : {}) });
  }
  return out;
}

/**
 * One row of a context popover menu (sidebar-row.contract RowMenu): `parent` names the
 * submenu row that holds it ("" at the top), `sub` opens a submenu, `separated` draws the
 * separator ElectronMenu.ts buildTemplate puts before it, `check` is a checkable row.
 */
export interface MenuRow { id: string; label: string; parent: string; sub: boolean; disabled: boolean; check: boolean; checked: boolean; destructive: boolean; separated: boolean }
export function menuRows(items: MenuItem[]): MenuRow[] {
  const out: MenuRow[] = [];
  const walk = (template: Obj[], parent: string) => {
    let separated = false;
    for (const entry of template) {
      if (entry.type === 'separator') { separated = true; continue; }
      const sub = entry.type === 'submenu';
      out.push({ id: str(entry.id), label: str(entry.label), parent, sub, disabled: entry.enabled === false,
        check: typeof entry.checked === 'boolean', checked: entry.checked === true, destructive: entry.destructive === true, separated });
      separated = false;
      if (sub) walk(arr(entry.children), str(entry.id));
    }
  };
  walk(nativeTemplate(items), '');
  return out;
}
