// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/terminal-layout.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Extracted from T3 Code 1e2ecbd975 ThreadTerminalDrawer.tsx (MIT, LICENSE-T3): the resolved active
// group, visible panes, tab-list visibility, group headers and action labels (:1212-1268), and the
// tab list's rows (:1630-1710: group heading, icon, count, per-terminal close label). The bottom
// drawer and the right-panel terminal surface (ChatView.tsx RightPanelTerminalSurface, one group per
// surface) share them. Contract consumes plain rows.
import { MAX_TERMINALS_PER_GROUP, type ThreadTerminalGroup } from './terminal-ui-state';
import { terminalCloseConfirmMessage } from './terminal-close';
export type TerminalPaneView = { terminalId: string; label: string; sessionKey: string; active: boolean; focusRequest: number; target: string };
export type TerminalTabView = { id: string; label: string; heading: string; groupIcon: string; groupActive: boolean; groupTarget: string; count: number;
  active: boolean; target: string; closeLabel: string; closeTitle: string; closeBody: string };
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
/** The group header's label and Lucide icon (Square, SquareSplitVertical, SquareSplitHorizontal). */
export function terminalGroupLabel(group: Pick<ThreadTerminalGroup, 'terminalIds' | 'splitDirection'>): { label: string; icon: string } {
  if (group.terminalIds.length <= 1) return { label: 'Single', icon: 'square' };
  return group.splitDirection === 'vertical' ? { label: 'Stacked', icon: 'split-vertical' } : { label: 'Side by side', icon: 'split-horizontal' };
}
/** The tab list's rows: one per terminal, the group's header on its first row when headers show. */
export function terminalTabs(groups: readonly ThreadTerminalGroup[], activeId: string, showHeaders: boolean, threadKey: string,
  labelFor: (id: string) => string, closeShortcut: string): TerminalTabView[] {
  return groups.flatMap(group => {
    const groupActive = group.terminalIds.includes(activeId), { label: heading, icon } = terminalGroupLabel(group);
    const groupTarget = `${threadKey}|${groupActive ? activeId : group.terminalIds[0] ?? activeId}`;
    return group.terminalIds.map((id, index) => {
      const label = labelFor(id), active = id === activeId, [closeTitle, closeBody] = terminalCloseConfirmMessage([label]);
      return { id, label, heading: showHeaders && index === 0 ? heading : '', groupIcon: icon, groupActive, groupTarget, count: group.terminalIds.length,
        active, target: `${threadKey}|${id}`, closeLabel: `Close ${label}${active && closeShortcut ? ` (${closeShortcut})` : ''}`, closeTitle, closeBody };
    });
  });
}
