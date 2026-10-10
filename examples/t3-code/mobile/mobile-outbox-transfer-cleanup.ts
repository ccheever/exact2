// Source365aa87982 defers attachment cleanup after local enqueue; Exact preserves newer edits.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { contextId, contextReferences } from './shared/composer-editor-menu';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftStore, mobileNewTaskDraftRemoveMetadata,
  mobileNewTaskDraftHasContent, mobileNewTaskDraftHasSelections } from './mobile-new-task-drafts';

const field = 'mobileOutboxTransferCompletions';
const dictionary = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const clone = <T>(value: T): T => value === undefined ? value : JSON.parse(JSON.stringify(value));
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (dictionary(value)) return `{${Object.keys(value).sort().filter(key => value[key] !== undefined)
    .map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  return JSON.stringify(value) ?? 'null';
}
const equal = (a: unknown, b: unknown) => canonical(a) === canonical(b);
/** Invalid saved values remain visible: dropping them could permit destructive replay. */
export function mobileOutboxTransferCompletionsHydrate(client: T3Client, saved: Obj): void {
  if (Object.hasOwn(saved, field)) Object.assign(client.local, { [field]: clone(saved[field]) });
}
export function mobileOutboxTransferCompletionsPersisted(client: T3Client): Obj[string] {
  return clone((client.local as unknown as Obj)[field]);
}
/** The caller supplies a claim validated by the native transfer API. No release or I/O here. */
export function mobileOutboxTransferApplyCleanup(client: T3Client, claim: MobileOutboxTransferClaim): 'applied' | 'already-applied' | 'blocked' {
  if (!client.preferencesLoaded || claim.state !== 'queued' || !claim.capture || !claim.record) return 'blocked';
  const captured = claim.capture.draft;
  if (claim.transferId !== claim.messageId || claim.record.messageId !== claim.messageId || captured.key !== claim.draftKey || !claim.fingerprint) return 'blocked';
  const saved = (client.local as unknown as Record<string, unknown>)[field];
  if (saved !== undefined && !dictionary(saved)) return 'blocked';
  const completions = saved ?? {}, marker = { version: 1, draftKey: claim.draftKey, fingerprint: claim.fingerprint };
  if (Object.hasOwn(completions, claim.transferId)) return equal(completions[claim.transferId], marker) ? 'already-applied' : 'blocked';
  const current = mobileNewTaskDraftPresentation(client, captured.key);
  if (current && current.environmentId === captured.environmentId && current.origin === captured.origin
    && current.projectId === captured.projectId && current.createdAt === captured.createdAt) {
    const key = captured.key, record = mobileNewTaskDraftStore(client).records[key]!;
    if (current.revision === captured.revision && equal(current.context, captured.context)) {
      if (current.text === captured.text) { delete client.local.drafts[key]; delete record.context; }
      const references = new Set(contextReferences(client.local.drafts[key] ?? '').map(reference => reference.id));
      client.local.snapshotDrafts[key] = current.images.filter(image => !captured.images.some(sent => equal(sent, image))
        || references.has(contextId('image', String(image.id))));
      setDraftFiles(client.local, draftFiles(client.local).filter(file => file.draftKey !== key
        || references.has(file.contextId) || !captured.files.some(sent => equal(sent, file))));
    }
    // Residual content keeps its settings; choices can change without incrementing revision.
    if (equal(current.context, captured.context) && !(client.local.drafts[key] ?? '').length && !mobileNewTaskDraftHasContent(client, key)) {
      if (equal(record.choices, captured.choices)) record.choices = null;
      if (equal(client.local.composerControls.contexts[key] ?? null, captured.workspace)) delete client.local.composerControls.contexts[key];
      if (equal(record.branchChoice, captured.branchChoice)) delete record.branchChoice;
      if (!mobileNewTaskDraftHasSelections(client, key)) {
        delete client.local.drafts[key];
        delete client.local.snapshotDrafts[key];
        mobileNewTaskDraftRemoveMetadata(client, key);
      }
    }
    const next = mobileNewTaskDraftPresentation(client, key);
    if (next && !equal(current, next)) record.revision++;
  }
  // A retargeted/absent incarnation records an intentional no-op, never a future cleanup promise.
  Object.assign(client.local, { [field]: { ...completions, [claim.transferId]: marker } });
  client.revision++;
  return 'applied';
}
/** Inherited flushes catch failure to preserve retry IDs. A retained success is not an unlink. */
export function mobileOutboxTransferReleaseHandle(native: Native): Native {
  return { available: native.available, watch: topic => native.watch(topic), async later(request) {
    const response = await native.later(request), operation = obj(request).op, reply = obj(response), value = obj(reply.value);
    if ((operation === 'snapshotDraftRemove' || operation === 'composerAttachRemove') && reply.ok === true
      && (value.removed !== true || value.retained === true)) throw new ClientError('The attachment is still retained by a local owner.', 'retained');
    return response;
  } };
}
