// The composer's effort as rising bars or a word. Adapted from T3 Code v0.1.5 (MIT; see LICENSE-T3):
// chat/EffortGlyph.tsx (resolveEffortGlyph, effortBarHeights), ported as is; the bars are drawn in
// composer-controls.contract EffortBars.
import { arr, str, type Obj } from '../client/domain';

// Ultracode changes the workflow, not only intensity, so it keeps its name like Ultrathink.
// Max is named too, so the full scale stops short of it.
const NAMED_LEVELS = new Set(['ultracode', 'max']);
const OFF_LEVELS = new Set(['off', 'none']);

export type EffortGlyph = { count: number; filled: number; special: string | null; label: string };

/**
 * Maps a reasoning select to bars: one per ordinary level, filled up to the current one, with an off level as none
 * filled. Max, prompt-injected and workflow levels are named instead of drawn.
 */
export function resolveEffortGlyph(descriptor: Obj, current: string | null): EffortGlyph | null {
  const injected = Array.isArray(descriptor.promptInjectedValues) ? (descriptor.promptInjectedValues as unknown[]).map(value => str(value)) : [];
  const special = new Set([...injected, ...NAMED_LEVELS]);
  const options = arr(descriptor.options), levels = options.filter(option => !special.has(str(option.id)));
  if (levels.length < 2 || current === null) return null;
  const startsAtOff = OFF_LEVELS.has(str(levels[0]!.id));
  const count = startsAtOff ? levels.length - 1 : levels.length;
  if (special.has(current)) {
    const label = str(options.find(option => option.id === current)?.label, current);
    return { count, filled: count, special: label, label };
  }
  const index = levels.findIndex(option => option.id === current);
  if (index < 0) return null;
  return { count, filled: startsAtOff ? index : index + 1, special: null, label: str(levels[index]!.label, str(levels[index]!.id)) };
}

/** Bar heights that rise by one whole-pixel step, from about half height up to `maxBar`. */
export function effortBarHeights(count: number, maxBar: number): number[] {
  // Rounding each bar separately mixes 1px and 2px steps, so round the step once.
  const step = Math.max(1, Math.round(maxBar / 2 / Math.max(count - 1, 1)));
  return Array.from({ length: count }, (_, index) => maxBar - (count - 1 - index) * step);
}

/** The composer's bars at 14pt: 2pt wide, 4pt apart, standing 2pt above the box's bottom (the text baseline). */
export function effortBars(glyph: EffortGlyph): Array<{ x: number; y: number; height: number; on: boolean }> {
  const maxBar = 14 - 2;
  return effortBarHeights(glyph.count, maxBar).map((height, index) => ({ x: index * 4, y: maxBar - height, height, on: index < glyph.filled }));
}
