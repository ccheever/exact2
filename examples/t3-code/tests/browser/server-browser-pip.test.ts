// A server Browser tab in the platform's picture-in-picture window (T3 Code v0.1.5, MIT, see LICENSE-T3:
// apps/web/src/browser/serverPictureInPicture.ts and ThreadPreviewMiniPlayer.tsx BrowserMiniPlayer
// `toggleNativePictureInPicture`, `canPopOut`). The floating player's pill pops the tab out and closes it again; the window
// has a stream of its own that outlives the player; the window's close, a refused stream and a PDF end it.
import { describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { browserHost } from '../../src/browser/browser-surface';
import type { PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { panelKey, panelView, surfaceLocal, surfaceStore, type PanelState } from '../../src/panel/panel-surfaces';
import { miniStoreOf } from '../../src/device/device-session';
import { serverPipOf, serverPipRuntimeId } from '../../src/browser/server-browser-pip';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

const threadRef = { environmentId: 'local', threadId: 'thread-1' };
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
const read = (file: string) => readFileSync(join(import.meta.dir, '../../src/browser', file), 'utf8');

/** A client whose server tab at `url` floats over the chat, its panel closed. */
function floating(url = 'https://example.com/') {
  const client = {
    scopes: AUTH_STANDARD_CLIENT_SCOPES, origin: 'http://127.0.0.1:41857',
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, available: true, diffOpen: false,
    draftKey: 'local:thread-1', presentation: {} as Obj, config: { environment: { capabilities: {} } }, shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/p' }] },
    local: { clientSettings: {}, deviceSettings: {}, composerControls: false },
    async rpc() { return {}; }, async raw() { return { ok: true, generation: 0, value: {} }; },
  } as unknown as T3Client;
  navigate(client, url);
  const state: PanelState = { surfaces: [], active: '', visible: false, userRevision: 0 };
  surfaceStore(client).panels.set(panelKey(client), state);
  miniStoreOf(client).open('thread-1', { kind: 'browser', tabId: 'tab-1' });
  return client;
}
function navigate(client: T3Client, url: string) {
  const snapshot: PreviewSessionSnapshot = { threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Success', url, title: 'Page' },
    canGoBack: false, canGoForward: false, updatedAt: new Date().toISOString(), runtime: 'server' };
  browserHost(client).store.applyServerSnapshot(threadRef, snapshot);
}

describe("a server tab's picture-in-picture window", () => {
  it('the pill pops the tab into the window and closes it again', async () => {
    const client = floating();
    const before = await panelView(client, module, 0), runtimeId = before.browserMini.runtimeId;
    expect([before.browserMini.server, before.browserMini.separateWindow, before.frames.pip.show]).toEqual([true, false, false]);
    await surfaceLocal(client, module, 'browser-mini-window', runtimeId, 'server');
    const open = await panelView(client, module, 0);
    // Its own stream, beside the player's, and a request the members' task floats it by.
    expect(open.browserMini.separateWindow).toBe(true);
    expect(open.frames.pip).toMatchObject({ show: true, runtimeId: serverPipRuntimeId(runtimeId) });
    expect(open.frames.pip.request).toBeGreaterThan(0);
    await surfaceLocal(client, module, 'browser-mini-window', runtimeId, 'server');
    const closed = await panelView(client, module, 0);
    expect([closed.browserMini.separateWindow, closed.frames.pip.show]).toEqual([false, false]);
  });

  it('outlives the floating player, and ends with its own close', async () => {
    const client = floating();
    const runtimeId = (await panelView(client, module, 0)).browserMini.runtimeId;
    await surfaceLocal(client, module, 'browser-mini-window', runtimeId, 'server');
    await surfaceLocal(client, module, 'browser-mini-close', runtimeId, '');
    const view = await panelView(client, module, 0);
    expect([view.browserMini.show, view.frames.pip.show]).toEqual([false, true]);
    await surfaceLocal(client, module, 'browser-pip-left', serverPipRuntimeId(runtimeId), '');
    expect(serverPipOf(client)).toBeNull();
    expect((await panelView(client, module, 0)).frames.pip.show).toBe(false);
  });

  it('closes when its tab shows a PDF, which renders in the client and not in the stream', async () => {
    const client = floating();
    const runtimeId = (await panelView(client, module, 0)).browserMini.runtimeId;
    await surfaceLocal(client, module, 'browser-mini-window', runtimeId, 'server');
    navigate(client, 'http://127.0.0.1:3773/api/assets/token/guide.pdf');
    const view = await panelView(client, module, 0);
    expect(view.frames.pip.show).toBe(false);
    // canPopOut: a PDF's player offers no window.
    expect(view.browserMini.pdfUrl).not.toBe('');
  });

  it('is floated by `requestPictureInPicture` over a hidden image of its stream (LLP 1042 §9)', () => {
    const capture = read('browser-capture.contract'), page = read('server-browser.contract'), members = read('server-browser-members.contract');
    expect(capture).toContain('derive canPopOut = not mini.server or mini.separateWindow or mini.pdfUrl == ""');
    expect(capture).toContain('press=local("surface-browser-mini-window", mini.runtimeId, mini.server ? "server" : "")');
    expect(page).toMatch(/image \(streamed \? serverPipFrame\.image : pip\.seed\) id="server-browser-pip"[^\n]* leavepictureinpicture=left/);
    expect(members).toContain('requestPictureInPicture("server-browser-pip")');
  });
});
