import type { ComposerFieldSnapshot } from './composer-field-commands';
import { ComposerChipPresses } from './composer-chip-state';
import { ComposerCitationReceipts, type ComposerCitationCompletion } from './composer-citation-receipts';
import type { ComposerChip } from './composer-source-chips';
import { windowClient } from '../client/scope-root';

export type ComposerChipPreview = ComposerFieldSnapshot & { inputSerial: number };
export type ComposerChipFinish = {
  seq: number; generation: number; snapshotOwner: string; owner: string; value: string; inputSerial: number;
  surface: 'composer' | 'preview'; start: number; end: number; focus: 'after' | 'select' | 'outside' | 'return' | 'none'; send: boolean; interaction: number;
};
export const NO_CHIP_FINISH: ComposerChipFinish = { seq: 0, generation: -1, snapshotOwner: '', owner: '', value: '', inputSerial: 0, surface: 'composer', start: 0, end: 0, focus: 'none', send: false, interaction: -1 };
type Entry = { presses: ComposerChipPresses; citations: ComposerCitationReceipts; preview: ComposerChipPreview | null; finish: ComposerChipFinish | null };
const entries = new WeakMap<object, Entry>();
/** The press numbers of a window's composers: its own and its side chats' (thread scopes). */
const numbering = new WeakMap<object, { last: number }>();
export function composerChipStore(owner: object): Entry {
  let state = entries.get(owner);
  if (!state) {
    const root = windowClient(owner);
    let numbers = numbering.get(root);
    if (!numbers) numbering.set(root, numbers = { last: 0 });
    state = { presses: new ComposerChipPresses(numbers), citations: new ComposerCitationReceipts(), preview: null, finish: null };
    entries.set(owner, state);
  }
  return state;
}

/** Called only after the field independently confirms its generated input. */
export function composerChipCompleted(owner: object, effect: ComposerCitationCompletion | null, current: ComposerFieldSnapshot, chips: readonly ComposerChip[]): boolean {
  if (!effect) return false;
  const state = composerChipStore(owner);
  if (state.presses.interactionRevision !== effect.interaction) return false;
  if (effect.kind === 'finish') {
    state.presses.finish(effect.seq);
    return effect.send && state.presses.latest === effect.seq;
  }
  if (state.presses.latest !== effect.after) return false;
  const chip = chips.find(candidate => candidate.kind === 'citation' && candidate.start === effect.fresh.start && candidate.end === effect.fresh.end && candidate.source === effect.fresh.source);
  if (chip) state.presses.open(current, 'composer', chips, chip, effect.sourceFrame, { comment: true, fresh: effect.fresh, sourceFrame: effect.sourceFrame });
  return false;
}
