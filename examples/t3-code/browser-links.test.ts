// browser-surface part 5. Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names:
// apps/web/src/browser/browserLinkTarget.test.ts (5 + its 2 `it.each` rows; the preference is read from the client's
// loaded settings here, so "rejects failed reads" becomes "reads the saved preference"),
// components/preview/openTerminalLinkInPreview.test.ts (7 + 1 `it.each` row; an interrupted open is a let-go
// answer here), and RightPanelTabs.test.tsx's three `tabMuteMenuItem` tests (:253-283). Then the clone's rows:
// `openLink` (useOpenLink: the setting, the modifier, the fallback), a chat link and a check's button through
// `chatlocal:link-open`, a terminal link in the app, the tab menu's Mute row, and the "Open links in" setting.
import { describe, expect, it } from 'bun:test';
import {
  TerminalLinkPreviewOpenError, linkTargetPreference, openLink, openLinkFromUi, openTerminalLinkInPreview, resolveLinkTarget, resolveLinkTargetPreference, type BrowserLinkTarget,
} from './browser-links';
import { tabAudioState, tabContextMenuItems, tabMuteMenuItem } from './right-panel-tabs';
import { browserTabMute } from './browser-automation-tabs';
import { browserHost, browserSurfaceId } from './browser-surface';
import { previewRuntimeTabId, type PreviewSessionSnapshot } from './browser-state';
import { surfaceStore, type Surface } from './r4-surfaces-panel';
import { restCommand } from './settings-rest-commands';
import { chatLocal } from './timeline-presentation';
import { terminalLinkAction } from './terminal-integrations';
import { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';
import { toasts } from './toast';
import type { Obj } from './domain';
import { scopedThreadKey } from './terminal-ui-state';
import { BrowserSettingsReadError } from './browser-profiles';

const click = { metaKey: false, ctrlKey: false };
describe('resolveLinkTarget', () => {
  it('keeps the system browser unless the user asked for in-app', () => {
    expect(resolveLinkTarget({ url: 'https://example.com/', event: click, preference: 'system', canOpenInApp: true })).toBe('system');
  });
  it('opens in-app when asked and the runtime can', () => {
    expect(resolveLinkTarget({ url: 'https://example.com/', event: click, preference: 'app', canOpenInApp: true })).toBe('app');
  });
  it('falls back to the system browser where there is no in-app browser', () => {
    expect(resolveLinkTarget({ url: 'https://example.com/', event: click, preference: 'app', canOpenInApp: false })).toBe('system');
  });
  it('treats a modifier click as the way out of the in-app default', () => {
    expect(resolveLinkTarget({ url: 'https://example.com/', event: { metaKey: true, ctrlKey: false }, preference: 'app', canOpenInApp: true })).toBe('system');
    expect(resolveLinkTarget({ url: 'https://example.com/', event: { metaKey: false, ctrlKey: true }, preference: 'app', canOpenInApp: true })).toBe('system');
  });
  it('leaves non-web schemes to the shell', () => {
    for (const url of ['mailto:someone@example.com', 'vscode://file/x', 'not a url']) expect(resolveLinkTarget({ url, event: click, preference: 'app', canOpenInApp: true })).toBe('system');
  });
});

describe('resolveBrowserLinkTargetPreference', () => {
  for (const preference of ['system', 'app'] as const) {
    it(`rejects failed reads instead of using the current ${preference} preference`, () => {
      // The clone's hydration is the command's preference read (client.ts load): until it has run, the read is refused.
      const client = new T3Client();
      client.local.clientSettings = { ...client.local.clientSettings, browserLinkTarget: preference } as typeof client.local.clientSettings;
      expect(() => resolveLinkTargetPreference(client)).toThrow(BrowserSettingsReadError);
      Object.assign(client, { loaded: true });
      expect(resolveLinkTargetPreference(client)).toBe(preference);
    });
  }
});

const threadRef = { environmentId: 'local', threadId: 'thread-1' };
const snapshot: PreviewSessionSnapshot = { threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Idle' }, canGoBack: false, canGoForward: false, updatedAt: '2026-06-20T00:00:00.000Z' };
const hydratedDefaults = { viewport: { _tag: 'freeform' as const, width: 1280, height: 720 }, profileId: 'work' };
function terminalLink(overrides: Partial<Parameters<typeof openTerminalLinkInPreview>[0]> = {}) {
  const calls = { fallback: 0, opened: [] as Obj[], reported: [] as TerminalLinkPreviewOpenError[] };
  const input: Parameters<typeof openTerminalLinkInPreview>[0] = {
    url: 'https://example.com/docs', threadRef, forceBrowser: false, supported: true, preference: () => 'app', defaults: async () => hydratedDefaults,
    openPreview: async request => { calls.opened.push(request as unknown as Obj); return snapshot; },
    fallbackToBrowser: () => { calls.fallback++; }, report: error => { calls.reported.push(error); }, ...overrides,
  };
  return { input, calls };
}
describe('openTerminalLinkInPreview', () => {
  for (const setting of ['target', 'defaults'] as const) {
    it(`does not open either browser when reading ${setting} fails`, async () => {
      const failure = new Error('Settings read failed');
      const { input, calls } = terminalLink(setting === 'target' ? { preference: () => { throw failure; } } : { defaults: async () => { throw failure; } });
      await expect(openTerminalLinkInPreview(input)).rejects.toBe(failure);
      expect(calls.fallback).toBe(0);
      expect(calls.opened).toEqual([]);
    });
  }
  it('opens in the system browser while that is the configured target', async () => {
    const { input, calls } = terminalLink({ url: 'http://localhost:3000/', preference: () => 'system' as BrowserLinkTarget });
    await openTerminalLinkInPreview(input);
    expect(calls.fallback).toBe(1);
    expect(calls.opened).toEqual([]);
  });
  it('opens public URLs in-app too, not only local servers', async () => {
    const { input, calls } = terminalLink();
    await openTerminalLinkInPreview(input);
    expect(calls.opened).toHaveLength(1);
    expect(calls.fallback).toBe(0);
  });
  it('waits for hydrated viewport and profile defaults before opening', async () => {
    let hydrate!: (defaults: typeof hydratedDefaults) => void;
    const { input, calls } = terminalLink({ url: 'http://localhost:3000/', defaults: () => new Promise(resolve => { hydrate = resolve; }) });
    const opening = openTerminalLinkInPreview(input);
    await Promise.resolve();
    expect(calls.opened).toEqual([]);
    hydrate(hydratedDefaults);
    await opening;
    expect(calls.opened).toEqual([{ environmentId: 'local', input: { threadId: 'thread-1', url: 'http://localhost:3000/', viewport: hydratedDefaults.viewport, profileId: 'work' } }]);
  });
  it('preserves the complete preview failure cause before falling back', async () => {
    const cause = new Error('preview unavailable');
    const { input, calls } = terminalLink({ url: 'http://127.0.0.1:5173/', openPreview: async () => { throw cause; } });
    await openTerminalLinkInPreview(input);
    expect(calls.fallback).toBe(1);
    expect(calls.reported).toHaveLength(1);
    expect(calls.reported[0]).toBeInstanceOf(TerminalLinkPreviewOpenError);
    expect(calls.reported[0]).toMatchObject({ threadRef: { environmentId: 'local', threadId: 'thread-1' }, targetOrigin: 'http://127.0.0.1:5173', cause });
    expect(calls.reported[0]?.message).not.toContain('preview unavailable');
  });
  it('does not report or fall back when opening the preview is interrupted', async () => {
    const { input, calls } = terminalLink({ url: 'http://localhost:5173/', openPreview: async () => { throw new ClientError('let go', 'superseded'); } });
    await expect(openTerminalLinkInPreview(input)).rejects.toThrow('let go');
    expect(calls.reported).toEqual([]);
    expect(calls.fallback).toBe(0);
  });
  it('opens in the system browser when Ctrl or Command is held', async () => {
    const { input, calls } = terminalLink({ forceBrowser: true });
    await openTerminalLinkInPreview(input);
    expect(calls.fallback).toBe(1);
    expect(calls.opened).toEqual([]);
  });
});

describe('tabMuteMenuItem', () => {
  const overlay = (audioMuted: boolean) => ({ audioMuted, audible: false });
  it('stays disabled until the desktop tab exists', () => {
    expect(tabMuteMenuItem({ overlay: null, canResolveRuntimeTabId: true })).toEqual({ label: 'Mute tab', disabled: true });
  });
  it('stays disabled when no runtime tab id can be resolved', () => {
    expect(tabMuteMenuItem({ overlay: overlay(false), canResolveRuntimeTabId: false })).toEqual({ label: 'Mute tab', disabled: true });
  });
  it('offers mute and unmute once the tab is addressable', () => {
    expect(tabMuteMenuItem({ overlay: overlay(false), canResolveRuntimeTabId: true })).toEqual({ label: 'Mute tab', disabled: false });
    expect(tabMuteMenuItem({ overlay: overlay(true), canResolveRuntimeTabId: true })).toEqual({ label: 'Unmute tab', disabled: false });
  });
});

// ── The clone's rows ──────────────────────────────────────────────────────────────────────────────
const storage: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
/** A client on thread-1 whose server answers preview.list/open and whose module records its ops. */
function linkClient(options: { preference?: BrowserLinkTarget; failOpen?: boolean; modifiers?: string } = {}) {
  const client = new T3Client(), rpcs: Array<{ method: string; payload: Obj }> = [], ops: Obj[] = [];
  client.environmentId = 'local'; client.threadId = 'thread-1'; client.projectId = 'p';
  Object.assign(client, { loaded: true }); // the saved settings were read: part 4's open defaults refuse unread ones
  client.local.clientSettings = { ...client.local.clientSettings, browserLinkTarget: options.preference ?? 'app' } as typeof client.local.clientSettings;
  client.request = async (_native, method, payload) => {
    rpcs.push({ method, payload });
    if (method === 'preview.list') return { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
    if (method === 'preview.open') { if (options.failOpen) throw new Error('preview.open refused'); return { ...snapshot, navStatus: { _tag: 'Loading', url: String(payload.url), title: '' } }; }
    return {};
  };
  const native: Native = { available: true, watch() {}, async later(request) {
    const op = obj(request); ops.push(op);
    if (op.op === 'composerSendIntent') return { ok: true, generation: 0, value: { modifiers: options.modifiers ?? '', source: 'pointer' } };
    if (op.op === 'terminalOpenExternal') return { ok: true, generation: 0, value: { opened: true } };
    return { ok: true, generation: 0, value: { opened: true } };
  } };
  return { client, rpcs, ops, native };
}
const obj = (value: unknown): Obj => (value && typeof value === 'object' ? value as Obj : {});
const external = (ops: Obj[]) => ops.filter(op => op.op === 'remoteEditorsOpen' || op.op === 'terminalOpenExternal').map(op => op.url);

describe('openLink (useOpenLink)', () => {
  it('opens a Browser tab beside the thread when the setting says T3 Code', async () => {
    const { client, rpcs, ops, native } = linkClient();
    expect(await openLink(client, native, 'https://example.com/docs')).toBe('app');
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toEqual({ threadId: 'thread-1', url: 'https://example.com/docs', viewport: { _tag: 'fill' }, profileId: 'default' });
    expect(surfaceStore(client).panels.get('local:thread-1')).toMatchObject({ active: browserSurfaceId('tab-1'), visible: true });
    expect(external(ops)).toEqual([]);
  });
  it('opens under the configured profile and viewport, and opens neither browser while the settings are unread (part 4)', async () => {
    const { client, rpcs, ops, native } = linkClient();
    Object.assign(client.local, { browserProfiles: [{ id: 'work', name: 'Work', kind: 'persistent' }], browserDefaultProfileId: 'work', browserDefaultViewport: { _tag: 'preset', presetId: 'ipad-mini', width: 768, height: 1024 } });
    expect(await openLink(client, native, 'https://example.com/docs')).toBe('app');
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ viewport: { _tag: 'preset', presetId: 'ipad-mini' }, profileId: 'work' });
    Object.assign(client, { loaded: false });
    await expect(openLink(client, native, 'https://example.com/later')).rejects.toBeInstanceOf(BrowserSettingsReadError);
    await expect(openLink(client, native, 'https://example.com/system', { event: { metaKey: true, ctrlKey: false } })).rejects.toBeInstanceOf(BrowserSettingsReadError);
    expect(rpcs.filter(call => call.method === 'preview.open')).toHaveLength(1);
    expect(external(ops)).toEqual([]);
  });
  it('falls back to the system browser when the in-app open fails', async () => {
    const { client, ops, native } = linkClient({ failOpen: true });
    expect(await openLink(client, native, 'https://example.com/docs')).toBe('system');
    expect(external(ops)).toEqual(['https://example.com/docs']);
  });
  it('keeps the system browser for the default setting, a modifier, a non-web link and a link with no thread', async () => {
    for (const [options, url, extra] of [[{ preference: 'system' as const }, 'https://a.test/', {}], [{}, 'https://b.test/', { event: { metaKey: true, ctrlKey: false } }], [{}, 'https://c.test/', { threadRef: null }]] as const) {
      const { client, rpcs, ops, native } = linkClient(options);
      expect(await openLink(client, native, url, extra)).toBe('system');
      expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
      expect(external(ops)).toEqual([url]);
    }
  });
});

describe('links from the UI (chatlocal:link-open)', () => {
  it('a chat link reads the click’s ⌘ or Ctrl from the module; a check’s button has none', async () => {
    const held = linkClient({ modifiers: 'meta' });
    await chatLocal(held.client, held.native, 'link-open', 'link', 'https://example.com/');
    expect(external(held.ops)).toEqual(['https://example.com/']);
    const plain = linkClient({ modifiers: '' });
    await chatLocal(plain.client, plain.native, 'link-open', 'link', 'https://example.com/');
    expect(plain.rpcs.some(call => call.method === 'preview.open')).toBe(true);
    const button = linkClient({ modifiers: 'control' });
    await openLinkFromUi(button.client, button.native, 'button', 'https://ci.example.com/run/1');
    expect(button.ops.some(op => op.op === 'composerSendIntent')).toBe(false);
    expect(button.rpcs.find(call => call.method === 'preview.open')?.payload.url).toBe('https://ci.example.com/run/1');
    const page = linkClient();
    await openLinkFromUi(page.client, page.native, 'button-page', 'https://ci.example.com/run/2');
    expect(external(page.ops)).toEqual(['https://ci.example.com/run/2']);
  });
  it('a work-log web search result ("external") opens the system browser even when the setting says T3 Code, ⌘ or not (timeline-work-rows TH-5)', async () => {
    // V2ItemInspector's result is a target=_blank link; the desktop's setWindowOpenHandler hands it to the shell.
    for (const modifiers of ['', 'meta']) {
      const { client, rpcs, ops, native } = linkClient({ preference: 'app', modifiers });
      await chatLocal(client, native, 'link-open', 'external', 'https://example.test/tables');
      expect(external(ops)).toEqual(['https://example.test/tables']);
      expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
      expect(ops.some(op => op.op === 'composerSendIntent')).toBe(false);
    }
    const { client } = linkClient();
    const refused: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: { opened: false } }; } };
    await openLinkFromUi(client, refused, 'external', 'https://example.test/tables');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Unable to open link' });
  });
  it('a link the system browser could not open says so', async () => {
    const { client } = linkClient({ preference: 'system' });
    const native: Native = { available: true, watch() {}, async later(request) { return obj(request).op === 'composerSendIntent' ? { ok: true, generation: 0, value: {} } : { ok: true, generation: 0, value: { opened: false } }; } };
    await openLinkFromUi(client, native, 'link', 'https://example.com/');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Unable to open link' });
  });
});

