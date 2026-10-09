// browser-surface part 2: a Browser tab's viewport and zoom (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// packages/shared/src/previewViewport.ts, packages/contracts/src/preview.ts `PREVIEW_VIEWPORT_*`,
// `PREVIEW_ZOOM_LEVELS`, apps/web/src/browser/{browserViewportLayout,browserViewportActions,
// browserDeviceToolbarState,browserDefaults}.ts, apps/web/src/components/preview/{previewViewportReadiness,
// previewViewportRollback}.ts, apps/desktop/src/preview/Manager.ts `findZoomStep`/`normalizeZoomFactor`/
// `nextZoomLevel`).
//
// A tab fills the panel, or shows a fixed CSS viewport (freeform, or one of the 17 device presets in Chrome
// DevTools' order) of 240 to 3,840 px a side and at most 3,840 × 2,160 in area, under the device toolbar
// (32 pt) with 10 pt resize rails. A fixed viewport larger than the panel is scaled down to fit (presentation
// only: the page keeps its CSS viewport). Zoom steps along Chrome's ladder, 25% to 500%.
//
// WebKit (X1 path B): the zoom is the web view's `pageZoom` (the page's CSS pixels grow, as Chromium's zoom
// does); a fixed viewport is a web view sized at `width × zoom` by `height × zoom` points inside a box whose
// bounds scale it into the fitted footprint (T3BrowserView.swift), so the page measures the requested viewport.
import type { PreviewViewportSetting } from './browser-state';

export const PREVIEW_VIEWPORT_MIN_DIMENSION = 240;
export const PREVIEW_VIEWPORT_MAX_DIMENSION = 3840;
export const PREVIEW_VIEWPORT_MAX_AREA = 3840 * 2160;
export type PreviewViewportSize = { readonly width: number; readonly height: number };
export const FILL_PREVIEW_VIEWPORT: PreviewViewportSetting = { _tag: 'fill' };
type FixedViewport = Exclude<PreviewViewportSetting, { _tag: 'fill' }>;

// ── Presets (previewViewport.ts) ─────────────────────────────────────────────────────────────────
export type PreviewViewportPreset = { id: string; label: string; category: 'Desktop' | 'Tablet' | 'Phone'; detail: string; width: number; height: number };
const preset = (id: string, label: string, category: PreviewViewportPreset['category'], width: number, height: number): PreviewViewportPreset => ({ id, label, category, detail: `${width} × ${height}`, width, height });
/** Chrome DevTools' default-device order; CSS viewport sizes from Chromium's EmulatedDevices.ts catalog. */
export const PREVIEW_VIEWPORT_PRESETS: ReadonlyArray<PreviewViewportPreset> = [
  preset('iphone-se', 'iPhone SE', 'Phone', 375, 667), preset('iphone-xr', 'iPhone XR', 'Phone', 414, 896), preset('iphone-12-pro', 'iPhone 12 Pro', 'Phone', 390, 844),
  preset('iphone-14-pro-max', 'iPhone 14 Pro Max', 'Phone', 430, 932), preset('pixel-7', 'Pixel 7', 'Phone', 412, 915), preset('samsung-galaxy-s8-plus', 'Samsung Galaxy S8+', 'Phone', 360, 740),
  preset('samsung-galaxy-s20-ultra', 'Samsung Galaxy S20 Ultra', 'Phone', 412, 915), preset('ipad-mini', 'iPad Mini', 'Tablet', 768, 1024), preset('ipad-air', 'iPad Air', 'Tablet', 820, 1180),
  preset('ipad-pro', 'iPad Pro', 'Tablet', 1024, 1366), preset('surface-pro-7', 'Surface Pro 7', 'Tablet', 912, 1368), preset('surface-duo', 'Surface Duo', 'Phone', 540, 720),
  preset('galaxy-z-fold-5', 'Galaxy Z Fold 5', 'Phone', 344, 882), preset('asus-zenbook-fold', 'Asus Zenbook Fold', 'Tablet', 853, 1280),
  preset('samsung-galaxy-a51-71', 'Samsung Galaxy A51/71', 'Phone', 412, 914), preset('nest-hub', 'Nest Hub', 'Tablet', 1024, 600), preset('nest-hub-max', 'Nest Hub Max', 'Tablet', 1280, 800),
];
export type PreviewViewportResizeInput = { mode: 'fill' | 'preset' | 'freeform'; preset?: string; orientation?: 'portrait' | 'landscape'; width?: number; height?: number };

