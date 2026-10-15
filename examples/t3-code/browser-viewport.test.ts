// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: packages/shared/src/previewViewport.test.ts
// (3), apps/web/src/browser/browserViewportLayout.test.ts (12), browserViewportActions.test.ts (4), BrowserDeviceToolbar.test.ts
// (`commitViewportAndAspectRatio`, 2), apps/web/src/components/preview/previewViewportReadiness.test.ts (3) and
// previewViewportRollback.test.ts (2). Substitutions: vitest's `vi.fn` is a recorded array, `vi.waitFor` a microtask flush,
// and `vi.useFakeTimers`/`advanceTimersByTimeAsync` bun's `jest.useFakeTimers`/`advanceTimersByTime` after a flush. A data source
// has no timers, so the reference's built-in 15 s deadline is the caller's (`deadline15s` here, as the reference times it).
// Clone rows (marked) cover the zoom ladder (Manager.ts has no pure test for it), the toggle's responsive viewport
// (browserDefaults.ts) and the server's viewport bounds.
import { describe, expect, it, jest } from 'bun:test';
import type { PreviewViewportSetting } from './browser-state';
import {
  BROWSER_VIEWPORT_COMMIT_TIMEOUT_MS, BrowserViewportCommitTimeoutError, type BrowserViewportCommitDeadline, DEFAULT_BROWSER_DEFAULTS, browserViewportSettingKey, PREVIEW_VIEWPORT_PRESETS, PREVIEW_ZOOM_LEVELS, browserResponsiveViewportForToggle, commitBrowserViewportChange,
  commitViewportAndAspectRatio, findZoomStep, isPreviewViewportReady, isValidViewport, nextZoomLevel, normalizeZoomFactor, resizeBrowserViewportFromRail, resizeFreeformViewport,
  resolveBrowserDeviceViewportLayout, resolveBrowserViewportLayout, resolveFittedBrowserViewport, resolvePreviewViewport, resolveResponsiveBrowserViewportSize, runBrowserViewportMutation,
  shouldRollbackPreviewViewport, subscribeBrowserViewportChange, zoomLabel,
} from './browser-viewport';

const flush = async (rounds = 20) => { for (let i = 0; i < rounds; i++) await Promise.resolve(); };
/** runHandlerWithTimeout, the reference's deadline. */
const deadline15s: BrowserViewportCommitDeadline = (tabId, operation) => {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timeoutId = setTimeout(() => reject(new BrowserViewportCommitTimeoutError(tabId)), BROWSER_VIEWPORT_COMMIT_TIMEOUT_MS); });
  return Promise.race([operation, timeout]).finally(() => { if (timeoutId !== undefined) clearTimeout(timeoutId); });
};

describe('previewViewport', () => {
  it('resolves fill and exact freeform viewports', () => {
    expect(resolvePreviewViewport({ mode: 'fill' })).toEqual({ _tag: 'fill' });
    expect(resolvePreviewViewport({ mode: 'freeform', width: 1024, height: 768 })).toEqual({ _tag: 'freeform', width: 1024, height: 768 });
  });
  it('resolves device presets in either orientation', () => {
    expect(resolvePreviewViewport({ mode: 'preset', preset: 'iphone-12-pro' })).toEqual({ _tag: 'preset', width: 390, height: 844, presetId: 'iphone-12-pro' });
    expect(resolvePreviewViewport({ mode: 'preset', preset: 'iphone-12-pro', orientation: 'landscape' })).toEqual({ _tag: 'preset', width: 844, height: 390, presetId: 'iphone-12-pro' });
  });
  it("matches Chrome's standard device catalog ordering", () => {
    expect(PREVIEW_VIEWPORT_PRESETS.map(preset => preset.label)).toEqual(['iPhone SE', 'iPhone XR', 'iPhone 12 Pro', 'iPhone 14 Pro Max', 'Pixel 7', 'Samsung Galaxy S8+', 'Samsung Galaxy S20 Ultra',
      'iPad Mini', 'iPad Air', 'iPad Pro', 'Surface Pro 7', 'Surface Duo', 'Galaxy Z Fold 5', 'Asus Zenbook Fold', 'Samsung Galaxy A51/71', 'Nest Hub', 'Nest Hub Max']);
  });
});

