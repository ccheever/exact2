// The clone's own rows for the chrome row's Annotate, Capture and Float preview, the
// More menu's separate window, their toasts and the floating player's browser source (browser-capture.ts), each named
// after the reference code it follows (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: PreviewView.tsx handlePickElement,
// handleCapture, handlePictureInPicture, handleNativePictureInPicture; PreviewChromeRow.tsx; ThreadPreviewMiniPlayer.tsx;
// ChatView.tsx closePreviewPanel and shouldRenderPreviewMiniPlayer). The tab's frame answers `@exact/browser-automation`
// through a fake (browser-automation-run.ts `useAutomationCalls`), and the module is a fake answering the page ops as
// T3BrowserPage.swift does; PreviewView.test.tsx's annotation rows ("forwards Cmd/Ctrl+Enter annotations to the
// composer send path", "warns when main dropped the crop before handing over the pick") are followed here.
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it } from 'bun:test';
import { artifactLocal, browserCaptureView, browserMiniView, cancelHiddenPicks, captureHost, captureLocal, emptyCaptureView, floatingTabOf } from '../../src/browser/browser-capture';
import { adoptFrameEvent, frameIdOf, syncFrames } from '../../src/browser/browser-frames';
import { automationTick, toggleFrameMute, useAutomationCalls, type AutomationCalls } from '../../src/browser/browser-automation-run';
import { serveRecordingRequests } from '../../src/browser/browser-automation-recording';
import { annotateState } from '../../src/browser/browser-annotate';
import { adoptAppFiles } from '../../src/media/save-file';
import { useClipboardCalls } from '../../src/media/media-copy-image';
import { acceptComposerEdit, observeComposer } from '../support/composer-field-fixture';
import { browserMiniSessions, browserHost, browserView } from '../../src/browser/browser-surface';
import { previewRuntimeTabId } from '../../src/browser/browser-state';
import { panelKey, panelState, panelView, surfaceLocal, surfaceStore, type PanelState } from '../../src/panel/panel-surfaces';
import { miniStoreOf } from '../../src/device/device-session';
import { toasts } from '../../src/shell/toast';
import { shellState, toastViews, copiedActions, copiedToasts } from '../../src/shell/shell';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { obj, type Obj } from '../../src/client/domain';
import { T3Client as Client } from '../../src/client/client';
import { connected, opened, running } from '../support/composer-controls-fixture';
import { annotationSendMarked, markAnnotationSend, takeAnnotationSend } from '../../src/browser/browser-annotation';
import { snapshot } from '../../src/client/presentation';
import { DEFAULT_SEND_RULES } from '../../src/composer/composer-editor-intent';
import { activeRef } from '../../src/terminal/terminal-drawer-view';
import { chatCanvasView } from '../../src/shell/chat-canvas-view';
import { contractFn } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

const ref = { environmentId: 'local', threadId: 'thread-1' };
const threadKey = 'local:thread-1';
const runtimeId = previewRuntimeTabId(ref, 'epoch-1', 'tab-1');
/** The composer's half of an insert: a fake module's applied edit lands in the observed composer. */
function shared(client: T3Client, native: Native): Native {
  observeComposer(client, client.draft);
  return { available: native.available, watch: topic => native.watch(topic), later: async request => {
    const reply = obj(await native.later(request)), value = obj(reply.value);
    if (obj(request).op === 'editorInsert' && reply.ok && value.applied === true) return { ...reply, value: { ...value, applied: acceptComposerEdit(client, request) } };
    return reply;
  } };
}
/** `@exact/browser-automation` as the tab's frame answers it: Annotate's overlay (`next` is its next outcome), the React
 *  read, the viewport and the crop. A crop's draft image reads 812 bytes. */