export function resolvePreviewViewport(input: PreviewViewportResizeInput): PreviewViewportSetting {
  if (input.mode === 'fill') return { _tag: 'fill' };
  if (input.mode === 'preset' && input.preset !== undefined) {
    const found = PREVIEW_VIEWPORT_PRESETS.find(candidate => candidate.id === input.preset);
    if (!found) throw new Error(`Unknown preview viewport preset: ${input.preset}`);
    const nativePortrait = found.height >= found.width;
    const shouldSwap = (input.orientation === 'landscape' && nativePortrait) || (input.orientation === 'portrait' && !nativePortrait);
    return { _tag: 'preset', width: shouldSwap ? found.height : found.width, height: shouldSwap ? found.width : found.height, presetId: found.id };
  }
  if (input.width === undefined || input.height === undefined) throw new Error('Custom preview viewport requires width and height');
  return { _tag: 'freeform', width: input.width, height: input.height };
}

/** The contract's viewportAreaFilter and dimension bounds: whether the server takes this setting. */
export const isValidViewport = (setting: PreviewViewportSetting): boolean => setting._tag === 'fill' || (Number.isInteger(setting.width) && Number.isInteger(setting.height)
  && setting.width >= PREVIEW_VIEWPORT_MIN_DIMENSION && setting.width <= PREVIEW_VIEWPORT_MAX_DIMENSION && setting.height >= PREVIEW_VIEWPORT_MIN_DIMENSION
  && setting.height <= PREVIEW_VIEWPORT_MAX_DIMENSION && setting.width * setting.height <= PREVIEW_VIEWPORT_MAX_AREA);

// ── Layout (browserViewportLayout.ts) ────────────────────────────────────────────────────────────
export type BrowserViewportLayout = { canvasWidth: number; canvasHeight: number; viewportX: number; viewportY: number; viewportWidth: number; viewportHeight: number; viewportScale: number; fillsPanel: boolean };
export const BROWSER_DEVICE_TOOLBAR_HEIGHT = 32;
export const BROWSER_VIEWPORT_RESIZE_RAIL_SIZE = 10;
export type BrowserViewportResizeDirection = 'north' | 'northeast' | 'east' | 'southeast' | 'south' | 'southwest' | 'west' | 'northwest';

export const browserViewportSettingKey = (setting: PreviewViewportSetting): string =>
  setting._tag === 'fill' ? 'fill' : `${setting._tag}:${setting.width}:${setting.height}:${setting._tag === 'preset' ? setting.presetId : ''}`;
const normalizeLayoutZoom = (zoomFactor: number): number => Number.isFinite(zoomFactor) && zoomFactor > 0 ? zoomFactor : 1;

export function resolveFittedBrowserViewport(setting: PreviewViewportSetting, sourceContent: { width: number; height: number; scale: number } | null, zoomFactor = 1): FixedViewport {
  if (setting._tag !== 'fill') return setting;
  const zoom = normalizeLayoutZoom(zoomFactor);
  if (sourceContent) return { _tag: 'freeform', width: Math.max(1, Math.round(sourceContent.width / sourceContent.scale / zoom)), height: Math.max(1, Math.round(sourceContent.height / sourceContent.scale / zoom)) };
  return { _tag: 'freeform', width: 1280, height: 800 };
}

export function resolveBrowserDeviceViewportArea(container: PreviewViewportSize): PreviewViewportSize {
  return { width: Math.max(1, container.width - BROWSER_VIEWPORT_RESIZE_RAIL_SIZE * 2), height: Math.max(1, container.height - BROWSER_DEVICE_TOOLBAR_HEIGHT - BROWSER_VIEWPORT_RESIZE_RAIL_SIZE) };
}

