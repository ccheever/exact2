// browser-surface part 3 (capture): the clone's own rows for the chrome row's Annotate, Capture and Float preview, the
// More menu's separate window, their toasts and the floating player's browser source (browser-capture.ts), each named
// after the reference code it follows (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: PreviewView.tsx handlePickElement,
// handleCapture, handlePictureInPicture, handleNativePictureInPicture; PreviewChromeRow.tsx; ThreadPreviewMiniPlayer.tsx;
// ChatView.tsx closePreviewPanel and shouldRenderPreviewMiniPlayer). The module is a fake answering the capture ops as
// T3BrowserCapture.swift does; PreviewView.test.tsx's annotation rows ("forwards Cmd/Ctrl+Enter annotations to the
// composer send path", "warns when main dropped the crop before handing over the pick") are followed here.
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it } from 'bun:test';
import { applyCaptureResults, artifactLocal, browserCaptureView, browserMiniView, cancelHiddenPicks, captureHost, captureLocal, emptyCaptureView, floatingTabOf } from './browser-capture';
import { browserMiniSessions, browserHost, browserView } from './browser-surface';
import { previewRuntimeTabId } from './browser-state';
import { panelKey, panelState, panelView, surfaceLocal, surfaceStore, type PanelState } from './r4-surfaces-panel';
import { miniStoreOf } from './r6-media-device';
import { toasts } from './toast';
import { shellState, toastViews, copiedActions, copiedToasts } from './shell';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { obj, type Obj } from './domain';
import { T3Client as Client } from './client';
import { connected, opened, running } from './composer-controls-fixture';
import { annotationSendMarked, markAnnotationSend, takeAnnotationSend } from './browser-annotation';
import { snapshot } from './presentation';
import { DEFAULT_SEND_RULES } from './composer-editor-intent';
import { activeRef } from './terminal-drawer-view';
import { chatCanvasView } from './chat-canvas-view';
import { contractFn } from './contract-fn-fixture';

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

