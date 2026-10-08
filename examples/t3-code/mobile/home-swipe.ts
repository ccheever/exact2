// @ref llp/1107.004-home-projection.decision.md#decision
// Pinned threadListV2.ts swipe resolver; raw guards remain shared with Home commands.
import { str, type Obj } from './shared/domain';
import { canSnooze, hasQueuedTurnStart, pendingApproval, pendingInput, sectionOf, sidebarVisible, type Caps } from './shared/sidebar-model';
import type { HomeMenuItem } from './home-actions';

export interface HomeSwipeData {
  primary: string; primaryLabel: string; primarySymbol: string; primaryAccessibilityLabel: string;
  secondary: string; secondaryAccessibilityLabel: string; resetKey: string; token: string; snoozable: boolean;
  gateExpiry: number; snoozeItems: HomeMenuItem[];
}
export function blankHomeSwipe(): HomeSwipeData {
  return { primary: '', primaryLabel: '', primarySymbol: '', primaryAccessibilityLabel: '', secondary: '',
    secondaryAccessibilityLabel: '', resetKey: '', token: '', snoozable: false, gateExpiry: 0, snoozeItems: [] };
}
/** Explicit swipe operation whitelist; never fall back to a generic Delete command. */
export function homeSwipeOperation(operation: string): string | null {
  if (!operation.startsWith('swipe:')) return null;
  const kind = operation.slice(6);
  return ['settle', 'unsettle', 'unsnooze', 'archive', 'snooze:hour', 'snooze:three-hours', 'snooze:evening',
    'snooze:tomorrow', 'snooze:next-week', 'snooze:custom'].includes(kind) ? kind : null;
}
/** Absolute source guard expiry. The root decides when to rebuild; no timer lives here. */
export function homeSwipeGateExpiry(thread: Obj, now: number): number {
  if (pendingApproval(thread) || pendingInput(thread) || !hasQueuedTurnStart(thread, now)) return 0;
  const at = Date.parse(str(thread.latestUserMessageAt));
  return Number.isFinite(at) ? at + 120_000 : 0;
}
export function homeSwipePolicy(thread: Obj, caps: Caps, now: number, queued: boolean, operable: boolean, environmentId: string): HomeSwipeData {
  if (!operable || !sidebarVisible(thread)) return blankHomeSwipe();
  const section = sectionOf(queued ? { ...thread, settledOverride: null } : thread, caps, now, false);
  const snoozed = section === 'snoozed', variant = section === 'settled' ? 'slim' : 'card';
  const primary = snoozed ? 'unsnooze' : caps.settlement ? variant === 'slim' ? 'unsettle' : 'settle' : 'archive';
  const [primaryLabel, primarySymbol] = { unsnooze: ['Wake', 'clock'], settle: ['Settle', 'checkmark'],
    unsettle: ['Un-settle', 'arrow.uturn.backward'], archive: ['Archive', 'archivebox'] }[primary]!;
  const title = str(thread.title), snoozable = !snoozed && caps.snooze && canSnooze(thread, now);
  const resetKey = JSON.stringify([environmentId, str(thread.id), variant, snoozed, thread.settledAt ?? null, thread.unsettledAt ?? null, thread.snoozedUntil ?? null]);
  return { primary, primaryLabel: primaryLabel!, primarySymbol: primarySymbol!,
    primaryAccessibilityLabel: primary === 'unsnooze' ? `Wake ${title} now` : `${primaryLabel} ${title}`,
    secondary: snoozable ? 'snooze' : '', secondaryAccessibilityLabel: snoozable ? `Choose when to snooze ${title}` : '', snoozable,
    resetKey, token: encodeURIComponent(resetKey),
    gateExpiry: caps.snooze ? homeSwipeGateExpiry(thread, now) : 0, snoozeItems: [] };
}