describe('resolveBrowserViewportLayout', () => {
  it('uses the current fixed viewport instead of stale fitted source dimensions', () => {
    expect(resolveFittedBrowserViewport({ _tag: 'freeform', width: 900, height: 600 }, { width: 1280, height: 800, scale: 1 })).toEqual({ _tag: 'freeform', width: 900, height: 600 });
  });
  it('preserves the last logical viewport when fitting a fill-mode surface', () => {
    expect(resolveFittedBrowserViewport({ _tag: 'fill' }, { width: 320, height: 200, scale: 0.25 })).toEqual({ _tag: 'freeform', width: 1280, height: 800 });
  });
  it('fills the available surface in fill mode', () => {
    expect(resolveBrowserViewportLayout({ width: 700, height: 500 }, { _tag: 'fill' })).toEqual({ canvasWidth: 700, canvasHeight: 500, viewportX: 0, viewportY: 0, viewportWidth: 700, viewportHeight: 500, viewportScale: 1, fillsPanel: true });
  });
  it('centers a smaller fixed viewport', () => {
    expect(resolveBrowserViewportLayout({ width: 700, height: 1000 }, { _tag: 'freeform', width: 393, height: 852 })).toMatchObject({ canvasWidth: 700, canvasHeight: 1000, viewportX: 154, viewportY: 74, viewportWidth: 393, viewportHeight: 852 });
  });
  it('scales a larger fixed viewport down to fit without creating overflow', () => {
    const layout = resolveBrowserViewportLayout({ width: 600, height: 700 }, { _tag: 'freeform', width: 1440, height: 900 });
    expect(layout).toMatchObject({ canvasWidth: 600, canvasHeight: 700, viewportX: 0, viewportY: 163, viewportWidth: 600, viewportHeight: 375 });
    expect(layout.viewportScale).toBeCloseTo(5 / 12);
  });
  it('keeps fixed dimensions in page CSS pixels when browser zoom changes', () => {
    expect(resolveBrowserViewportLayout({ width: 800, height: 700 }, { _tag: 'freeform', width: 400, height: 300 }, 1.5)).toMatchObject({ viewportX: 100, viewportY: 125, viewportWidth: 600, viewportHeight: 450 });
    expect(resizeFreeformViewport({ width: 400, height: 300 }, { x: 150, y: 75 }, 1.5)).toEqual({ width: 500, height: 350 });
  });
  it('bounds freeform drag sizes and total render area', () => {
    expect(resizeFreeformViewport({ width: 1024, height: 768 }, { x: -2000, y: -2000 })).toEqual({ width: 240, height: 240 });
    const large = resizeFreeformViewport({ width: 1920, height: 1080 }, { x: 2000, y: 2000 });
    expect(large.width * large.height).toBeLessThanOrEqual(3840 * 2160);
  });
  it('resizes only the axes controlled by each edge', () => {
    expect(resizeFreeformViewport({ width: 800, height: 600 }, { x: -100, y: 500 }, 1, 'west')).toEqual({ width: 900, height: 600 });
    expect(resizeFreeformViewport({ width: 800, height: 600 }, { x: 500, y: 100 }, 1, 'north')).toEqual({ width: 800, height: 500 });
    expect(resizeFreeformViewport({ width: 800, height: 600 }, { x: -100, y: -50 }, 1, 'northwest')).toEqual({ width: 900, height: 650 });
  });
  it('preserves a locked aspect ratio from either axis', () => {
    expect(resizeFreeformViewport({ width: 800, height: 600 }, { x: 200, y: 0 }, 1, 'east', 4 / 3)).toEqual({ width: 1000, height: 750 });
    expect(resizeFreeformViewport({ width: 800, height: 600 }, { x: 0, y: 150 }, 1, 'south', 4 / 3)).toEqual({ width: 1000, height: 750 });
  });
  it('reserves persistent device-toolbar rails around the guest viewport', () => {
    expect(resolveBrowserDeviceViewportLayout({ width: 1200, height: 900 }, { _tag: 'freeform', width: 1180, height: 858 })).toEqual({
      canvasWidth: 1200, canvasHeight: 900, viewportX: 10, viewportY: 32, viewportWidth: 1180, viewportHeight: 858, viewportScale: 1, fillsPanel: false,
    });
  });
  it('captures the available framed area when responsive mode is enabled', () => {
    expect(resolveResponsiveBrowserViewportSize({ width: 1200, height: 900 })).toEqual({ width: 1180, height: 858 });
    expect(resolveResponsiveBrowserViewportSize({ width: 1200, height: 900 }, 2)).toEqual({ width: 590, height: 429 });
  });
  it('keeps the grabbed rail under the pointer across centered layout boundaries', () => {
    const available = { width: 1120, height: 818 };
    expect(resizeBrowserViewportFromRail({ width: 1120, height: 818 }, { x: -100, y: -50 }, available, 1, 'southeast')).toEqual({ width: 920, height: 718 });
    expect(resizeBrowserViewportFromRail({ width: 800, height: 600 }, { x: 300, y: 0 }, { width: 1200, height: 800 }, 1, 'east')).toEqual({ width: 1300, height: 600 });
    expect(resizeBrowserViewportFromRail({ width: 560, height: 409 }, { x: -100, y: 0 }, available, 2, 'east')).toEqual({ width: 460, height: 409 });
  });
});

