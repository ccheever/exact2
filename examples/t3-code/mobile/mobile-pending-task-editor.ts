// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// One awaited operation owns its handles. Durable markers hold delivery across restart.
import type { MobileDraftClient } from './mobile-draft-recovery';
import { ClientError, type Files, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxCapture, mobileOutboxHold, mobileOutboxReleaseHold,
  mobileOutboxPrepareUpdate, mobileOutboxResumeUpdate, mobileOutboxAcknowledge, type MobileOutboxRow } from './mobile-outbox';
import { mobilePendingTaskEditorKey, mobilePendingTaskEditorsSnapshot, mobilePendingTaskEditorsCreate,
  mobilePendingTaskEditorsReplace, mobilePendingTaskEditorsRemove,
  type MobilePendingTaskMarker, type MobilePendingTaskExpected } from './mobile-pending-task-state';
import { mobilePendingTaskDraftAdopt, mobilePendingTaskDraftBuildRecord, mobilePendingTaskDraftFingerprint,
  mobilePendingTaskDraftCleanup } from './mobile-pending-task-draft';
import { mobileNewTaskDraftBoundKey, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { mobileOutboxDraftHandoffStatus } from './mobile-outbox-draft-handoff';
import { mobilePendingTaskReceiptsPrepare, mobilePendingTaskReceiptsRead } from './mobile-pending-task-receipts';

export interface MobilePendingTaskEditorInput { owner: MobileOutboxWireOwner; current(): boolean }
export interface MobilePendingTaskEditorResult {
  status: 'ready' | 'saved' | 'recovery-saved' | 'finished' | 'retained'; reason: string;
  marker: MobilePendingTaskMarker | null; fingerprint: string | null;
}
const busy = new WeakMap<MobileDraftClient, Set<string>>();
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const holdOwner = (marker: MobilePendingTaskMarker) => `pending-editor:${mobilePendingTaskEditorKey(marker.owner)}:${marker.session}`;
const changed = () => new ClientError('The pending task editor changed. Reopen its saved status.', 'superseded');
function lookup(client: MobileDraftClient, owner: MobileOutboxWireOwner): MobilePendingTaskMarker | null {
  return mobilePendingTaskEditorsSnapshot(client).markers.find(marker => mobilePendingTaskEditorKey(marker.owner) === mobilePendingTaskEditorKey(owner)) ?? null;
}
function matching(client: MobileDraftClient, marker: MobilePendingTaskExpected): boolean {
  const live = lookup(client, marker.owner);
  return !!live && live.session === marker.session && live.revision === marker.revision;
}
function baseline(row: MobileOutboxRow) { return { record: row.record, token: row.token, revision: row.nativeRevision }; }
function exactRow(client: MobileDraftClient, marker: MobilePendingTaskMarker): MobileOutboxRow | null {
  const row = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === marker.owner.messageId);
  return row && row.held && row.status === 'confirmed' && canonical(baseline(row)) === canonical(marker.baseline) ? row : null;
}
function replace(client: MobileDraftClient, marker: MobilePendingTaskMarker, changes: Partial<MobilePendingTaskMarker>) {
  const next = mobilePendingTaskEditorsReplace(client, marker, { ...marker, ...changes, revision: marker.revision + 1 });
  if (!next) throw changed(); return next;
}
function contentRevision(client: MobileDraftClient, marker: MobilePendingTaskMarker): number {
  return Math.max(marker.contentRevision, mobileNewTaskDraftLookup(client, marker.draftKey)?.revision ?? 0);
}
interface Context {
  native: Native; storage: Files; check(): void;
  persist(): Promise<void>;
}
function context(client: MobileDraftClient, handle: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput): Context {
  const stamp = () => canonical([client.origin, client.environmentId, client.generation, client.projectId,
    client.threadId, client.threadEpoch, client.draftKey, mobileQueuedEditOrigin(client)]);
  const initial = stamp();
  const check = () => {
    if (!input.current() || stamp() !== initial || input.owner.origin !== mobileQueuedEditOrigin(client)
      || input.owner.environmentId !== client.environmentId) throw changed();
    if (!client.preferencesLoaded || !mobilePendingTaskEditorsSnapshot(client).ready)
      throw new ClientError('Read complete saved editor ownership before changing this task.');
  };
  check();
  if (!handle?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to edit this pending task.');
  const base = letGoAware(handle);
  const native: Native = { available: true, watch(topic) { check(); base.watch(topic); }, async later(request) {
    check(); const result = await base.later(request); check(); return result;
  } };
  const files: Files = { fs: {
    async mkdir(path) { check(); const result = await storage.fs.mkdir(path); check(); return result; },
    async readFile(path) { check(); const result = await storage.fs.readFile(path); check(); return result; },
    async atomicWriteFile(path, bytes) { check(); const result = await storage.fs.atomicWriteFile(path, bytes); check(); return result; },
  } };
  return { native, storage: files, check, async persist() { check(); await client.persist(files); check(); } };
}
async function run(client: MobileDraftClient, handle: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput, action: (ctx: Context) => Promise<MobilePendingTaskEditorResult>): Promise<MobilePendingTaskEditorResult> {
  const key = mobilePendingTaskEditorKey(input.owner), active = busy.get(client) ?? new Set<string>(); busy.set(client, active);
  const reply = (status: MobilePendingTaskEditorResult['status'], reason: string): MobilePendingTaskEditorResult =>
    ({ status, reason, marker: lookup(client, input.owner), fingerprint: null });
  if (active.has(key)) return reply('retained', 'This pending task is already being saved or recovered.');
  active.add(key);
  try { return await action(context(client, handle, storage, input)); }
  catch (error) {
    if (letGo(error)) throw error;
    return reply('retained', error instanceof Error ? error.message : String(error));
  } finally { active.delete(key); }
}
const result = (status: MobilePendingTaskEditorResult['status'], marker: MobilePendingTaskMarker | null,
  reason = '', fingerprint: string | null = null): MobilePendingTaskEditorResult => ({ status, marker: marker && clone(marker), reason, fingerprint });
async function read(client: MobileDraftClient, ctx: Context) {
  if (!await mobileOutboxRead(client, ctx.native)) throw new ClientError('Read complete pending task storage before editing.'); ctx.check();
}
async function hold(client: MobileDraftClient, ctx: Context, marker: MobilePendingTaskMarker): Promise<boolean> {
  const row = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === marker.owner.messageId);
  if (!row || mobilePendingTaskEditorKey(row.record) !== mobilePendingTaskEditorKey(marker.owner)) return false;
  // Pending/terminal recovery can project the saved request token rather than its old baseline.
  // Native resumeUpdate independently verifies the complete original request and CAS.
  const capture = mobileOutboxCapture(client, marker.owner.messageId);
  if (!capture || !await mobileOutboxHold(client, ctx.native, capture, holdOwner(marker))) return false;
  ctx.check(); if (!matching(client, marker)) throw changed(); await read(client, ctx); return true;
}
async function receipts(client: MobileDraftClient, ctx: Context, marker: MobilePendingTaskMarker, terminalRecovery = false): Promise<string> {
  const row = exactRow(client, marker);
  if (!row) return 'The queued task changed. Your editor is retained; reopen its recovery status.';
  const input = { owner: marker.owner, row, current: () => { ctx.check(); return matching(client, marker); } };
  let value = await (terminalRecovery ? mobilePendingTaskReceiptsRead : mobilePendingTaskReceiptsPrepare)(client, ctx.native, input);
  if (terminalRecovery && value.final.operation && ['acknowledged', 'rejected'].includes(value.final.operation.state)
    && value.status === 'prepare-required') value = await mobilePendingTaskReceiptsPrepare(client, ctx.native, input);
  ctx.check(); if (!matching(client, marker)) throw changed();
  if (!terminalRecovery) return value.status === 'editable' ? '' : value.reason || 'Recover the original send before saving these edits.';
  const receipt = value.final.operation;
  if (!receipt || !value.final.durable || receipt.stage !== 'start-turn'
    || !(value.status === 'original-rejected' || value.status === 'original-accepted' && receipt.cleanup?.phase === 'edited'))
    return value.reason || 'Resolve the original terminal send before preparing this editor for recovery.';
  const completed = await mobileOutboxDraftHandoffStatus(ctx.native, receipt);
  ctx.check(); if (!matching(client, marker)) throw changed();
  return completed.handoff ? 'This task already has a recovered destination. Keep newer editor changes until its saved handoff is resolved.' : '';
}
/** Resolve only the saved request. Never recapture newer typing for an interrupted mutation. */
async function resume(client: MobileDraftClient, ctx: Context, marker: MobilePendingTaskMarker): Promise<MobilePendingTaskMarker> {
  const pending = marker.pending;
  if (!pending) return marker;
  // A prior attempt may have failed before its preference write was admitted.
  await ctx.persist(); if (!matching(client, marker)) throw changed();
  const outcome = await mobileOutboxResumeUpdate(client, ctx.native, pending.request, holdOwner(marker)); ctx.check();
  if (!matching(client, marker)) throw changed();
  if (outcome.status === 'unknown' || outcome.status === 'uncertain')
    throw new ClientError(outcome.message || 'The saved update is still unresolved. Keep the editor until recovery finishes.');
  if (outcome.status !== 'committed') {
    // A terminal refusal is durable native evidence; retain the editor and its old baseline.
    marker = replace(client, marker, { pending: null }); await ctx.persist();
    await mobileOutboxAcknowledge(client, ctx.native, marker.owner.messageId, pending.mutationId); ctx.check();
    throw new ClientError(outcome.message || 'The pending task changed before the save. Your draft is retained.');
  }
  if (canonical(outcome.record) !== canonical(pending.request.record) || !Number.isSafeInteger(outcome.revision)
    || outcome.revision! <= marker.baseline.revision)
    throw new ClientError('The saved update receipt does not match this editor. Keep its original recovery request.');
  marker = replace(client, marker, { baseline: { record: pending.request.record, token: pending.mutationId, revision: outcome.revision! }, pending: null });
  // After a crash the old pending marker may replay this receipt, until this exact adoption is durable.
  await ctx.persist(); if (!matching(client, marker)) throw changed();
  await mobileOutboxAcknowledge(client, ctx.native, marker.owner.messageId, pending.mutationId); ctx.check();
  await read(client, ctx); return marker;
}
/** Open/adopt once, reacquire the persisted session's hold, and recover its exact saved request. */
export function mobilePendingTaskEditorOpen(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput): Promise<MobilePendingTaskEditorResult> {
  return run(client, native, storage, input, async ctx => {
    await read(client, ctx); let marker = lookup(client, input.owner);
    const reopening = !!marker;
    if (!marker) {
      const row = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === input.owner.messageId);
      if (!row?.record.creation || row.status !== 'confirmed' || row.nativeRevision === null
        || mobilePendingTaskEditorKey(row.record) !== mobilePendingTaskEditorKey(input.owner))
        return result('retained', null, 'This pending task is no longer available for editing.');
      const [session] = await client.ids(ctx.native, 1); ctx.check();
      marker = mobilePendingTaskEditorsCreate(client, { version: 1, owner: clone(input.owner), session: session!, revision: 1,
        draftKey: `new-task:pending-${input.owner.messageId}`, contentRevision: 0,
        baseline: { record: row.record, token: row.token, revision: row.nativeRevision }, pending: null });
      if (!marker) throw changed();
    }
    if (!await hold(client, ctx, marker)) return result('retained', marker, 'The queued task is unavailable. Your saved editor is retained.');
    if (reopening && !mobileNewTaskDraftLookup(client, marker.draftKey))
      return result('retained', marker, 'The saved editor content is missing. Recover its original capture before editing.');
    const adopted = mobilePendingTaskDraftAdopt(client, marker.baseline.record);
    if (adopted.reason) return result('retained', marker, adopted.reason);
    marker = replace(client, marker, { contentRevision: contentRevision(client, marker) });
    await ctx.persist(); if (!matching(client, marker)) throw changed();
    marker = await resume(client, ctx, marker);
    const reason = await receipts(client, ctx, marker);
    return result(reason ? 'retained' : 'ready', marker, reason);
  });
}
/** Save retains draft/marker/hold. The caller unbinds the editor before explicit Finish. */
export function mobilePendingTaskEditorSave(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput & { expected: MobilePendingTaskExpected }): Promise<MobilePendingTaskEditorResult> {
  return saveEditor(client, native, storage, input, false);
}
/** Stage the latest retained editor in the local queue for a separate draft handoff.
 * Terminal receipts remain immutable. This result never authorizes Send or Finish. */
