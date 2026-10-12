// browser-surface part 4: the Browser defaults group (clone rows, each named after the reference code it follows:
// packages/contracts settings.ts's decoding defaults, browserDefaults.ts, IntegrationsSettings.tsx's
// BrowserViewportSetting selectViewport / commitDimension / rotateViewport and the other rows' writes and resets).
import { describe, expect, it } from 'bun:test';
import {
  adoptBrowserPrefs, applyBrowserDefault, browserDefaultsView, browserOpenDefaults, changedBrowserSettingLabels, decodeViewport, resolveBrowserOpenDefaults, viewportSelect,
} from './browser-defaults';
import { BrowserSettingsReadError } from './browser-profiles';
import type { Obj } from './domain';
import { decodeClientPrefs } from './settings-core';
import { DEFAULT_BROWSER_DEFAULTS, browserResponsiveViewportForToggle } from './browser-viewport';

const owner = (local: Obj = {}) => ({ local: { ...local } as object });

describe('ClientSettingsSchema browser defaults', () => {
  it('decodes saved values and keeps each default for a malformed one', () => {
    const next: Obj = {};
    adoptBrowserPrefs(next, { browserDefaultViewport: { _tag: 'preset', presetId: 'iphone-se', width: 375, height: 667 }, browserDefaultZoomFactor: 1.25, browserDefaultAppearance: 'dark' });
    expect(next).toEqual({ browserDefaultViewport: { _tag: 'preset', presetId: 'iphone-se', width: 375, height: 667 }, browserDefaultZoomFactor: 1.25, browserDefaultAppearance: 'dark',
      browserProfiles: [], browserDefaultProfileId: 'default' });
    const bad: Obj = {};
    adoptBrowserPrefs(bad, { browserDefaultViewport: { _tag: 'freeform', width: 100, height: 100 }, browserDefaultZoomFactor: 1.2, browserDefaultAppearance: 'sepia' });
    expect(bad).toMatchObject({ browserDefaultViewport: { _tag: 'fill' }, browserDefaultZoomFactor: 1, browserDefaultAppearance: 'system' });
    // The recording rows and Auto-show are part 3's client settings (settings-core.ts CLIENT_DEFAULTS).
    const settings = decodeClientPrefs({ browserRecordingFrameRate: 60, browserRecordingShowKeyPresses: true, browserRecordingShowMousePresses: 'yes', browserAutoShowFloatingPreview: false });
    expect([settings.browserRecordingFrameRate, settings.browserRecordingShowKeyPresses, settings.browserRecordingShowMousePresses, settings.browserAutoShowFloatingPreview]).toEqual([60, true, false, false]);
    expect(decodeClientPrefs({ browserRecordingFrameRate: 24 }).browserRecordingFrameRate).toBe(30);
    expect(decodeViewport({ _tag: 'preset', presetId: 'not-a-device', width: 800, height: 600 })).toEqual({ _tag: 'freeform', width: 800, height: 600 });
    expect(decodeViewport({ _tag: 'freeform', width: 3840, height: 3840 })).toBeUndefined();
  });
});

