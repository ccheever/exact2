// Ports of T3 Code 1e2ecbd975 (MIT; see LICENSE-T3) tests:
// apps/web/src/components/composerFooterLayout.test.ts (resolveRestingComposerControlsLayout,
// its hysteresis, the context strip case) and chat/restingComposerControlsMeasurement.test.ts
// (the layouts a measured cluster resolves to; the DOM reads themselves are not ported).
import { describe, expect, it } from 'bun:test';
import { resolveRestingComposerControlsLayout, resolveRestingComposerControlsNaturalWidth } from './composer-resting-layout';

// Picker 140 natural / 96 minimum, plus a 9px separator. Traits 60, mode 140, overflow 24, gap 4.
const base = { gap: 4, naturalFixedWidth: 149, minimumFixedWidth: 105, blockWidths: [60, 140], overflowWidth: 24 };

describe('resolveRestingComposerControlsLayout', () => {
  it('shows everything when the host has room', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 357 })).toEqual({ hiddenCount: 0, visible: true });
  });
  it('moves trailing blocks into the overflow menu until the rest fits', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 356 })).toEqual({ hiddenCount: 1, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 241 })).toEqual({ hiddenCount: 1, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 240 })).toEqual({ hiddenCount: 2, visible: true });
  });
  it('shrinks the picker after moving every trailing block into overflow', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 176 })).toEqual({ hiddenCount: 2, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 133 })).toEqual({ hiddenCount: 2, visible: true });
  });
  it("hides the whole cluster below the picker's minimum readable width", () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 132 })).toEqual({ hiddenCount: 2, visible: false });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 0 })).toEqual({ hiddenCount: 2, visible: false });
  });
  it('uses the same thresholds while shrinking and growing', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 240 })).toEqual({ hiddenCount: 2, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 241 })).toEqual({ hiddenCount: 1, visible: true });
  });
  it('supports a single leading control without overflow blocks', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, naturalFixedWidth: 140, minimumFixedWidth: 140, blockWidths: [], overflowWidth: 0, hostWidth: 139 }))
      .toEqual({ hiddenCount: 0, visible: false });
  });
});

describe('context strip labels and resting composer controls', () => {
  const measurement = { gap: 4, naturalFixedWidth: 96.3828125 + 5 + 4, minimumFixedWidth: 52 + 5 + 4, blockWidths: [130.6953125, 119.671875], overflowWidth: 28 };
  it('reserves the natural width and settles on the same layout', () => {
    expect(resolveRestingComposerControlsNaturalWidth(measurement)).toBeCloseTo(363.75, 2);
    const layout = resolveRestingComposerControlsLayout({ ...measurement, hostWidth: 724 - 125 });
    expect(layout).toEqual({ hiddenCount: 0, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...measurement, hostWidth: 724 - 125, previous: layout })).toEqual(layout);
  });
  it('hides both blocks when the strip expands its labels', () => {
    expect(resolveRestingComposerControlsLayout({ ...measurement, hostWidth: 724 - 125 - 327 })).toEqual({ hiddenCount: 2, visible: true });
  });
});

describe('resolveRestingComposerControlsLayout hysteresis', () => {
  it('keeps a block in overflow when re-showing it would leave no slack', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 357, previous: { hiddenCount: 1, visible: true } })).toEqual({ hiddenCount: 1, visible: true });
  });
  it('re-shows a block once the host clears the slack margin', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 358, previous: { hiddenCount: 1, visible: true } })).toEqual({ hiddenCount: 0, visible: true });
  });
  it('restores the blocks that fit when the full cluster has no slack', () => {
    const partlyRestored = resolveRestingComposerControlsLayout({ ...base, hostWidth: 357, previous: { hiddenCount: 2, visible: true } });
    expect(partlyRestored).toEqual({ hiddenCount: 1, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 357, previous: partlyRestored })).toEqual(partlyRestored);
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 358, previous: partlyRestored })).toEqual({ hiddenCount: 0, visible: true });
  });
  it('requires slack before partially restoring a cluster', () => {
    const previous = { hiddenCount: 2, visible: true };
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 241, previous })).toEqual(previous);
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 242, previous })).toEqual({ hiddenCount: 1, visible: true });
  });
  it('settles when the measured picker width jitters below a pixel', () => {
    let layout = { hiddenCount: 0, visible: true };
    const settled: string[] = [];
    for (let index = 0; index < 10; index += 1) {
      layout = resolveRestingComposerControlsLayout({ ...base, naturalFixedWidth: index % 2 === 0 ? 149 : 149.5, hostWidth: 357, previous: layout });
      if (index >= 2) settled.push(`${layout.hiddenCount}:${layout.visible}`);
    }
    expect(new Set(settled).size).toBe(1);
  });
  it('keeps the cluster hidden until its minimum width clears the slack', () => {
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 133, previous: { hiddenCount: 2, visible: false } })).toEqual({ hiddenCount: 2, visible: false });
    expect(resolveRestingComposerControlsLayout({ ...base, hostWidth: 134, previous: { hiddenCount: 2, visible: false } })).toEqual({ hiddenCount: 2, visible: true });
  });
});

describe('measured clusters (restingComposerControlsMeasurement.test.ts)', () => {
  // Picker 52 (label collapsed) or 192/212 (label recovered), one 140 block, overflow 24, gap 4.
  const cluster = (natural: number) => ({ gap: 4, naturalFixedWidth: natural, minimumFixedWidth: 52, blockWidths: [140], iconOnlyBlockWidths: [140], overflowWidth: 24 });
  it('keeps controls inline when the model label is deliberately collapsed', () => {
    expect(resolveRestingComposerControlsNaturalWidth(cluster(52))).toBe(196);
    expect(resolveRestingComposerControlsLayout({ ...cluster(52), hostWidth: 200 })).toEqual({ hiddenCount: 0, iconOnlyCount: 0, visible: true });
  });
  it('moves the block into overflow when a recovered label needs the room', () => {
    expect(resolveRestingComposerControlsLayout({ ...cluster(192), hostWidth: 200 })).toEqual({ hiddenCount: 1, iconOnlyCount: 1, visible: true });
    expect(resolveRestingComposerControlsLayout({ ...cluster(212), hostWidth: 200 })).toEqual({ hiddenCount: 1, iconOnlyCount: 1, visible: true });
  });
});
