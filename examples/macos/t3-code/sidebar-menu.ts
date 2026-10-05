// The thread action menu (threadActionMenu.logic.ts buildThreadActionMenuItems)
// and the multi-select bulk menu (Sidebar.tsx handleMultiSelectContextMenu;
// Sidebar.logic.ts buildBulk*). Items are the contracts' ContextMenuItem: the
// desktop shell turns them into a native menu (ElectronMenu.ts), and so does
// this app's module (T3Sidebar.swift).
import { str, type Obj } from './domain';
import type { Caps } from './sidebar-model';

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