describe('⌘↩ in Annotate\'s editor (PickPreload: the page has the key)', () => {
  it('while Annotate is on in the thread\'s Browser tab, the send button leaves Meta+Enter to the page', async () => {
    const { client } = await opened();
    client.config.keybindings = DEFAULT_SEND_RULES;
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
    const ref = activeRef(client)!, tab = previewRuntimeTabId(ref, 'epoch-1', 'tab-1');
    (client.presentation as Obj).browserTabs = { [tab]: { kind: 'Success', pick: { active: true, serial: 1, ready: true } } };
    expect(snapshot(client).composer.sendChords).toBe('Meta+Alt+Enter');
    // Another thread's overlay, or one that has closed, leaves the shortcut alone.
    (client.presentation as Obj).browserTabs = { [previewRuntimeTabId({ ...ref, threadId: 'other' }, 'epoch-1', 'tab-1')]: { pick: { active: true } }, [tab]: { pick: { active: false } } };
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
  });
  // ChatView's onSendAnnotation: onSend(undefined, "auto", "foreground", …). The window's key monitor saw the page's ⌘↩,
  // which in a draft resolves to composer.sendBackground (drive 7 found the thread started out of view).
  it('sends a new thread\'s draft in the foreground, whatever ⌘↩ the window saw', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES;
    native.gesture = { modifiers: 'meta', source: 'key', ageMs: 3 };
    markAnnotationSend(client);
    await command('send', '', 'Send this one too');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).not.toBe('');
    expect(toasts(client).map(toast => toast.title)).not.toContain('Started in background');
    // The mark is spent: the next ⌘↩ in a draft is the composer's own, and starts in the background.
    await command('new-thread', client.projectId);
    native.gesture = { modifiers: 'meta', source: 'key', ageMs: 3 };
    await command('send', '', 'Background from command return');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Started in background' });
  });
  it('dispatches "auto" while the thread runs, not the follow-up queue', async () => {
    const { client, native, command } = await opened();
    running(client);
    markAnnotationSend(client);
    await command('send', '', 'Look at this');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } });
    expect(client.threadId).toBe('t1');
  });
  it('a mark older than 10 s is dropped', () => {
    const client = {} as T3Client;
    markAnnotationSend(client, 1_000);
    expect(takeAnnotationSend(client, 11_001)).toBe(false);
    markAnnotationSend(client, 1_000);
    expect(takeAnnotationSend(client, 2_000)).toBe(true);
    expect(takeAnnotationSend(client, 2_000)).toBe(false);
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

  // Review of 2026-10-10: Reveal is `void bridge.revealArtifact(path)` (no toast when it fails); a recording's failed Copy
  // path keeps only Reveal (actionProps: revealAction), while updateScreenshotToast keeps the screenshot's three buttons.
  it('a failed Reveal says nothing; a recording\'s failed Copy path keeps only Reveal, a screenshot\'s keeps its buttons', async () => {
    const client = fakeClient(), dir = '/lane/t3-home/userdata/browser-artifacts';
    const saved = fakeModule({ timelineSleep, browserScreenshot: () => ({ id: 'browser-screenshot-a', path: `${dir}/browser-screenshot-a.png`, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }),
      'browserRecord:capture': () => ({ width: 800, height: 600, frameRate: 30 }), 'browserRecord:begin': () => ({ mimeType: 'video/mp4;codecs=avc1' }),
      'browserRecord:finish': () => ({ path: '/var/folders/x/T/r.mp4', mimeType: 'video/mp4;codecs=avc1', sizeBytes: 9 }),
      'browserRecord:save': request => ({ id: 'browser-recording-b', tabId: request.tab, path: `${dir}/browser-recording-b.mp4`, mimeType: request.mimeType, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }) });
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, '');
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, 'record');
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, '');
    const [shot, recording] = toasts(client);
    expect([shot?.title, recording?.title]).toEqual(['Screenshot saved', 'Recording saved']);
    const failing = fakeModule({ browserArtifact: () => { throw new Error('Clipboard is busy.'); } });
    await artifactLocal(client, failing.native, 'reveal', `${dir}/browser-recording-b.mp4`, String(recording?.id));
    await artifactLocal(client, failing.native, 'reveal', `${dir}/browser-screenshot-a.png`, '');
    expect(toasts(client).map(toast => [toast.kind, toast.title])).toEqual([['success', 'Screenshot saved'], ['success', 'Recording saved']]);
    await artifactLocal(client, failing.native, 'copy-path', `${dir}/browser-recording-b.mp4`, String(recording?.id));
    const failed = toasts(client).find(toast => toast.id === recording?.id);
    expect(failed).toMatchObject({ kind: 'error', title: 'Unable to copy recording path', description: 'Clipboard is busy.' });
    expect([failed?.action?.label, failed?.secondary ?? null]).toEqual(['Reveal in Finder', null]);
    await artifactLocal(client, failing.native, 'copy-path', `${dir}/browser-screenshot-a.png`, String(shot?.id));
    const shotFailed = toasts(client).find(toast => toast.id === shot?.id);
    expect(shotFailed).toMatchObject({ kind: 'error', title: 'Unable to copy screenshot path' });
    expect([shotFailed?.action?.label, shotFailed?.extra?.label, shotFailed?.secondary?.label]).toEqual(['Copy image', 'Copy path', 'Reveal in Finder']);
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
    // realinput-1010d RD-1: the hover is the player's (its box holds the handle and the pill), the group the handle's box.
    expect(lines.find(line => line.includes('testId="browser-mini-player"'))).toContain('hover=overPlayer pointermove=movePlayer');
    expect(lines[handle]).not.toContain('hover=');
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

// Review of 2026-10-10 (findings 1, 2 and 4): the window's send reads a serial the panel projects whatever it shows, says
// "Annotation attached to draft" when Send cannot go (ChatView onSend's notifyDirectAnnotationAttached), and the
// window-level rules of this task (Float closes the right panel, a screenshot's buttons keep their toast) are held here.
describe('an annotation\'s ⌘↩ at the window (ChatView onSendAnnotation, onSend\'s guard)', () => {
  const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
  const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';
  const block = (text: string, start: string, lines: number) => { const all = text.split('\n'), at = all.findIndex(candidate => candidate.includes(start)); return at < 0 ? '' : all.slice(at, at + lines).join('\n'); };

  /** A server thread's client and a module answering Annotate's take with `result` (audit-wave-followups' harness). */
  function threadClient(result: Obj) {
    const client = new Client(), scoped = { environmentId: 'env', threadId: 't1' }, tab = previewRuntimeTabId(scoped, 'epoch-1', 'tab-1'), calls: Obj[] = [];
    Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1', configLive: true, shellLive: true, threadLive: true,
      scopes: ['orchestration:read', 'orchestration:operate'], config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
    client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
    client.shell.threads = [{ id: 't1', projectId: 'p1' }];
    client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input); calls.push(request);
      if (request.op === 'browserAnnotate' && request.action === 'take') return { ok: true, generation: 1, value: { result, serial: 1 } };
      if (request.op === 'editorInsert') return { ok: true, generation: 1, value: { applied: true } };
      return { ok: true, generation: 1, value: {} };
    } };
    const cancels = () => calls.filter(call => call.op === 'browserAnnotate' && call.action === 'cancel').map(call => call.tab);
    return { client, native, tab, cancels };
  }

  // PreviewView unmounts with the panel (usePanelPresence), and its cleanup cancels the pick: in the reference, Annotate
  // and then Float leaves the floating page without the overlay, and a pick there does nothing (review drive, 2026-10-10).
  it('Float or a closed panel ends the tab\'s pick, once (PreviewView\'s unmount: cancelPickElement)', async () => {
    const { client, native, tab, cancels } = threadClient({});
    client.presentation.browserTabs = { [tab]: { kind: 'Success', pick: { active: true, serial: 3, ready: false } } };
    await panelView(client, native, 0); // the thread's panel is closed
    expect(cancels()).toEqual([tab]);
    await panelView(client, native, 0); // the module's status still says active: asked once
    expect(cancels()).toEqual([tab]);
    // A later pick (its own serial) on a tab the panel shows is left alone.
    client.presentation.browserTabs = { [tab]: { kind: 'Success', pick: { active: true, serial: 4, ready: false } } };
    await cancelHiddenPicks(client, native, tab);
    expect(cancels()).toEqual([tab]);
    await cancelHiddenPicks(client, native, null);
    expect(cancels()).toEqual([tab, tab]);
  });

  it('a pick that settled with ⌘↩ as the panel hid still sends at once: the serial is the panel\'s, not the Browser tab\'s view', async () => {
    const { client, native, tab } = threadClient({ annotation, submission: 'send' });
    client.presentation.browserTabs = { [tab]: { kind: 'Success', pick: { active: false, serial: 1, ready: true } } };
    await applyCaptureResults(client, native, [{ runtimeId: tab, threadKey: 'env:t1' }]);
    const view = await panelView(client, native, 0);
    expect(view).toMatchObject({ open: false, annotationSend: 1 });
    expect(view.browser.capture).toEqual(emptyCaptureView());
    expect('sendSerial' in view.browser.capture).toBe(false);
  });

  it('the window sends at once whatever the panel shows, or asks for "Annotation attached to draft" when Send cannot go', () => {
    const app = source('app.contract');
    expect(app).not.toContain('capture.sendSerial');
    expect(line(app, 'task annotationSend when')).toBe('  task annotationSend when shell.panel.annotationSend > annotationSendHandled key=shell.panel.annotationSend');
    expect(block(app, 'action annotationSendNow', 6)).toBe(['  action annotationSendNow', '    annotationSendHandled = shell.panel.annotationSend',
      '    if commandPending or not data.canSend or data.requestMode == "question"', '      chatLocal("surface-browser-annotation-held", "", "")', '    else', '      send()'].join('\n'));
    expect(line(source('r4-surfaces-shapes.contract'), '  annotationSend: number')).not.toBe('');
    expect(block(source('browser-shapes.contract'), 'shape BrowserCapture', 12)).not.toContain('sendSerial');
  });

  it('the held send says "Annotation attached to draft" and spends the mark', async () => {
    const client = fakeClient(), module = fakeModule({});
    surfaceStore(client).panels.set(panelKey(client), { ...browserPanel(), visible: false });
    markAnnotationSend(client, 0);
    expect(annotationSendMarked(client, 0)).toBe(true);
    await surfaceLocal(client, module.native, 'browser-annotation-held', '', '');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description, toast.stacked])).toEqual([['info', 'Annotation attached to draft', 'Sending is unavailable right now. Finish the current action, then send.', true]]);
    expect(takeAnnotationSend(client, 0)).toBe(false);
  });

  it('a pending question: the annotation\'s send does not answer it', async () => {
    const { client, native, command } = await opened();
    Object.assign(client.thread!.projection, { runtimeRequests: [{ id: 'r1', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T00:00:00Z', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'i1', type: 'user_input_request', requestId: 'r1', questions: [{ id: 'scope', header: 'Scope', question: 'Which scope?', options: [{ label: 'Workspace', description: 'Here.' }] }] }] });
    const committed = native.committed.length;
    markAnnotationSend(client);
    await command('send', '', '[Tighten](t3-context://v1/preview-annotation/preview-annotation_annotation_1) ');
    expect(native.committed.length).toBe(committed);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: 'Annotation attached to draft' });
  });

  it('an unavailable provider: the annotation stays in the draft with the info toast, not the send error', async () => {
    const { client, native, command } = await opened();
    for (const entry of client.config.providers as Obj[]) entry.auth = { status: 'unauthenticated' };
    const committed = native.committed.length;
    markAnnotationSend(client);
    const result = await command('send', '', 'Send this one too');
    expect(result.message).toBe('');
    expect(native.committed.length).toBe(committed);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: 'Annotation attached to draft' });
    expect(client.draft).toBe('Send this one too');
  });

  it('Float preview closes the window\'s right panel (handlePictureInPicture: rightPanelStore.close)', () => {
    // The data module hides its surfaces (the row above), but the window's own open state kept the panel on its
    // launcher (drive 7): chatLocal clears it when Float floats the tab, not when the same press closes the player.
    const app = source('app.contract');
    const rule = line(app, 'op == "surface-browser-float"');
    expect(rule).toContain('(op == "surface-browser-float" and not shell.panel.browser.capture.floating)');
    expect(rule.trimStart().startsWith('if ')).toBe(true);
    expect(app.split('\n')[app.split('\n').indexOf(rule) + 1]).toBe('      rightPanelAt = ""');
  });

  it('a screenshot\'s and a recording\'s buttons keep their toast (toastAct)', () => {
    const rule = line(source('app.contract'), '    toastDismissed = op == "copy"');
    expect(rule).toContain('op == "copy" or startsWith(op, "shelllocal:surface-browser-artifact-") ? toastDismissed :');
  });
});

