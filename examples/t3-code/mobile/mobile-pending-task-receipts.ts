// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Editing never changes a command whose original native attempt owns the wire.
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileOutboxRead, mobileOutboxSnapshot, type MobileOutboxRow } from './mobile-outbox';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileOutboxDeliveryStatus, mobileOutboxDeliveryRecover, mobileOutboxDeliveryRetire,
  type MobileOutboxDeliveryStatus } from './mobile-outbox-delivery';
import { mobileOutboxInlineLookup, mobileOutboxInlineRecover, mobileOutboxInlineRetire,
  type MobileOutboxInlineStatus } from './mobile-outbox-inline-delivery';

export interface MobilePendingTaskReceiptsInput {
  owner: MobileOutboxWireOwner;
  row: MobileOutboxRow;
  /** Caller proves its exact persisted editor session and its own native hold.
   * A row's held boolean alone does not identify which editor owns that hold. */
  current(): boolean;
}
export interface MobilePendingTaskReceiptsResult {
  status: 'editable' | 'prepare-required' | 'original-accepted' | 'original-rejected' | 'recovery-required' | 'stale';
  reason: string; owner: MobileOutboxWireOwner; row: MobileOutboxRow;
  final: MobileOutboxDeliveryStatus; settings: MobileOutboxDeliveryStatus[]; inline: MobileOutboxInlineStatus[];
}
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const ownerOf = (record: MobileOutboxRecord): MobileOutboxWireOwner => ({ origin: record.origin, environmentId: record.environmentId,
  threadId: record.threadId, messageId: record.messageId, commandId: record.commandId });
const baseline = (row: MobileOutboxRow) => canonical({ record: row.record, token: row.token, revision: row.nativeRevision });
const empty = (): MobileOutboxDeliveryStatus => ({ operation: null, durable: false });
class Changed extends Error {}

/** Read-only receipt discovery. Neither absence nor a hold cancels admitted wire
 * work. The result is a point-in-time gate; save must call Prepare again under
 * the same caller-owned editor/hold and use its original row CAS. */
export function mobilePendingTaskReceiptsRead(client: T3Client, native: Native | null | undefined,
  input: MobilePendingTaskReceiptsInput): Promise<MobilePendingTaskReceiptsResult> {
  return inspect(client, native, input, false);
}
/** Explicit local recovery and exact never-issued retirement. No send, reserve,
 * ACK cleanup, hold release, row update, preference write or retained handle. */