export function resolveBrowserViewportLayout(container: PreviewViewportSize, setting: PreviewViewportSetting, zoomFactor = 1): BrowserViewportLayout {
  const containerWidth = Math.max(1, Math.round(container.width)), containerHeight = Math.max(1, Math.round(container.height));
  if (setting._tag === 'fill') return { canvasWidth: containerWidth, canvasHeight: containerHeight, viewportX: 0, viewportY: 0, viewportWidth: containerWidth, viewportHeight: containerHeight, viewportScale: 1, fillsPanel: true };
  const zoom = normalizeLayoutZoom(zoomFactor), renderedWidth = setting.width * zoom, renderedHeight = setting.height * zoom;
  const viewportScale = Math.min(1, containerWidth / renderedWidth, containerHeight / renderedHeight);
  const viewportWidth = renderedWidth * viewportScale, viewportHeight = renderedHeight * viewportScale;
  return { canvasWidth: containerWidth, canvasHeight: containerHeight, viewportX: Math.max(0, Math.round((containerWidth - viewportWidth) / 2)),
    viewportY: Math.max(0, Math.round((containerHeight - viewportHeight) / 2)), viewportWidth, viewportHeight, viewportScale, fillsPanel: false };
}

export function resolveBrowserDeviceViewportLayout(container: PreviewViewportSize, setting: FixedViewport, zoomFactor = 1): BrowserViewportLayout {
  const layout = resolveBrowserViewportLayout(resolveBrowserDeviceViewportArea(container), setting, zoomFactor);
  return { ...layout, canvasWidth: Math.max(1, Math.round(container.width)), canvasHeight: Math.max(1, Math.round(container.height)),
    viewportX: layout.viewportX + BROWSER_VIEWPORT_RESIZE_RAIL_SIZE, viewportY: layout.viewportY + BROWSER_DEVICE_TOOLBAR_HEIGHT };
}

const clampViewportDimension = (value: number): number => Math.min(PREVIEW_VIEWPORT_MAX_DIMENSION, Math.max(PREVIEW_VIEWPORT_MIN_DIMENSION, value));
const validAspectRatio = (aspectRatio: number | undefined): aspectRatio is number => aspectRatio !== undefined && Number.isFinite(aspectRatio) && aspectRatio > 0;

function resizeAtAspectRatio(desired: number, aspectRatio: number, primaryAxis: 'width' | 'height'): PreviewViewportSize {
  if (primaryAxis === 'width') {
    const minimum = Math.ceil(Math.max(PREVIEW_VIEWPORT_MIN_DIMENSION, PREVIEW_VIEWPORT_MIN_DIMENSION * aspectRatio));
    const maximum = Math.floor(Math.min(PREVIEW_VIEWPORT_MAX_DIMENSION, PREVIEW_VIEWPORT_MAX_DIMENSION * aspectRatio, Math.sqrt(PREVIEW_VIEWPORT_MAX_AREA * aspectRatio)));
    let width = Math.min(maximum, Math.max(minimum, Math.round(desired)));
    let height = Math.round(width / aspectRatio);
    while (width * height > PREVIEW_VIEWPORT_MAX_AREA && width > minimum) { width -= 1; height = Math.round(width / aspectRatio); }
    return { width, height };
  }
  const minimum = Math.ceil(Math.max(PREVIEW_VIEWPORT_MIN_DIMENSION, PREVIEW_VIEWPORT_MIN_DIMENSION / aspectRatio));
  const maximum = Math.floor(Math.min(PREVIEW_VIEWPORT_MAX_DIMENSION, PREVIEW_VIEWPORT_MAX_DIMENSION / aspectRatio, Math.sqrt(PREVIEW_VIEWPORT_MAX_AREA / aspectRatio)));
  let height = Math.min(maximum, Math.max(minimum, Math.round(desired)));
  let width = Math.round(height * aspectRatio);
  while (width * height > PREVIEW_VIEWPORT_MAX_AREA && height > minimum) { height -= 1; width = Math.round(height * aspectRatio); }
  return { width, height };
}