// The real-input session of 2026-10-10 (realinput-1010c, row A, bundle d1ae77d7): what a person's keys and pointer did
// that agent drives could not show. Each row fails without its fix, except the canvas row ("a resize's pans under one
// serial …"): the canvas did not change, so it passes on the previous code too. It shows why one serial per gesture
// matters; the fix itself, the contract's `pan` taking one serial per gesture, is the line row before it and the drive.
describe('realinput-1010c: a real Escape, the pill by a real pointer, the drag and the resize, the header tooltip', () => {
  const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
  const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';
  const block = (text: string, start: string, end: string) => { const all = text.split('\n'), at = all.findIndex(candidate => candidate.includes(start)); return at < 0 ? '' : all.slice(at, all.findIndex((candidate, index) => index > at && candidate.startsWith(end))).join('\n'); };

  it('Escape while the shown tab annotates is the page\'s: the panel toggle declares it only otherwise (PickPreload cancels, the panel stays)', () => {
    // A real Escape met the toggle's aria-keyshortcuts before the page: the panel closed and floated the tab. Since
    // right-panel-escape the toggle declares Escape only in a sheet (the reference's inline panel binds none); the pick holds it there.
    const toggle = line(source('r4-surfaces.contract'), 'testId="panel-toggle-right"');
    expect(toggle).toContain('aria-keyshortcuts=((sheet and not (panel.deviceSetup or panel.files.editorsOpen or panel.files.editing or searchFocused or panel.browser.capture.pickActive)) ? "Escape" : "")');
  });

  it('the handle\'s box covers the pill while it shows, and the pill\'s buttons hear no hover of their own (X62)', () => {
    const contract = source('browser-capture.contract');
    const handle = line(contract, 'testId="browser-mini-handle"');
    expect(handle).toContain('width=(pill ? pillWidth : 0.75 * rem) height=(pill ? 2 * rem : 0.75 * rem) cursor=');
    expect(line(contract, 'testId="browser-mini-pill"')).toContain('position="absolute" top=0 right=0 width=pillWidth height="2rem"');
    expect(line(contract, 'derive pillWidth')).toBe('  derive pillWidth = bcPillWidth(rem, mini.recording ? 4 : 3)');
    const button = block(contract, 'component BcPillButton', '//');
    expect(button).not.toContain('hover=');
    expect(button).toContain('TipBubble(label=tip, side="top", align=align, offset=28, inset=0, shown=tipShown)');
    // Base UI shifts a tooltip inside the window: near the canvas's right edge the pill's tooltips end at their buttons.
    expect(line(contract, 'derive tipAlign')).toBe('  derive tipAlign = frame.x + frame.width > canvasWidth - 7.5 * rem ? "end" : "center"');
    // The tooltip follows the pointer's button, and a press closes it until the pointer moves to another (Base UI).
    for (const id of ['restore', 'window', 'close']) expect(contract).toContain(`align=tipAlign, hot=(tipAt == "${id}"), tipShown=(pill and tipAt == "${id}" and tipShut != "${id}")`);
  });

  it('realinput-1010d RD-1: the pill follows the player\'s own hover and moves, not an enter of the 12 pt handle', async () => {
    // A real pointer that came to rest on the dot showed the grab hand but no pill until it moved again (3 of 3): the
    // handle's small box heard no enter. The player's box hears the pointer from the moment it crosses the page, so each
    // move places it on the handle ("dot"), the rest of the pill's box ("pill") or the page; the pill shows on the
    // handle and stays on its box while it shows (group-hover), and leaving the player hides it.
    const zone = await contractFn('browser-capture.contract', 'bcPlayerZone', ['x', 'y', 'width', 'inset', 'pill', 'rem']);
    const pillWidth = await contractFn('browser-capture.contract', 'bcPillWidth', ['rem', 'items']);
    // The default float (240 pt wide, radius 12, pillInset 8) at rem 16: the pill's box spans x 150..232, y 8..40, and
    // the 12 pt handle its top-right corner, x 220..232, y 8..20. Each edge is checked on both sides.
    const [width, inset, rem] = [240, 8, 16], pill = pillWidth(rem, 3) as number, at = (x: number, y: number, w = pill, i = inset) => zone(x, y, width, i, w, rem);
    expect(pill).toBe(82);
    for (const [x, y] of [[226, 14], [220, 8], [232, 8], [220, 20], [232, 20]]) expect(at(x, y), `dot at ${x},${y}`).toBe('dot');
    for (const [x, y] of [[219, 14], [226, 21], [150, 8], [150, 40], [232, 40], [190, 24]]) expect(at(x, y), `pill at ${x},${y}`).toBe('pill');
    for (const [x, y] of [[120, 200], [226, 7], [233, 14], [149, 24], [190, 41], [236, 4], [4, 4]]) expect(at(x, y), `page at ${x},${y}`).toBe('');
    // Recording adds the status item: the box grows left (108 pt) and the handle stays in the corner.
    const recording = pillWidth(rem, 4) as number;
    expect([recording, at(130, 24, recording), at(130, 24), at(226, 14, recording)]).toEqual([108, 'pill', '', 'dot']);
    // A larger radius insets the pill further (resolvePreviewMiniPlayerPillInset): the handle moves with it.
    expect([at(230, 10), at(230, 10, pill, 13), at(215, 19, pill, 13), at(221, 26, pill, 13), at(226, 12, pill, 13)]).toEqual(['dot', '', 'dot', 'pill', '']);
    const contract = source('browser-capture.contract');
    const player = block(contract, 'component BrowserMiniPlayer', '//');
    // The agent's hover moves first and enters second: entering reads the zone the move found.
    expect(player).toContain('  action overPlayer(value: bool)\n    over = value\n    dotOver = value and (zone == "dot" or (zone == "pill" and (pillFocus != "" or dragging != "")))\n    zone = value ? zone : ""');
    expect(player).toContain('    let at = bcPlayerZone(e.offsetX, e.offsetY, frame.width, frame.pillInset, pillWidth, rem)\n    let shown = over and (at == "dot" or (at == "pill" and pill))');
    // The tooltips' slot is the pill's own coordinates: the pill's box sits `pillInset` from the player's top right.
    expect(player).toContain('bcPillSlot(e.offsetX - (frame.width - frame.pillInset - pillWidth), e.offsetY - frame.pillInset, rem, mini.recording)');
    expect(player).toContain('  derive pill = dotOver or pillFocus != "" or dragging != ""');
    expect(player.match(/hover=/g)).toHaveLength(1);
  });

  it('one gesture keeps one serial: the canvas takes the frame at its first pan and moves it by the pan\'s total', () => {
    // The clock (floor(now()), performanceNow() since #384) in every pan's serial made each move a new gesture from the frame on screen: a (-200, -100) drag moved
    // (-100, -188) and an 80 pt north-west resize took 240 × 333 to 416 × 577 (the reference: 305 × 423).
    const pan = block(source('browser-capture.contract'), '  action pan(direction: string, dx: number, dy: number)', '  action release');
    expect(pan).toContain('let id = starting ? `${floor(performanceNow())}.${count}` : serial');
    expect(pan).toContain('serial = id');
    expect(pan).toContain('gesture(`${id}|${mini.sourceKey}|${direction}|${nx}|${ny}`)');
    expect(pan.match(/performanceNow\(\)/g)).toHaveLength(1);
  });

  it('a resize\'s pans under one serial land where one pan of their total does; a serial per pan overshoots', async () => {
    // The canvas's side of the row above, unchanged by the fix (this row passes on the previous code too): the frame of a
    // gesture's start moves by the pan's total only while the serial holds; a new serial per pan restarts from the frame on
    // screen. The chat canvas of chat-canvas-view.test.ts (1280 × 840, the panel closed) and the reference's floating page (539 × 748).
    const frames = { chat: [256, 0, 1024, 840], overlay: [400, 668, 736, 172] };
    const run = async (gestures: string[]) => {
      const client = { environmentId: 'env', threadId: 't1', draftKey: 'env:t1', generation: 1, diffOpen: false, presentation: { frames }, local: {}, shell: { projects: [], threads: [] } } as unknown as T3Client;
      miniStoreOf(client).open('t1', { kind: 'browser', tabId: 'tab-1' });
      captureHost(client).miniSize = { key: 'browser:tab-1', width: 539, height: 748 };
      const ask = async (gesture: string) => (await chatCanvasView(client, null as unknown as Native, { width: 1024, viewportHeight: 840, detailsInline: false, chatMax: 736, overlaid: true, gesture })).mini;
      const start = await ask('');
      let last = start;
      for (const gesture of gestures) last = await ask(gesture);
      return { start, last };
    };
    const pans = (serials: string[]) => serials.map((serial, index) => `${serial}|browser:tab-1|northwest|${-20 * (index + 1)}|${-20 * (index + 1)}`);
    const direct = await run(['1.1|browser:tab-1|northwest|-80|-80']);
    expect(direct.start).toMatchObject({ width: 240, height: 333 });
    expect([direct.last.width, direct.last.height]).toEqual([305, 423]); // the reference's NW grip, (-80, -80) from 240 × 333
    const held = await run(pans(['7.1', '7.1', '7.1', '7.1'])), fresh = await run(pans(['7.1', '8.1', '9.1', '10.1']));
    expect([held.last.width, held.last.height]).toEqual([305, 423]);
    expect(fresh.last.width).toBeGreaterThan(305 + 40);
  });

  it('a press on a pill button never drags the player: the drag is a box behind the pill, not the handle that holds its buttons', () => {
    // Review of 2026-10-10 (second): the handle's `pan` took a nested button's contact past the slop (LLP 1057.001 rule 3),
    // so a drag that started on Open in right panel moved the player. The reference's buttons stop the pointer-down
    // (MiniPlayerShell), and its pill's padding and the dot still drag (the pointer-down reaches the handle's group).
    const contract = source('browser-capture.contract'), lines = contract.split('\n');
    const indent = (text: string) => text.length - text.trimStart().length;
    const at = (id: string) => lines.findIndex(candidate => candidate.includes(`testId="${id}"`));
    const handle = at('browser-mini-handle'), grab = at('browser-mini-grab'), pill = at('browser-mini-pill');
    expect(lines[handle]).not.toContain('pan=');
    expect(lines[grab]).toContain('box position="absolute" left=0 top=0 right=0 bottom=0 pan=pan("move") panrelease=release touch-action="none"');
    // The grab box fills the handle and sits behind the pill, which lets the pointer through to it; only the buttons take it.
    expect([grab, pill]).toEqual([handle + 1, handle + 2]);
    expect([indent(lines[grab]!), indent(lines[pill]!)]).toEqual([indent(lines[handle]!) + 2, indent(lines[handle]!) + 2]);
    expect(lines[pill]).toContain(' pointer-events="none" ');
    expect(lines[pill]).not.toContain('pan=');
    expect(block(contract, 'component BcPillButton', '//')).toContain('button press=pressed focus=focusChange(true) blur=focusChange(false) pointer-events=(shown ? "auto" : "none")');
    expect(contract.match(/pan=pan\("move"\)/g)).toHaveLength(1);
  });

  it('the header\'s Toggle right panel drops its hover on press: it leaves the header as the panel opens, so no leave comes', () => {
    const chat = source('chat.contract');
    expect(line(chat, 'testId="toggle-right-panel"')).toContain('button press=toggleRight hover=hover("right")');
    expect(block(chat, '  action toggleRight', '  view')).toContain('    hovered = hovered == "right" ? "" : hovered\n    panel("toggle", "")');
  });
});
