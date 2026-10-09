// browser-surface part 3 (capture): the clone's own rows for the chrome row's Annotate, Capture and Float preview, the
// More menu's separate window, their toasts and the floating player's browser source (browser-capture.ts), each named
// after the reference code it follows (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: PreviewView.tsx handlePickElement,
// handleCapture, handlePictureInPicture, handleNativePictureInPicture; PreviewChromeRow.tsx; ThreadPreviewMiniPlayer.tsx;
// ChatView.tsx closePreviewPanel and shouldRenderPreviewMiniPlayer). The module is a fake answering the capture ops as
// T3BrowserCapture.swift does; PreviewView.test.tsx's annotation rows ("forwards Cmd/Ctrl+Enter annotations to the
// composer send path", "warns when main dropped the crop before handing over the pick") are followed here.
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it } from 'bun:test';
import { applyCaptureResults, artifactLocal, browserCaptureView, browserMiniView, captureHost, captureLocal, floatingTabOf } from './browser-capture';
import { browserMiniSessions, browserHost, browserView } from './browser-surface';
import { previewRuntimeTabId } from './browser-state';
import { panelKey, panelState, surfaceLocal, surfaceStore, type PanelState } from './r4-surfaces-panel';
import { miniStoreOf } from './r6-media-device';
import { toasts } from './toast';
import { shellState, toastViews, copiedActions, copiedToasts } from './shell';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';

const ref = { environmentId: 'local', threadId: 'thread-1' };
const threadKey = 'local:thread-1';
const runtimeId = previewRuntimeTabId(ref, 'epoch-1', 'tab-1');
type Call = Obj & { op: string };

/** The module's capture ops (T3BrowserSessions.performCapture), answered from `handlers`. */
function fakeModule(handlers: Record<string, (request: Call) => Obj | Promise<Obj>>) {
  const calls: Call[] = [];
  const native: Native = { available: true, watch() {}, later: async request => {
    const call = request as Call;
    calls.push(call);
    const key = call.action ? `${call.op}:${call.action}` : call.op;
    const handler = handlers[key] ?? handlers[call.op];
    if (!handler) return { ok: true, generation: 0, value: {} };
    try { return { ok: true, generation: 0, value: await handler(call) }; }
    catch (error) { return { ok: false, generation: 0, error: { kind: 'Fake', message: (error as Error).message } }; }
  } };
  return { native, calls, ops: () => calls.map(call => (call.action ? `${call.op}:${call.action}` : call.op)) };
}
function fakeClient(tab: Obj = { kind: 'Success', url: 'https://example.com/', title: 'Example' }) {
  return {
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, diffOpen: false, draftKey: threadKey,
    presentation: { browserTabs: { [runtimeId]: tab } } as Obj, config: { environment: { capabilities: {} } }, shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [] },
    local: { clientSettings: {}, deviceSettings: {}, drafts: {} as Record<string, string>, snapshotDrafts: {} as Record<string, Obj[]> },
    get snapshotDrafts() { return (this.local.snapshotDrafts as Record<string, Obj[]>)[threadKey] ?? []; },
    get draft() { return (this.local.drafts as Record<string, string>)[threadKey] ?? ''; },
    async rpc() { return {}; }, async raw() { return { ok: true, generation: 0, value: {} }; },
  } as unknown as T3Client;
}
const target = { ref, tabId: 'tab-1', runtimeId };
const browserPanel = (): PanelState => ({ surfaces: [{ id: 'browser:tab-1', kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId: 'tab-1', threadKey } }], active: 'browser:tab-1', visible: true, userRevision: 0 });
const annotation = {
  id: 'annotation_1', pageUrl: 'https://example.com/', pageTitle: 'Example', comment: 'Tighten this spacing', elements: [], regions: [{ id: 'region_1', rect: { x: 1, y: 2, width: 30, height: 40 } }],
  strokes: [], styleChanges: [], screenshot: null, createdAt: '2026-07-27T00:00:00.000Z',
};
/** The module's sleep, scaled down: a timeout still loses to an answer that comes at once. */
const timelineSleep = (request: Call) => new Promise<Obj>(resolve => setTimeout(() => resolve({}), Math.min(Number(request.ms) || 0, 40)));
const crop = { id: '0f8c1b52-9a4c-4d2e-8a0b-3c2d1e0f9a8b', name: 'preview-annotation-annotation_1.png', sizeBytes: 812, width: 120, height: 80, cropRect: { x: 0, y: 0, width: 60, height: 40 } };

