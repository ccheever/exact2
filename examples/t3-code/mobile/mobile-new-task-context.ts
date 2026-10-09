// Source365aa87982 composerContext.ts and use-composer-drafts.ts; app-owned full-key payloads.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { contextReferences } from './shared/composer-editor-menu';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileNewTaskDraftClone as clone, mobileNewTaskDraftStore, type MobileNewTaskDraft } from './mobile-new-task-drafts';

export interface MobileMessageContext { version: 1; records: Obj[] }
export interface MobileNewTaskContextGuard { key: string; createdAt: string; revision: number }
export type MobileNewTaskContextResult = { ok: true; context: MobileMessageContext | undefined } | { ok: false; error: string };
const invalid = () => ({ ok: false as const, error: 'This draft contains unsupported or invalid context.' });

/** Source ordered dependency pass. Repeated references need only one identity here. */
export function mobileReferencedComposerContext(text: string, context?: MobileMessageContext): MobileMessageContext | undefined {
  if (!context) return undefined;
  const ids = new Set(contextReferences(text).map(reference => reference.id));
  for (const record of context.records) {
    if (ids.has(str(record.contextId)) && record.kind === 'preview-annotation' && record.screenshotContextId)
      ids.add(str(record.screenshotContextId));
  }
  const records = context.records.filter(record => ids.has(str(record.contextId)));
  if (records.length === context.records.length) return context;
  return records.length ? { version: 1, records } : undefined;
}

/** Only the live record plus at most 200 undo payloads stay in this invocation-owned history. */
export function mobileCreateContextHistory() {
  const records = new Map<string, Obj>();
  const restore = (text: string, current?: MobileMessageContext): MobileMessageContext | undefined => {
    for (const record of current?.records ?? []) {
      records.delete(str(record.contextId)); records.set(str(record.contextId), clone(record));
    }
    const limit = Math.max(200, current?.records.length ?? 0);
    while (records.size > limit) records.delete(records.keys().next().value!);
    return mobileReferencedComposerContext(text, { version: 1, records: [...records.values()] });
  };
  // Batch publication must inspect dependency records without changing undo history.
  return Object.assign(restore, { snapshot: (): Obj[] => [...records.values()].map(record => clone(record)) });
}
function decode(raw: unknown): MobileNewTaskContextResult {
  if (raw === undefined) return { ok: true, context: undefined };
  const value = obj(raw);
  if (value.version !== 1 || !Array.isArray(value.records)) return invalid();
  const records: Obj[] = [], ids = new Set<string>();
  for (const rawRecord of value.records) {
    const record = obj(rawRecord);
    if (!mobileContextRecordValid(record) || ids.has(str(record.contextId))) return invalid();
    ids.add(str(record.contextId)); records.push(record);
  }
  return { ok: true, context: { version: 1, records } };
}
/** Validate before pruning, so removing a link never launders malformed saved payloads.
 * Attachment synthesis and missing-reference checks remain the capture owner's responsibility. */
export function mobileNewTaskContextProject(text: string, raw: unknown): MobileNewTaskContextResult {
  const decoded = decode(raw);
  if (!decoded.ok) return decoded;
  const context = mobileReferencedComposerContext(text, decoded.context);
  if (context && (context.records.length > 200 || JSON.stringify(context.records).length > 16000000))
    return { ok: false, error: 'This draft has too much context to send.' };
  return { ok: true, context: context ? clone(context) : undefined };
}
export function mobileNewTaskContextRead(client: T3Client, key: string): MobileNewTaskContextResult {
  const record = mobileNewTaskDraftStore(client).records[key];
  return record ? mobileNewTaskContextProject(client.local.drafts[key] ?? '', record.context) : invalid();
}
export function mobileNewTaskContextGuard(client: T3Client, key: string): MobileNewTaskContextGuard | null {
  const record = mobileNewTaskDraftStore(client).records[key];
  return record ? { key, createdAt: record.createdAt, revision: record.revision } : null;
}
// Keying by the actual registry record makes hydration/discard discard its undo history too.
const histories = new WeakMap<MobileNewTaskDraft, ReturnType<typeof mobileCreateContextHistory>>();
/** Atomically write the producer's text and frozen payload. The caller owns route/focus admission.
 * Ordinary typing preserves invalid raw context; producer insertion/removal must not overwrite it. */
export function mobileNewTaskContextWrite(client: T3Client, guard: MobileNewTaskContextGuard, text: string,
  added?: Obj, removeId = ''): boolean {
  const record = mobileNewTaskDraftStore(client).records[guard.key];
  if (!record || record.createdAt !== guard.createdAt || record.revision !== guard.revision) return false;
  const decoded = decode(record.context), producer = added !== undefined || !!removeId;
  if ((!decoded.ok && producer) || (added !== undefined && !mobileContextRecordValid(added))) return false;
  let next: MobileMessageContext | undefined;
  if (decoded.ok) {
    const history = histories.get(record) ?? mobileCreateContextHistory();
    const restored = history(text, decoded.context);
    if (producer) {
      const records = new Map((restored?.records ?? []).map(entry => [str(entry.contextId), entry]));
      if (removeId) records.delete(removeId);
      if (added) records.set(str(added.contextId), clone(added));
      next = mobileReferencedComposerContext(text, { version: 1, records: [...records.values()] });
      if ((next?.records.length ?? 0) > 200) return false;
    } else next = restored;
    histories.set(record, history);
    if (next === undefined) delete record.context;
    else record.context = clone(next);
  }
  client.local.drafts[guard.key] = text;
  record.revision++; client.revision++;
  return true;
}