describe('browserViewportActions', () => {
  it('routes drag commits to the visible tab handler and cleans up exactly that handler', async () => {
    const first: PreviewViewportSetting[] = [], second: PreviewViewportSetting[] = [];
    const unsubscribeFirst = subscribeBrowserViewportChange('tab-1', async setting => { first.push(setting); });
    const unsubscribeSecond = subscribeBrowserViewportChange('tab-1', async setting => { second.push(setting); });
    unsubscribeFirst();
    await commitBrowserViewportChange('tab-1', { _tag: 'freeform', width: 900, height: 700 });
    expect(first).toEqual([]);
    expect(second).toEqual([{ _tag: 'freeform', width: 900, height: 700 }]);
    unsubscribeSecond();
    await expect(commitBrowserViewportChange('tab-1', { _tag: 'freeform', width: 800, height: 600 })).rejects.toThrow('No visible browser viewport handler');
  });
  it('commits viewport changes in order for each tab', async () => {
    let releaseFirst: (() => void) | undefined, markFirstStarted: (() => void) | undefined;
    const firstPending = new Promise<void>(resolve => { releaseFirst = resolve; });
    const firstStarted = new Promise<void>(resolve => { markFirstStarted = resolve; });
    const calls: number[] = [];
    const unsubscribe = subscribeBrowserViewportChange('tab-serial', async setting => {
      if (setting._tag === 'fill') return;
      calls.push(setting.width);
      if (setting.width === 800) { markFirstStarted?.(); await firstPending; }
    });
    const first = commitBrowserViewportChange('tab-serial', { _tag: 'freeform', width: 800, height: 600 });
    const second = commitBrowserViewportChange('tab-serial', { _tag: 'freeform', width: 900, height: 700 });
    await firstStarted;
    expect(calls).toEqual([800]);
    releaseFirst?.();
    await Promise.all([first, second]);
    expect(calls).toEqual([800, 900]);
    unsubscribe();
  });
  it('serializes background mutations with visible viewport commits', async () => {
    let releaseBackground: (() => void) | undefined;
    const backgroundPending = new Promise<void>(resolve => { releaseBackground = resolve; });
    const calls: string[] = [];
    const background = runBrowserViewportMutation('tab-shared', async () => { calls.push('background'); await backgroundPending; });
    const unsubscribe = subscribeBrowserViewportChange('tab-shared', async () => { calls.push('visible'); });
    const visible = commitBrowserViewportChange('tab-shared', { _tag: 'freeform', width: 900, height: 700 });
    await flush();
    expect(calls).toEqual(['background']);
    releaseBackground?.();
    await Promise.all([background, visible]);
    expect(calls).toEqual(['background', 'visible']);
    unsubscribe();
  });
  it('does not let a timed-out handler overtake a newer viewport commit', async () => {
    jest.useFakeTimers();
    try {
      let releaseFirst: (() => void) | undefined;
      const delayed = new Promise<void>(resolve => { releaseFirst = resolve; });
      const calls: PreviewViewportSetting[] = [];
      const unsubscribe = subscribeBrowserViewportChange('tab-timeout', setting => { calls.push(setting); return calls.length === 1 ? delayed : Promise.resolve(); });
      const first = commitBrowserViewportChange('tab-timeout', { _tag: 'freeform', width: 800, height: 600 }, deadline15s);
      let firstError: unknown = null;
      first.catch(error => { firstError = error; });
      const second = commitBrowserViewportChange('tab-timeout', { _tag: 'freeform', width: 900, height: 700 }, deadline15s);
      // advanceTimersByTimeAsync: the timeout starts only once the commit reaches the front of the queue.
      for (let i = 0; i < 50 && !firstError; i++) { await flush(); jest.advanceTimersByTime(BROWSER_VIEWPORT_COMMIT_TIMEOUT_MS); }
      expect(String(firstError)).toContain('Timed out committing the browser viewport for tab tab-timeout');
      expect(calls).toHaveLength(1);
      releaseFirst?.();
      let secondDone = false;
      void second.then(() => { secondDone = true; });
      for (let i = 0; i < 50 && !secondDone; i++) { await flush(); jest.advanceTimersByTime(1); }
      expect(secondDone).toBe(true);
      expect(calls).toHaveLength(2);
      expect(calls[1]).toMatchObject({ width: 900, height: 700 });
      unsubscribe();
    } finally { jest.useRealTimers(); }
  });
});

