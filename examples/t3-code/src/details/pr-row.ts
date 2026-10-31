// The pull request row of the details card with the host's detail (MIT
// reference, see LICENSE-T3, upstream 7c3b3b3605ef: chat/ThreadDetailsPrRow.tsx,
// pullRequest/PullRequestChecksPopover.tsx; the card's change breakdown at 74541bab17): the title takes the whole first
// line; the second carries the number, the checks (rollup glyph, then
// "completed/total" while runs are in flight or "N failed") with the popover they
// open, and the one action ranked by what unblocks the merge next. The tooltip
// card answers what opening the panel would.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { isActing, normalRef, prState, rowTerminology } from './pr-row-actions';
import type { PrBreakdown } from '../pull-requests/pr-change-breakdown';
import { CHECKS_HEADLINE, allowedMergeMethods, checksCount, checksList, checksRollup, emptyCard, prCard, resolvePullRequestMergeMethod, rowAction, summarizeChecks, trailingAction, width12, type CheckRow, type PrCard } from './pr-row-logic';
import { lastMergeMethod, projectDefaultMergeMethod } from '../pull-requests/pages-pr-actions';

export type RowExtra = {
  destructive: boolean; suffix: boolean;
  /** The checks popover's transparent tail: the window's width right of the checks trigger. */
  tail: number; popId: string; method: string;
  checksTitle: string; checksSummary: string; checksList: CheckRow[]; checksLabels: string[]; checksDetails: boolean; collapsible: boolean; expanded: boolean;
  card: PrCard;
};
export const emptyExtra = (): RowExtra => ({ destructive: false, suffix: false, tail: 0, popId: '', method: '', checksTitle: '', checksSummary: '', checksList: [], checksLabels: [], checksDetails: false, collapsible: false, expanded: false, card: emptyCard() });

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
 * The trigger (part "meta") is 26pt (the 14pt glyph, 6pt insets) plus its count after a 4pt gap.
 */
export const checksTrigger = (count: string) => 26 + (count ? 4 + width12(tabular(count)) : 0);
/** The second line starts 36pt in (ps-9, under the title); the number keeps 4pt after it and 2pt before the checks. */
export const checksTriggerLeft = (number: number) => 36 + width12(tabular(`#${number}`)) + 4 + 2;
/** Tabular digits are each as wide as a "0". */
const tabular = (text: string) => text.replace(/\d/g, '0');
export function checksTail(rightOfTrigger: number, count: string): number {
  const trigger = checksTrigger(count);
  return rightOfTrigger + trigger >= 320 + 5 ? 0 : Math.round(rightOfTrigger * 10) / 10;
}

export function rowExtra(client: T3Client, detail: Obj, reference: Obj, input: { key: string; lifecycleKey: string; rightGap: number; inline: boolean; breakdown?: PrBreakdown }) {
  const state = prState(client), checks = arr(detail.checks), rollup = checksRollup(checks);
  const conflicting = detail.state === 'open' && detail.mergeability === 'conflicting';
  // Counts beat the rollup's own wording where they are known.
  const count = checksCount(checks);
  // The project's default merge method, else the one last chosen on this device (as the panel's Merge resolves it).
  const method = resolvePullRequestMergeMethod(allowedMergeMethods(detail), null, projectDefaultMergeMethod(obj(client.config.settings), str(reference.projectId)), lastMergeMethod(client));
  const action = rowAction(detail), acting = isActing(client, reference);
  const trailing = trailingAction(action, { handoff: state.handoff, actionPending: acting, method, term: rowTerminology(client, reference, detail) });
  const { rowWidth, rowRight } = rowGeometry(input.inline, input.rightGap);
  const shownChecks = rollup && !conflicting && detail.isDraft !== true ? rollup : '';
  const summary = summarizeChecks(checks), list = checksList(checks, state.showAll.has(input.key));
  const ref = normalRef(reference);
  const extra: RowExtra = {
    destructive: trailing?.destructive ?? false, suffix: trailing?.suffix ?? false,
    tail: checksTail(rowRight + rowWidth - checksTriggerLeft(num(detail.number)) - checksTrigger(count), count), popId: `pr-checks-${num(detail.number)}`, method,
    checksTitle: CHECKS_HEADLINE[rollup] ?? '', checksSummary: summary, checksList: list.rows, checksLabels: list.labels, checksDetails: list.details, collapsible: list.collapsible, expanded: state.showAll.has(input.key),
    // The title's line is the whole row wide.
    card: prCard(detail, input.lifecycleKey, rowWidth + rowRight, input.breakdown),
  };
  return {
    // The plain tooltip only stands in where the card cannot (no state); the card is the row's tooltip.
    tooltip: `${str(detail.title)} #${num(detail.number)}`,
    checks: shownChecks, checksCount: count,
    checksAria: `Open checks: ${summary}`,
    action: trailing ? action : '', actionLabel: trailing?.label ?? '', actionTooltip: trailing?.tooltip ?? '',
    // Every action is held while one runs: a merge mid-hand-off would race the checkout.
    actionDisabled: acting || state.handoff !== '',
    actionTarget: JSON.stringify(ref),
    extra,
  };
}
