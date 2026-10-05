// Lane r6-pr: the split pull request row of the details card with the host's
// detail (MIT reference, see LICENSE-T3, upstream f90b77d809:
// chat/ThreadDetailsPrRow.tsx, pullRequest/PullRequestChecksPopover.tsx):
// the checks segment (rollup glyph, "running/total" while runs are in flight)
// and the popover it opens, the one trailing action ranked by what unblocks the
// merge next, and the tooltip card that answers what opening the panel would.
import type { T3Client } from './client';
import { arr, num, str, type Obj } from './domain';
import { isActing, normalRef, prState } from './r6-pr-actions';
import { CHECKS_HEADLINE, allowedMergeMethods, checksList, checksRollup, emptyCard, prCard, rowAction, selectedMergeMethod, summarizeChecks, trailingAction, type CheckRow, type PrCard } from './r6-pr-logic';

export type RowExtra = {
  destructive: boolean; suffix: boolean; actionWidth: number;
  /** The checks popover's transparent tail: the window's width right of the checks trigger. */
  tail: number; popId: string; method: string;
  checksTitle: string; checksSummary: string; checksList: CheckRow[]; collapsible: boolean; expanded: boolean;
  card: PrCard;
};
export const emptyExtra = (): RowExtra => ({ destructive: false, suffix: false, actionWidth: 0, tail: 0, popId: '', method: '', checksTitle: '', checksSummary: '', checksList: [], collapsible: false, expanded: false, card: emptyCard() });

/**
 * The details card's rows are 262pt wide inline (the 280pt card 12pt from the canvas edge, 1pt
 * border, 8pt inset) and 246pt in the header popover (264pt, 13pt in). `rightGap` is the window
 * width right of the canvas (an open right panel), as the overlay places the popover.
 */
export function rowGeometry(inline: boolean, rightGap: number): { rowWidth: number; rowRight: number } {
  return { rowWidth: inline ? 262 : 246, rowRight: Math.max(0, rightGap) + (inline ? 12 : 13) + 1 + 8 };
}

/**
 * The checks popover's tail: the window right of the trigger, so the window clamp lands the popover
 * end-aligned with it, as Base UI flips a start-aligned popup that would cross the window's 5pt
 * collision edge. Where the 320pt popup fits start-aligned (an open right panel), no tail.
 * The trigger is 54pt (two glyphs, 11pt insets) plus a "running/total" count while runs go.
 */
export const checksTrigger = (count: string) => 54 + (count ? 6 + 6.9 * count.length : 0);
export function checksTail(rightOfTrigger: number, count: string): number {
  const trigger = checksTrigger(count);
  return rightOfTrigger + trigger >= 320 + 5 ? 0 : Math.round(rightOfTrigger * 10) / 10;
}

export function rowExtra(client: T3Client, detail: Obj, reference: Obj, input: { key: string; lifecycleKey: string; rightGap: number; inline: boolean }) {
  const state = prState(client), checks = arr(detail.checks), rollup = checksRollup(checks);
  const conflicting = detail.state === 'open' && detail.mergeability === 'conflicting';
  const running = checks.filter(check => check.status === 'pending').length;
  const method = selectedMergeMethod(allowedMergeMethods(detail), 'merge');
  const action = rowAction(detail), acting = isActing(client, reference);
  const trailing = trailingAction(action, { handoff: state.handoff, actionPending: acting, method });
  const { rowWidth, rowRight } = rowGeometry(input.inline, input.rightGap);
  const actionWidth = trailing ? trailing.width : 0;
  const shownChecks = rollup && !conflicting && detail.isDraft !== true ? rollup : '';
  const summary = summarizeChecks(checks), list = checksList(checks, state.showAll.has(input.key));
  const ref = normalRef(reference);
  const r6: RowExtra = {
    destructive: trailing?.destructive ?? false, suffix: trailing?.suffix ?? false, actionWidth,
    tail: checksTail(rowRight + (trailing ? actionWidth + 1 : 0), running > 0 ? `${running}/${checks.length}` : ''), popId: `pr-checks-${num(detail.number)}`, method,
    checksTitle: CHECKS_HEADLINE[rollup] ?? '', checksSummary: summary, checksList: list.rows, collapsible: list.collapsible, expanded: state.showAll.has(input.key),
    card: prCard(detail, input.lifecycleKey, rowWidth + rowRight, { width: rowWidth, linkRight: rowWidth - (shownChecks ? 1 + checksTrigger(running > 0 ? `${running}/${checks.length}` : '') : 0) - (trailing ? actionWidth + 1 : 0) }),
  };
  return {
    // The plain tooltip only stands in where the card cannot (no state); the card is the row's tooltip.
    tooltip: `${str(detail.title)} #${num(detail.number)}`,
    checks: shownChecks, checksCount: running > 0 ? `${running}/${checks.length}` : '',
    checksAria: `Open checks: ${summary}`,
    action: trailing ? action : '', actionLabel: trailing?.label ?? '', actionTooltip: trailing?.tooltip ?? '',
    // Every action is held while one runs: a merge mid-hand-off would race the checkout.
    actionDisabled: acting || state.handoff !== '',
    actionTarget: JSON.stringify(ref),
    r6,
  };
}