export function mobilePendingTaskEditorSaveForRecovery(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput & { expected: MobilePendingTaskExpected }): Promise<MobilePendingTaskEditorResult> {
  return saveEditor(client, native, storage, input, true);
}
function saveEditor(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput & { expected: MobilePendingTaskExpected }, terminalRecovery: boolean): Promise<MobilePendingTaskEditorResult> {
  return run(client, native, storage, input, async ctx => {
    if (terminalRecovery && client.local.pending[input.owner.environmentId])
      throw new ClientError('Resolve the active send before preparing this editor for recovery.');
    let marker = lookup(client, input.owner);
    if (!marker || mobilePendingTaskEditorKey(input.expected.owner) !== mobilePendingTaskEditorKey(input.owner)
      || !matching(client, input.expected)) throw changed();
    await read(client, ctx); if (!matching(client, marker)) throw changed();
    if (!await hold(client, ctx, marker)) return result('retained', marker, 'The queued task is unavailable. Keep the saved editor.');
    marker = await resume(client, ctx, marker);
    const reason = await receipts(client, ctx, marker, terminalRecovery); if (reason) return result('retained', marker, reason);
    const captured = mobilePendingTaskDraftBuildRecord(client, marker);
    if (captured.status !== 'ready') { await ctx.persist(); return result('retained', marker, captured.reason); }
    if (canonical(captured.record) === canonical(marker.baseline.record)) {
      await ctx.persist(); return result(terminalRecovery ? 'recovery-saved' : 'saved', marker, '', captured.fingerprint);
    }
    const request = mobileOutboxPrepareUpdate(client, captured.record, { expectedToken: marker.baseline.token, expectedRevision: marker.baseline.revision });
    const revision = contentRevision(client, marker);
    marker = replace(client, marker, { contentRevision: revision, pending: { mutationId: request.mutationId, contentRevision: revision, request } });
    await ctx.persist(); if (!matching(client, marker)) throw changed();
    marker = await resume(client, ctx, marker);
    const newer = mobilePendingTaskDraftFingerprint(client, marker.draftKey) !== captured.fingerprint;
    return result(newer ? 'retained' : terminalRecovery ? 'recovery-saved' : 'saved', marker, newer ? 'The captured edits were saved. Newer changes are still in this editor.' : '', captured.fingerprint);
  });
}
/** Persist projected cleanup before touching live content. Never close a bound editor.
 * A failed write leaves both live draft and gate intact; native bytes release later
 * through the existing persisted release queues and ownership filters. */