describe('PreviewChromeRow: Annotate, Capture and Float preview', () => {
  it('names Annotate after its state and gives a failed page its reason', () => {
    const client = fakeClient();
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false)).toMatchObject({ pickActive: false, pickDisabled: false, pickTip: 'Annotate elements, regions, and drawings',
      captureDisabled: false, recording: false, floating: false, floatDisabled: false, separateWindow: false });
    (client.presentation.browserTabs as Obj)[runtimeId] = { kind: 'Success', pick: { active: true, serial: 0, ready: false }, pip: true };
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false)).toMatchObject({ pickActive: true, pickTip: 'Cancel annotation (Esc)', separateWindow: true });
    // pickDisabled: a failed load covers the page; captureDisabled and floatDisabled: no web contents, or a failed load.
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, true)).toMatchObject({ pickDisabled: true, pickTip: 'Page didn’t load — pick unavailable until the page renders', captureDisabled: true, floatDisabled: true });
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, false, false)).toMatchObject({ captureDisabled: true, floatDisabled: true, pickDisabled: false });
    expect(browserCaptureView(client, ref, '', runtimeId, true, false).pickDisabled).toBe(true);
  });

  it('handlePickElement: a press starts the overlay in the app\'s scheme, a second press cancels it', async () => {
    const client = fakeClient(), module = fakeModule({});
    await captureLocal(client, module.native, browserPanel(), 'annotate', target, 'dark');
    expect(module.calls[0]).toMatchObject({ op: 'browserAnnotate', tab: runtimeId, action: 'start', theme: { colorScheme: 'dark', primary: '#346bf1' } });
    (client.presentation.browserTabs as Obj)[runtimeId] = { kind: 'Success', pick: { active: true, serial: 0, ready: false } };
    await captureLocal(client, module.native, browserPanel(), 'annotate', target, 'dark');
    expect(module.ops()).toEqual(['browserAnnotate:start', 'browserAnnotate:cancel']);
  });
});