function fakePage(next: () => unknown | Promise<unknown>, fail: Partial<Record<'captureScreenshot', boolean>> = {}, crop?: () => void | Promise<void>) {
  const calls: Obj[] = [];
  const fake: AutomationCalls = {
    async evaluate(options) {
      calls.push({ call: 'evaluate', ...options });
      if (options.expression.includes('.next()')) return { result: JSON.stringify(await next()) };
      if (options.expression.includes('__reactFiber')) return { result: '{}' };
      if (options.expression.includes('innerWidth')) return { result: '[800,600]' };
      return { result: true };
    },
    async callFunction(options) { calls.push({ call: 'callFunction', ...options }); return { result: true }; },
    async captureScreenshot(options) {
      calls.push({ call: 'captureScreenshot', ...options });
      await crop?.();
      if (fail.captureScreenshot) throw Object.assign(new Error('WebKit gave no picture.'), { code: 'UnknownError' });
      return { path: options.path, width: 120, height: 80 };
    },
    async addPreloadScript(options) { calls.push({ call: 'addPreloadScript', ...options }); return { script: 'p' }; },
    async performActions(options) { calls.push({ call: 'performActions', ...options }); return {}; },
    async handleUserPrompt(options) { calls.push({ call: 'handleUserPrompt', ...options }); return {}; },
    async setDownloadBehavior(options) { calls.push({ call: 'setDownloadBehavior', ...options }); return {}; },
  };
  useAutomationCalls(fake);
  adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(812), writeFile: async () => {}, mkdir: async () => {},
    rm: async (path: string) => { calls.push({ call: 'rm', path }); },
    transformImage: async (from: string, to: string, options: Obj) => { calls.push({ call: 'transformImage', from, to, ...options }); return { path: to, width: 120, height: 80 }; } } });
  return calls;
}
/** Annotate on the tab, then the window's queued send its pick waits in (browser-annotate.ts). */
async function pick(client: T3Client, native: Native, next: () => unknown | Promise<unknown>, theTarget = target, crop?: () => void | Promise<void>) {
  const calls = fakePage(next, {}, crop);
  await captureLocal(client, native, browserPanel(), 'annotate', theTarget, 'dark');
  await serveRecordingRequests(client, shared(client, native), 0);
  useAutomationCalls(null);
  return calls;
}
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
    catch (error) { return { ok: false, generation: 0, error: { kind: (error as { kind?: string }).kind ?? 'Fake', message: (error as Error).message } }; }
  } };
  return { native, calls, ops: () => calls.map(call => (call.action ? `${call.op}:${call.action}` : call.op)) };
}
/** A client with the tab's frame mounted (browser-frames.ts) and `page` as its hatch reports it (T3BrowserPage.swift). */
function fakeClient(page: Obj = {}) {
  const client = {
    scopes: ['preview:operate'],
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', snapshotOwner: threadKey, projection: { turnItems: [], runtimeRequests: [] }, generation: 1, connection: 'connected', ready: true, diffOpen: false, draftKey: threadKey,
    presentation: { browserPages: { [frameIdOf(runtimeId)]: page } } as Obj, config: { environment: { capabilities: {} } }, shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [] },
    local: { clientSettings: {}, deviceSettings: {}, drafts: {} as Record<string, string>, snapshotDrafts: {} as Record<string, Obj[]> },
    get snapshotDrafts() { return (this.local.snapshotDrafts as Record<string, Obj[]>)[threadKey] ?? []; },
    get draft() { return (this.local.drafts as Record<string, string>)[threadKey] ?? ''; },
    async rpc() { return {}; }, async raw() { return { ok: true, generation: 0, value: {} }; },
  } as unknown as T3Client;
  syncFrames(client, [{ runtimeId, url: 'https://example.com/', environment: 'local', profile: 'default', zoom: 1, colorScheme: 'system' }]);
  return client;
}
const target = { ref, tabId: 'tab-1', runtimeId };
const browserPanel = (): PanelState => ({ surfaces: [{ id: 'browser:tab-1', kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId: 'tab-1', threadKey } }], active: 'browser:tab-1', visible: true, userRevision: 0 });
const annotation = {
  id: 'annotation_1', pageUrl: 'https://example.com/', pageTitle: 'Example', comment: 'Tighten this spacing', elements: [], regions: [{ id: 'region_1', rect: { x: 1, y: 2, width: 30, height: 40 } }],
  strokes: [], styleChanges: [], screenshot: null, createdAt: '2026-07-27T00:00:00.000Z',
};
const crop = { id: '0f8c1b52-9a4c-4d2e-8a0b-3c2d1e0f9a8b', name: 'preview-annotation-annotation_1.png', sizeBytes: 812, width: 120, height: 80, cropRect: { x: 0, y: 0, width: 60, height: 40 } };

