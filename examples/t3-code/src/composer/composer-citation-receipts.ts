import type { ComposerFieldIdentity, ComposerFieldSnapshot } from './composer-field-commands';
import { moveFreshCitation, type FreshCitation } from './composer-source-chips';

type Frame = readonly [number, number, number, number];
export type ComposerCitationEffect =
  | { kind: 'open'; fresh: FreshCitation; sourceFrame: Frame }
  | { kind: 'finish'; seq: number; send: boolean };
export type ComposerCitationReceipt = ComposerFieldIdentity & { citation: number };
export type ComposerCitationCompletion =
  | { kind: 'open'; fresh: FreshCitation; sourceFrame: Frame; after: number; interaction: number }
  | { kind: 'finish'; seq: number; send: boolean; interaction: number };
type Pending = { identity: ComposerFieldIdentity; expected: string; effect: ComposerCitationEffect; after: number; interaction: number };
const sameOwner = (a: ComposerFieldIdentity, b: ComposerFieldIdentity) => a.generation === b.generation && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;

/** Finite product effects travel through the independent field ACK. Receipts
 *  never store an editable value, native history or a caller continuation. */
export class ComposerCitationReceipts {
  private sequence = 0;
  private pending = new Map<number, Pending>();

  prepare(snapshot: ComposerFieldSnapshot, expected: string, effect: ComposerCitationEffect, after = 0, interaction = 0): ComposerCitationReceipt | null {
    if (!snapshot.live || snapshot.composing || expected === snapshot.value || this.pending.size >= 16 || !Number.isSafeInteger(this.sequence + 1)) return null;
    if (effect.kind === 'open' && (effect.fresh.value !== expected || effect.sourceFrame.some(value => !Number.isFinite(value))
      || effect.sourceFrame[2] <= 0 || effect.sourceFrame[3] <= 0 || !Number.isSafeInteger(effect.fresh.start) || !Number.isSafeInteger(effect.fresh.end)
      || effect.fresh.start < 0 || effect.fresh.end <= effect.fresh.start || effect.fresh.end > expected.length
      || expected.slice(effect.fresh.start, effect.fresh.end) !== effect.fresh.source)) return null;
    if (effect.kind === 'finish' && (!Number.isSafeInteger(effect.seq) || effect.seq <= 0)) return null;
    const identity = { generation: snapshot.generation, snapshotOwner: snapshot.snapshotOwner, owner: snapshot.owner };
    const stored = effect.kind === 'open' ? { ...effect, fresh: { ...effect.fresh, spaces: { ...effect.fresh.spaces } }, sourceFrame: [...effect.sourceFrame] as Frame } : { ...effect };
    const citation = ++this.sequence;
    this.pending.set(citation, { identity, expected, effect: stored, after, interaction });
    return { ...identity, citation };
  }

  complete(receipt: ComposerCitationReceipt, applied: boolean, current: ComposerFieldSnapshot): ComposerCitationCompletion | null {
    const pending = this.pending.get(receipt.citation);
    if (!pending || !sameOwner(pending.identity, receipt)) return null;
    this.pending.delete(receipt.citation);
    if (!applied || !current.live || current.composing || !sameOwner(receipt, current)) return null;
    if (pending.effect.kind === 'finish') return { ...pending.effect, send: pending.effect.send && pending.expected === current.value, interaction: pending.interaction };
    const fresh = moveFreshCitation(pending.effect.fresh, current.value);
    return fresh ? { ...pending.effect, fresh, after: pending.after, interaction: pending.interaction } : null;
  }
  refuse(receipt: ComposerCitationReceipt): void {
    const pending = this.pending.get(receipt.citation);
    if (pending && sameOwner(pending.identity, receipt)) this.pending.delete(receipt.citation);
  }
  reset(): void { this.pending.clear(); }
}
