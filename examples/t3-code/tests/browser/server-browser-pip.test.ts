// A server Browser tab in the platform's picture-in-picture window (T3 Code v0.1.5, MIT, see LICENSE-T3:
// apps/web/src/browser/serverPictureInPicture.ts and ThreadPreviewMiniPlayer.tsx BrowserMiniPlayer
// `toggleNativePictureInPicture`, `canPopOut`). The floating player's pill pops the tab out and closes it again; the window
// has a stream of its own that outlives the player; the window's close, a refused stream and a PDF end it.
import { describe, expect, it } from 'bun:test';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { browserHost } from '../../src/browser/browser-surface';
import type { PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { panelKey, panelView, surfaceLocal, surfaceStore, type PanelState } from '../../src/panel/panel-surfaces';
import { miniStoreOf } from '../../src/device/device-session';
import { serverPipOf, serverPipRuntimeId } from '../../src/browser/server-browser-pip';
import { prepareServerStream, serverStreamUrl } from '../../src/browser/server-browser-runtime';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { contractComponent, contractFns } from '../support/contract-action-fixture';

const threadRef = { environmentId: 'local', threadId: 'thread-1' };
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
const members = contractComponent('src/browser/server-browser-members.contract', 'T3Code');
const pipImage = contractComponent('src/browser/server-browser.contract', 'ServerBrowserPip');
const player = contractComponent('src/browser/browser-capture.contract', 'BrowserMiniPlayer');

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

  it("ends on a PDF in the floating tab's thread whichever thread is shown", async () => {
    const client = floating();
    const runtimeId = (await panelView(client, module, 0)).browserMini.runtimeId;
    await surfaceLocal(client, module, 'browser-mini-window', runtimeId, 'server');
    Object.assign(client, { threadId: 'thread-2', draftKey: 'local:thread-2' });
    navigate(client, 'http://127.0.0.1:3773/api/assets/token/guide.pdf');
    await panelView(client, module, 0);
    expect(serverPipOf(client)).toBeNull();
  });

  it("connects as the reference's picture-in-picture client: watching, at 1280 × 1280", async () => {
    const client = { connection: 'connected', generation: 1, local: {}, ids: async () => ['frames'], call: async () => ({ wsBase: 'ws://127.0.0.1:1', ticket: 't' }) } as unknown as T3Client;
    await prepareServerStream(client, module, 'rt:pip', 'thread-1', 'tab-1', true);
    await prepareServerStream(client, module, 'rt', 'thread-1', 'tab-1');
    const query = (id: string) => Object.fromEntries(new URL(serverStreamUrl(client, id)).searchParams);
    expect(query('rt:pip')).toMatchObject({ maxWidth: '1280', maxHeight: '1280', interactive: 'false' });
    expect(query('rt')).toMatchObject({ maxWidth: '2560', maxHeight: '1600' });
    expect(query('rt').interactive).toBeUndefined();
  });

  it('is floated by `requestPictureInPicture` over a hidden image its own stream paints (LLP 1042 §9)', () => {
    // The members' task floats the image by its id.
    const floated: unknown[][] = [];
    members.call('streamPipFloatNow', {}, { requestPictureInPicture: (...args: unknown[]) => floated.push(args) }, []);
    expect(floated).toEqual([['server-browser-pip']]);
    // The image shows the seed until the window's stream has a picture, then the stream's; with neither, no image.
    const pip = { show: true, runtimeId: 'rt:pip', streamUrl: '', seed: 'app:/tmp/seed.jpg', request: 1 }, image = 'id="server-browser-pip"';
    const frame = (runtimeId: string, picture: string) => ({ serverPipFrame: { runtimeId, image: picture } });
    expect(pipImage.drawn(image, {}, { pip, ...frame('', '') })).toBe(true);
    expect(pipImage.drawn(image, {}, { pip: { ...pip, seed: '' }, ...frame('', '') })).toBe(false);
    expect(pipImage.drawn(image, {}, { pip: { ...pip, seed: '' }, ...frame('rt:pip', 'app:/tmp/1.jpg') })).toBe(true);
    // A painted picture is acknowledged to its stream; the window's close is the panel's `pip-left`.
    const painted: unknown[][] = [], said: unknown[][] = [];
    const scope = { pip, ...frame('rt:pip', 'app:/tmp/1.jpg'), streamPainted: (...args: unknown[]) => painted.push(args), local: (...args: unknown[]) => said.push(args) };
    pipImage.call('painted', {}, scope, []);
    pipImage.call('left', {}, scope, []);
    expect([painted, said]).toEqual([[['rt:pip', 'app:/tmp/1.jpg']], [['surface-browser-pip-left', 'rt:pip', '']]]);
  });

  it("offers the pill's window button as canPopOut does", () => {
    const button = 'icon="picture-in-picture-2"', fns = contractFns('src/browser/browser-capture.contract');
    const shown = (mini: Record<string, unknown>) => player.drawn(button, player.state(), { ...fns, rem: 16, framed: false, canvasWidth: 800,
      frame: { x: 0, width: 300, pillInset: 8 }, mini: { show: true, server: true, separateWindow: false, pdfUrl: '', recording: false, ...mini } });
    expect(shown({})).toBe(true);
    expect(shown({ pdfUrl: 'https://env/api/assets/t/guide.pdf' })).toBe(false);
    // Popped out, the button stays, to close the window; a local tab's is its separate window.
    expect(shown({ pdfUrl: 'https://env/api/assets/t/guide.pdf', separateWindow: true })).toBe(true);
    expect(shown({ server: false, pdfUrl: 'https://env/api/assets/t/guide.pdf' })).toBe(true);
  });
});