describe('commitViewportAndAspectRatio', () => {
  it('commits the aspect ratio only after the viewport succeeds', async () => {
    let resolveChange: (() => void) | undefined;
    const ratios: Array<number | null> = [];
    const commit = commitViewportAndAspectRatio({ _tag: 'freeform', width: 900, height: 600 }, 1.5, () => new Promise<void>(resolve => { resolveChange = resolve; }), ratio => ratios.push(ratio));
    expect(ratios).toEqual([]);
    resolveChange?.();
    await commit;
    expect(ratios).toEqual([1.5]);
  });
  it('keeps the previous aspect ratio when the viewport commit fails', async () => {
    const ratios: Array<number | null> = [];
    await expect(commitViewportAndAspectRatio({ _tag: 'fill' }, null, async () => Promise.reject(new Error('resize failed')), ratio => ratios.push(ratio))).rejects.toThrow('resize failed');
    expect(ratios).toEqual([]);
  });
});

describe('isPreviewViewportReady', () => {
  const landscape: PreviewViewportSetting = { _tag: 'preset', width: 844, height: 390, presetId: 'iphone-12-pro' };
  it('rejects a stale same-mode preset while React applies the requested orientation', () => {
    expect(isPreviewViewportReady({ setting: landscape, appliedSettingKey: 'preset:390:844:iphone-12-pro', declaredViewport: { width: 390, height: 844 }, renderedViewport: { width: 390, height: 844 } })).toBe(false);
  });
  it('requires both the declaration and guest viewport to match a fixed request', () => {
    const appliedSettingKey = browserViewportSettingKey(landscape);
    expect(isPreviewViewportReady({ setting: landscape, appliedSettingKey, declaredViewport: { width: 390, height: 844 }, renderedViewport: { width: 844, height: 390 } })).toBe(false);
    expect(isPreviewViewportReady({ setting: landscape, appliedSettingKey, declaredViewport: { width: 844, height: 390 }, renderedViewport: { width: 844, height: 390 } })).toBe(true);
  });
  it('allows one pixel of Electron rounding tolerance in every mode', () => {
    expect(isPreviewViewportReady({ setting: { _tag: 'fill' }, appliedSettingKey: 'fill', declaredViewport: { width: 500, height: 700 }, renderedViewport: { width: 501, height: 699 } })).toBe(true);
    expect(isPreviewViewportReady({ setting: landscape, appliedSettingKey: browserViewportSettingKey(landscape), declaredViewport: { width: 844, height: 390 }, renderedViewport: { width: 845, height: 389 } })).toBe(true);
    expect(isPreviewViewportReady({ setting: landscape, appliedSettingKey: browserViewportSettingKey(landscape), declaredViewport: { width: 844, height: 390 }, renderedViewport: { width: 846, height: 390 } })).toBe(false);
  });
});