export function resizeFreeformViewport(start: PreviewViewportSize, delta: { x: number; y: number }, zoomFactor = 1, direction: BrowserViewportResizeDirection = 'southeast', aspectRatio?: number): PreviewViewportSize {
  const zoom = normalizeLayoutZoom(zoomFactor);
  const horizontalDelta = direction.includes('east') ? delta.x : direction.includes('west') ? -delta.x : 0;
  const verticalDelta = direction.includes('south') ? delta.y : direction.includes('north') ? -delta.y : 0;
  const desiredWidth = start.width + horizontalDelta / zoom, desiredHeight = start.height + verticalDelta / zoom;
  if (validAspectRatio(aspectRatio)) {
    const controlsWidth = horizontalDelta !== 0 || direction === 'east' || direction === 'west';
    const controlsHeight = verticalDelta !== 0 || direction === 'north' || direction === 'south';
    const primaryAxis = controlsWidth && !controlsHeight ? 'width' : controlsHeight && !controlsWidth ? 'height'
      : Math.abs(desiredWidth - start.width) / start.width >= Math.abs(desiredHeight - start.height) / start.height ? 'width' : 'height';
    return resizeAtAspectRatio(primaryAxis === 'width' ? desiredWidth : desiredHeight, aspectRatio, primaryAxis);
  }
  let width = clampViewportDimension(Math.round(desiredWidth)), height = clampViewportDimension(Math.round(desiredHeight));
  if (width * height <= PREVIEW_VIEWPORT_MAX_AREA) return { width, height };
  if (Math.abs(horizontalDelta) >= Math.abs(verticalDelta)) width = Math.max(PREVIEW_VIEWPORT_MIN_DIMENSION, Math.floor(PREVIEW_VIEWPORT_MAX_AREA / height));
  else height = Math.max(PREVIEW_VIEWPORT_MIN_DIMENSION, Math.floor(PREVIEW_VIEWPORT_MAX_AREA / width));
  return { width, height };
}

const resizeFromEndRail = (start: number, pointerDelta: number, available: number): number => {
  const startEdge = start < available ? (available + start) / 2 : start;
  const targetEdge = startEdge + pointerDelta;
  return targetEdge <= available ? targetEdge * 2 - available : targetEdge;
};
const resizeFromStartRail = (start: number, pointerDelta: number, available: number): number => {
  if (start > available) {
    const distanceToFit = start - available;
    return pointerDelta <= distanceToFit ? start - pointerDelta : available - (pointerDelta - distanceToFit) * 2;
  }
  const targetEdge = (available - start) / 2 + pointerDelta;
  return targetEdge >= 0 ? available - targetEdge * 2 : available - targetEdge;
};

/** A drag on a rail: the grabbed edge stays under the pointer while the viewport stays centered. */
export function resizeBrowserViewportFromRail(start: PreviewViewportSize, pointerDelta: { x: number; y: number }, available: PreviewViewportSize, zoomFactor = 1,
  direction: BrowserViewportResizeDirection = 'southeast', aspectRatio?: number): PreviewViewportSize {
  const zoom = normalizeLayoutZoom(zoomFactor), startWidth = start.width * zoom, startHeight = start.height * zoom;
  const desiredWidth = direction.includes('east') ? resizeFromEndRail(startWidth, pointerDelta.x, available.width) : direction.includes('west') ? resizeFromStartRail(startWidth, pointerDelta.x, available.width) : startWidth;
  const desiredHeight = direction.includes('south') ? resizeFromEndRail(startHeight, pointerDelta.y, available.height) : direction.includes('north') ? resizeFromStartRail(startHeight, pointerDelta.y, available.height) : startHeight;
  const widthDelta = desiredWidth - startWidth, heightDelta = desiredHeight - startHeight;
  return resizeFreeformViewport(start, { x: direction.includes('west') ? -widthDelta : widthDelta, y: direction.includes('north') ? -heightDelta : heightDelta }, zoom, direction, aspectRatio);
}

