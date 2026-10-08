// @ref llp/1109.005-composer-and-transcript.decision.md#terminal-draft-publication-and-recovery-coordinator
import type { T3Client } from './shared/client';
import { mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { homeDraftLocation } from './home-drafts';
import type { MobileDraftClient } from './mobile-draft-recovery';
import type { Native, Files } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { mobileOutboxHomeAvailable } from './mobile-outbox-connection';
import { mobileOutboxDecode, mobileOutboxRetryDelay, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobilePendingTaskEditorsSnapshot } from './mobile-pending-task-state';
import { mobileOutboxDraftRecover } from './mobile-outbox-draft-recovery';
import { mobileOutboxDriveSnapshot, mobileOutboxDriveRead, mobileOutboxDriveRun, mobileOutboxDriveRecoverable } from './mobile-outbox-drive';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';

interface Attempt { signature: string; sequence: number; tries: number; retryAt: number }
interface CreationFailure { record: MobileOutboxRecord; draftKey: string; reason: string }
interface Root { busy: string; attempts: Map<string, Attempt>; failures: Map<string, CreationFailure> }
const roots = new WeakMap<T3Client, Root>();
function state(client: T3Client): Root {
  let root = roots.get(client);
  if (!root) { root = { busy: '', attempts: new Map(), failures: new Map() }; roots.set(client, root); }
  return root;
}
const ownerOf = (record: MobileOutboxWireOwner): MobileOutboxWireOwner => ({ origin: record.origin, environmentId: record.environmentId,
  threadId: record.threadId, messageId: record.messageId, commandId: record.commandId });
const identity = (owner: MobileOutboxWireOwner) => canonical(ownerOf(owner));
/** The persisted intent is discoverable even after its queue row was removed.
 * This schedules discovery only; the coordinator validates the entire native proof. */
function candidates(client: MobileDraftClient) {
  const candidates = new Map<string, { record: MobileOutboxRecord; signature: string }>();
  for (const record of mobileOutboxDriveRecoverable(client)) candidates.set(identity(record), { record, signature: canonical(record) });
  const raw = obj(client.local).mobileOutboxDraftHandoffs;
  let valid = raw === undefined || !!raw && typeof raw === 'object' && !Array.isArray(raw);
  for (const [id, value] of Object.entries(obj(raw))) {
    const decoded = mobileOutboxDecode(obj(value).record);
    if (!decoded.ok || decoded.record.commandId !== id || obj(value).operationId !== id) { valid = false; continue; }
    candidates.set(identity(decoded.record), { record: decoded.record, signature: canonical(value) });
  }
  const editors = mobilePendingTaskEditorsSnapshot(client), root = state(client);
  const values = [...candidates.values()].filter(({ record }) => mobileOutboxHomeAvailable(client, record) && !editors.markers.some(marker => identity(marker.owner) === identity(record)))
    .sort((a, b) => a.record.createdAt.localeCompare(b.record.createdAt) || identity(a.record).localeCompare(identity(b.record)));
  const ready = values.map(({ record, signature }) => {
    const id = identity(record); let attempt = root.attempts.get(id);
    if (!attempt || attempt.signature !== signature && root.busy !== id) {
      attempt = { signature, sequence: (attempt?.sequence ?? 0) + 1, tries: 0, retryAt: 0 }; root.attempts.set(id, attempt);
    }
    return { owner: ownerOf(record), id, attempt };
  });
  return { valid: valid && editors.ready && client.preferencesLoaded, values: ready };
}

/** Same root timer as delivery. No connection focus, retained Native or new clock. */
export function mobileOutboxRootSnapshot(client: MobileDraftClient, now: number) {
  const drive = mobileOutboxDriveSnapshot(client, now), root = state(client), recovery = candidates(client);
  if (root.busy || drive.busy || !recovery.valid) return { ...drive, busy: !!root.busy || drive.busy, next: '', delay: 0 };
  if (!drive.complete) return drive;
  const next = [...recovery.values].sort((a, b) => a.attempt.retryAt - b.attempt.retryAt)[0];
  if (!next) return drive;
  const delay = Math.max(1, next.attempt.retryAt - (Number.isFinite(now) ? now : 0));
  // A blocked recovery may wait while another owner's normal delivery proceeds.
  const sending = obj(parse(drive.next)).owner;
  const otherDelivery = sending && !recovery.values.some(candidate => identity(candidate.owner) === canonical(sending));
  if (drive.next && otherDelivery && drive.delay < delay) return drive;
  return { ...drive, next: JSON.stringify({ recovery: next.owner, sequence: next.attempt.sequence }), delay };
}
function parse(value: string): Obj { try { return obj(JSON.parse(value)); } catch { return {}; } }

export async function mobileOutboxRootAction(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  kind: string, key: string, now: number) {
  const root = state(client), result = (message = '') => ({ revision: client.revision, message });
  if (root.busy) return result();
  if (kind === 'read') return mobileOutboxDriveRead(client, native);
  if (!native?.available) return result('Open T3 Code on your iPhone or iPad.');
  if (!Number.isFinite(now) || now <= 0) return result('Wait for the app clock before sending.');
  const parsed = parse(key), recovery = candidates(client);
  if (!recovery.valid) return result('Read complete saved recovery ownership before sending.');
  const requested = parsed.recovery ?? (kind === 'retry' ? parsed : parsed.owner);
  const candidate = recovery.values.find(value => canonical(value.owner) === canonical(requested));
  if (!parsed.recovery && !candidate) return mobileOutboxDriveRun(client, native, key, now, kind === 'retry');
  if (!candidate || kind !== 'retry' && (!parsed.recovery || parsed.sequence !== candidate.attempt.sequence || candidate.attempt.retryAt > now)
    || mobileOutboxDriveSnapshot(client, now).busy || !mobileOutboxDriveSnapshot(client, now).complete) return result();
  root.busy = candidate.id; client.revision++;
  try {
    const recovered = await mobileOutboxDraftRecover(client, native, storage, { owner: candidate.owner, current: () => root.busy === candidate.id });
    if (recovered.status === 'recovered') {
      root.attempts.delete(candidate.id);
      if (recovered.handoff && recovered.rejectionReason) root.failures.set(candidate.id, {
        record: JSON.parse(JSON.stringify(recovered.handoff.record)), draftKey: recovered.draftKey, reason: recovered.rejectionReason });
    }
    else {
      candidate.attempt.sequence++; candidate.attempt.tries++;
      candidate.attempt.retryAt = now + mobileOutboxRetryDelay(candidate.attempt.tries);
      // The coordinator may have saved an intent. Its next render must retain backoff.
      const live = obj(obj(client.local).mobileOutboxDraftHandoffs)[candidate.owner.commandId];
      if (live) candidate.attempt.signature = canonical(live);
    }
    return result(kind === 'retry' ? recovered.reason : '');
  } catch (error) {
    if (letGo(error)) throw error;
    candidate.attempt.sequence++; candidate.attempt.tries++;
    candidate.attempt.retryAt = now + mobileOutboxRetryDelay(candidate.attempt.tries);
    return result(kind === 'retry' ? error instanceof Error ? error.message : String(error) : '');
  } finally { root.busy = ''; client.revision++; }
}


/** Transient thread presentation, like the source creation outcome. It grants no
 * send or recovery permission and is never projected as another Home task. */
export function mobileOutboxRootFailure(client: T3Client, environmentId: string, threadId: string) {
  const failure = [...state(client).failures.values()].find(value => value.record.environmentId === environmentId && value.record.threadId === threadId);
  if (!failure) return null;
  const draft = mobileNewTaskDraftLookup(client, failure.draftKey), record = failure.record;
  const editable = !!draft && draft.origin === record.origin && draft.environmentId === record.environmentId
    && draft.projectId === record.creation?.projectId && draft.createdAt === record.createdAt;
  return { ...failure, editable };
}
export function mobileOutboxRootEdit(client: T3Client, key: string) {
  const requested = parse(key), failure = state(client).failures.get(canonical(requested));
  if (!failure || canonical(ownerOf(failure.record)) !== canonical(requested))
    return { revision: client.revision, message: 'This task no longer has a recovered draft.', nextLocation: '' };
  const current = mobileOutboxRootFailure(client, failure.record.environmentId, failure.record.threadId);
  if (!current?.editable || current.draftKey !== failure.draftKey)
    return { revision: client.revision, message: 'The saved draft was already used or changed. Open its current state from Home.', nextLocation: '' };
  const draft = mobileNewTaskDraftLookup(client, failure.draftKey)!;
  return { revision: client.revision, message: '', nextLocation: homeDraftLocation(draft) };
}