describe('shouldRollbackPreviewViewport', () => {
  const fill: PreviewViewportSetting = { _tag: 'fill' }, requested: PreviewViewportSetting = { _tag: 'freeform', width: 900, height: 600 };
  it('rolls back a timed-out request that still owns the latest setting', () => {
    expect(shouldRollbackPreviewViewport(fill, requested, requested, 'server-a', 'server-a')).toBe(true);
  });
  it('does not overwrite a newer resize, replacement server, or repeated setting', () => {
    expect(shouldRollbackPreviewViewport(fill, requested, { _tag: 'freeform', width: 1024, height: 768 }, 'server-a', 'server-a')).toBe(false);
    expect(shouldRollbackPreviewViewport(fill, requested, requested, 'server-a', 'server-b')).toBe(false);
    expect(shouldRollbackPreviewViewport(requested, requested, requested, 'server-a', 'server-a')).toBe(false);
  });
});

describe('the zoom ladder (clone)', () => {
  it('steps along Chrome\'s ladder and stops at its ends', () => {
    expect(PREVIEW_ZOOM_LEVELS[0]).toBe(0.25);
    expect(PREVIEW_ZOOM_LEVELS.at(-1)).toBe(5);
    expect(nextZoomLevel(1, 'in')).toBe(1.1);
    expect(nextZoomLevel(1, 'out')).toBe(0.9);
    expect(nextZoomLevel(5, 'in')).toBe(5);
    expect(nextZoomLevel(0.25, 'out')).toBe(0.25);
    expect(nextZoomLevel(1.2, 'in')).toBe(1.25); // between steps: the next step up
    expect(nextZoomLevel(1.2, 'out')).toBe(1); // findZoomStep takes the step below (1.1), as Manager.ts does
    expect(findZoomStep(9)).toBe(PREVIEW_ZOOM_LEVELS.length - 1);
  });
  it('snaps an off-ladder value and labels it', () => {
    expect(normalizeZoomFactor(undefined)).toBe(1);
    expect(normalizeZoomFactor(Number.NaN)).toBe(1);
    expect(normalizeZoomFactor(1.3)).toBe(1.25);
    expect(normalizeZoomFactor(0.1)).toBe(0.25);
    expect(normalizeZoomFactor(9)).toBe(5);
    expect([0.33, 0.67, 1, 1.25, 5].map(zoomLabel)).toEqual(['33%', '67%', '100%', '125%', '500%']);
  });
});