describe('Annotate\'s result (handlePickElement → addPreviewAnnotation)', () => {
  let client: T3Client;
  beforeEach(() => { client = fakeClient({ kind: 'Success', pick: { active: false, serial: 1, ready: true } }); });

  it('puts the chip at the composer\'s caret and the crop on the shelf, once per pick', async () => {
    const inserted: string[] = [];
    const module = fakeModule({
      'browserAnnotate:take': () => ({ result: { annotation, submission: 'attach', screenshotFailed: false, screenshot: crop }, serial: 1 }),
      editorState: () => ({ text: inserted.join('') }), editorInsert: request => { inserted.push(String(request.text)); return { applied: true }; },
    });
    await applyCaptureResults(client, module.native, [{ runtimeId, threadKey }]);
    expect(inserted).toEqual(['[Tighten this spacing](t3-context://v1/preview-annotation/preview-annotation_annotation_1)']);
    expect(client.snapshotDrafts).toEqual([{ id: crop.id, name: crop.name, mimeType: 'image/png', sizeBytes: 812 }]);
    expect(module.ops()).toContain('editorFocus'); // the focus goes back to the composer, as previouslyFocused.focus()
    expect(toasts(client)).toEqual([]);
    await applyCaptureResults(client, module.native, [{ runtimeId, threadKey }]);
    expect(module.ops().filter(op => op === 'browserAnnotate:take')).toHaveLength(1);
    expect(module.calls.find(call => call.action === 'applied')).toMatchObject({ serial: 1, outcome: 'chip+image' });
    expect(captureHost(client).sendSerial).toBe(0);
  });

  it('goes to a new thread\'s draft composer, whose tab names the id the draft was given', async () => {
    const draft = fakeClient({ kind: 'Success', pick: { active: false, serial: 1, ready: true } });
    Object.assign(draft, { threadId: '', draftKey: 'local:new:p1' });
    (draft.local as Obj).composerControls = { draftThreads: { 'local:new:p1': 'thread-1' } };
    const inserted: string[] = [];
    const module = fakeModule({ 'browserAnnotate:take': () => ({ result: { annotation, submission: 'attach' }, serial: 1 }), editorInsert: request => { inserted.push(String(request.text)); return { applied: true }; } });
    await applyCaptureResults(draft, module.native, [{ runtimeId, threadKey }]);
    expect(inserted).toHaveLength(1);
    expect(module.calls.at(-1)).toMatchObject({ op: 'browserAnnotate', action: 'applied', serial: 1, outcome: 'chip' });
    // Float preview keys the player by the same thread.
    await captureLocal(draft, module.native, browserPanel(), 'float', target, '');
    expect(floatingTabOf(draft)).toBe('tab-1');
  });

  it('warns when main dropped the crop before handing over the pick, and keeps the annotation', async () => {
    const module = fakeModule({ 'browserAnnotate:take': () => ({ result: { annotation, submission: 'attach', screenshotFailed: true }, serial: 1 }), editorInsert: () => ({ applied: false }) });
    await applyCaptureResults(client, module.native, [{ runtimeId, threadKey }]);
    expect(client.snapshotDrafts).toEqual([]);
    expect(client.draft).toBe('[Tighten this spacing](t3-context://v1/preview-annotation/preview-annotation_annotation_1) ');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not capture the picked element', 'The annotation was kept without the screenshot.']]);
  });

  it('forwards Cmd/Ctrl+Enter annotations to the composer send path', async () => {
    const module = fakeModule({ 'browserAnnotate:take': () => ({ result: { annotation, submission: 'send' }, serial: 1 }), editorInsert: () => ({ applied: true }) });
    await applyCaptureResults(client, module.native, [{ runtimeId, threadKey }]);
    expect(captureHost(client).sendSerial).toBe(1);
  });

  it('drops a cancelled pick and a payload the validator refuses', async () => {
    const module = fakeModule({ 'browserAnnotate:take': () => ({ result: { annotation: { ...annotation, screenshot: { dataUrl: 'from the page' } }, submission: 'attach' }, serial: 1 }) });
    await applyCaptureResults(client, module.native, [{ runtimeId, threadKey }]);
    expect(client.draft).toBe('');
    const cancelled = fakeClient({ kind: 'Success', pick: { active: false, serial: 2, ready: true } });
    const quiet = fakeModule({ 'browserAnnotate:take': () => ({ result: { cancelled: true }, serial: 2 }) });
    await applyCaptureResults(cancelled, quiet.native, [{ runtimeId, threadKey }]);
    expect(quiet.ops()).toEqual(['browserAnnotate:take', 'browserAnnotate:applied']);
    expect(cancelled.draft).toBe('');
  });
});