describe('PreviewChromeRow: Annotate, Capture and Float preview', () => {
  it('names Annotate after its state and gives a failed page its reason', async () => {
    const client = fakeClient();
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false)).toMatchObject({ pickActive: false, pickDisabled: false, pickTip: 'Annotate elements, regions, and drawings',
      captureDisabled: false, recording: false, floating: false, floatDisabled: false, separateWindow: false });
    (client.presentation.browserPages as Obj)[frameIdOf(runtimeId)] = { pip: true };
    fakePage(() => new Promise(() => {}));
    await captureLocal(client, fakeModule({}).native, browserPanel(), 'annotate', target, 'dark');
    useAutomationCalls(null);
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, false)).toMatchObject({ pickActive: true, pickTip: 'Cancel annotation (Esc)', separateWindow: true });
    // pickDisabled: a failed load covers the page; captureDisabled and floatDisabled: no web contents, or a failed load.
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, true, true)).toMatchObject({ pickDisabled: true, pickTip: 'Page didn\'t load. Pick is unavailable until the page renders', captureDisabled: true, floatDisabled: true });
    expect(browserCaptureView(client, ref, 'tab-1', runtimeId, false, false)).toMatchObject({ captureDisabled: true, floatDisabled: true, pickDisabled: false });
    expect(browserCaptureView(client, ref, '', runtimeId, true, false).pickDisabled).toBe(true);
  });

  it('handlePickElement: a press starts the overlay in a sandbox of the frame in the app\'s scheme, a second press cancels it', async () => {
    const client = fakeClient(), module = fakeModule({});
    const calls = fakePage(() => new Promise(() => {}));
    await captureLocal(client, module.native, browserPanel(), 'annotate', target, 'dark');
    expect(calls[0]).toMatchObject({ call: 'callFunction', id: frameIdOf(runtimeId), sandbox: 't3-annotate', arguments: [{ colorScheme: 'dark', primary: '#346bf1' }] });
    expect(String(calls[0]!.functionDeclaration)).toContain('globalThis.__t3codeAnnotate.start(theme)');
    expect(annotateState(client, runtimeId)).toEqual({ active: true, serial: 0 });
    await captureLocal(client, module.native, browserPanel(), 'annotate', target, 'dark');
    expect(calls[1]).toMatchObject({ call: 'evaluate', sandbox: 't3-annotate' });
    expect(String(calls[1]!.expression)).toContain('.cancel()');
    expect(annotateState(client, runtimeId)).toEqual({ active: false, serial: 1 });
    useAutomationCalls(null);
  });
});

