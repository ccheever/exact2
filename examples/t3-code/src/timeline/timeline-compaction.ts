// Compactions T3 Code starts, adapted from T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3):
// packages/contracts/src/orchestrationV2.ts (isCompactionCommandMessage, idleCompactionOf),
// packages/client-runtime/src/userMessage.ts (IDLE_COMPACTING_LABELS, IDLE_COMPACTION_TITLES),
// packages/shared/src/claudeCompaction.ts (claudeResumeCompactionAge) and
// apps/web/src/components/chat/MessagesTimeline.logic.ts deriveMessagesTimelineRows.
import { arr, obj, str, type Obj } from '../client/domain';
import type { WorkEntry } from './timeline-worklog';
import type { Entry } from './timeline-rows';

/** OrchestrationV2IdleCompaction: why T3 Code compacted a thread on its own, on the `/compact` it sent. */
export type IdleCompaction = 'beforeCacheExpires' | 'onReturn';

/** The working row while T3 Code's own compaction runs. */
export const IDLE_COMPACTING_LABELS: Record<IdleCompaction, string> = {
  beforeCacheExpires: 'Compacting before the cache expires…',
  onReturn: 'Compacting after being idle…',
};

/** Leads the finished compaction's token counts, in place of "Context compacted". */
export const IDLE_COMPACTION_TITLES: Record<IdleCompaction, string> = {
  beforeCacheExpires: 'Context automatically compacted before cache expiry',
  onReturn: 'Context compacted after being idle',
};

/** isCompactionCommandMessage: a prompt that only asks the provider to compact, so its turn is the compaction itself. */
export function isCompactionCommand(item: Obj): boolean {
  if (item.type !== 'user_message') return false;
  // Measured before it is lowercased: a long message is no command, and lowercasing it each turn costs its length.
  const text = str(item.text).trim();
  return text.length === '/compact'.length && text.toLowerCase() === '/compact' && arr(item.attachments).length === 0;
}

/** idleCompactionOf: why T3 Code sent this `/compact` on its own, or undefined for one someone asked for. */
export function idleCompactionOf(item: Obj): IdleCompaction | undefined {
  if (!isCompactionCommand(item)) return undefined;
  if (item.idleCompaction === 'beforeCacheExpires' || item.idleCompaction === 'onReturn') return item.idleCompaction;
  // Servers before `idleCompaction` marked their own as agent-created.
  return item.createdBy === 'agent' && item.creationSource === 'server' ? 'beforeCacheExpires' : undefined;
}

const RESUME_QUESTION = /^This session is (\d+h \d+m|\d+m) old and uses \d{1,3}(?:,\d{3})* tokens\. Compact it before continuing\?$/u;
const COMPACT_ANSWER = 'Compact and continue';

/**
 * claudeResumeCompactionAge: the session age from an answered resume question whose answer was to compact, or null.
 * Answers are keyed by question id, and the id may itself be the question text.
 */
export function resumeCompactionAge(answer: Obj): string | null {
  const texts = obj(answer.questionTextById);
  for (const [id, reply] of Object.entries(obj(answer.answers))) {
    const age = RESUME_QUESTION.exec(str(texts[id], id))?.[1];
    const chosen: unknown[] = Array.isArray(reply) ? reply : [reply];
    if (age !== undefined && chosen.includes(COMPACT_ANSWER)) return age;
  }
  return null;
}

export interface Compactions {
  /** Each `/compact` turn's run, and why T3 Code sent it (undefined for one someone asked for). */
  runs: Map<string, IdleCompaction | undefined>;
  /** The runs whose compaction item arrived. */
  compacted: Set<string>;
  /** The running `/compact` turn, until its compaction item finishes. */
  running: string | null;
  /** The running compaction whose Working row says why the message just sent waits: compacting on return, or one a
   * resume question asked for. Any other compaction shows only its live divider. */
  waiting: string | null;
  /** The Working row's label while `waiting` compacts. */
  label: string | undefined;
  /** A compaction is running, so no Thinking row shows. */
  active: boolean;
  /** The active `/compact` turn compacted and did nothing else: no Working or Thinking row is left. */
  onlyFinished: boolean;
  /** "Context compacted"'s replacement for a compaction T3 Code started without being asked. */
  title(runId: string, entryId: string): string | undefined;
}

/**
 * A bare `/compact` turn is the compaction itself: one compaction row stands in for its prompt and the Working row,
 * from the moment the turn starts. Likewise an answered "compact before continuing?" question folds into the
 * compaction it asked for. `runOf` is an entry's response run (a user message has none).
 */
export function compactions(entries: Entry[], isWorking: boolean, unsettled: string, entryActive: (entry: WorkEntry) => boolean,
  runOf: (entry: Entry) => string): Compactions {
  const runs = new Map<string, IdleCompaction | undefined>(), compacted = new Set<string>();
  const pendingAge = new Map<string, string>(), ages = new Map<string, string>();
  const isCompaction = (entry: Entry): entry is Extract<Entry, { kind: 'work' }> => entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction';
  for (const entry of entries) {
    if (entry.kind === 'message' && str(entry.item.runId) && isCompactionCommand(entry.item)) runs.set(str(entry.item.runId), idleCompactionOf(entry.item));
    else if (entry.kind === 'work' && entry.entry.runId) {
      const runId = entry.entry.runId;
      if (isCompaction(entry)) {
        compacted.add(runId);
        const age = pendingAge.get(runId);
        if (age !== undefined) { ages.set(entry.id, age); pendingAge.delete(runId); }
      }
      const age = entry.entry.questionAnswer ? resumeCompactionAge(entry.entry.questionAnswer) : null;
      if (age !== null) pendingAge.set(runId, age);
    }
  }
  const activeEntry = entries.find((entry): entry is Extract<Entry, { kind: 'work' }> => isCompaction(entry) && entryActive(entry.entry));
  const activeRun = isWorking && !!unsettled && runs.has(unsettled);
  // The item can finish before its run settles; only other work should keep activity visible.
  const finished = activeRun && compacted.has(unsettled) && activeEntry === undefined;
  const onlyFinished = finished && !entries.some(entry => runOf(entry) === unsettled && !isCompaction(entry)
    && !(entry.kind === 'message' && isCompactionCommand(entry.item)));
  const running = activeRun && !finished ? unsettled : null;
  const waiting = running !== null && runs.get(running) === 'onReturn' ? running
    : activeEntry?.entry.runId && ages.has(activeEntry.id) ? activeEntry.entry.runId : null;
  const age = activeEntry ? ages.get(activeEntry.id) : undefined;
  return {
    runs, compacted, running, waiting, onlyFinished,
    label: waiting === null ? undefined : age ? `Compacting after ${age} idle…` : IDLE_COMPACTING_LABELS.onReturn,
    active: activeEntry !== undefined || activeRun && !finished,
    title: (runId, entryId) => {
      if (!runId) return undefined;
      const reason = runs.get(runId);
      if (reason !== undefined) return IDLE_COMPACTION_TITLES[reason];
      const idle = ages.get(entryId);
      return idle ? `Context compacted after ${idle} idle` : undefined;
    },
  };
}
