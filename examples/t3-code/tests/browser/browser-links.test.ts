// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names:
// apps/web/src/browser/browserLinkTarget.test.ts (5 + its 2 `it.each` rows; the preference is read from the client's
// loaded settings here, so "rejects failed reads" becomes "reads the saved preference"),
// components/preview/openTerminalLinkInPreview.test.ts (7 + 1 `it.each` row; an interrupted open is a let-go
// answer here), and RightPanelTabs.test.tsx's three `tabMuteMenuItem` tests (:253-283). Then the clone's rows:
// `openLink` (useOpenLink: the setting, the modifier, the fallback), a chat link and a check's button through
// `chatlocal:link-open`, a terminal link in the app, a page's target=_blank links (ElectronBrowserHost `onOpenLink`,
// openUrlInPreview's `profileId` and `background`), the tab menu's Mute row, and the "Open links in" setting.
import { describe, expect, it } from 'bun:test';
import {
  TerminalLinkPreviewOpenError, linkTargetPreference, openLink, openLinkFromUi, openRequestedLinks, openTerminalLinkInPreview, resolveLinkTarget, resolveLinkTargetPreference, type BrowserLinkTarget,
} from '../../src/browser/browser-links';
import { tabAudioState, tabContextMenuItems, tabMuteMenuItem } from '../../src/panel/right-panel-tabs';
import { browserTabMute } from '../../src/browser/browser-automation-tabs';
import { browserHost, browserSurfaceId } from '../../src/browser/browser-surface';
import { previewRuntimeTabId, type PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { surfaceStore, type Surface } from '../../src/panel/panel-surfaces';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { chatLocal } from '../../src/timeline/timeline-presentation';
import { terminalLinkAction } from '../../src/terminal/terminal-integrations';
import { T3Client } from '../../src/client/client';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';
import type { Obj } from '../../src/client/domain';
import { scopedThreadKey } from '../../src/terminal/terminal-ui-state';
import { BrowserSettingsReadError } from '../../src/browser/browser-profiles';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';

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
  Object.assign(client, { loaded: true }); // the saved settings were read: the open defaults refuse unread ones
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
  it('opens under the configured profile and viewport, and opens neither browser while the settings are unread', async () => {
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
  it('a chat link reads the click’s ⌘ or Ctrl from its own press; a check’s button has none', async () => {
    // app-window.contract linkOpen sends `link|meta+shift`, the press's MouseEvent; the
    // module's gesture recorder is never asked.
    for (const held of ['meta', 'control', 'meta+shift', 'control+alt']) {
      const link = linkClient({ modifiers: '' });
      await chatLocal(link.client, link.native, 'link-open', `link|${held}`, 'https://example.com/');
      expect(external(link.ops)).toEqual(['https://example.com/']);
      expect(link.ops.some(op => op.op === 'composerSendIntent')).toBe(false);
    }
    for (const held of ['', 'alt', 'shift']) { // ⌥ keeps the in-app default: resolveLinkTarget reads no ⌥
      const plain = linkClient({ modifiers: 'meta' }); // a recorded gesture is not this click's
      await chatLocal(plain.client, plain.native, 'link-open', `link|${held}`, 'https://example.com/');
      expect(plain.rpcs.some(call => call.method === 'preview.open')).toBe(true);
      expect(external(plain.ops)).toEqual([]);
    }
    const bare = linkClient({ modifiers: '' });
    await chatLocal(bare.client, bare.native, 'link-open', 'link', 'https://example.com/');
    expect(bare.rpcs.some(call => call.method === 'preview.open')).toBe(true);
    const button = linkClient({ modifiers: 'control' });
    await openLinkFromUi(button.client, button.native, 'button|meta', 'https://ci.example.com/run/1');
    expect(button.ops.some(op => op.op === 'composerSendIntent')).toBe(false);
    expect(button.rpcs.find(call => call.method === 'preview.open')?.payload.url).toBe('https://ci.example.com/run/1');
    const page = linkClient();
    await openLinkFromUi(page.client, page.native, 'button-page', 'https://ci.example.com/run/2');
    expect(external(page.ops)).toEqual(['https://ci.example.com/run/2']);
  });
  it('a work-log web search result ("external") opens the system browser even when the setting says T3 Code, ⌘ or not', async () => {
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
  it('a mailto, irc or xmpp link in a reply opens nothing, the setting and ⌘ aside', async () => {
    // The reference (measured 2026-10-11, its main process's shell.openExternal and openExternal permission hooked): the
    // anchor's click is not prevented, its `_blank` reaches setWindowOpenHandler, which opens only safe external URLs
    // (parseSafeExternalUrl: http(s) and remote editor links), and nothing opens. A table cell's link is the same anchor
    // (measured 2026-10-12 on `[the list](mailto:list@example.test)` in a cell).
    for (const [preference, modifiers] of [['app', ''], ['system', ''], ['app', 'meta']] as const) {
      const { client, rpcs, ops, native } = linkClient({ preference, modifiers });
      for (const url of ['mailto:team@example.test', 'irc://irc.example.test/t3', 'ircs://irc.example.test/t3', 'xmpp:team@example.test']) {
        await chatLocal(client, native, 'link-open', 'link', url);
      }
      expect(ops).toEqual([]);
      expect(rpcs).toEqual([]);
      expect(toasts(client)).toEqual([]);
      expect(client.threadId).toBe('thread-1');
    }
    // A web link still follows the setting.
    const { client, ops, native } = linkClient({ preference: 'system' });
    await chatLocal(client, native, 'link-open', 'link', 'https://example.com/c');
    expect(external(ops)).toEqual(['https://example.com/c']);
  });
  it('a fragment link keeps the active thread; the Markdown component handles scrolling', async () => {
    const projects = [{ id: 'p', title: 'Verification fixture', updatedAt: '2026-10-01T00:00:00Z' }, { id: 'work', title: 'work', updatedAt: '2026-10-05T00:00:00Z' }];
    const threads = [{ id: 'thread-1', projectId: 'p', updatedAt: '2026-10-03T00:00:00Z' }];
    for (const preference of ['app', 'system'] as const) {
      for (const url of ['#notes', '#summary']) {
        const { client, rpcs, ops, native } = linkClient({ preference });
        client.shell = { projects, threads, sequence: 1 };
        expect(await openLinkFromUi(client, native, 'link|', url)).toBe('');
        expect([client.threadId, client.projectId]).toEqual(['thread-1', 'p']);
        expect(ops.map(op => op.op)).toEqual([]); // the click's own modifiers ride with it
        expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
        expect(toasts(client)).toEqual([]);
      }
    }
    // ⌘, ⌥, ⇧ or ⌃ held: the handler returns before preventDefault and the shell denies the new window it asks for.
    for (const modifiers of ['meta', 'alt', 'shift', 'control', 'meta+shift']) {
      const { client, rpcs, ops, native } = linkClient();
      client.shell = { projects, threads, sequence: 1 };
      expect(await openLinkFromUi(client, native, `link|${modifiers}`, '#notes')).toBe('');
      expect([client.threadId, client.projectId]).toEqual(['thread-1', 'p']);
      expect(external(ops)).toEqual([]);
      expect(rpcs).toEqual([]);
    }
    // Only a Markdown link's fragment: a button's or a work-log result's URL is never one, and stays where it was.
    const { client, native } = linkClient();
    client.shell = { projects, threads, sequence: 1 };
    await openLinkFromUi(client, native, 'external', '#notes');
    expect(client.threadId).toBe('thread-1');
  });
  it('a link the system browser could not open says so', async () => {
    const { client } = linkClient({ preference: 'system' });
    const native: Native = { available: true, watch() {}, async later(request) { return obj(request).op === 'composerSendIntent' ? { ok: true, generation: 0, value: {} } : { ok: true, generation: 0, value: { opened: false } }; } };
    await openLinkFromUi(client, native, 'link', 'https://example.com/');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Unable to open link' });
  });
});

describe('a page’s target=_blank links (ElectronBrowserHost onOpenLink)', () => {
  const ref = { environmentId: 'local', threadId: 'thread-1' };
  function pageClient() {
    const link = linkClient(), host = browserHost(link.client);
    host.store.reconcileServerSessions(ref, { sessions: [{ ...snapshot, tabId: 'tab-src', profileId: 'work', navStatus: { _tag: 'Success', url: 'https://src.test/', title: 'Source' } }], serverEpoch: 'epoch-1', revision: 1 });
    host.listed.set(scopedThreadKey(ref), link.client.generation);
    const panel = { surfaces: [], active: browserSurfaceId('tab-src'), visible: true, userRevision: 0 };
    surfaceStore(link.client).panels.set('local:thread-1', panel);
    const requests = (list: Obj[]) => { (link.client as unknown as { presentation: Obj }).presentation = { browserTabs: { [previewRuntimeTabId(ref, 'epoch-1', 'tab-src')]: { kind: 'Success', openLinks: list } } }; };
    return { ...link, host, panel, requests };
  }
  it('opens a link as another tab of the thread under the source tab’s profile, once, and the module forgets it', async () => {
    const { client, rpcs, ops, native, host, panel, requests } = pageClient();
    requests([{ serial: 7, url: 'https://docs.test/a', background: false }]);
    await openRequestedLinks(client, native);
    await openRequestedLinks(client, native); // the same status again: nothing new
    expect(rpcs.filter(call => call.method === 'preview.open').map(call => call.payload)).toEqual([{ threadId: 'thread-1', url: 'https://docs.test/a', viewport: { _tag: 'fill' }, profileId: 'work' }]);
    expect(panel.active).toBe(browserSurfaceId('tab-1'));
    expect(host.store.read(ref).activeTabId).toBe('tab-1');
    expect(ops.filter(op => op.op === 'browserOpenLinks')).toEqual([{ op: 'browserOpenLinks', tab: previewRuntimeTabId(ref, 'epoch-1', 'tab-src'), taken: 7 }]);
    expect(external(ops)).toEqual([]);
  });
  it('a background link (⌘- or middle-click) leaves the shown tab where it was; a non-web request opens nothing', async () => {
    const { client, rpcs, native, host, panel, requests } = pageClient();
    requests([{ serial: 3, url: 'https://docs.test/b', background: true }, { serial: 4, url: 'javascript:alert(1)', background: false }]);
    await openRequestedLinks(client, native);
    expect(rpcs.filter(call => call.method === 'preview.open').map(call => call.payload.url)).toEqual(['https://docs.test/b']);
    expect(panel.active).toBe(browserSurfaceId('tab-src'));
    expect(host.store.read(ref).activeTabId).toBe('tab-src');
    expect(Object.keys(host.store.read(ref).sessions).sort()).toEqual(['tab-1', 'tab-src']);
  });
  it('the server’s "opened" event during a background open is handed back, unless the person picked a tab meanwhile', async () => {
    for (const picked of [false, true]) {
      const { client, native, host, panel, requests } = pageClient();
      const request = client.request;
      client.request = async (nativeArg, method, payload) => {
        const answer = await request(nativeArg, method, payload);
        if (method === 'preview.open') {
          host.store.applyServerEvent(ref, { type: 'opened', threadId: 'thread-1', tabId: 'tab-1', snapshot: { ...snapshot, navStatus: { _tag: 'Loading', url: String(payload.url), title: '' } }, createdAt: snapshot.updatedAt, serverEpoch: 'epoch-1', revision: 2 });
          if (picked) panel.active = 'files';
        }
        return answer;
      };
      requests([{ serial: 1, url: 'https://docs.test/c', background: true }]);
      await openRequestedLinks(client, native);
      expect(host.store.read(ref).activeTabId).toBe(picked ? 'tab-1' : 'tab-src');
    }
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

// A link's click modifiers are its press's MouseEvent's, sent with it, not the module's
// recorded gesture (T3ComposerIntent no longer reports pointers).
describe('a link press carries its own modifiers', () => {
  const fns = contractFns('press-modifiers.contract');
  const mouse = (mods: string) => ({ metaKey: mods.includes('meta'), ctrlKey: mods.includes('ctrl'), altKey: mods.includes('alt'), shiftKey: mods.includes('shift') });
  it('the window\'s linkOpen sends `kind|modifiers`', () => {
    const window = contractComponent('app-window.contract', 'T3Window');
    for (const [kind, mods, id] of [['link', '', 'link|'], ['link', 'meta', 'link|meta'], ['link', 'ctrl shift', 'link|control+shift'], ['button', 'alt', 'button|alt']]) {
      const sent: unknown[][] = [];
      window.call('linkOpen', {}, { ...fns, chatLocal: (...args: unknown[]) => sent.push(args) }, [kind, 'https://example.com/', mouse(mods!)]);
      expect(sent).toEqual([['link-open', id, 'https://example.com/']]);
    }
  });
  it('a pull request link in the text adds them as a fourth line', () => {
    const run = contractComponent('markdown.contract', 'PrLinkRun');
    for (const [mods, held] of [['', ''], ['meta', 'meta'], ['shift', 'shift']]) {
      const sent: unknown[][] = [];
      run.call('open', {}, { ...fns, local: (...args: unknown[]) => sent.push(args), chip: { detail: 'reference', target: '{}' }, run: { href: 'https://h/p/1' } }, [mouse(mods!)]);
      expect(sent).toEqual([['pr-link-open', '', `reference\n{}\nhttps://h/p/1\n${held}`]]);
    }
  });
});