describe('handleCapture', () => {
  it('saves a screenshot and offers Copy path, Reveal in Finder and Copy image; a copy reads "Copied!" for 2 s', async () => {
    const client = fakeClient();
    const path = '/lane/t3-home/userdata/browser-artifacts/browser-screenshot-example-com-abc.png';
    const module = fakeModule({ browserScreenshot: () => ({ id: 'browser-screenshot-example-com-abc', path, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }) });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, '');
    const [toast] = toasts(client);
    expect(toast).toMatchObject({ kind: 'success', title: 'Screenshot saved', stacked: true, secondaryVariant: 'outline' });
    expect(toast?.action).toEqual({ label: 'Copy image', op: 'shelllocal:surface-browser-artifact-copy-image', id: path, value: String(toast?.id) });
    expect(toast?.extra).toEqual({ label: 'Copy path', op: 'shelllocal:surface-browser-artifact-copy-path', id: path, value: String(toast?.id) });
    expect(toast?.secondary).toEqual({ label: 'Reveal in Finder', op: 'shelllocal:surface-browser-artifact-reveal', id: path, value: String(toast?.id) });
    shellState(client).now = 10_000;
    await artifactLocal(client, module.native, 'copy-image', path, String(toast?.id));
    expect(module.calls.at(-1)).toMatchObject({ op: 'browserArtifact', action: 'copy-image', path });
    let [view] = toastViews(toasts(client), copiedToasts(client, 10_000), copiedActions(client));
    expect([view?.actionLabel, view?.actionDone, view?.extraLabel, view?.extraDone, view?.secondaryLabel]).toEqual(['Copied!', true, 'Copy path', false, 'Reveal in Finder']);
    shellState(client).now = 12_000;
    [view] = toastViews(toasts(client), copiedToasts(client, 12_000), copiedActions(client));
    expect([view?.actionLabel, view?.actionDone]).toEqual(['Copy image', false]);
  });

  it('reports a failed capture and a failed copy in the reference\'s words', async () => {
    const client = fakeClient();
    const module = fakeModule({ browserScreenshot: () => { throw new Error('The page could not be captured.'); }, browserArtifact: () => { throw new Error('Preview artifact path is outside /x'); } });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, '');
    expect(toasts(client).map(toast => [toast.title, toast.description])).toEqual([['Unable to capture screenshot', 'The page could not be captured.']]);
    await artifactLocal(client, module.native, 'copy-path', '/x/browser-recording-1.mp4', '');
    expect(toasts(client).at(-1)?.title).toBe('Unable to copy recording path');
  });

  it('records on a Shift-click and stops on the next press, saving the recording with Reveal in Finder and Copy path', async () => {
    const client = fakeClient();
    (client.local.clientSettings as Obj).browserRecordingFrameRate = 60;
    const encoded = '/var/folders/x/T/t3-recording-1.mp4';
    const module = fakeModule({
      timelineSleep, 'browserRecord:arm': () => ({ armed: true }), 'browserRecord:capture': request => ({ width: 1280, height: 800, frameRate: request.frameRate }),
      'browserRecord:begin': () => ({ mimeType: 'video/mp4;codecs=avc1' }), 'browserRecord:disarm': () => ({ armed: false }),
      'browserRecord:finish': () => ({ path: encoded, mimeType: 'video/mp4;codecs=avc1', sizeBytes: 4096, frames: 90 }), 'browserRecord:release': () => ({}),
      'browserRecord:save': request => ({ id: 'browser-recording-x', tabId: request.tab, path: '/lane/t3-home/userdata/browser-artifacts/browser-recording-x.mp4', mimeType: request.mimeType, sizeBytes: 4096, createdAt: '2026-10-09T00:00:00.000Z' }),
    });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, 'record');
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false).recording).toBe(true);
    expect(module.calls.find(call => call.action === 'capture')?.frameRate).toBe(60);
    expect(module.calls.find(call => call.action === 'begin')).toMatchObject({ mimeType: 'video/mp4;codecs=avc1', bitsPerSecond: 3_072_000 });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, '');
    expect(module.ops().filter(op => op.startsWith('browserRecord:'))).toEqual(['browserRecord:arm', 'browserRecord:capture', 'browserRecord:begin', 'browserRecord:disarm', 'browserRecord:finish', 'browserRecord:release', 'browserRecord:save']);
    expect(module.calls.at(-1)).toMatchObject({ action: 'save', path: encoded, mimeType: 'video/mp4;codecs=avc1' });
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false).recording).toBe(false);
    const toast = toasts(client).at(-1);
    expect(toast).toMatchObject({ kind: 'success', title: 'Recording saved', stacked: true });
    expect([toast?.secondary?.label, toast?.action?.label]).toEqual(['Copy path', 'Reveal in Finder']);
  });

  it('reports a recording that cannot start, and turns decorations on only when a setting asks', async () => {
    const client = fakeClient();
    const module = fakeModule({ timelineSleep, 'browserRecord:arm': () => { throw new Error('The page could not be captured.'); } });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, 'record');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Unable to start recording' });
    const decorated = fakeClient();
    Object.assign(decorated.local.clientSettings as Obj, { browserRecordingShowKeyPresses: true });
    const ok = fakeModule({ timelineSleep, 'browserRecord:capture': () => ({ width: 800, height: 600, frameRate: 30 }), 'browserRecord:decorate': () => ({ active: true }), 'browserRecord:begin': () => ({ mimeType: 'video/mp4;codecs=avc1' }) });
    await captureLocal(decorated, ok.native, browserPanel(), 'capture', target, 'record');
    expect(ok.calls.find(call => call.action === 'decorate')).toMatchObject({ showKeyPresses: true, showMousePresses: false, primaryColor: '#1b4ed8' });
    expect(ok.calls.find(call => call.action === 'arm')).toMatchObject({ showKeyPresses: true, showMousePresses: false });
  });
});