describe('terminal links in the app', () => {
  it('opens an http(s) link in a Browser tab when the setting says T3 Code, ⌘-click the system browser', async () => {
    const { client, rpcs, ops, native } = linkClient();
    await terminalLinkAction(client, native, { text: 'http://localhost:5173/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1' });
    expect(rpcs.find(call => call.method === 'preview.open')?.payload.url).toBe('http://localhost:5173/');
    expect(external(ops)).toEqual([]);
    const held = linkClient();
    await terminalLinkAction(held.client, held.native, { text: 'http://localhost:5173/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1', metaKey: true });
    expect(external(held.ops)).toEqual(['http://localhost:5173/']);
  });
  it('a terminal link whose in-app open fails goes to the system browser', async () => {
    const { client, ops, native } = linkClient({ failOpen: true });
    await terminalLinkAction(client, native, { text: 'http://localhost:5173/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1' });
    expect(external(ops)).toEqual(['http://localhost:5173/']);
  });
  it('opens under the configured viewport and profile, and opens neither browser while the settings are unread', async () => {
    const { client, rpcs, ops, native } = linkClient();
    Object.assign(client.local, { browserProfiles: [{ id: 'work', name: 'Work', kind: 'persistent' }], browserDefaultProfileId: 'work', browserDefaultViewport: { _tag: 'preset', presetId: 'iphone-12-pro', width: 390, height: 844 } });
    await terminalLinkAction(client, native, { text: 'http://localhost:5173/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1' });
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toEqual({ threadId: 'thread-1', url: 'http://localhost:5173/', viewport: { _tag: 'preset', presetId: 'iphone-12-pro', width: 390, height: 844 }, profileId: 'work' });
    Object.assign(client, { loaded: false });
    await expect(terminalLinkAction(client, native, { text: 'http://localhost:5174/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1' })).rejects.toBeInstanceOf(BrowserSettingsReadError);
    expect(rpcs.filter(call => call.method === 'preview.open')).toHaveLength(1);
    expect(external(ops)).toEqual([]);
    // ⌘-click asks for the system browser, which needs no setting.
    await terminalLinkAction(client, native, { text: 'http://localhost:5175/', threadId: 'thread-1', environmentId: 'local', terminalId: 'term-1', metaKey: true });
    expect(external(ops)).toEqual(['http://localhost:5175/']);
  });
});

