// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/previewMiniPlayerStore.test.ts,
// its 6 tests under their own names. This client floats devices only, so the tab sources the
// reference switches between are two devices here (an iPhone and an iPad on one host), and the
// last test's browser tab is the iPhone.
import { beforeEach, describe, expect, it } from 'bun:test';
import { PreviewMiniPlayerStore, previewMiniPlayerSourceKey, type PreviewMiniPlayerSource } from './previewMiniPlayerStore';

const refA = 'env-1:thread-A';
const refB = 'env-1:thread-B';
const tabA: PreviewMiniPlayerSource = { kind: 'device', hostId: 'local', deviceId: 'tab-a', platform: 'ios', name: 'iPhone' };
const tabB: PreviewMiniPlayerSource = { kind: 'device', hostId: 'local', deviceId: 'tab-b', platform: 'ios', name: 'iPad' };
const pixel: PreviewMiniPlayerSource = { kind: 'device', hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Pixel' };
let store = new PreviewMiniPlayerStore();

beforeEach(() => { store = new PreviewMiniPlayerStore(); });

describe('previewMiniPlayerStore', () => {
  it('keeps floating previews scoped to their thread', () => {
    store.open(refA, tabA);
    store.open(refB, tabB);
    expect(store.get(refA)).toMatchObject({ source: tabA });
    expect(store.get(refB)).toMatchObject({ source: tabB });
  });

  it('preserves position when switching the floating tab within one thread', () => {
    store.open(refA, tabA);
    store.move(refA, 'device:local:tab-a', { x: 24, y: 48 });
    store.open(refA, tabB);
    expect(store.get(refA)).toEqual({ source: tabB, position: { x: 24, y: 48 }, width: null, lastInteraction: 'drag' });
  });

  it('ignores stale drag updates after the floating tab changes', () => {
    store.open(refA, tabA);
    store.open(refA, tabB);
    store.move(refA, 'device:local:tab-a', { x: 100, y: 100 });
    expect(store.get(refA)).toEqual({ source: tabB, position: null, width: null, lastInteraction: 'drag' });
  });

  it('preserves a thread-bound width while switching tabs', () => {
    store.open(refA, tabA);
    store.resize(refA, 'device:local:tab-a', 480);
    store.open(refA, tabB);
    expect(store.get(refA)).toMatchObject({ source: tabB, width: 480 });
  });

  it('keeps a resize placement after release and clears it when dragging', () => {
    store.open(refA, tabA);
    store.resize(refA, 'device:local:tab-a', 460, { x: 1112, y: 275 });
    expect(store.get(refA)).toMatchObject({ width: 460, position: { x: 1112, y: 275 }, lastInteraction: 'resize' });
    store.move(refA, 'device:local:tab-a', { x: 1112, y: 275 });
    expect(store.get(refA)).toMatchObject({ width: 460, position: { x: 1112, y: 275 }, lastInteraction: 'drag' });
  });

  it('floats one source per thread, so a device replaces the browser tab', () => {
    store.open(refA, tabA);
    store.open(refA, pixel);
    const floating = store.get(refA);
    expect(floating).toMatchObject({ source: pixel });
    expect(previewMiniPlayerSourceKey(floating!.source)).toBe('device:nucbox:emulator-5580');
    // The same device under a new label is still the same floating source.
    store.open(refA, { ...pixel, name: 'Renamed' });
    expect(store.get(refA)).toBe(floating);
  });
});