describe('Annotate\'s result (handlePickElement → addPreviewAnnotation)', () => {
  let client: T3Client;
  beforeEach(() => { client = fakeClient(); });
  const picked = (submission: string, extra: Obj = {}) => () => ({ type: 'picked', annotation: { ...annotation, ...extra }, rect: { x: 1, y: 2, width: 30, height: 40 }, submission });

  it('never directly appends into a background target that becomes the active field while its crop is taken', async () => {
    const targetKey = 'local:thread-2', targetRuntime = previewRuntimeTabId({ environmentId: 'local', threadId: 'thread-2' }, 'epoch-1', 'tab-2');
    syncFrames(client, [{ runtimeId, url: '', environment: 'local', profile: 'default', zoom: 1, colorScheme: 'system' }, { runtimeId: targetRuntime, url: '', environment: 'local', profile: 'default', zoom: 1, colorScheme: 'system' }]);
    const module = fakeModule({});
    await pick(client, module.native, picked('send'), { ref: { environmentId: 'local', threadId: 'thread-2' }, tabId: 'tab-2', runtimeId: targetRuntime }, async () => {
      await Promise.resolve();
      Object.assign(client, { threadId: 'thread-2', draftKey: targetKey, snapshotOwner: targetKey });
      client.local.drafts[targetKey] = 'new active source';
      observeComposer(client, 'new active source', true);
    });
    expect(client.local.drafts[targetKey]).toBe('new active source');
    expect(module.calls.some(call => call.op === 'editorInsert')).toBe(false);
    expect(captureHost(client).sendSerial).toBe(0);
  });

  it('puts the chip at the composer\'s caret and the crop, the pick\'s rect from the frame, on the shelf', async () => {
    const inserted: string[] = [];
    const module = fakeModule({ editorState: () => ({ text: inserted.join('') }), editorInsert: request => { inserted.push(String(request.text)); return { applied: true }; } });
    const calls = await pick(client, module.native, picked('attach'));
    expect(inserted).toEqual(['[Tighten this spacing](t3-context://v1/preview-annotation/preview-annotation_annotation_1)']);
    const shot = calls.find(call => call.call === 'captureScreenshot')!;
    expect(shot).toMatchObject({ id: frameIdOf(runtimeId), clip: { x: 1, y: 2, width: 30, height: 40 } });
    expect(String(shot.path)).toMatch(/^app:\/data\/snapshots\/drafts\/[0-9a-f-]{36}\.png$/);
    const id = String(shot.path).slice('app:/data/snapshots/drafts/'.length, -4);
    expect(client.snapshotDrafts).toEqual([{ id, name: 'preview-annotation-annotation_1.png', mimeType: 'image/png', sizeBytes: 812 }]);
    expect(module.calls.find(call => call.op === 'editorInsert')).toMatchObject({ focus: true });
    expect(module.ops()).not.toContain('editorFocus');
    expect(toasts(client)).toEqual([]);
    // The overlay tears down once its marks are in the crop.
    expect(calls.some(call => call.call === 'evaluate' && String(call.expression).includes('.captured()'))).toBe(true);
    expect(captureHost(client).sendSerial).toBe(0);
    expect(annotateState(client, runtimeId)).toEqual({ active: false, serial: 1 });
  });

  it('Annotate, Capture, Mute and the page scripts only read: no call makes a person\'s frame the agent\'s (LLP 1020.002 D9)', async () => {
    const module = fakeModule({ editorState: () => ({ text: '' }), editorInsert: () => ({ applied: true }),
      'browserArtifact:adopt': () => ({ id: 'browser-screenshot-example-com-abc', path: '/a/browser-screenshot-example-com-abc.png', sizeBytes: 9, createdAt: '2026-10-25T00:00:00.000Z' }) });
    const annotate = await pick(client, module.native, picked('attach'));
    const rest = fakePage(() => null);
    await captureLocal(client, module.native, browserPanel(), 'capture', target, '');
    await toggleFrameMute(client, module.native, runtimeId, true);
    const commit = new URLSearchParams({ event: 'currententrychange', id: '1', type: 'push', loading: 'false', back: 'false', forward: 'false', controller: 'person', error: '', recovering: 'false', url: 'https://example.com/', title: '' });
    adoptFrameEvent(client, frameIdOf(runtimeId), commit.toString(), 0);
    await automationTick(client);
    useAutomationCalls(null);
    const all = [...annotate, ...rest];
    for (const name of ['captureScreenshot', 'callFunction', 'evaluate']) expect(all.some(call => call.call === name)).toBe(true);
    expect(all.filter(call => ['addPreloadScript', 'performActions', 'handleUserPrompt', 'setDownloadBehavior'].includes(String(call.call)) || call.userActivation === true)).toEqual([]);
  });

  it('goes to a new thread\'s draft composer, whose tab names the id the draft was given', async () => {
    const draft = fakeClient();
    Object.assign(draft, { threadId: '', draftKey: 'local:new:p1' });
    (draft.local as Obj).composerControls = { draftThreads: { 'local:new:p1': 'thread-1' } };
    const inserted: string[] = [];
    const module = fakeModule({ editorInsert: request => { inserted.push(String(request.text)); return { applied: true }; } });
    await pick(draft, module.native, picked('attach'));
    expect(inserted).toHaveLength(1);
    // Float preview keys the player by the same thread.
    await captureLocal(draft, module.native, browserPanel(), 'float', target, '');
    expect(floatingTabOf(draft)).toBe('tab-1');
  });

  it('warns when the crop failed, and keeps the annotation', async () => {
    const module = fakeModule({ editorInsert: () => ({ applied: false }) });
    const calls = fakePage(picked('attach'), { captureScreenshot: true });
    await captureLocal(client, module.native, browserPanel(), 'annotate', target, 'dark');
    await serveRecordingRequests(client, shared(client, module.native), 0);
    useAutomationCalls(null);
    expect(calls.some(call => call.call === 'captureScreenshot')).toBe(true);
    expect(client.snapshotDrafts).toEqual([]);
    expect(client.draft).toBe('');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([
      ['info', 'Annotation was not added', 'The prompt changed. The annotation was kept; insert it again when the prompt is ready.'],
      ['error', 'Could not capture the picked element', 'The annotation was kept without the screenshot.'],
    ]);
  });

  it('forwards Cmd/Ctrl+Enter annotations to the composer send path', async () => {
    const module = fakeModule({ editorInsert: () => ({ applied: true }) });
    await pick(client, module.native, picked('send'));
    expect(captureHost(client).sendSerial).toBe(1);
  });

  it('drops a cancelled pick and a payload the validator refuses', async () => {
    const module = fakeModule({});
    await pick(client, module.native, picked('attach', { screenshot: { dataUrl: 'from the page' }, pageUrl: 42 }));
    expect(client.draft).toBe('');
    const cancelled = fakeClient();
    await pick(cancelled, module.native, () => ({ type: 'cancelled' }));
    expect(module.calls.some(call => call.op === 'editorInsert')).toBe(false);
    expect(cancelled.draft).toBe('');
    expect(annotateState(cancelled, runtimeId)).toEqual({ active: false, serial: 1 });
  });
});