export function mobilePendingTaskEditorFinish(client: MobileDraftClient, native: Native | null | undefined, storage: Files,
  input: MobilePendingTaskEditorInput & { expected: MobilePendingTaskExpected; fingerprint: string }): Promise<MobilePendingTaskEditorResult> {
  return run(client, native, storage, input, async ctx => {
    const marker = lookup(client, input.owner);
    if (!marker || mobilePendingTaskEditorKey(input.expected.owner) !== mobilePendingTaskEditorKey(input.owner)
      || !matching(client, input.expected) || marker.pending) throw changed();
    const unchanged = () => matching(client, marker) && mobileNewTaskDraftBoundKey(client) !== marker.draftKey
      && mobilePendingTaskDraftFingerprint(client, marker.draftKey) === input.fingerprint;
    if (!unchanged()) return result('retained', marker, 'Finish the current editor before releasing its saved task.');
    await read(client, ctx); if (!unchanged()) throw changed();
    // A released hold's reply may have been lost. Reacquire this same session
    // before retrying Finish; the baseline check still rejects another winner.
    if (!await hold(client, ctx, marker)) return result('retained', marker, 'The queued task is unavailable. Keep the saved editor.');
    if (!unchanged() || !exactRow(client, marker)) throw changed();
    const reason = await receipts(client, ctx, marker); if (reason) return result('retained', marker, reason);
    if (!unchanged()) throw changed();
    const captured = mobilePendingTaskDraftBuildRecord(client, marker);
    if (captured.status !== 'ready' || canonical(captured.record) !== canonical(marker.baseline.record))
      return result('retained', marker, 'Save the current edits before releasing this task.');
    if (client.pendingTaskCleanup) return result('retained', marker, 'Another pending editor is finishing.');
    const lease = { marker: clone(marker), fingerprint: input.fingerprint };
    client.pendingTaskCleanup = lease;
    let finished = false;
    try {
      // MobileDraftClient applies this projection to every concurrent writer, so
      // an ordinary preference save cannot queue the old editor behind cleanup.
      await client.persist(storage);
      if (!unchanged()) {
        client.pendingTaskCleanup = null; await client.persist(storage);
        return result('retained', lookup(client, input.owner), 'Newer editor changes were retained.');
      }
      try { ctx.check(); } catch (error) {
        // The file answer fulfilled; retire the projection and repair current
        // local ownership before reporting a route change. No native retry.
        client.pendingTaskCleanup = null; await client.persist(storage); throw error;
      }
      if (!await mobileOutboxReleaseHold(client, ctx.native, marker.owner.messageId, holdOwner(marker)))
        throw new ClientError('The pending task hold was not released. Keep the editor and retry.');
      ctx.check();
      if (!unchanged()) {
        client.pendingTaskCleanup = null; await client.persist(storage);
        return result('retained', lookup(client, input.owner), 'Newer editor changes were retained.');
      }
      if (!mobilePendingTaskDraftCleanup(client, marker.draftKey, input.fingerprint)
        || !mobilePendingTaskEditorsRemove(client, marker)) throw changed();
      finished = true;
    } catch (error) {
      client.pendingTaskCleanup = null;
      // An expired answer cannot issue a repair. Its unchanged captured content
      // remains in the committed queue; live ownership remains for the next open.
      if (!letGo(error)) await client.persist(storage);
      throw error;
    } finally { if (client.pendingTaskCleanup === lease) client.pendingTaskCleanup = null; }
    if (!finished) return result('retained', marker, 'The pending editor is still retained.');
    await read(client, ctx); return result('finished', null);
  });
}