describe('the device toolbar toggle and bounds (clone)', () => {
  it('opens at a configured fixed default, else the panel\'s framed area, else 1024 × 768', () => {
    const phone: PreviewViewportSetting = { _tag: 'preset', width: 390, height: 844, presetId: 'iphone-12-pro' };
    expect(browserResponsiveViewportForToggle({ defaults: { ...DEFAULT_BROWSER_DEFAULTS, viewport: phone }, panelRect: { width: 600, height: 700 }, zoomFactor: 1 })).toEqual(phone);
    expect(browserResponsiveViewportForToggle({ defaults: DEFAULT_BROWSER_DEFAULTS, panelRect: { width: 600, height: 700 }, zoomFactor: 1 })).toEqual({ _tag: 'freeform', width: 580, height: 658 });
    expect(browserResponsiveViewportForToggle({ defaults: DEFAULT_BROWSER_DEFAULTS, panelRect: null, zoomFactor: 1 })).toEqual({ _tag: 'freeform', width: 1024, height: 768 });
  });
  it('accepts 240 to 3,840 a side and an area of at most 3,840 × 2,160', () => {
    expect(isValidViewport({ _tag: 'freeform', width: 240, height: 3840 })).toBe(true);
    expect(isValidViewport({ _tag: 'freeform', width: 239, height: 600 })).toBe(false);
    expect(isValidViewport({ _tag: 'freeform', width: 3841, height: 600 })).toBe(false);
    expect(isValidViewport({ _tag: 'freeform', width: 3840, height: 2160 })).toBe(true);
    expect(isValidViewport({ _tag: 'freeform', width: 3840, height: 2161 })).toBe(false);
    expect(isValidViewport({ _tag: 'freeform', width: 800.5, height: 600 })).toBe(false);
  });
});

// BrowserStage hands the module the requested CSS size (`page-size`) beside the
// fitted box, so the module sizes the page from the fields at its zoom, not from the box over its scale.
describe('BrowserStage page size (clone)', () => {
  it('passes the requested size, held or dragged, beside a fractional fit scale; fill passes none', async () => {
    const { contractComponent } = await import('./contract-action-fixture');
    const { contractFn } = await import('./contract-fn-fixture');
    const file = 'browser-stage.contract', math = { min: Math.min, max: Math.max, round: Math.round, floor: Math.floor, ceil: Math.ceil };
    const bvEndRail = await contractFn(file, 'bvEndRail', ['start', 'd', 'avail']), bvStartRail = await contractFn(file, 'bvStartRail', ['start', 'd', 'avail']);
    const fns = { ...math, bvRail: await contractFn(file, 'bvRail', ['size', 'z', 'd', 'avail', 'end', 'start'], { bvEndRail, bvStartRail }),
      bvClamp: await contractFn(file, 'bvClamp', ['v'], math), bvLockedWidth: await contractFn(file, 'bvLockedWidth', ['w', 'ratio'], math),
      bvLockedHeight: await contractFn(file, 'bvLockedHeight', ['h', 'ratio'], math) };
    const stage = contractComponent(file, 'BrowserStage');
    const view = { viewportMode: 'preset', zoomFactor: 1.25, viewportWidth: 390, viewportHeight: 844, aspectLocked: false, viewportKey: 'preset:390:844:iphone-12-pro', viewportSerial: 0 };
    const derived = (state: Record<string, unknown>, v: Record<string, unknown>, pw = 620, ph = 560) => stage.derives(state, { ...fns, view: v, pw, ph, urlFocused: false });
    const at125 = derived(stage.state(), view);
    expect(at125.pageSize).toBe('390x844');
    expect(Number.isInteger(at125.vw)).toBe(false); // the box: 390 × 1.25 × the fit scale
    expect(at125.scale).toBeLessThan(1);
    const dragging = { ...stage.state(), dragDir: 'east', dragX: 37, dragY: 0 };
    expect(derived(dragging, view).pageSize).toBe(`${derived(dragging, view).liveW}x844`);
    expect(derived(stage.state(), { ...view, viewportMode: 'fill' }).pageSize).toBe('');
    const page = (await Bun.file(new URL(`./${file}`, import.meta.url)).text()).split('\n').find(line => line.trim().startsWith('t3-browser '))!;
    expect(page).toContain('fit-scale=`${scale}` page-size=pageSize');
  });
});
