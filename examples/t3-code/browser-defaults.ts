// browser-surface part 4 (profiles, and the Browser defaults group the coordinator added on 2026-10-09): the client
// settings a new Browser tab opens with and Settings › Integrations › Browser's rows for them (MIT reference, see
// LICENSE-T3, T3 Code 1e2ecbd975: packages/contracts/src/settings.ts `browserDefault*`, `browserRecording*`,
// `browserAutoShowFloatingPreview`; apps/web/src/browser/browserDefaults.ts; apps/web/src/components/settings/
// IntegrationsSettings.tsx BrowserViewportSetting, BrowserZoomSetting, BrowserAppearanceSetting,
// BrowserRecordingFrameRateSetting, BrowserRecordingInputSettings, BrowserAutoShowFloatingPreviewSetting).
//
// They are client-local, as the reference's are (its Chromium guest is desktop-local). The viewport, zoom and appearance
// are kept beside the profiles in the preference file (t3-code.json); a new tab is born at them (`preview.open`, and
// part 2's `browserSet`). The recording rows and Auto-show write the client settings part 3 reads
// (settings-core.ts `CLIENT_DEFAULTS`: browserRecordingFrameRate, browserRecordingShowKeyPresses,
// browserRecordingShowMousePresses, browserAutoShowFloatingPreview). "Open links in" is part 5's row (`browserLinkTarget`).
import { obj, type Obj } from './domain';
import { applyDeviceSetting, decodeClientPrefs, type ClientPrefs } from './settings-core';
import type { PreviewViewportSetting } from './browser-state';
import {
  DEFAULT_PREVIEW_ZOOM_FACTOR, FILL_PREVIEW_VIEWPORT, PREVIEW_VIEWPORT_MAX_AREA, PREVIEW_VIEWPORT_MAX_DIMENSION, PREVIEW_VIEWPORT_MIN_DIMENSION,
  PREVIEW_VIEWPORT_PRESETS, PREVIEW_ZOOM_LEVELS, isValidViewport, zoomLabel, type PreviewAppearancePreference,
} from './browser-viewport';
import { adoptBrowserProfilePrefs, browserDefaults, resolveBrowserDefaults, type BrowserDefaults } from './browser-profiles';

export const BROWSER_RECORDING_FRAME_RATES = [30, 60] as const;
export const DEFAULT_BROWSER_RECORDING_FRAME_RATE = 30;
export const DEFAULT_BROWSER_AUTO_SHOW_FLOATING_PREVIEW = true;
/** "Responsive" before a size was typed: Fill has no dimensions to carry over. */
export const RESPONSIVE_SEED_SIZE = { width: 1280, height: 800 } as const;

export type BrowserTabDefaultsPrefs = {
  browserDefaultViewport: PreviewViewportSetting;
  browserDefaultZoomFactor: number;
  browserDefaultAppearance: PreviewAppearancePreference;
};
const defaultsOf = (): BrowserTabDefaultsPrefs => ({ browserDefaultViewport: FILL_PREVIEW_VIEWPORT, browserDefaultZoomFactor: DEFAULT_PREVIEW_ZOOM_FACTOR, browserDefaultAppearance: 'system' });
type Holder = { local: object };
/** The client settings (settings-core.ts) the recording rows and Auto-show read and write. */
const clientPrefs = (owner: Holder): ClientPrefs => (owner.local as { clientSettings?: ClientPrefs }).clientSettings ?? decodeClientPrefs({});
const write = (owner: Holder, key: string, value: string): boolean => {
  try { return applyDeviceSetting(owner.local as Parameters<typeof applyDeviceSetting>[0], key, value); } catch { return false; }
};

/** PreviewViewportSetting as the settings schema decodes it; anything else is undefined (the default stays). */
export function decodeViewport(value: unknown): PreviewViewportSetting | undefined {
  const raw = obj(value), tag = raw._tag, width = raw.width, height = raw.height;
  if (tag === 'fill') return FILL_PREVIEW_VIEWPORT;
  if ((tag !== 'freeform' && tag !== 'preset') || typeof width !== 'number' || typeof height !== 'number') return undefined;
  const setting: PreviewViewportSetting = tag === 'preset'
    ? (typeof raw.presetId === 'string' && PREVIEW_VIEWPORT_PRESETS.some(preset => preset.id === raw.presetId) ? { _tag: 'preset', presetId: raw.presetId, width, height } : { _tag: 'freeform', width, height })
    : { _tag: 'freeform', width, height };
  return isValidViewport(setting) ? setting : undefined;
}
const zoomOf = (value: unknown) => PREVIEW_ZOOM_LEVELS.find(level => level === value);
const appearanceOf = (value: unknown) => value === 'system' || value === 'light' || value === 'dark' ? value : undefined;