describe('browserDefaults (a new tab is born at the configured state)', () => {
  it('opens at the saved profile and viewport, and refuses unread settings', () => {
    const client = { ...owner({ browserProfiles: [{ id: 'work', name: 'Work', kind: 'persistent' }], browserDefaultProfileId: 'work' }), preferencesLoaded: false };
    applyBrowserDefault(client, 'viewport', 'ipad-mini');
    expect(() => resolveBrowserOpenDefaults(client)).toThrow(BrowserSettingsReadError);
    client.preferencesLoaded = true;
    expect(resolveBrowserOpenDefaults(client)).toMatchObject({ profileId: 'work', viewport: { _tag: 'preset', presetId: 'ipad-mini', width: 768, height: 1024 }, zoomFactor: 1, appearance: 'system', autoShowFloatingPreview: true });
  });
  it('starts a fresh home at part 2’s defaults, and Show device toolbar opens at a fixed configured viewport', () => {
    const fresh = owner();
    expect(browserOpenDefaults(fresh)).toMatchObject({ viewport: DEFAULT_BROWSER_DEFAULTS.viewport, zoomFactor: DEFAULT_BROWSER_DEFAULTS.zoomFactor, appearance: DEFAULT_BROWSER_DEFAULTS.appearance });
    const panelRect = { width: 900, height: 700 };
    expect(browserResponsiveViewportForToggle({ defaults: browserOpenDefaults(fresh), panelRect, zoomFactor: 1 })).toEqual(browserResponsiveViewportForToggle({ defaults: DEFAULT_BROWSER_DEFAULTS, panelRect, zoomFactor: 1 }));
    applyBrowserDefault(fresh, 'viewport', 'ipad-mini');
    expect(browserResponsiveViewportForToggle({ defaults: browserOpenDefaults(fresh), panelRect, zoomFactor: 1 })).toMatchObject({ _tag: 'preset', presetId: 'ipad-mini' });
  });
});

describe('BrowserViewportSetting', () => {
  it('chooses Fill, Responsive (the seed, or the typed size) and presets, and labels them', () => {
    const client = owner();
    expect(browserDefaultsView(client)).toMatchObject({ viewportValue: 'fill', viewportLabel: 'Fill panel', viewportSized: false, viewportReset: false });
    applyBrowserDefault(client, 'viewport', 'responsive');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'freeform', width: 1280, height: 800 });
    expect(browserDefaultsView(client)).toMatchObject({ viewportValue: 'responsive', viewportLabel: 'Responsive', viewportSized: true, viewportWidth: 1280, viewportHeight: 800, rotateLabel: 'Rotate to portrait', viewportReset: true });
    applyBrowserDefault(client, 'viewport', 'iphone-se');
    expect(viewportSelect(browserOpenDefaults(client).viewport)).toEqual({ value: 'iphone-se', label: 'iPhone SE' });
    applyBrowserDefault(client, 'viewport', 'responsive');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'freeform', width: 375, height: 667 });
    applyBrowserDefault(client, 'reset', 'viewport');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'fill' });
  });
  it('commits a typed size within the bounds and the area cap, and rotates keeping a preset', () => {
    const client = owner();
    applyBrowserDefault(client, 'viewport', 'iphone-se');
    applyBrowserDefault(client, 'viewport-width', '1024');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'freeform', width: 1024, height: 667 });
    for (const value of ['239', '3841', '1024.5', 'x']) applyBrowserDefault(client, 'viewport-width', value);
    applyBrowserDefault(client, 'viewport-height', '3840');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'freeform', width: 1024, height: 3840 });
    applyBrowserDefault(client, 'viewport-width', '3000');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'freeform', width: 1024, height: 3840 }, );
    applyBrowserDefault(client, 'viewport', 'ipad-mini');
    applyBrowserDefault(client, 'viewport-rotate', '');
    expect(browserOpenDefaults(client).viewport).toEqual({ _tag: 'preset', presetId: 'ipad-mini', width: 1024, height: 768 });
    expect(browserDefaultsView(client).viewportLabel).toBe('iPad Mini');
  });
  it('lists Fill panel and Responsive, then the presets under "Standard", each size beside its name', () => {
    const client = owner();
    applyBrowserDefault(client, 'viewport', 'iphone-12-pro');
    const options = browserDefaultsView(client).viewportOptions;
    expect(options.slice(0, 4)).toEqual([
      { value: 'fill', label: 'Fill panel', detail: '', heading: '', selected: false }, { value: 'responsive', label: 'Responsive', detail: '', heading: '', selected: false },
      { value: 'iphone-se', label: 'iPhone SE', detail: '375 × 667', heading: 'Standard', selected: false }, { value: 'iphone-xr', label: 'iPhone XR', detail: '414 × 896', heading: '', selected: false }]);
    expect(options.filter(option => option.heading !== '').map(option => option.value)).toEqual(['iphone-se']);
    expect(options.find(option => option.selected)).toMatchObject({ label: 'iPhone 12 Pro', detail: '390 × 844' });
  });
});

