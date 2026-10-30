// The simpler composer's rules (T3 Code v0.1.5 #25869): composerControlsOwnership.test.ts and EffortGlyph.test.ts.
import { describe, expect, test } from 'bun:test';
import { resolveComposerControlsOwnership } from '../../src/composer/composer-controls-ownership';
import { effortBarHeights, resolveEffortGlyph } from '../../src/composer/effort-glyph';

const base = { matchesDefault: true, hasTraits: true, planModeUiEnabled: false, collapsed: false, measuredHiddenBlockCount: 0 };

describe('resolveComposerControlsOwnership', () => {
  test('hides the chip at the default and lets the effort menu answer composer.mode', () => {
    expect(resolveComposerControlsOwnership(base)).toMatchObject({ accessDisplay: 'hidden', showModeBlock: false, hiddenBlockIds: [], effortOwnsModeShortcut: true });
  });
  test('shows a labelled chip that owns composer.mode when the mode differs', () => {
    expect(resolveComposerControlsOwnership({ ...base, matchesDefault: false })).toMatchObject({ accessDisplay: 'label', showModeBlock: true, effortOwnsModeShortcut: false });
  });
  test('never leaves an empty overflow menu: the effort chip overflows with the chip', () => {
    const result = resolveComposerControlsOwnership({ ...base, matchesDefault: false, measuredHiddenBlockCount: 1 });
    expect(result.hiddenBlockIds).toEqual(['traits', 'mode']);
    expect(result.overflowShowsPermissions).toBe(true);
    // One composer.mode target: the overflow menu, not the overflowed effort chip.
    expect(result.effortOwnsModeShortcut).toBe(false);
  });
  test('keeps the Plan toggle block at the default and overflows it before the effort chip', () => {
    const result = resolveComposerControlsOwnership({ ...base, planModeUiEnabled: true, measuredHiddenBlockCount: 1 });
    expect(result).toMatchObject({ showModeBlock: true, hiddenBlockIds: ['mode'], overflowShowsPermissions: false, effortOwnsModeShortcut: true });
  });
  test('overflows only the chip when the Plan toggle gives the overflow menu content', () => {
    const result = resolveComposerControlsOwnership({ ...base, matchesDefault: false, planModeUiEnabled: true, measuredHiddenBlockCount: 1 });
    expect(result).toMatchObject({ hiddenBlockIds: ['mode'], overflowShowsPermissions: false, effortOwnsModeShortcut: true });
  });
  test('keeps a permissions icon for models without traits, even at the default', () => {
    expect(resolveComposerControlsOwnership({ ...base, hasTraits: false }).accessDisplay).toBe('icon');
    const overflowed = resolveComposerControlsOwnership({ ...base, hasTraits: false, measuredHiddenBlockCount: 1 });
    expect(overflowed).toMatchObject({ hiddenBlockIds: ['mode'], overflowShowsPermissions: true });
  });
  test('shows the non-default chip as an icon in the compacted row', () => {
    expect(resolveComposerControlsOwnership({ ...base, matchesDefault: false, collapsed: true }).accessDisplay).toBe('icon');
  });
});

const descriptor = (ids: string[], promptInjectedValues?: string[]) => ({ id: 'effort', label: 'Reasoning', type: 'select',
  options: ids.map(id => ({ id, label: id[0]!.toUpperCase() + id.slice(1) })), ...(promptInjectedValues ? { promptInjectedValues } : {}) });

describe('resolveEffortGlyph', () => {
  const claude = descriptor(['low', 'medium', 'high', 'xhigh', 'max', 'ultracode', 'ultrathink'], ['ultrathink']);
  test('fills bars up to the current level and leaves max and workflow levels off the scale', () => {
    expect(resolveEffortGlyph(claude, 'xhigh')).toEqual({ count: 4, filled: 4, special: null, label: 'Xhigh' });
  });
  test('names max, Ultracode and Ultrathink instead of drawing them', () => {
    expect(resolveEffortGlyph(claude, 'max')?.special).toBe('Max');
    expect(resolveEffortGlyph(claude, 'ultracode')?.special).toBe('Ultracode');
    expect(resolveEffortGlyph(claude, 'ultrathink')?.special).toBe('Ultrathink');
  });
  test('draws the lowest level as one bar, and an off level as none', () => {
    expect(resolveEffortGlyph(claude, 'low')).toMatchObject({ count: 4, filled: 1 });
    expect(resolveEffortGlyph(claude, 'medium')).toMatchObject({ count: 4, filled: 2 });
    expect(resolveEffortGlyph(descriptor(['off', 'low', 'high']), 'off')).toEqual({ count: 2, filled: 0, special: null, label: 'Off' });
  });
  test('falls back to the word when the value is unknown or the scale has one level', () => {
    expect(resolveEffortGlyph(claude, 'unknown')).toBeNull();
    expect(resolveEffortGlyph(descriptor(['high']), 'high')).toBeNull();
  });
});

describe('effortBarHeights', () => {
  test.each([3, 4, 5, 6, 7])('rises by one whole-pixel step and ends at full height: %i bars', count => {
    const heights = effortBarHeights(count, 12);
    expect(heights).toHaveLength(count);
    expect(heights.at(-1)).toBe(12);
    const steps = new Set(heights.slice(1).map((height, index) => height - heights[index]!));
    expect(steps.size).toBe(1);
    expect([...steps][0]! > 0).toBe(true);
  });
});