/** load(): the saved defaults; a malformed value keeps its default (Schema.withDecodingDefault). */
export function adoptBrowserDefaultsPrefs(next: object, saved: Obj): void {
  const target = next as Partial<BrowserTabDefaultsPrefs>, fallback = defaultsOf();
  target.browserDefaultViewport = decodeViewport(saved.browserDefaultViewport) ?? fallback.browserDefaultViewport;
  target.browserDefaultZoomFactor = zoomOf(saved.browserDefaultZoomFactor) ?? fallback.browserDefaultZoomFactor;
  target.browserDefaultAppearance = appearanceOf(saved.browserDefaultAppearance) ?? fallback.browserDefaultAppearance;
}
/** load(): the profiles (browser-profiles.ts) and the tab defaults. */
export function adoptBrowserPrefs(next: object, saved: Obj): void {
  adoptBrowserProfilePrefs(next, saved);
  adoptBrowserDefaultsPrefs(next, saved);
}
/** The live settings on the client's preference record (filled with the defaults on first use). */
export function browserTabDefaultsPrefs(owner: Holder): BrowserTabDefaultsPrefs {
  const local = owner.local as Record<string, unknown>;
  for (const [key, value] of Object.entries(defaultsOf())) if (local[key] === undefined) local[key] = value;
  return local as unknown as BrowserTabDefaultsPrefs;
}

/** browserDefaults.ts `BrowserDefaults`: the profile half (browser-profiles.ts) and the tab state a new tab is born at. */
export type BrowserOpenDefaults = BrowserDefaults & { viewport: PreviewViewportSetting; zoomFactor: number; appearance: PreviewAppearancePreference; autoShowFloatingPreview: boolean };
export function browserOpenDefaults(owner: Holder): BrowserOpenDefaults {
  const prefs = browserTabDefaultsPrefs(owner);
  return { ...browserDefaults(owner), viewport: prefs.browserDefaultViewport, zoomFactor: prefs.browserDefaultZoomFactor, appearance: prefs.browserDefaultAppearance,
    autoShowFloatingPreview: clientPrefs(owner).browserAutoShowFloatingPreview };
}

/** resolveBrowserDefaults: refused (BrowserSettingsReadError) until the settings were read, then the open defaults. */
export function resolveBrowserOpenDefaults(owner: Holder & { preferencesLoaded?: boolean }): BrowserOpenDefaults {
  resolveBrowserDefaults(owner);
  return browserOpenDefaults(owner);
}

// ── Settings › General's Restore defaults (useSettingsRestore) ─────────────────────────────────────
/** SettingsPanels.logic.ts isSamePreviewViewport: the setting is a tagged union, compared by what it describes. */
function isSamePreviewViewport(left: PreviewViewportSetting, right: PreviewViewportSetting): boolean {
  if (left._tag !== right._tag) return false;
  if (left._tag === 'fill' || right._tag === 'fill') return true;
  if (left.width !== right.width || left.height !== right.height) return false;
  return left._tag === 'preset' && right._tag === 'preset' ? left.presetId === right.presetId : true;
}
/** getChangedBrowserSettingLabels: the browser-default rows that differ from the defaults, in the reference's order. */
export function changedBrowserSettingLabels(owner: Holder): string[] {
  const local = owner.local as Partial<BrowserTabDefaultsPrefs>, fallback = defaultsOf(), client = clientPrefs(owner);
  return [
    ...(isSamePreviewViewport(local.browserDefaultViewport ?? fallback.browserDefaultViewport, fallback.browserDefaultViewport) ? [] : ['Browser viewport']),
    ...((local.browserDefaultZoomFactor ?? fallback.browserDefaultZoomFactor) !== fallback.browserDefaultZoomFactor ? ['Browser zoom'] : []),
    ...((local.browserDefaultAppearance ?? fallback.browserDefaultAppearance) !== fallback.browserDefaultAppearance ? ['Browser appearance'] : []),
    ...(client.browserRecordingFrameRate !== DEFAULT_BROWSER_RECORDING_FRAME_RATE ? ['Recording frame rate'] : []),
    ...(client.browserRecordingShowKeyPresses ? ['Recording key presses'] : []),
    ...(client.browserRecordingShowMousePresses ? ['Recording mouse presses'] : []),
    ...(client.browserLinkTarget !== 'system' ? ['Open links in'] : []),
    ...(client.browserAutoShowFloatingPreview !== DEFAULT_BROWSER_AUTO_SHOW_FLOATING_PREVIEW ? ['Floating preview'] : []),
  ];
}
/** restoreDefaults' browser keys kept at the preference root (viewport, zoom, appearance); the recording rows, "Open links
 *  in" and Auto-show are client settings, which restoreDeviceDefaults resets with the rest. */