export function resolveResponsiveBrowserViewportSize(container: PreviewViewportSize, zoomFactor = 1): PreviewViewportSize {
  const area = resolveBrowserDeviceViewportArea(container), zoom = normalizeLayoutZoom(zoomFactor);
  return resizeFreeformViewport({ width: area.width / zoom, height: area.height / zoom }, { x: 0, y: 0 });
}

// ── Commits (browserViewportActions.ts, browserDeviceToolbarState.ts) ────────────────────────────
type BrowserViewportHandler = (setting: PreviewViewportSetting) => Promise<void>;
export const BROWSER_VIEWPORT_COMMIT_TIMEOUT_MS = 15_000;
export class BrowserViewportCommitTimeoutError extends Error {
  override readonly name = 'BrowserViewportCommitTimeoutError';
  constructor(readonly tabId: string) { super(`Timed out committing the browser viewport for tab ${tabId}`); }
}
const handlers = new Map<string, BrowserViewportHandler>();
const commitTails = new Map<string, Promise<void>>();
function queueBrowserViewportMutation<A>(tabId: string, start: () => Promise<A>): { started: Promise<{ operation: Promise<A> }>; execution: Promise<A> } {
  const previous = commitTails.get(tabId) ?? Promise.resolve();
  const started = previous.catch(() => undefined).then(() => ({ operation: Promise.resolve().then(start) }));
  const execution = started.then(({ operation }) => operation);
  const tail = execution.then(() => undefined);
  commitTails.set(tabId, tail);
  const clear = () => { if (commitTails.get(tabId) === tail) commitTails.delete(tabId); };
  void tail.then(clear, clear);
  return { started, execution };
}
/** Serializes every server-side viewport change of one runtime tab, so a rollback cannot overtake a newer resize. */
export function runBrowserViewportMutation<A>(tabId: string, mutation: () => Promise<A>): Promise<A> { return queueBrowserViewportMutation(tabId, mutation).execution; }
/** The caller-facing deadline of a commit (the reference's `runHandlerWithTimeout`, 15 s). A data source has no timers
 *  (js/bake: "there are no timers"), so a caller with a clock passes it; without one a commit waits for its handler. */
export type BrowserViewportCommitDeadline = (tabId: string, operation: Promise<void>) => Promise<void>;
const noDeadline: BrowserViewportCommitDeadline = (_tabId, operation) => operation;
export function subscribeBrowserViewportChange(tabId: string, handler: BrowserViewportHandler): () => void {
  handlers.set(tabId, handler);
  return () => { if (handlers.get(tabId) === handler) handlers.delete(tabId); };
}
export function commitBrowserViewportChange(tabId: string, setting: PreviewViewportSetting, deadline: BrowserViewportCommitDeadline = noDeadline): Promise<void> {
  const { started } = queueBrowserViewportMutation(tabId, () => {
    const handler = handlers.get(tabId);
    return handler ? handler(setting) : Promise.reject(new Error(`No visible browser viewport handler for tab ${tabId}`));
  });
  // The queue follows the real handler's lifetime; the deadline starts once this commit reaches the front.
  return started.then(({ operation }) => deadline(tabId, operation));
}
/** The device toolbar's commit: the aspect lock changes only once the viewport did. */
export async function commitViewportAndAspectRatio(setting: PreviewViewportSetting, aspectRatio: number | null, onChange: (setting: PreviewViewportSetting) => Promise<void>,
  onAspectRatioChange: (aspectRatio: number | null) => void): Promise<void> {
  await onChange(setting);
  onAspectRatioChange(aspectRatio);
}

