import { bridgeReply, type Native } from '../client/protocol';
import { obj } from '../client/domain';
import type { ComposerFieldResult } from './composer-field-commands';

export type ComposerFieldCompletion = {
  focus?: string;
  removedImage?: string;
  applied?: (native: Native) => void | Promise<void>;
  refused?: (result: ComposerFieldResult) => void;
};
type Receipt = { effects: ComposerFieldCompletion; settled: boolean };
const completions = new WeakMap<object, Receipt>();
export function fieldCompletion(request: object): Receipt | undefined { return completions.get(request); }
/** Only the ACK's current native ticket may run completion work. Refusal is state-only. */
export function completeFieldRequest(receipt: Receipt | undefined, result: ComposerFieldResult, native?: Native): void | Promise<void> {
  if (!receipt || receipt.settled) return;
  if (result.applied && receipt.effects.applied && !native) throw new Error('Applied field completion needs the current native turn.');
  receipt.settled = true;
  if (result.applied) return receipt.effects.applied?.(native!);
  receipt.effects.refused?.(result);
}
export type ComposerFieldAdmission = { accepted: boolean; queued: boolean; applied: boolean; seq: number; reason: string };
/** Admission finishes in this turn. Actual input and the independent ACK own application. */
export async function requestComposerField(native: Native, request: Record<string, unknown>, effects: ComposerFieldCompletion = {}): Promise<ComposerFieldAdmission> {
  const receipt: Receipt = { effects, settled: false };
  try {
    const operation = { ...request };
    completions.set(operation, receipt);
    const reply = await bridgeReply(native, operation);
    const value = obj(reply.value), applied = reply.ok && value.applied === true;
    const queued = reply.ok && value.accepted === true && value.queued === true;
    const reason = typeof value.reason === 'string' ? value.reason : reply.error?.message ?? '';
    if (!queued) await completeFieldRequest(receipt, { applied, reason, value: '', start: 0, end: 0 }, native);
    return { accepted: applied || queued, queued, applied, seq: typeof value.seq === 'number' ? value.seq : 0, reason };
  } catch (error) {
    completeFieldRequest(receipt, { applied: false, reason: error instanceof Error ? error.message : 'field edit failed', value: '', start: 0, end: 0 });
    throw error;
  }
}
