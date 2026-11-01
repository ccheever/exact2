// The window's Back and Forward as the sidebar header's buttons and a mouse's side buttons reach them
// (navigationHistory.ts, SidebarChrome.tsx HistoryNavigationButton; T3 Code 2359643cff, MIT, see LICENSE-T3).
// The reference steps the browser's session history, which covers every route; the clone's is the main view's
// visits (keyboard-dispatch.ts historySteps), which ⌘[ and ⌘] step too. Settings is a location of its own: Back
// leaves it, and Forward from where it was left reopens it. Each visit's location (`here`) is mirrored onto the router,
// so the Mac's swipe steps it and its `navigate` names the step (sidebar-members.contract historyGo).
import type { T3Client } from '../client/client';
import { historySteps, SETTINGS_HREF, visitHere, type HistoryStep } from '../shell/keyboard-dispatch';
import { brandMarkWidth } from './sidebar-min-width';
import { shortcutLabel } from './sidebar-view';

const NONE: HistoryStep = { kind: '', target: '', extra: '' };
const step = (prefix: 'back' | 'forward', to: HistoryStep | null) => {
  const { kind, target, extra } = to ?? NONE;
  return prefix === 'back' ? { backKind: kind, backTarget: target, backExtra: extra } : { forwardKind: kind, forwardTarget: target, forwardExtra: extra };
};

/** The `navigationHistory` resource: each way's step ('' kind at that end), the tooltips and where the header puts the buttons. */
export function navigationHistory(client: T3Client, page: string, settingsOpen: boolean) {
  // Settings is a visit of its own over the one it was opened from, which its Back returns to.
  const { back, forward, here, backHref, forwardHref } = settingsOpen
    ? { back: { kind: 'back', target: '', extra: '' }, forward: null, here: SETTINGS_HREF, backHref: visitHere(client), forwardHref: '' } : historySteps(client, page);
  const tip = (label: string, command: string) => { const shortcut = shortcutLabel(client, command); return shortcut ? `${label} (${shortcut})` : label; };
  return { ...step('back', back), ...step('forward', forward), here, backHref: back ? backHref : '', forwardHref: forward ? forwardHref : '', backTip: tip('Go back', 'navigation.back'), forwardTip: tip('Go forward', 'navigation.forward'),
    brandWidth: brandMarkWidth(client.local.clientSettings?.fontSizeInterface) };
}
export const navigationHistorySource = (client: T3Client, args: unknown[]) => navigationHistory(client, String(args[1] || ''), args[2] === true);
