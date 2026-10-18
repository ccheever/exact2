import type { ComposerFieldSnapshot } from './composer-field-commands';
import { ComposerChipPresses } from './composer-chip-state';
import { ComposerCitationReceipts, type ComposerCitationCompletion } from './composer-citation-receipts';
import type { ComposerChip } from './composer-source-chips';

export type ComposerChipPreview = ComposerFieldSnapshot & { inputSerial: number };
type Entry = { presses: ComposerChipPresses; citations: ComposerCitationReceipts; preview: ComposerChipPreview | null };
const entries = new WeakMap<object, Entry>();
export function composerChipStore(owner: object): Entry {
  let state = entries.get(owner);
  if (!state) { state = { presses: new ComposerChipPresses(), citations: new ComposerCitationReceipts(), preview: null }; entries.set(owner, state); }
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