// ── Readiness and rollback (previewViewportReadiness.ts, previewViewportRollback.ts; part 5's automation reads them) ──
export function isPreviewViewportReady(input: { setting: PreviewViewportSetting; appliedSettingKey: string | null; declaredViewport: PreviewViewportSize | null; renderedViewport: PreviewViewportSize | null }): boolean {
  const { setting, appliedSettingKey, declaredViewport, renderedViewport } = input;
  if (appliedSettingKey !== browserViewportSettingKey(setting) || declaredViewport === null || renderedViewport === null) return false;
  const expected = setting._tag === 'fill' ? declaredViewport : { width: setting.width, height: setting.height };
  if (setting._tag !== 'fill' && (declaredViewport.width !== expected.width || declaredViewport.height !== expected.height)) return false;
  // CSS pixels round through a fractional zoom or device scale, so an applied fixed viewport can measure one pixel either way.
  return Math.abs(renderedViewport.width - expected.width) <= 1 && Math.abs(renderedViewport.height - expected.height) <= 1;
}
export function shouldRollbackPreviewViewport(previous: PreviewViewportSetting, requested: PreviewViewportSetting, latest: PreviewViewportSetting, operationServerEpoch: string | null, currentServerEpoch: string | null): boolean {
  const requestedKey = browserViewportSettingKey(requested);
  return currentServerEpoch === operationServerEpoch && browserViewportSettingKey(latest) === requestedKey && browserViewportSettingKey(previous) !== requestedKey;
}

// ── Zoom (contracts PREVIEW_ZOOM_LEVELS; Manager.ts) ─────────────────────────────────────────────
export const PREVIEW_ZOOM_LEVELS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0] as const;
export const DEFAULT_PREVIEW_ZOOM_FACTOR = 1.0;
const ZOOM_EPSILON = 0.001;
export function findZoomStep(current: number): number {
  const index = PREVIEW_ZOOM_LEVELS.findIndex(level => Math.abs(level - current) < ZOOM_EPSILON || level > current);
  if (index < 0) return PREVIEW_ZOOM_LEVELS.length - 1;
  return Math.abs(PREVIEW_ZOOM_LEVELS[index]! - current) < ZOOM_EPSILON ? index : index - 1;
}
/** A value off the ladder snaps to its nearest step; a missing one is 100%. */
export function normalizeZoomFactor(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) return DEFAULT_PREVIEW_ZOOM_FACTOR;
  let closest: number = PREVIEW_ZOOM_LEVELS[0];
  for (const level of PREVIEW_ZOOM_LEVELS) if (Math.abs(level - value) < Math.abs(closest - value)) closest = level;
  return closest;
}
export function nextZoomLevel(current: number, direction: 'in' | 'out'): number {
  const step = findZoomStep(current);
  return direction === 'in' ? PREVIEW_ZOOM_LEVELS[Math.min(step + 1, PREVIEW_ZOOM_LEVELS.length - 1)] ?? current : PREVIEW_ZOOM_LEVELS[Math.max(step - 1, 0)] ?? current;
}
export const zoomLabel = (zoomFactor: number): string => `${Math.round(zoomFactor * 100)}%`;

// ── Defaults (browserDefaults.ts) ────────────────────────────────────────────────────────────────
export type PreviewAppearancePreference = 'system' | 'light' | 'dark';
export type BrowserDefaults = { viewport: PreviewViewportSetting; zoomFactor: number; appearance: PreviewAppearancePreference };
export const DEFAULT_BROWSER_DEFAULTS: BrowserDefaults = { viewport: FILL_PREVIEW_VIEWPORT, zoomFactor: DEFAULT_PREVIEW_ZOOM_FACTOR, appearance: 'system' };
// The Settings rows that make these defaults configurable (browserDefaultTabState, browserDefaultOpenViewport) are not
// built yet: new tabs open at the reference's defaults (fill, 100%, System); see the browser-surface-navigation record.
export const FALLBACK_RESPONSIVE_VIEWPORT_SIZE = { width: 1024, height: 768 } as const;
/** Show device toolbar on a fill tab: a configured fixed default wins; else the panel's framed area, else 1024 × 768. */
export function browserResponsiveViewportForToggle(input: { defaults: BrowserDefaults; panelRect: PreviewViewportSize | null; zoomFactor: number | undefined }): PreviewViewportSetting {
  if (input.defaults.viewport._tag !== 'fill') return input.defaults.viewport;
  const size = input.panelRect ? resolveResponsiveBrowserViewportSize(input.panelRect, input.zoomFactor) : FALLBACK_RESPONSIVE_VIEWPORT_SIZE;
  return { _tag: 'freeform', ...size };
}