describe('the tab menu’s Mute row and the audible indicator', () => {
  it('a Browser tab’s menu has Mute until it is muted, disabled until its page exists', () => {
    const { client } = linkClient();
    const ref = { environmentId: 'local', threadId: 'thread-1' }, store = browserHost(client).store;
    store.reconcileServerSessions(ref, { sessions: [{ ...snapshot, navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example' } }], serverEpoch: 'epoch-1', revision: 1 });
    const keyed: Surface = { id: browserSurfaceId('tab-1'), kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId: 'tab-1', threadKey: scopedThreadKey(ref) } };
    expect(tabContextMenuItems(keyed, [keyed], entry => browserTabMute(client, entry))[0]).toEqual({ id: 'toggle-mute', label: 'Mute tab', disabled: true });
    client.presentation = { browserTabs: { [previewRuntimeTabId(ref, 'epoch-1', 'tab-1')]: { kind: 'Success', url: 'https://example.com/', title: 'Example' } } };
    store.applyDesktopState(ref, 'tab-1', { hasWebContents: true, canGoBack: false, canGoForward: false, loading: false, zoomFactor: 1, pictureInPicture: false, colorScheme: 'system', audioMuted: true, audible: true, controller: 'none', favicon: null });
    expect(tabContextMenuItems(keyed, [keyed], entry => browserTabMute(client, entry))[0]).toEqual({ id: 'toggle-mute', label: 'Unmute tab', disabled: false });
    expect(tabAudioState({ audioMuted: true, audible: true })).toBe('muted');
    expect(tabAudioState({ audioMuted: true, audible: false })).toBe('none');
    expect(tabAudioState({ audioMuted: false, audible: true })).toBe('audible');
  });
});

describe('the "Open links in" setting', () => {
  it('writes the client setting and refuses anything else', async () => {
    const client = new T3Client();
    let saved = '';
    client.savePreferences = async () => { saved = String((client.local.clientSettings as { browserLinkTarget?: string }).browserLinkTarget); };
    const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } };
    await restCommand(client, native, storage, 'browser-link-target', ':', 'app');
    expect(linkTargetPreference(client)).toBe('app');
    expect(saved).toBe('app');
    await expect(restCommand(client, native, storage, 'browser-link-target', ':', 'chrome')).rejects.toThrow('Unsupported device setting.');
    expect(linkTargetPreference(client)).toBe('app');
  });
});