describe('⌘↩ in Annotate\'s editor (PickPreload: the page has the key)', () => {
  it('while Annotate is on in the thread\'s Browser tab, the composer\'s send chords leave Meta+Enter to the page', async () => {
    const { client } = await opened();
    client.config.keybindings = DEFAULT_SEND_RULES;
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
    const ref = activeRef(client)!, tab = previewRuntimeTabId(ref, 'epoch-1', 'tab-1'), other = previewRuntimeTabId({ ...ref, threadId: 'other' }, 'epoch-1', 'tab-1');
    syncFrames(client, [tab, other].map(id => ({ runtimeId: id, url: '', environment: ref.environmentId, profile: 'default', zoom: 1, colorScheme: 'system' as const })));
    fakePage(() => new Promise(() => {}));
    const quiet = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
    await captureLocal(client, quiet, { surfaces: [], active: '', visible: true, userRevision: 0 }, 'annotate', { ref, tabId: 'tab-1', runtimeId: tab }, 'light');
    expect(snapshot(client).composer.sendChords).toBe('Meta+Alt+Enter');
    // Another thread's overlay, or one that has closed, leaves the shortcut alone.
    await captureLocal(client, quiet, { surfaces: [], active: '', visible: true, userRevision: 0 }, 'annotate', { ref, tabId: 'tab-1', runtimeId: tab }, 'light');
    await captureLocal(client, quiet, { surfaces: [], active: '', visible: true, userRevision: 0 }, 'annotate', { ref: { ...ref, threadId: 'other' }, tabId: 'tab-1', runtimeId: other }, 'light');
    useAutomationCalls(null);
    expect(snapshot(client).composer.sendChords).toBe('Meta+Enter Meta+Alt+Enter');
  });
  // ChatView's onSendAnnotation: onSend(undefined, "auto", "foreground", …): even a press that names ⌘↩, which in a
  // draft resolves to composer.sendBackground, sends an annotation in the foreground.
  it('sends a new thread\'s draft in the foreground, whatever ⌘↩ pressed it', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = DEFAULT_SEND_RULES;
    markAnnotationSend(client);
    await command('send', 'key:meta', 'Send this one too');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).not.toBe('');
    expect(toasts(client).map(toast => toast.title)).not.toContain('Started in background');
    // The mark is spent: the next ⌘↩ in a draft is the composer's own, and starts in the background.
    await command('new-thread', client.projectId);
    await command('send', 'key:meta', 'Background from command return');
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
    const path = '/lane/t3-home/userdata/browser-artifacts/browser-screenshot-example-com-abc.png', staged = 'app:/tmp/copy-image/artifact-1.png';
    const module = fakeModule({ 'browserArtifact:adopt': () => ({ id: 'browser-screenshot-example-com-abc', path, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }),
      'browserArtifact:stage': () => ({ path: staged }) });
    const calls = fakePage(() => null);
    await captureLocal(client, module.native, browserPanel(), 'capture', target, '');
    useAutomationCalls(null);
    // The frame's picture under app:/tmp/browser-shots, which the module moves into the artifact directory.
    const shot = calls.find(call => call.call === 'captureScreenshot')!;
    expect(shot).toMatchObject({ id: frameIdOf(runtimeId) });
    expect(String(shot.path)).toMatch(/^app:\/tmp\/browser-shots\/browser-frame-[0-9a-f]{16}-1\.png$/);
    expect(module.calls.find(call => call.op === 'browserArtifact')).toMatchObject({ action: 'adopt', path: shot.path });
    const [toast] = toasts(client);
    expect(toast).toMatchObject({ kind: 'success', title: 'Screenshot saved', stacked: true, secondaryVariant: 'outline' });
    expect(toast?.action).toEqual({ label: 'Copy image', op: 'shelllocal:surface-browser-artifact-copy-image', id: path, value: String(toast?.id) });
    expect(toast?.extra).toEqual({ label: 'Copy path', op: 'shelllocal:surface-browser-artifact-copy-path', id: path, value: String(toast?.id) });
    expect(toast?.secondary).toEqual({ label: 'Reveal in Finder', op: 'shelllocal:surface-browser-artifact-reveal', id: path, value: String(toast?.id) });
    shellState(client).now = 10_000;
    // copyArtifactToClipboard: the artifact by its path, which the module copies into the app's files (`stage`), as a PNG
    // through @exact/clipboard; the copy and its PNG are removed once written.
    const written: Obj[][] = [];
    useClipboardCalls({ write: async items => { written.push(items as Obj[]); }, ClipboardItem: class { constructor(items: Obj) { Object.assign(this, items); } } });
    await artifactLocal(client, module.native, 'copy-image', path, String(toast?.id));
    useClipboardCalls(null);
    expect(module.calls.filter(call => call.op === 'browserArtifact').map(call => [call.action, call.path])).toEqual([['adopt', shot.path], ['stage', path]]);
    const drawn = calls.find(call => call.call === 'transformImage')!;
    expect(drawn).toMatchObject({ from: staged, type: 'image/png' });
    expect(written).toEqual([[{ 'image/png': drawn.to }]]);
    expect(calls.filter(call => call.call === 'rm').map(call => call.path)).toEqual(expect.arrayContaining([staged, drawn.to]));
    let [view] = toastViews(toasts(client), copiedToasts(client, 10_000), copiedActions(client));
    expect([view?.actionLabel, view?.actionDone, view?.extraLabel, view?.extraDone, view?.secondaryLabel]).toEqual(['Copied!', true, 'Copy path', false, 'Reveal in Finder']);
    shellState(client).now = 12_000;
    [view] = toastViews(toasts(client), copiedToasts(client, 12_000), copiedActions(client));
    expect([view?.actionLabel, view?.actionDone]).toEqual(['Copy image', false]);
  });

  // captureErrors.ts normalizeCaptureFailure (74541bab17): the toast says what to check, not the host's error.
  it('reports a failed capture and a failed copy in the reference\'s words', async () => {
    const client = fakeClient();
    const module = fakeModule({ browserArtifact: () => { throw Object.assign(new Error('Preview artifact path is outside /x'), { kind: 'Screenshot' }); } });
    fakePage(() => null);
    await captureLocal(client, module.native, browserPanel(), 'capture', target, ''); // the module could not save it
    fakePage(() => null, { captureScreenshot: true });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, ''); // the frame gave no picture
    syncFrames(client, []);
    await captureLocal(client, module.native, browserPanel(), 'capture', target, ''); // the tab has no frame
    useAutomationCalls(null);
    expect(toasts(client).map(toast => [toast.title, toast.description])).toEqual([
      ['Unable to capture screenshot', 'Preview capture failed during screenshot. Check the preview host and target tab before retrying.'],
      ['Unable to capture screenshot', 'Preview capture failed during capture-page. Check the preview host and target tab before retrying.'],
      ['Unable to capture screenshot', 'The preview renderer is unavailable. Reopen the preview tab before retrying.'],
    ]);
    await artifactLocal(client, module.native, 'copy-path', '/x/browser-recording-1.mp4', '');
    expect(toasts(client).at(-1)?.title).toBe('Unable to copy recording path');
  });

  // Review of 2026-10-10: Reveal is `void bridge.revealArtifact(path)` (no toast when it fails); a recording's failed Copy
  // path keeps only Reveal (actionProps: revealAction), while updateScreenshotToast keeps the screenshot's three buttons.
  it('a failed Reveal says nothing; a recording\'s failed Copy path keeps only Reveal, a screenshot\'s keeps its buttons', async () => {
    const client = fakeClient(), dir = '/lane/t3-home/userdata/browser-artifacts';
    fakePage(() => null);
    const saved = fakeModule({ 'browserArtifact:adopt': () => ({ id: 'browser-screenshot-a', path: `${dir}/browser-screenshot-a.png`, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }),
      'browserRecord:capture': () => ({ width: 800, height: 600, frameRate: 30 }), 'browserRecord:begin': () => ({ mimeType: 'video/mp4;codecs=avc1' }),
      'browserRecord:finish': () => ({ path: '/var/folders/x/T/r.mp4', mimeType: 'video/mp4;codecs=avc1', sizeBytes: 9 }),
      'browserRecord:save': request => ({ id: 'browser-recording-b', tabId: request.tab, path: `${dir}/browser-recording-b.mp4`, mimeType: request.mimeType, sizeBytes: 9, createdAt: '2026-10-09T00:00:00.000Z' }) });
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, '');
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, 'record');
    await captureLocal(client, saved.native, browserPanel(), 'capture', target, '');
    useAutomationCalls(null);
    const [shot, recording] = toasts(client);
    expect([shot?.title, recording?.title]).toEqual(['Screenshot saved', 'Recording saved']);
    expect(saved.calls.find(call => call.op === 'browserRecord')).toMatchObject({ frame: frameIdOf(runtimeId) });
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
    // An artifact the module cannot read (the reference: its image would not load).
    await artifactLocal(client, failing.native, 'copy-image', `${dir}/browser-screenshot-elsewhere.png`, '');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Unable to copy screenshot', description: 'Clipboard is busy.' });
  });

  it('records on a Shift-click and stops on the next press, saving the recording with Reveal in Finder and Copy path', async () => {
    const client = fakeClient();
    (client.local.clientSettings as Obj).browserRecordingFrameRate = 60;
    const encoded = '/var/folders/x/T/t3-recording-1.mp4';
    const module = fakeModule({
      'browserRecord:arm': () => ({ armed: true }), 'browserRecord:capture': request => ({ width: 1280, height: 800, frameRate: request.frameRate }),
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
    const module = fakeModule({ 'browserRecord:arm': () => { throw new Error('The page could not be captured.'); } });
    await captureLocal(client, module.native, browserPanel(), 'capture', target, 'record');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Unable to start recording',
      description: 'Preview capture failed during recording-start. Check the preview host and target tab before retrying.' });
    const decorated = fakeClient();
    Object.assign(decorated.local.clientSettings as Obj, { browserRecordingShowKeyPresses: true });
    const ok = fakeModule({ 'browserRecord:capture': () => ({ width: 800, height: 600, frameRate: 30 }), 'browserRecord:decorate': () => ({ active: true }), 'browserRecord:begin': () => ({ mimeType: 'video/mp4;codecs=avc1' }) });
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
    const client = fakeClient({ size: [640, 360], pip: true });
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

  it('handleNativePictureInPicture opens and closes the separate window, and the More menu names it', async () => {
    const client = fakeClient(), module = fakeModule({});
    await captureLocal(client, module.native, browserPanel(), 'window', target, '');
    (client.presentation.browserPages as Obj)[frameIdOf(runtimeId)] = { pip: true };
    await captureLocal(client, module.native, browserPanel(), 'window', target, '');
    expect(module.calls.map(call => [call.action, call.frame])).toEqual([['open', frameIdOf(runtimeId)], ['close', frameIdOf(runtimeId)]]);
    expect(browserView(client, browserPanel().surfaces[0]!).capture.separateWindow).toBe(false); // no session: the empty view
  });
});

