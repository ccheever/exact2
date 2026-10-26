// A pull request's +/- split by what the changed files are (T3 Code 74541bab17, MIT, see LICENSE-T3:
// apps/web/src/components/pullRequest/PullRequestChangeBreakdownStat.tsx, the `changeBreakdown` query
// of packages/client-runtime/src/state/pullRequests.ts). The host classifies the files and answers
// `pullRequests.changeBreakdown`: code, tests, mocks, generated and docs, or null when it gives no
// per-file counts. A row reads its split on the first hover or focus of its +/- (`pr-breakdown`,
// `surface-pr-breakdown`); a view of one pull request reads it when it opens (`eager`).
//
// Reads are shared per environment and pull request. A request (a hover, an opened view) reads once;
// after that a split is read again only when the server announces a change (the query's
// refreshTrigger) or the +/- beside it no longer matches the totals it was read with, which is how a
// surface without the announcement stream (a linked row's snapshot, another environment's row) sees
// the change move. Requests belong to their surface (`scope`) and end when it closes, as the
// reference's per-component state does, so a closed surface asks for nothing more.
//
// A read is the presenting resource's: the first one shows "Loading breakdown" and wakes the
// resource, which reads on its next run (the pattern of pages-pr-quick.ts's popovers).
import type { T3Client } from '../client/client';
import { decodeChangeBreakdown, num, str, type LineCount, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { width12 } from '../details/pr-row-logic';
import { pullRequestRefreshEpoch } from './pages-pr-refresh';

/** One line of the split: a kind's label and its counts, already formatted, and its grid row (from 1). */
export type BreakdownLine = { key: string; label: string; additions: string; deletions: string; row: number };
/**
 * What a +/- needs to draw its split: the key a hover asks with, the lines once read, and the bubble's
 * estimated size in points (the window's hover layer keeps a centred bubble inside the window and
 * flips it above the stat when it does not fit below).
 */
export type PrBreakdown = { ref: string; requested: boolean; loading: boolean; rows: BreakdownLine[]; total: BreakdownLine[]; width: number; height: number };
type Breakdown = NonNullable<ReturnType<typeof decodeChangeBreakdown>>;
/** The surface a request belongs to. */
export type BreakdownScope = 'list' | 'linked' | 'card' | 'detail';

/** The PullRequestRef the read takes (pr-row-actions.ts normalRef, kept here so the details card's logic can import this). */
const normalRef = (reference: Obj): Obj => {
  const host = str(reference.host);
  return { projectId: str(reference.projectId), ...(host ? { host } : {}), repository: str(reference.repository), number: num(reference.number) };
};
const KIND_LABELS: Record<keyof Breakdown, string> = { code: 'Code', tests: 'Tests', mocks: 'Mocks', generated: 'Generated', docs: 'Docs' };

const count = (value: number) => value.toLocaleString('en-US');
const line = (key: string, label: string, counts: LineCount, row: number): BreakdownLine => ({ key, label, additions: `+${count(counts.additions)}`, deletions: `-${count(counts.deletions)}`, row });
/** ChangeBreakdownGrid: each kind that changed, in the reference's order. */
export function breakdownLines(breakdown: Breakdown): BreakdownLine[] {
  return (Object.keys(KIND_LABELS) as (keyof Breakdown)[])
    .filter(kind => breakdown[kind].additions !== 0 || breakdown[kind].deletions !== 0)
    .map((kind, index) => line(kind, KIND_LABELS[kind], breakdown[kind], index + 1));
}

/** A 12pt ui-monospace advance. */
export const MONO12 = 7.22;
const LOADING = 'Loading breakdown';
/**
 * The bubble's size: TooltipPopup's 8×4 insets and 1pt border around the grid (three auto columns 16pt apart,
 * 16pt lines 4pt apart, 2pt above and below, and the rule's 1pt line before the Total), or the loading words.
 */
function bubbleSize(rows: BreakdownLine[], total: BreakdownLine[], loading: boolean): { width: number; height: number } {
  if (!rows.length) return loading ? { width: Math.ceil(width12(LOADING) + 18), height: 26 } : { width: 0, height: 0 };
  const lines = [...rows, ...total], widest = (pick: (entry: BreakdownLine) => number) => Math.max(...lines.map(pick));
  const width = widest(entry => width12(entry.label)) + 16 + widest(entry => entry.additions.length * MONO12) + 16 + widest(entry => entry.deletions.length * MONO12);
  const height = lines.length * 16 + (lines.length - 1 + (total.length ? 1 : 0)) * 4 + (total.length ? 1 : 0) + 4;
  return { width: Math.ceil(width + 18), height: Math.ceil(height + 10) };
}

/** A read: the split (null for none) and the totals and announcement it answers for. */
type Read = { data: Breakdown | null; epoch: number; additions: number; deletions: number };
type State = { requested: Map<string, BreakdownScope>; owed: Set<string>; reads: Map<string, Read>; shown: Set<string> };
const states = new WeakMap<object, State>();
function breakdownState(client: object): State {
  let state = states.get(client);
  if (!state) { state = { requested: new Map(), owed: new Set(), reads: new Map(), shown: new Set() }; states.set(client, state); }
  return state;
}

/** The pull request a split is read for: its reference, on the server it was listed on (`environmentId`, '' for the focused one). */
export type BreakdownTarget = { reference: Obj; environmentId?: string; additions: number; deletions: number };
export const breakdownKey = (client: T3Client, target: BreakdownTarget) => JSON.stringify([target.environmentId || client.environmentId, normalRef(target.reference)]);
const hasChange = (target: BreakdownTarget) => target.additions !== 0 || target.deletions !== 0;

/** `pr-breakdown` / `surface-pr-breakdown`: the first pointer or focus intent on a +/-. A new request owes one read. */
export function requestChangeBreakdown(client: object, key: string, scope: BreakdownScope): string {
  const state = breakdownState(client);
  if (key && !state.requested.has(key)) { state.requested.set(key, scope); state.owed.add(key); }
  return '';
}
/** A surface closed (or moved to another pull request): its requests end, except `keep`. */
export function releaseChangeBreakdowns(client: object, scope: BreakdownScope, keep: ReadonlySet<string> = new Set()): void {
  const state = breakdownState(client);
  for (const [key, owner] of state.requested) if (owner === scope && !keep.has(key)) { state.requested.delete(key); state.owed.delete(key); }
}

/**
 * The reads the targets owe: each requested one (an `eager` scope requests its targets itself) that was
 * just requested, has no answer, was answered before the latest announcement, or was answered for other
 * totals. A first read is shown as loading before it is asked: it wakes `wakes` (by default the page's and
 * the shell's topics) and answers 'wake', so the caller presents that first; 'done' when nothing more is owed.
 */
export async function readChangeBreakdowns(client: T3Client, native: Native | null | undefined, targets: readonly BreakdownTarget[],
  options: { eager?: BreakdownScope; wakes?: readonly string[] } = {}): Promise<'wake' | 'done'> {
  if (!native?.available) return 'done';
  const state = breakdownState(client), epoch = pullRequestRefreshEpoch(client);
  let wake = false;
  for (const target of targets) {
    const key = breakdownKey(client, target);
    if (!hasChange(target) || !str(target.reference.repository) || num(target.reference.number) <= 0) continue;
    if (options.eager) requestChangeBreakdown(client, key, options.eager);
    if (!state.requested.has(key)) continue;
    const read = state.reads.get(key);
    const current = !!read && read.epoch === epoch && read.additions === target.additions && read.deletions === target.deletions;
    if (current && !state.owed.has(key)) continue;
    if (!read && !state.shown.has(key)) { state.shown.add(key); wake = true; continue; }
    state.shown.delete(key);
    try {
      const answer = await client.rpc(native, 'pullRequests.changeBreakdown', { ...normalRef(target.reference), ...(target.environmentId ? { environmentId: target.environmentId } : {}) });
      state.reads.set(key, { data: decodeChangeBreakdown(answer), epoch, additions: target.additions, deletions: target.deletions });
    } catch (error) {
      if (letGo(error)) throw error;
      // A host without the read (an older server), or one that failed it: no split, the +/- alone.
      state.reads.set(key, { data: null, epoch, additions: target.additions, deletions: target.deletions });
    }
    state.owed.delete(key);
  }
  if (!wake) return 'done';
  // The Pull Requests page and the detail read on `t3.pr`, the shell's surfaces (the right panel, the details card) on `t3.notify`.
  for (const topic of options.wakes ?? ['t3.pr', 't3.notify']) { try { await native.later({ op: 'wake', topic }); } catch (error) { if (letGo(error)) throw error; } }
  return 'wake';
}

/**
 * The split as it stands for one +/-: its lines once read, "Loading breakdown" while a first read is owed,
 * and nothing for a host that gives no per-file counts. `total` adds the reference's Total line
 * (the hover over a +/-); the details card's inline split has none.
 */
export function presentChangeBreakdown(client: T3Client, target: BreakdownTarget, options: { eager?: boolean; total?: boolean } = {}): PrBreakdown {
  const key = breakdownKey(client, target), state = breakdownState(client);
  const requested = options.eager === true || state.requested.has(key);
  if (!hasChange(target)) return { ...emptyBreakdown(), ref: key };
  const data = state.reads.get(key)?.data ?? null;
  if (!data) {
    const loading = requested && !state.reads.has(key);
    return { ref: key, requested, loading, rows: [], total: [], ...bubbleSize([], [], loading) };
  }
  // The Total sits under a rule that takes the grid row after the kinds.
  const rows = breakdownLines(data);
  const total = options.total === false ? [] : [line('total', 'Total', { additions: target.additions, deletions: target.deletions }, rows.length + 2)];
  return { ref: key, requested, loading: false, rows, total, ...bubbleSize(rows, total, false) };
}
export function emptyBreakdown(): PrBreakdown { return { ref: '', requested: true, loading: false, rows: [], total: [], width: 0, height: 0 }; }
