// Extracted from T3 Code 1e2ecbd975 ThreadTerminalDrawer.tsx (MIT, LICENSE-T3),
// resolved active group / visible panes / tab headers / action labels. Contract consumes plain rows.
import { MAX_TERMINALS_PER_GROUP, type ThreadTerminalGroup } from './terminal-ui-state';
export type TerminalPaneView = { terminalId: string; label: string; sessionKey: string; active: boolean; focusRequest: number; target: string };
export type TerminalTabView = { id: string; label: string; heading: string; count: number; active: boolean; target: string; closeTitle: string; closeBody: string };
export function terminalLayout(ids: readonly string[], groups: readonly ThreadTerminalGroup[], activeId: string, activeGroupId: string) {
  const active = ids.includes(activeId) ? activeId : ids[0] ?? '';
  const group = groups.find(group => group.id === activeGroupId) ?? groups.find(group => group.terminalIds.includes(active)) ?? groups[0];
  const visible = group?.terminalIds ?? (active ? [active] : []);
  return { active, visible, direction: group?.splitDirection === 'vertical' ? 'column' : 'row',
    showTabs: ids.length > 1, showHeaders: groups.length > 1 || groups.some(group => group.terminalIds.length > 1),
    splitDisabled: visible.length >= MAX_TERMINALS_PER_GROUP };
}
export function terminalSplitLabel(vertical: boolean, disabled: boolean, shortcut: string): string {
  const label = `Split Terminal ${vertical ? 'Vertically' : 'Horizontally'}`;
  return disabled ? `${label} (max ${MAX_TERMINALS_PER_GROUP} per group)` : shortcut ? `${label} (${shortcut})` : label;
}
