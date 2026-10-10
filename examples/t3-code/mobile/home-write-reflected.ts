// @ref llp/1109.004-home-projection.decision.md#arrange-threads
// Pinned Orchestrator.ts thread lifecycle projection; no transport or mutable owner.
import type { Obj } from './shared/domain';

export function homeWriteReflected(pending: { payload: Obj; before: Obj }, thread: Obj | undefined): boolean {
  const p = pending.payload;
  if (p.type === 'thread.delete') return !thread;
  if (!thread) return false;
  if (p.type === 'thread.archive') return thread.archivedAt != null;
  if (p.type === 'thread.pin') return thread.pinnedAt != null && thread.snoozedUntil == null
    && (pending.before.settledOverride !== 'settled' || thread.settledOverride === 'active')
    && (pending.before.pinnedAt != null ? (thread.pinOrderKey ?? null) === (pending.before.pinOrderKey ?? null)
      : p.orderKey === undefined || thread.pinOrderKey === p.orderKey);
  if (p.type === 'thread.unpin') return thread.pinnedAt == null;
  if (p.type === 'thread.settle') return thread.settledOverride === 'settled';
  if (p.type === 'thread.unsettle') return thread.settledOverride === 'active';
  if (p.type === 'thread.snooze') return thread.snoozedUntil === p.snoozedUntil;
  if (p.type === 'thread.unsnooze') return thread.snoozedUntil == null;
  if (p.type === 'thread.auto-settle.set') return (thread.autoSettleDisabledAt == null) === p.enabled;
  if (p.type === 'thread.metadata.update' && typeof p.title === 'string') return thread.title === p.title;
  return p.regenerateTitle === true && (thread.titleRegeneration != null || thread.title !== pending.before.title);
}
