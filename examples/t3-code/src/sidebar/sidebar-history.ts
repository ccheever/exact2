// The window's Back and Forward as the sidebar header's buttons and a mouse's side buttons reach them
// (navigationHistory.ts, SidebarChrome.tsx HistoryNavigationButton; T3 Code 2359643cff, MIT, see LICENSE-T3).
// The reference steps the browser's session history, which covers every route; the clone's is the main view's
// visits (keyboard-dispatch.ts historySteps), which ⌘[ and ⌘] step too. Settings is a location of its own: Back
// leaves it, and Forward from where it was left reopens it.
import type { T3Client } from '../client/client';
import { historySteps, type HistoryStep } from '../shell/keyboard-dispatch';
import { brandMarkWidth } from './sidebar-min-width';
import { shortcutLabel } from './sidebar-view';

const NONE: HistoryStep = { kind: '', target: '', extra: '' };
const step = (prefix: 'back' | 'forward', to: HistoryStep | null) => {
  const { kind, target, extra } = to ?? NONE;
  return prefix === 'back' ? { backKind: kind, backTarget: target, backExtra: extra } : { forwardKind: kind, forwardTarget: target, forwardExtra: extra };
};

/** The `navigationHistory` resource: each way's step ('' kind at that end), the tooltips and where the header puts the buttons. */
export function navigationHistory(client: T3Client, page: string, settingsOpen: boolean) {
  const { back, forward } = settingsOpen ? { back: { kind: 'back', target: '', extra: '' }, forward: null } : historySteps(client, page);
  const tip = (label: string, command: string) => { const shortcut = shortcutLabel(client, command); return shortcut ? `${label} (${shortcut})` : label; };
  return { ...step('back', back), ...step('forward', forward), backTip: tip('Go back', 'navigation.back'), forwardTip: tip('Go forward', 'navigation.forward'),
    brandWidth: brandMarkWidth(client.local.clientSettings?.fontSizeInterface) };
}
export const navigationHistorySource = (client: T3Client, args: unknown[]) => navigationHistory(client, String(args[1] || ''), args[2] === true);