export function restoreBrowserTabDefaults(owner: Holder): void {
  Object.assign(owner.local, defaultsOf());
}

// ── The settings writes (useUpdatePrimarySettings for the group's rows) ───────────────────────────
const sized = (viewport: PreviewViewportSetting) => viewport._tag === 'fill' ? null : viewport;
/** BrowserViewportSetting selectViewport: Fill, Responsive (keeping a typed size, else the seed) or a preset. */
export function chooseViewport(prefs: BrowserTabDefaultsPrefs, value: string): void {
  const current = sized(prefs.browserDefaultViewport);
  if (value === 'fill') { prefs.browserDefaultViewport = FILL_PREVIEW_VIEWPORT; return; }
  if (value === 'responsive') { prefs.browserDefaultViewport = { _tag: 'freeform', width: current?.width ?? RESPONSIVE_SEED_SIZE.width, height: current?.height ?? RESPONSIVE_SEED_SIZE.height }; return; }
  const preset = PREVIEW_VIEWPORT_PRESETS.find(candidate => candidate.id === value);
  if (preset) prefs.browserDefaultViewport = { _tag: 'preset', width: preset.width, height: preset.height, presetId: preset.id };
}
/** commitDimension: a typed size within the bounds and the area cap; typing a size means the preset no longer describes it. */
export function commitViewportDimension(prefs: BrowserTabDefaultsPrefs, axis: 'width' | 'height', value: number): void {
  const current = sized(prefs.browserDefaultViewport);
  const presented = { width: current?.width ?? RESPONSIVE_SEED_SIZE.width, height: current?.height ?? RESPONSIVE_SEED_SIZE.height };
  if (!Number.isInteger(value) || value < PREVIEW_VIEWPORT_MIN_DIMENSION || value > PREVIEW_VIEWPORT_MAX_DIMENSION) return;
  const next = { ...presented, [axis]: value };
  if (next.width * next.height > PREVIEW_VIEWPORT_MAX_AREA) return;
  if (current && next.width === current.width && next.height === current.height) return;
  prefs.browserDefaultViewport = { _tag: 'freeform', ...next };
}
/** rotateViewport: width and height swapped; a rotated preset keeps its identity. */
export function rotateDefaultViewport(prefs: BrowserTabDefaultsPrefs): void {
  const current = sized(prefs.browserDefaultViewport);
  if (current) prefs.browserDefaultViewport = { ...current, width: current.height, height: current.width };
}
/** One row's write (`restlocal:browser-defaults`, `key=…&value=…`); false when the value is not one the row offers. */
export function applyBrowserDefault(owner: Holder, key: string, value: string): boolean {
  const prefs = browserTabDefaultsPrefs(owner);
  switch (key) {
    case 'viewport': chooseViewport(prefs, value); return true;
    case 'viewport-width': case 'viewport-height': commitViewportDimension(prefs, key === 'viewport-width' ? 'width' : 'height', Number(value)); return true;
    case 'viewport-rotate': rotateDefaultViewport(prefs); return true;
    case 'zoom': { const zoom = zoomOf(Number(value)); if (zoom === undefined) return false; prefs.browserDefaultZoomFactor = zoom; return true; }
    case 'appearance': { const appearance = appearanceOf(value); if (!appearance) return false; prefs.browserDefaultAppearance = appearance; return true; }
    case 'frame-rate': return (BROWSER_RECORDING_FRAME_RATES as readonly number[]).includes(Number(value)) && write(owner, 'browserRecordingFrameRate', value);
    case 'key-presses': return write(owner, 'browserRecordingShowKeyPresses', value === 'true' ? 'true' : 'false');
    case 'mouse-presses': return write(owner, 'browserRecordingShowMousePresses', value === 'true' ? 'true' : 'false');
    case 'auto-show': return write(owner, 'browserAutoShowFloatingPreview', value === 'true' ? 'true' : 'false');
    case 'reset':
      if (value === 'frame-rate') return write(owner, 'browserRecordingFrameRate', String(DEFAULT_BROWSER_RECORDING_FRAME_RATE));
      if (value === 'auto-show') return write(owner, 'browserAutoShowFloatingPreview', String(DEFAULT_BROWSER_AUTO_SHOW_FLOATING_PREVIEW));
      if (value === 'viewport' || value === 'zoom' || value === 'appearance') {
        const field = ({ viewport: 'browserDefaultViewport', zoom: 'browserDefaultZoomFactor', appearance: 'browserDefaultAppearance' } as const)[value];
        (prefs as Record<string, unknown>)[field] = defaultsOf()[field];
        return true;
      }
      return false;
  }
  return false;
}