describe('the zoom, appearance, recording and auto-show rows', () => {
  it('writes only the values each row offers and resets to the defaults', () => {
    const client = owner();
    expect(browserDefaultsView(client)).toMatchObject({ frameRateLabel: '30 fps', keyPresses: false, mousePresses: false, autoShow: true });
    expect(applyBrowserDefault(client, 'zoom', '1.5')).toBe(true);
    expect(applyBrowserDefault(client, 'zoom', '1.4')).toBe(false);
    expect(applyBrowserDefault(client, 'appearance', 'dark')).toBe(true);
    expect(applyBrowserDefault(client, 'frame-rate', '60')).toBe(true);
    expect(applyBrowserDefault(client, 'frame-rate', '24')).toBe(false);
    applyBrowserDefault(client, 'key-presses', 'true');
    applyBrowserDefault(client, 'auto-show', 'false');
    expect(browserDefaultsView(client)).toMatchObject({ zoomLabel: '150%', zoomReset: true, appearanceLabel: 'Dark', appearanceReset: true, frameRateLabel: '60 fps', frameRateReset: true,
      keyPresses: true, mousePresses: false, autoShow: false, autoShowReset: true });
    expect(browserDefaultsView(client).zoomOptions.find(option => option.selected)?.label).toBe('150%');
    for (const key of ['zoom', 'appearance', 'frame-rate', 'auto-show']) applyBrowserDefault(client, 'reset', key);
    expect(browserDefaultsView(client)).toMatchObject({ zoomLabel: '100%', zoomReset: false, appearanceLabel: 'System', frameRateLabel: '30 fps', autoShow: true, autoShowReset: false });
    expect(applyBrowserDefault(client, 'reset', 'key-presses')).toBe(false);
    expect((client.local as { clientSettings: Obj }).clientSettings).toMatchObject({ browserRecordingFrameRate: 30, browserRecordingShowKeyPresses: true, browserAutoShowFloatingPreview: true });
  });
});

// SettingsPanels.logic.test.ts getChangedBrowserSettingLabels (3, under their own names); Settings › General's Restore
// defaults over them is settings-core.test.ts's.
describe('getChangedBrowserSettingLabels', () => {
  it('reports nothing for the defaults', () => {
    expect(changedBrowserSettingLabels(owner())).toEqual([]);
    expect(changedBrowserSettingLabels(owner({ browserDefaultViewport: { _tag: 'fill' }, browserDefaultZoomFactor: 1, browserDefaultAppearance: 'system', clientSettings: decodeClientPrefs({}) }))).toEqual([]);
  });
  it('treats a structurally equal viewport as unchanged', () => {
    expect(changedBrowserSettingLabels(owner({ browserDefaultViewport: { ...{ _tag: 'fill' } } }))).toEqual([]);
  });
  it('labels each browser default that differs', () => {
    const client = owner({ browserDefaultViewport: { _tag: 'freeform', width: 900, height: 600 }, browserDefaultZoomFactor: 1.5, browserDefaultAppearance: 'dark',
      clientSettings: decodeClientPrefs({ browserRecordingFrameRate: 60, browserRecordingShowKeyPresses: true, browserRecordingShowMousePresses: true, browserLinkTarget: 'app', browserAutoShowFloatingPreview: false }) });
    expect(changedBrowserSettingLabels(client)).toEqual(['Browser viewport', 'Browser zoom', 'Browser appearance', 'Recording frame rate', 'Recording key presses', 'Recording mouse presses', 'Open links in', 'Floating preview']);
  });
});