describe('the floating player and the separate window', () => {
  it('handlePictureInPicture floats the tab and closes the panel; pressing again closes the player', async () => {
    const client = fakeClient(), state = browserPanel(), module = fakeModule({});
    await captureLocal(client, module.native, state, 'float', target, '');
    expect(floatingTabOf(client)).toBe('tab-1');
    expect(state.visible).toBe(false);
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false).floating).toBe(true);
    await captureLocal(client, module.native, state, 'float', target, '');
    expect(floatingTabOf(client)).toBeNull();
  });

  it('shows the floating tab unless the panel shows it (shouldRenderPreviewMiniPlayer), sized by its page', () => {
    const client = fakeClient({ kind: 'Success', url: 'https://example.com/', size: [640, 360], pip: true });
    browserHost(client).store.applyServerSnapshot(ref, { threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-09T00:00:00.000Z' });
    browserHost(client).store.reconcileServerSessions(ref, { sessions: [{ threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-09T00:00:00.000Z' }], serverEpoch: 'epoch-1', revision: 1 });
    miniStoreOf(client).open('thread-1', { kind: 'browser', tabId: 'tab-1' });
    const view = browserMiniView(client, scoped => browserMiniSessions(client, scoped), null);
    expect(view).toMatchObject({ show: true, key: 'browser:tab-1', tabId: 'tab-1', runtimeId, url: 'https://example.com/', hasWebContents: true, separateWindow: true, width: 640, height: 360 });
    expect(browserMiniView(client, scoped => browserMiniSessions(client, scoped), 'tab-1').show).toBe(false);
  });

  it('closing the panel on a Browser tab floats it (closePreviewPanel)', async () => {
    const client = fakeClient(), state = browserPanel(), module = fakeModule({});
    surfaceStore(client).panels.set(panelKey(client), state);
    await surfaceLocal(client, module.native, 'hide', '', '');
    expect(panelState(client).visible).toBe(false);
    expect(floatingTabOf(client)).toBe('tab-1');
  });

  it('the pill\'s Open in right panel and Close (a new thread\'s draft too)', async () => {
    for (const draft of [false, true]) {
      const client = fakeClient(), module = fakeModule({});
      if (draft) {
        Object.assign(client, { threadId: '', draftKey: 'local:new:p1' });
        (client.local as Obj).composerControls = { draftThreads: { 'local:new:p1': 'thread-1' } };
      }
      const state = panelState(client);
      Object.assign(state, browserPanel());
      await captureLocal(client, module.native, state, 'float', target, '');
      expect([floatingTabOf(client), panelState(client).visible]).toEqual(['tab-1', false]);
      await surfaceLocal(client, module.native, 'browser-mini-restore', runtimeId, '');
      expect([floatingTabOf(client), panelState(client).visible, panelState(client).active]).toEqual([null, true, 'browser:tab-1']);
      await captureLocal(client, module.native, panelState(client), 'float', target, '');
      await surfaceLocal(client, module.native, 'browser-mini-close', runtimeId, '');
      expect([floatingTabOf(client), panelState(client).visible]).toEqual([null, false]);
    }
  });

  it('the pill sits inside the handle\'s hover box, as the reference\'s group (no hover hand-off that flips every frame)', () => {
    // ThreadPreviewMiniPlayer: the pill is a child of the handle's `group`, so moving from the handle onto the pill
    // keeps one hover. Two sibling hover boxes hand the hover over through a state where neither is hovered: the pill
    // hides, refuses the pointer, the handle is hit again and shows it (drive 3: ~110 renders a second, an untappable pill).
    const lines = readFileSync(new URL('./browser-capture.contract', import.meta.url), 'utf8').split('\n');
    const indent = (line: string) => line.length - line.trimStart().length;
    const handle = lines.findIndex(line => line.includes('testId="browser-mini-handle"'));
    const pill = lines.findIndex(line => line.includes('testId="browser-mini-pill"'));
    expect(lines[handle]).toContain('hover=');
    expect(pill).toBeGreaterThan(handle);
    expect(lines.slice(handle + 1, pill).every(line => !line.trim() || indent(line) > indent(lines[handle]!))).toBe(true);
    expect(indent(lines[pill]!)).toBeGreaterThan(indent(lines[handle]!));
    expect(lines[pill]).not.toContain('hover=');
  });

  it('handleNativePictureInPicture opens and closes the separate window, and the More menu names it', async () => {
    const client = fakeClient(), module = fakeModule({});
    await captureLocal(client, module.native, browserPanel(), 'window', target, '');
    (client.presentation.browserTabs as Obj)[runtimeId] = { kind: 'Success', pip: true };
    await captureLocal(client, module.native, browserPanel(), 'window', target, '');
    expect(module.calls.map(call => call.action)).toEqual(['open', 'close']);
    expect(browserView(client, browserPanel().surfaces[0]!).capture.separateWindow).toBe(false); // no session: the empty view
  });
});
