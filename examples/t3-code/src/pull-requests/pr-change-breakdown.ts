// A pull request's +/- split by what the changed files are (T3 Code 74541bab17, MIT, see LICENSE-T3:
// apps/web/src/components/pullRequest/PullRequestChangeBreakdownStat.tsx, the `changeBreakdown` query
// of packages/client-runtime/src/state/pullRequests.ts). The host classifies the files and answers
// `pullRequests.changeBreakdown`: code, tests, mocks, generated and docs, or null when it gives no
// per-file counts. A list row reads its split on the first hover or focus of its +/- (`pr-breakdown`,
// `surface-pr-breakdown`); a view of one pull request reads it at once (`eager`). Reads are shared per
// environment and pull request, fresh for 60 s and until the server announces a change.
//
// A read is the presenting resource's: the first one shows "Loading breakdown" and wakes the
// resource, which reads on its next run (the pattern of pages-pr-quick.ts's popovers).
import type { T3Client } from '../client/client';
import { decodeChangeBreakdown, num, str, type LineCount, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pullRequestRefreshEpoch } from './pages-pr-refresh';

/** One line of the split: a kind's label and its counts, already formatted, and its grid row (from 1). */
export type BreakdownLine = { key: string; label: string; additions: string; deletions: string; row: number };
/** What a +/- needs to draw its split: the key a hover asks with, and the lines once read. */
export type PrBreakdown = { ref: string; requested: boolean; loading: boolean; rows: BreakdownLine[]; total: BreakdownLine[] };
type Breakdown = NonNullable<ReturnType<typeof decodeChangeBreakdown>>;

/** The PullRequestRef the read takes (pr-row-actions.ts normalRef, kept here so the details card's logic can import this). */
const normalRef = (reference: Obj): Obj => {
  const host = str(reference.host);
  return { projectId: str(reference.projectId), ...(host ? { host } : {}), repository: str(reference.repository), number: num(reference.number) };
};
const KIND_LABELS: Record<keyof Breakdown, string> = { code: 'Code', tests: 'Tests', mocks: 'Mocks', generated: 'Generated', docs: 'Docs' };
const STALE_MS = 60_000;

const count = (value: number) => value.toLocaleString('en-US');
const line = (key: string, label: string, counts: LineCount, row: number): BreakdownLine => ({ key, label, additions: `+${count(counts.additions)}`, deletions: `-${count(counts.deletions)}`, row });
/** ChangeBreakdownGrid: each kind that changed, in the reference's order. */
export function breakdownLines(breakdown: Breakdown): BreakdownLine[] {
  return (Object.keys(KIND_LABELS) as (keyof Breakdown)[])
    .filter(kind => breakdown[kind].additions !== 0 || breakdown[kind].deletions !== 0)
    .map((kind, index) => line(kind, KIND_LABELS[kind], breakdown[kind], index + 1));
}

type Read = { data: Breakdown | null; at: number; epoch: number };
type State = { requested: Set<string>; reads: Map<string, Read>; shown: Set<string> };
const states = new WeakMap<object, State>();
function breakdownState(client: object): State {
  let state = states.get(client);
  if (!state) { state = { requested: new Set(), reads: new Map(), shown: new Set() }; states.set(client, state); }
  return state;
}

/** The pull request a split is read for: its reference, on the server it was listed on (`environmentId`, '' for the focused one). */
export type BreakdownTarget = { reference: Obj; environmentId?: string; additions: number; deletions: number };
export const breakdownKey = (client: T3Client, target: BreakdownTarget) => JSON.stringify([target.environmentId || client.environmentId, normalRef(target.reference)]);
const hasChange = (target: BreakdownTarget) => target.additions !== 0 || target.deletions !== 0;

/** `pr-breakdown` / `surface-pr-breakdown`: the first pointer or focus intent on a row's +/-. */
export function requestChangeBreakdown(client: object, key: string): string {
  if (key) breakdownState(client).requested.add(key);
  return '';
}

/**
 * The reads the targets owe: each requested (or `eager`) one with no answer younger than 60 s and
 * no announcement since. A first read is shown as loading before it is asked: it wakes `wakes` (by
 * default the page's and the shell's topics) and answers 'wake', so the caller presents that first;
 * 'done' when nothing more is owed now.
 */
export async function readChangeBreakdowns(client: T3Client, native: Native | null | undefined, targets: readonly BreakdownTarget[], now: number,
  options: { eager?: boolean; wakes?: readonly string[] } = {}): Promise<'wake' | 'done'> {
  const eager = options.eager === true;
  if (!native?.available) return 'done';
  const state = breakdownState(client), epoch = pullRequestRefreshEpoch(client);
  let wake = false;
  for (const target of targets) {
    const key = breakdownKey(client, target);
    if (!hasChange(target) || !str(target.reference.repository) || num(target.reference.number) <= 0) continue;
    if (!eager && !state.requested.has(key)) continue;
    const read = state.reads.get(key);
    if (read && read.epoch === epoch && (!now || now - read.at < STALE_MS)) continue;
    if (!read && !state.shown.has(key)) { state.shown.add(key); wake = true; continue; }
    state.shown.delete(key);
    try {
      const answer = await client.rpc(native, 'pullRequests.changeBreakdown', { ...normalRef(target.reference), ...(target.environmentId ? { environmentId: target.environmentId } : {}) });
      state.reads.set(key, { data: decodeChangeBreakdown(answer), at: now, epoch });
    } catch (error) {
      if (letGo(error)) throw error;
      // A host without the read (an older server), or one that failed it: no split, the +/- alone.
      state.reads.set(key, { data: read?.data ?? null, at: now, epoch });
    }
  }
  if (!wake) return 'done';
  // The Pull Requests page and the detail read on `t3.pr`, the shell's surfaces (the right panel, the details card) on `t3.notify`.
  for (const topic of options.wakes ?? ['t3.pr', 't3.notify']) { try { await native.later({ op: 'wake', topic }); } catch (error) { if (letGo(error)) throw error; } }
  return 'wake';
}

/**
 * The split as it stands for one +/-: its lines once read, "Loading breakdown" while a read is owed,
 * and nothing for a host that gives no per-file counts. `total` adds the reference's Total line
 * (the hover over a +/-); the details card's inline split has none.
 */
export function presentChangeBreakdown(client: T3Client, target: BreakdownTarget, options: { eager?: boolean; total?: boolean } = {}): PrBreakdown {
  const key = breakdownKey(client, target), state = breakdownState(client);
  const requested = options.eager === true || state.requested.has(key);
  if (!hasChange(target)) return { ref: key, requested: true, loading: false, rows: [], total: [] };
  const data = state.reads.get(key)?.data ?? null;
  if (!data) return { ref: key, requested, loading: requested && !state.reads.has(key), rows: [], total: [] };
  // The Total sits under a rule that takes the grid row after the kinds.
  const rows = breakdownLines(data);
  return { ref: key, requested, loading: false, rows,
    total: options.total === false ? [] : [line('total', 'Total', { additions: target.additions, deletions: target.deletions }, rows.length + 2)] };
}
export const emptyBreakdown = (): PrBreakdown => ({ ref: '', requested: true, loading: false, rows: [], total: [] });