// Review of 2026-10-10 (findings 1, 2 and 4): the window's send reads a serial the panel projects whatever it shows, says
// "Annotation attached to draft" when Send cannot go (ChatView onSend's notifyDirectAnnotationAttached), and the
// window-level rules of this task (Float closes the right panel, a screenshot's buttons keep their toast) are held here.
describe('an annotation\'s ⌘↩ at the window (ChatView onSendAnnotation, onSend\'s guard)', () => {
  /** A server thread's client with its Browser tab's frame, and a module that applies a composer insert. */
  function threadClient() {
    const client = new Client(), scoped = { environmentId: 'env', threadId: 't1' }, tab = previewRuntimeTabId(scoped, 'epoch-1', 'tab-1');
    Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1', configLive: true, shellLive: true, threadLive: true,
      scopes: ['orchestration:read', 'orchestration:operate', 'preview:operate'], config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
    client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
    client.shell.threads = [{ id: 't1', projectId: 'p1' }];
    client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
    browserHost(client).store.reconcileServerSessions(scoped, { sessions: [{ threadId: 't1', tabId: 'tab-1', navStatus: { _tag: 'Idle' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-09T00:00:00.000Z' }], serverEpoch: 'epoch-1', revision: 1 });
    syncFrames(client, [{ runtimeId: tab, url: '', environment: 'env', profile: 'default', zoom: 1, colorScheme: 'system' }]);
    const native: Native = { available: true, watch() {}, async later(input) {
      if (obj(input).op === 'editorInsert') return { ok: true, generation: 1, value: { applied: true } };
      return { ok: true, generation: 1, value: {} };
    } };
    return { client, native, tab, target: { ref: scoped, tabId: 'tab-1', runtimeId: tab } };
  }

  // PreviewView unmounts with the panel (usePanelPresence), and its cleanup cancels the pick: in the reference, Annotate
  // and then Float leaves the floating page without the overlay, and a pick there does nothing (review drive, 2026-10-10).
  it('Float or a closed panel ends the tab\'s pick, once (PreviewView\'s unmount: cancelPickElement)', async () => {
    const { client, native, tab, target: own } = threadClient();
    const calls = fakePage(() => new Promise(() => {}));
    const cancels = () => calls.filter(call => call.call === 'evaluate' && String(call.expression).includes('.cancel()')).length;
    await captureLocal(client, native, browserPanel(), 'annotate', own, 'light');
    await panelView(client, native, 0); // the thread's panel is closed
    expect([cancels(), annotateState(client, tab).active]).toEqual([1, false]);
    await panelView(client, native, 0); // asked once
    expect(cancels()).toBe(1);
    // A later pick on a tab the panel shows is left alone.
    await captureLocal(client, native, browserPanel(), 'annotate', own, 'light');
    await cancelHiddenPicks(client, tab);
    expect(cancels()).toBe(1);
    await cancelHiddenPicks(client, null);
    expect(cancels()).toBe(2);
    useAutomationCalls(null);
  });

  it('a pick that settled with ⌘↩ as the panel hid still sends at once: the serial is the panel\'s, not the Browser tab\'s view', async () => {
    const { client, native, target: own } = threadClient();
    fakePage(() => ({ type: 'picked', annotation, rect: null, submission: 'send' }));
    await captureLocal(client, native, browserPanel(), 'annotate', own, 'light');
    await serveRecordingRequests(client, shared(client, native), 0);
    useAutomationCalls(null);
    const view = await panelView(client, native, 0);
    expect(view).toMatchObject({ open: false, annotationSend: 1 });
    expect(view.browser.capture).toEqual(emptyCaptureView());
    expect('sendSerial' in view.browser.capture).toBe(false);
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

  it('no provider: the annotation stays in the draft with the info toast, not the send error', async () => {
    const { client, native, command } = await opened();
    for (const entry of client.config.providers as Obj[]) entry.enabled = false;
    const committed = native.committed.length;
    markAnnotationSend(client);
    const result = await command('send', '', 'Send this one too');
    expect(result.message).toBe('');
    expect(native.committed.length).toBe(committed);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: 'Annotation attached to draft' });
    expect(client.draft).toBe('Send this one too');
  });
});

// Found under real input (a person's keys and pointer, 2026-10-10): what they did
// that agent drives could not show. Each row fails without its fix, except the canvas row ("a resize's pans under one
// serial …"): the canvas did not change, so it passes on the previous code too. It shows why one serial per gesture
// matters; the fix itself, the contract's `pan` taking one serial per gesture, is the line row before it and the drive.
describe('a real Escape, the pill by a real pointer, the drag and the resize, the header tooltip', () => {
  const source = (file: string) => readFileSync(appPath(file), 'utf8');
  const block = (text: string, start: string, end: string) => { const all = text.split('\n'), at = all.findIndex(candidate => candidate.includes(start)); return at < 0 ? '' : all.slice(at, all.findIndex((candidate, index) => index > at && candidate.startsWith(end))).join('\n'); };

  it('the pill follows the player\'s own hover and moves, not an enter of the 12 pt handle', async () => {
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
    expect(player).toContain('  action movePlayer(e: PointerEvent)\n    interacted()\n    moveAt(e.offsetX, e.offsetY)');
    expect(player).toContain('    let at = bcPlayerZone(x, y, frame.width, frame.pillInset, pillWidth, rem)\n    let shown = over and (at == "dot" or (at == "pill" and pill))');
    // The tooltips' slot is the pill's own coordinates: the pill's box sits `pillInset` from the player's top right.
    expect(player).toContain('bcPillSlot(x - (frame.width - frame.pillInset - pillWidth), y - frame.pillInset, rem, mini.recording)');
    expect(player).toContain('  derive pill = dotOver or pillFocus != "" or dragging != ""');
    // Over a frame the player's box lets the pointer through, and the handle's padded box hears the hover and the moves
    // in the player's place, in the player's coordinates.
    expect(player).toContain('  action moveHandle(e: PointerEvent)\n    interacted()\n    moveAt(frame.width - frame.pillInset + grace - handleBoxWidth + e.offsetX, frame.pillInset - grace + e.offsetY)');
    expect(player).toContain('  action overHandle(value: bool)\n    if framed\n      overPlayer(value)');
    expect(player.match(/hover=/g)).toHaveLength(2);
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
});