export function mobilePendingTaskReceiptsPrepare(client: T3Client, native: Native | null | undefined,
  input: MobilePendingTaskReceiptsInput): Promise<MobilePendingTaskReceiptsResult> {
  return inspect(client, native, input, true);
}
async function inspect(client: T3Client, handle: Native | null | undefined, input: MobilePendingTaskReceiptsInput,
  prepare: boolean): Promise<MobilePendingTaskReceiptsResult> {
  const owner = copy(input.owner), captured = copy(input.row);
  const value: MobilePendingTaskReceiptsResult = { status: 'recovery-required', reason: '', owner, row: captured,
    final: empty(), settings: [empty(), empty()], inline: [] };
  const reply = (status: MobilePendingTaskReceiptsResult['status'], reason = '') => ({ ...value, status, reason });
  const selection = () => canonical([client.origin, client.environmentId, client.generation, client.threadId, client.threadEpoch, client.draftKey]);
  const expected = selection();
  const current = () => {
    if (!input.current() || selection() !== expected) throw new ClientError('The pending editor changed.', 'superseded');
  };
  const row = () => {
    const snapshot = mobileOutboxSnapshot(client), found = snapshot.rows.find(row => row.record.messageId === owner.messageId);
    if (!snapshot.complete || !snapshot.ownerEpoch || !found || !found.held || found.status !== 'confirmed'
      || baseline(found) !== baseline(captured) || canonical(ownerOf(found.record)) !== canonical(owner)
      || snapshot.outcomes.some(item => item.messageId === owner.messageId && ['unknown', 'uncertain'].includes(item.status)))
      throw new Changed('The pending row changed or no longer has a confirmed editor hold. Reopen its saved status.');
    value.row = copy(found);
  };
  function verify(saved: MobileOutboxDeliveryStatus, stage: 'final' | 'settings') {
    if (saved.operation && (canonical(ownerOf(saved.operation.record)) !== canonical(owner)
      || (saved.operation.stage === 'settings-sync') !== (stage === 'settings')))
      throw new Changed('A saved receipt belongs to another pending task or command stage.');
  }
  function policy(): MobilePendingTaskReceiptsResult {
    const deliveries = [value.final, ...value.settings], all = [...deliveries, ...value.inline];
    if (all.some(saved => saved.operation && !saved.durable))
      return reply('prepare-required', 'Recover the exact saved receipt durability before editing.');
    const final = value.final.operation;
    if (final?.state === 'acknowledged') return reply('original-accepted', 'The server accepted the original task. Preserve these edits and recover them into a new task.');
    if (final?.state === 'rejected') return reply('original-rejected', 'The original task was rejected. Preserve its draft and recover it before creating another command.');
    if (final && ['issued', 'uncertain'].includes(final.state))
      return reply('recovery-required', 'Resolve the original task send before changing its saved command. An editor hold cannot cancel it.');
    if (value.settings.some(saved => saved.operation && !['reserved', 'retired'].includes(saved.operation.state)))
      return reply('recovery-required', 'The original settings command has already been attempted. Preserve these edits for recovery with a new identity.');
    if (value.inline.some(saved => saved.operation && !['reserved', 'retired'].includes(saved.operation.state)))
      return reply('recovery-required', 'The original image request has already been attempted. Preserve these edits for recovery with a new identity.');
    if (all.some(saved => saved.operation?.state === 'reserved'))
      return reply('prepare-required', 'Retire the exact unissued reservations before saving changed content.');
    return reply('editable');
  }
  try {
    current();
    const decoded = mobileOutboxDecode(captured.record);
    if (!decoded.ok || canonical(ownerOf(decoded.record)) !== canonical(owner) || !captured.held
      || captured.status !== 'confirmed' || !captured.token || !Number.isSafeInteger(captured.nativeRevision) || captured.nativeRevision! < 0)
      return reply('stale', 'Choose the complete original task owner and its confirmed held row.');
    if (!handle?.available) return reply('recovery-required', 'Open T3 Code on your iPhone or iPad to read pending task receipts.');
    const base = letGoAware(handle); let ready = false;
    const native: Native = { available: true, watch(topic) { current(); base.watch(topic); }, async later(request) {
      current(); if (ready) row(); const result = await base.later(request); current(); if (ready) row(); return result;
    } };
    const refresh = async () => { if (!await mobileOutboxRead(client, native)) throw new Changed('Read complete pending-task ownership before editing.'); current(); row(); ready = true; };
    const discover = async () => {
      const ids = [owner.commandId, owner.commandId + ':runtime-mode', owner.commandId + ':interaction-mode'];
      const found: MobileOutboxDeliveryStatus[] = [];
      for (let i = 0; i < ids.length; i++) {
        const saved = await mobileOutboxDeliveryStatus(native, ids[i]!); current(); row(); verify(saved, i ? 'settings' : 'final'); found.push(saved);
      }
      const inline = await mobileOutboxInlineLookup(native, owner); current(); row();
      value.final = found[0]!; value.settings = found.slice(1); value.inline = inline.operations;
    };
    await refresh(); await discover();
    if (!prepare) { await refresh(); return policy(); }
    // Inspect every original stage before retiring any reservation. Recovery
    // confirms durability only and cannot reinterpret issued work as unsent.
    const deliveries = [value.final, ...value.settings];
    for (let i = 0; i < deliveries.length; i++) {
      const saved = deliveries[i]!;
      if (saved.operation && !saved.durable) {
        deliveries[i] = await mobileOutboxDeliveryRecover(native, saved.operation); current(); row();
      }
    }
    value.final = deliveries[0]!; value.settings = deliveries.slice(1);
    for (let i = 0; i < value.inline.length; i++) {
      const saved = value.inline[i]!;
      if (saved.operation && !saved.durable) { value.inline[i] = await mobileOutboxInlineRecover(native, saved.operation); current(); row(); }
    }
    const decision = policy();
    if (!['editable', 'prepare-required'].includes(decision.status)) { await refresh(); return decision; }
    // Each native retire validates exact lifecycle revision and active-attempt
    // exclusion, including a wire attempt admitted before the hold was acquired.
    for (let i = 0; i < deliveries.length; i++) {
      const saved = deliveries[i]!;
      if (saved.operation?.state === 'reserved') { deliveries[i] = await mobileOutboxDeliveryRetire(native, saved.operation); current(); row(); }
    }
    value.final = deliveries[0]!; value.settings = deliveries.slice(1);
    for (let i = 0; i < value.inline.length; i++) {
      const saved = value.inline[i]!;
      if (saved.operation?.state === 'reserved') { value.inline[i] = await mobileOutboxInlineRetire(native, saved.operation); current(); row(); }
    }
    // Re-discover even absent IDs: a late earlier attempt must not be mistaken
    // for permission to save. No automatic second recovery/retirement pass.
    await refresh(); await discover(); await refresh(); return policy();
  } catch (error) {
    if (letGo(error)) throw error;
    return reply(error instanceof Changed ? 'stale' : 'recovery-required', error instanceof Error ? error.message : String(error));
  }
}
