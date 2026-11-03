// Readers for the fields a server built at T3 Code 5d3d78ab93 adds to the wire (MIT, see LICENSE-T3):
// packages/contracts/src/scheduledTask.ts (`once` schedules), environment.ts (`oneTimeSchedules`), threadTags.ts,
// settings.ts (`hiddenThreadTags`, `hideTaggedThreads`, `usageModelFilter`), usage.ts (`UsageModelFilter`) and
// model.ts (`role: "reasoning"`); and from packages/client-runtime/src: modelTraits.ts (`isReasoningControl`,
// `isReasoningSelect`), state/server.ts (`hiddenThreadTagsAtom`), state/usage.ts (`usageSummaryInput`) and
// shared/usageMerge.ts. A user message's `modelSwitch` is domain.ts's. A value the reference's schema refuses reads
// as absent here, as its ForwardCompatibleOptional and catchDecoding fields do, so a hand-edited settings.json or a
// later server never fails a whole snapshot.
import { obj, str, type Obj } from './domain';

// MARK: Scheduled tasks

const ISO_WITH_OFFSET = /^(\d{4})-(\d{2})-(\d{2})T([01]\d|2[0-3]):[0-5]\d(:[0-5]\d(\.\d+)?)?(Z|[+-]\d{2}:\d{2})$/;
const YEAR_0000 = Date.parse('0000-01-01T00:00:00Z');
const YEAR_10000 = Date.parse('+010000-01-01T00:00:00Z');

/** An ISO date and time with an offset that names a real day; the server stores UTC, which must keep a four-digit year. */
export function isIsoWithOffset(value: string): boolean {
  const match = ISO_WITH_OFFSET.exec(value);
  const time = Date.parse(value);
  if (!match || !(time >= YEAR_0000 && time < YEAR_10000)) return false;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  return days !== undefined && day >= 1 && day <= days;
}

/** ScheduledTaskOnceSchedule: `{ type: "once", runAt }`, or null for any other schedule. */
export function onceSchedule(schedule: unknown): { type: 'once'; runAt: string } | null {
  const value = obj(schedule), runAt = str(value.runAt).trim();
  return value.type === 'once' && runAt !== '' && isIsoWithOffset(runAt) ? { type: 'once', runAt } : null;
}

/** `capabilities.oneTimeSchedules`: the server accepts `once` schedules; older ones refuse them on save. */
export const oneTimeSchedules = (config: Obj): boolean => obj(obj(config.environment).capabilities).oneTimeSchedules === true;

// MARK: Thread tags

/** StoredThreadTags: a thread's tags, empty when absent or when any entry is not a string. */
export function threadTags(thread: Obj): string[] {
  const tags = thread.tags;
  return Array.isArray(tags) && tags.every(tag => typeof tag === 'string') ? tags : [];
}

/**
 * hiddenThreadTagsAtom: the tags whose threads display lists leave out, trimmed, or null while
 * `hideTaggedThreads` is off or no tag is left. Entries that are not strings drop alone.
 */
export function hiddenThreadTags(settings: Obj): string[] | null {
  if (settings.hideTaggedThreads !== true) return null;
  const listed: unknown[] = Array.isArray(settings.hiddenThreadTags) ? settings.hiddenThreadTags : [];
  const tags = listed.filter((tag): tag is string => typeof tag === 'string').map(tag => tag.trim()).filter(tag => tag !== '');
  return tags.length > 0 ? tags : null;
}

// MARK: Usage

export interface UsageModelSelector { provider?: string; model: string }
export interface UsageModelFilter { include?: UsageModelSelector[]; exclude?: UsageModelSelector[] }

/** TrimmedNonEmptyString: the trimmed text, or null. */
const trimmed = (value: unknown): string | null => typeof value === 'string' && value.trim() !== '' ? value.trim() : null;

function selectors(value: unknown): UsageModelSelector[] | null {
  if (!Array.isArray(value)) return null;
  const read: UsageModelSelector[] = [];
  for (const entry of value) {
    const item = obj(entry), model = trimmed(item.model), provider = item.provider === undefined ? undefined : trimmed(item.provider);
    if (Array.isArray(entry) || typeof entry !== 'object' || entry === null || model === null || provider === null) return null;
    read.push(provider === undefined ? { model } : { provider, model });
  }
  return read;
}

/** ServerSettings.usageModelFilter (ForwardCompatibleOptional): the filter, or undefined when absent or invalid. */
export function usageModelFilter(settings: Obj): UsageModelFilter | undefined {
  const value = settings.usageModelFilter;
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return undefined;
  const filter = value as Obj, include = filter.include === undefined ? undefined : selectors(filter.include), exclude = filter.exclude === undefined ? undefined : selectors(filter.exclude);
  if (include === null || exclude === null) return undefined;
  return { ...(include ? { include } : {}), ...(exclude ? { exclude } : {}) };
}

/** usageSummaryInput: each environment is asked with its own configured filter. */
export function usageSummaryInput<T extends Obj>(input: T, filter: UsageModelFilter | undefined): T & { modelFilter?: UsageModelFilter } {
  return filter === undefined ? input : { ...input, modelFilter: filter };
}

/** usageMerge: a summary read under another filter counted different records, so its cells are not missing ones. */
export const sameUsageModelFilter = (left: Obj, right: Obj): boolean => JSON.stringify(left.modelFilter) === JSON.stringify(right.modelFilter);

// MARK: Models

/** threadExecution.ts REASONING_EFFORT_OPTION_IDS (v0.1.5): the option ids that carry a reasoning effort (Cursor's is
 *  `reasoning_effort`, Antigravity's `thinking_level`). */
export const REASONING_EFFORT_OPTION_IDS: readonly string[] = ['reasoningEffort', 'effort', 'reasoning', 'variant', 'reasoning_effort', 'thinking_level'];

const REASONING_SELECT_DESCRIPTOR_IDS: ReadonlySet<string> = new Set([...REASONING_EFFORT_OPTION_IDS, 'thinking']);

/** isReasoningControl: a known effort id (REASONING_SELECT_DESCRIPTOR_IDS), or a select its agent marked `role: "reasoning"` (an ACP `thought_level`). */
export function isReasoningControl(descriptor: Obj): boolean {
  return (descriptor.type === 'select' && descriptor.role === 'reasoning') || REASONING_SELECT_DESCRIPTOR_IDS.has(str(descriptor.id));
}
export const isReasoningSelect = (descriptor: Obj): boolean => descriptor.type === 'select' && isReasoningControl(descriptor);
