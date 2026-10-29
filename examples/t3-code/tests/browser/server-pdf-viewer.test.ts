// A server Browser tab at one of the environment's PDF assets (ServerBrowserSurface `pdfUrl`): the panel shows the PDF
// in the viewer page, which reads the signed URL with PDF.js and reaches only its origin; no frame shows the URL itself.
// The page's messages come back by the tab's runtime and place, and a link in the PDF opens in the default browser.
import { afterEach, describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { browserHost, openBrowserIn } from '../../src/browser/browser-surface';
import type { PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { panelKey, panelView, surfaceLocal, surfaceStore, type PanelState } from '../../src/panel/panel-surfaces';
import { VIEWER_PAGE, viewerFrameId } from '../../src/panel/viewer-frames';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { externalOpensForTests, resetExternalOpensForTests } from '../../src/client/external-open';

afterEach(resetExternalOpensForTests);
const threadRef = { environmentId: 'local', threadId: 'thread-1' };
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;

/** A client of the environment at 127.0.0.1:41857 whose server tab shows `url`, its panel open on that tab. */
function shown(url: string) {
  const client = {
    scopes: AUTH_STANDARD_CLIENT_SCOPES, origin: 'http://127.0.0.1:41857',
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, available: true, diffOpen: false,
    draftKey: 'local:thread-1', presentation: {} as Obj, config: { environment: { capabilities: {} } }, shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/p' }] },
    local: { clientSettings: {}, deviceSettings: {}, composerControls: false },
    async rpc() { return {}; }, async raw() { return { ok: true, generation: 0, value: {} }; },
  } as unknown as T3Client;
  const snapshot: PreviewSessionSnapshot = { threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Success', url, title: 'guide.pdf' },
    canGoBack: false, canGoForward: false, updatedAt: '2026-10-30T00:00:00.000Z', runtime: 'server' };
  browserHost(client).store.applyServerSnapshot(threadRef, snapshot);
  const state: PanelState = { surfaces: [], active: '', visible: true, userRevision: 0 };
  openBrowserIn(state, 'tab-1', 'local:thread-1');
  surfaceStore(client).panels.set(panelKey(client), state);
  return client;
}

describe("a server tab's PDF", () => {
  it('is the viewer page reading its signed URL, whatever origin the tab names', async () => {
    for (const [url, signed, scope] of [
      ['http://127.0.0.1:3773/api/assets/token/guide.pdf', 'http://127.0.0.1:41857/api/assets/token/guide.pdf', 'http://127.0.0.1:41857'],
      ['https://other.example/api/assets/token/guide.pdf', 'https://other.example/api/assets/token/guide.pdf', 'https://other.example'],
    ]) {
      const browser = (await panelView(shown(url!), module, 0)).browser;
      const config = JSON.parse(browser.pdfViewer.config) as Obj;
      // One frame per runtime and origin: its page narrows its connections to the URL's origin once.
      expect([browser.pdfUrl, browser.pdfViewer.src, config.view, config.src, config.name, browser.pdfViewer.id])
        .toEqual([signed, VIEWER_PAGE, 'pdf', signed, 'guide.pdf', viewerFrameId(`server-pdf-tab|${browser.runtimeId}|${scope}`)]);
    }
    // A page that is no PDF asset has none.
    expect((await panelView(shown('https://example.com/paper.pdf'), module, 0)).browser.pdfViewer.src).toBe('');
  });

  it("hears its page by the tab's runtime: the config after a boot, and a link opened outside the app", async () => {
    const client = shown('http://127.0.0.1:3773/api/assets/token/guide.pdf');
    const browser = (await panelView(client, module, 0)).browser;
    await surfaceLocal(client, module, 'server-pdf-viewer:{"type":"boot"}', browser.runtimeId, 'tab');
    await surfaceLocal(client, module, 'server-pdf-viewer:{"type":"link","url":"https://example.com/cited"}', browser.runtimeId, 'tab');
    expect(externalOpensForTests()).toEqual(['https://example.com/cited']);
    // Try again mounts the viewer afresh: its config's key changes.
    const before = (JSON.parse((await panelView(client, module, 0)).browser.pdfViewer.config) as Obj).key;
    await surfaceLocal(client, module, 'server-pdf-viewer:{"type":"retry"}', browser.runtimeId, 'tab');
    expect((JSON.parse((await panelView(client, module, 0)).browser.pdfViewer.config) as Obj).key).not.toBe(before);
  });

  it('is never an iframe of the URL itself', () => {
    const contract = readFileSync(join(import.meta.dir, '../../src/browser/server-browser.contract'), 'utf8');
    expect([contract.includes('MediaFrame'), /iframe[^\n]*pdfUrl/.test(contract), contract.includes('ViewerFrame(frame=pdfViewer')]).toEqual([false, false, true]);
    const frame = readFileSync(join(import.meta.dir, '../../src/media/media-frame.contract'), 'utf8');
    expect(frame.match(/^\s*iframe /gm)?.length).toBe(1);
    expect(frame).toContain('sandbox="allow-scripts allow-forms allow-popups allow-downloads"');
  });
});
