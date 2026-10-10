// Pinned365aa87982 use-thread-outbox-drain.ts and use-composer-drafts.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import { MAX_ATTACHMENTS } from './shared/composer-editor-files';
import { mobileReferencedComposerContext, type MobileMessageContext } from './mobile-new-task-context';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';

export type MobileOutboxRecoveryContent = Pick<MobileOutboxRecord,
  'text' | 'context' | 'attachments' | 'modelSelection' | 'runtimeMode' | 'interactionMode'>;
export type MobileOutboxRecoveryKind = 'accepted-edits' | 'rejected';
export type MobileOutboxRecoveryMerge = { ok: true; content: MobileOutboxRecoveryContent } | { ok: false; reason: string };

/** This chooses a destination, not permission to recover. The caller must prove
 * the final native receipt, ownership and exact queue revision independently. */
export function mobileOutboxRecoveryKey(record: MobileOutboxRecord, kind: MobileOutboxRecoveryKind): string {
  return kind === 'rejected' && record.creation ? `new-task:restored-${record.messageId}`
    : `${record.environmentId}:${record.threadId}`;
}

/** Detached content only; no draft publication, native removal or byte release.
 * Storage validation retains unknown context kinds for a later supported editor. */
export function mobileOutboxRecoveryMergeContent(input: MobileOutboxRecord, existing: MobileOutboxRecoveryContent,
  kind: MobileOutboxRecoveryKind): MobileOutboxRecoveryMerge {
  const incoming = mobileOutboxDecode(input);
  if (!incoming.ok) return { ok: false, reason: incoming.error };
  // Reuse the outbox's lossless storage checks for the destination's content.
  // Specify every optional field so absent target choices never inherit input picks.
  const target = mobileOutboxDecode({ ...incoming.record, text: existing.text, context: existing.context,
    attachments: existing.attachments, modelSelection: existing.modelSelection,
    runtimeMode: existing.runtimeMode, interactionMode: existing.interactionMode });
  if (!target.ok) return { ok: false, reason: 'The destination draft contains invalid saved content. Keep both copies until it is resolved.' };
  const saved = incoming.record, current = target.record;
  const ids = new Set(current.attachments.map(file => file.id.toLowerCase()));
  const attachments = [...current.attachments, ...saved.attachments.filter(file => !ids.has(file.id.toLowerCase()))];
  if (kind === 'rejected' && attachments.length > MAX_ATTACHMENTS)
    return { ok: false, reason: `Remove attachments from the draft before restoring this message. Messages can contain at most ${MAX_ATTACHMENTS} attachments.` };
  const text = !saved.text || current.text === saved.text || current.text.endsWith(`\n\n${saved.text}`) ? current.text
    : current.text ? `${current.text}\n\n${saved.text}` : saved.text;
  const records = new Map<string, MobileMessageContext['records'][number]>();
  for (const context of [current.context, saved.context])
    for (const record of (context?.records ?? []) as MobileMessageContext['records']) records.set(String(record.contextId), record);
  const context = records.size ? mobileReferencedComposerContext(text, { version: 1, records: [...records.values()] }) : undefined;
  return { ok: true, content: { text, ...(context ? { context: { version: context.version, records: context.records } } : {}), attachments,
    ...(current.modelSelection === undefined ? {} : { modelSelection: current.modelSelection }),
    ...(current.runtimeMode === undefined ? {} : { runtimeMode: current.runtimeMode }),
    ...(current.interactionMode === undefined ? {} : { interactionMode: current.interactionMode }),
    ...(saved.modelSelection === undefined ? {} : { modelSelection: saved.modelSelection }),
    ...(saved.runtimeMode === undefined ? {} : { runtimeMode: saved.runtimeMode }),
    ...(saved.interactionMode === undefined ? {} : { interactionMode: saved.interactionMode }) } };
}