// ── The rows' projection ─────────────────────────────────────────────────────────────────────────
export type DefaultsOption = { value: string; label: string; selected: boolean };
/** The viewport menu's rows: Fill panel, Responsive, then the presets under "Standard" (SelectGroupLabel), each with its size. */
export type ViewportOption = DefaultsOption & { detail: string; heading: string };
export type BrowserDefaultsView = {
  viewportValue: string; viewportLabel: string; viewportSized: boolean; viewportWidth: number; viewportHeight: number; rotateLabel: string; viewportReset: boolean; viewportOptions: ViewportOption[];
  zoomLabel: string; zoomReset: boolean; zoomOptions: DefaultsOption[]; appearanceLabel: string; appearanceReset: boolean; appearanceOptions: DefaultsOption[];
  frameRateLabel: string; frameRateReset: boolean; frameRateOptions: DefaultsOption[]; keyPresses: boolean; mousePresses: boolean; autoShow: boolean; autoShowReset: boolean;
};
const APPEARANCE_LABELS: Record<PreviewAppearancePreference, string> = { system: 'System', light: 'Light', dark: 'Dark' };
/** viewportSelectValue / viewportSelectLabel: a preset's id while it is one, else Responsive. */
export function viewportSelect(viewport: PreviewViewportSetting): { value: string; label: string } {
  if (viewport._tag === 'fill') return { value: 'fill', label: 'Fill panel' };
  const preset = viewport._tag === 'preset' ? PREVIEW_VIEWPORT_PRESETS.find(candidate => candidate.id === viewport.presetId) : undefined;
  return preset ? { value: preset.id, label: preset.label } : { value: 'responsive', label: 'Responsive' };
}
export function browserDefaultsView(owner: Holder): BrowserDefaultsView {
  const prefs = browserTabDefaultsPrefs(owner), client = clientPrefs(owner), viewport = prefs.browserDefaultViewport, select = viewportSelect(viewport), fixed = sized(viewport);
  const width = fixed?.width ?? RESPONSIVE_SEED_SIZE.width, height = fixed?.height ?? RESPONSIVE_SEED_SIZE.height;
  return {
    viewportValue: select.value, viewportLabel: select.label, viewportSized: !!fixed, viewportWidth: width, viewportHeight: height,
    rotateLabel: `Rotate to ${height >= width ? 'landscape' : 'portrait'}`, viewportReset: viewport._tag !== 'fill',
    viewportOptions: [{ value: 'fill', label: 'Fill panel', detail: '', heading: '', selected: select.value === 'fill' }, { value: 'responsive', label: 'Responsive', detail: '', heading: '', selected: select.value === 'responsive' },
      ...PREVIEW_VIEWPORT_PRESETS.map((preset, index) => ({ value: preset.id, label: preset.label, detail: preset.detail, heading: index === 0 ? 'Standard' : '', selected: select.value === preset.id }))],
    zoomLabel: zoomLabel(prefs.browserDefaultZoomFactor), zoomReset: prefs.browserDefaultZoomFactor !== DEFAULT_PREVIEW_ZOOM_FACTOR,
    zoomOptions: PREVIEW_ZOOM_LEVELS.map(level => ({ value: String(level), label: zoomLabel(level), selected: level === prefs.browserDefaultZoomFactor })),
    appearanceLabel: APPEARANCE_LABELS[prefs.browserDefaultAppearance], appearanceReset: prefs.browserDefaultAppearance !== 'system',
    appearanceOptions: (['system', 'light', 'dark'] as const).map(value => ({ value, label: APPEARANCE_LABELS[value], selected: value === prefs.browserDefaultAppearance })),
    frameRateLabel: `${client.browserRecordingFrameRate} fps`, frameRateReset: client.browserRecordingFrameRate !== DEFAULT_BROWSER_RECORDING_FRAME_RATE,
    frameRateOptions: BROWSER_RECORDING_FRAME_RATES.map(rate => ({ value: String(rate), label: `${rate} fps`, selected: rate === client.browserRecordingFrameRate })),
    keyPresses: client.browserRecordingShowKeyPresses, mousePresses: client.browserRecordingShowMousePresses,
    autoShow: client.browserAutoShowFloatingPreview, autoShowReset: client.browserAutoShowFloatingPreview !== DEFAULT_BROWSER_AUTO_SHOW_FLOATING_PREVIEW,
  };
}
