// browser-surface part 3 (capture): the chrome row's Annotate, Capture (Shift-click records) and Float preview
// buttons, the More menu's separate window, their toasts and the floating player's browser source (MIT reference,
// see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/PreviewView.tsx `handlePickElement`,
// `handleCapture`, `handlePictureInPicture`, `handleNativePictureInPicture`; PreviewChromeRow.tsx;
// ThreadPreviewMiniPlayer.tsx BrowserMiniPlayer; ChatView.tsx `closePreviewPanel` and `shouldRenderPreviewMiniPlayer`;
// apps/web/src/browser/annotationTheme.ts).
//
// The module does the work (T3BrowserAnnotate.swift, T3BrowserRecorder.swift, T3BrowserCapture.swift); this file
// asks it, keeps the client's recordings (browser-recording.ts) and turns the results into the composer's chip, the
// toasts and the panel's projection. Annotate's result arrives through the module's status (`browserTabs[id].pick`)
// and is applied once per pick by the panel's effects (`applyCaptureResults`), so an answer Exact lets go never loses
// one. The recording's native calls go through the newest answer's native (`host.native`): the reference's lifecycle
// outlives the answer that started it, and a stop sent while the start runs must not wait on a native that was let go.
import type { T3Client } from './client';
import { bridgeReply, ClientError, type Native } from './protocol';
import { obj, str, type Obj } from './domain';
import { letGo } from './let-go';
import { pushToast, toasts, updateToast, type ToastAction } from './toast';
import { markActionCopied } from './shell';
import { composerNow } from './composer-controls';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';
import { activeRef } from './terminal-drawer-view';
import { previewRuntimeTabId } from './browser-state';
import { BrowserRecordings, type RecordingArtifact, type RecordingBlob, type RecordingHost, type RecordingRecorder, type RecordingStream } from './browser-recording';
import { isPreviewAnnotationPayload, markAnnotationSend, previewAnnotationLink, savePreviewAnnotation, type PreviewAnnotationPayload } from './browser-annotation';
import { browserMiniPlayerSource, previewMiniPlayerSourceKey } from './previewMiniPlayerStore';
import { deviceThreadId, miniStoreOf } from './r6-media-device'; // the thread's floating player (previewMiniPlayerStore)
import { contextReferences } from './composer-editor-menu';
import { MAX_ATTACHMENTS } from './composer-editor-files';
import { reservedAttachments } from './composer-editor-attach';
import type { PanelState } from './r4-surfaces-panel';

// ── The annotation theme (annotationTheme.ts readPreviewAnnotationTheme; Manager.ts DEFAULT_ANNOTATION_THEME) ──────
/** The app's tokens for the overlay, in sRGB hex (the recording compositor parses no oklch()). */
export function annotationTheme(scheme: string): Record<string, string> {
  const dark = scheme === 'dark';
  return {
    colorScheme: dark ? 'dark' : 'light', radius: '0.625rem', background: dark ? '#0a0a0a' : '#ffffff', foreground: dark ? '#f5f5f5' : '#27272a',
    popover: dark ? '#171717' : '#ffffff', popoverForeground: dark ? '#f5f5f5' : '#27272a', primary: dark ? '#346bf1' : '#1b4ed8', primaryForeground: '#ffffff',
    muted: dark ? 'rgb(255 255 255 / 4%)' : 'rgb(0 0 0 / 4%)', mutedForeground: dark ? '#a1a1aa' : '#71717b', accent: dark ? 'rgb(255 255 255 / 4%)' : 'rgb(0 0 0 / 4%)',
    accentForeground: dark ? '#f5f5f5' : '#27272a', border: dark ? 'rgb(255 255 255 / 8%)' : 'rgb(0 0 0 / 8%)', input: dark ? 'rgb(255 255 255 / 10%)' : 'rgb(0 0 0 / 10%)',
    ring: dark ? '#346bf1' : '#1b4ed8', fontSans: 'system-ui, sans-serif', fontMono: 'ui-monospace, monospace',
  };
}

// ── The client's capture host ──────────────────────────────────────────────────────────────────
type Host = {
  /** The newest answer's native (panel effects and chrome-row ops set it). */
  native: Native | null;
  recordings: BrowserRecordings;
  /** Runtime tab → the last pick serial applied to the composer. */
  applied: Map<string, number>;
  /** Picks whose submission was ⌘Return: the window's task sends the composer (onSendAnnotation). */
  sendSerial: number;
  /** The scheme the overlay was last started in (the recording's decorations use its primary colour). */
  scheme: string;
  /** The floating tab's page size, as the panel last measured it (the chat canvas sizes the player from it). */
  miniSize: { key: string; width: number; height: number } | null;
};
const hosts = new WeakMap<T3Client, Host>();
export function captureHost(client: T3Client): Host {
  let host = hosts.get(client);
  if (!host) {
    const fresh: Host = { native: null, recordings: null as unknown as BrowserRecordings, applied: new Map(), sendSerial: 0, scheme: 'light', miniSize: null };
    fresh.recordings = new BrowserRecordings(nativeRecordingHost(client, fresh));
    hosts.set(client, fresh);
    host = fresh;
  }
  return host;
}
/** Every answer that can reach the browser hands its native over; calls in flight keep theirs. */
export function adoptCaptureNative(client: T3Client, native: Native): void { captureHost(client).native = native; }

async function nativeCall(host: Host, request: Obj): Promise<Obj> {
  const native = host.native;
  if (!native?.available) throw new ClientError('The browser is unavailable in this window.');
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(str(obj(reply.error).message, 'The browser could not do that.'));
  return obj(reply.value);
}
/** The module's sleep (timelineSleep): the data module has no timers. A sleep whose answer was let go sleeps on through the newest native. */
async function sleep(host: Host, ms: number): Promise<void> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const native = host.native;
    if (!native?.available) return;
    try { await native.later({ op: 'timelineSleep', ms: Math.max(0, Math.round(ms)) }); return; }
    catch (error) { if (!letGo(error) || host.native === native) return; }
  }
}
const num = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) ? value : 0);
/** The H.264 MP4 forms the module's encoder writes (T3BrowserRecording.supports). */
export const NATIVE_RECORDING_TYPES = new Set(['video/mp4;codecs=avc1', 'video/mp4;codecs=avc1.640028', 'video/mp4;codecs=avc1.42e01e', 'video/mp4']);

/** The module's encoder as MediaRecorder: `begin` starts it, `finish` flushes it into a file `save` then moves. */
class NativeRecorder implements RecordingRecorder {
  state: 'inactive' | 'recording' = 'inactive';
  private actual = '';
  private file: RecordingBlob | null = null;
  constructor(private readonly host: Host, private readonly tabId: string, private readonly options: { mimeType?: string; videoBitsPerSecond: number }) {}
  get mimeType(): string { return this.actual || this.options.mimeType || ''; }
  async start(): Promise<void> {
    const value = await nativeCall(this.host, { op: 'browserRecord', tab: this.tabId, action: 'begin', ...(this.options.mimeType ? { mimeType: this.options.mimeType } : {}), bitsPerSecond: this.options.videoBitsPerSecond });
    this.actual = str(value.mimeType);
    this.state = 'recording';
  }
  async stop(): Promise<void> {
    const value = await nativeCall(this.host, { op: 'browserRecord', tab: this.tabId, action: 'finish' });
    this.state = 'inactive';
    if (str(value.mimeType)) this.actual = str(value.mimeType);
    this.file = { type: this.actual, size: num(value.sizeBytes), path: str(value.path) };
  }
  output(): RecordingBlob | null { return this.file; }
}

/** The recording defaults (contracts settings.ts names and defaults; their Settings rows are part 4's). */
export function browserRecordingSettings(client: T3Client): { browserRecordingFrameRate: 30 | 60; browserRecordingShowKeyPresses: boolean; browserRecordingShowMousePresses: boolean } {
  const prefs = obj((client.local as unknown as { clientSettings?: Obj } | undefined)?.clientSettings);
  return { browserRecordingFrameRate: prefs.browserRecordingFrameRate === 60 ? 60 : 30, browserRecordingShowKeyPresses: prefs.browserRecordingShowKeyPresses === true,
    browserRecordingShowMousePresses: prefs.browserRecordingShowMousePresses === true };
}
/** browserDefaults.ts autoShowFloatingPreview (default on): whether an agent's preview floats (read by part 5). */
export function browserAutoShowFloatingPreview(client: T3Client): boolean {
  return obj((client.local as unknown as { clientSettings?: Obj } | undefined)?.clientSettings).browserAutoShowFloatingPreview !== false;
}
const recordingSettings = browserRecordingSettings;

function nativeRecordingHost(client: T3Client, host: Host): RecordingHost {
  const record = (tabId: string, action: string, extra: Obj = {}) => nativeCall(host, { op: 'browserRecord', tab: tabId, action, ...extra });
  let frames = 0;
  return {
    bridge: {
      startScreencast: async tabId => {
        const settings = recordingSettings(client);
        await record(tabId, 'arm', { showKeyPresses: settings.browserRecordingShowKeyPresses, showMousePresses: settings.browserRecordingShowMousePresses,
          theme: annotationTheme(host.scheme), controller: 'none' });
        // The armed module hands the stream to this tab only (the reference's capture trigger).
        host.recordings.triggerTabMediaCapture(tabId);
      },
      stopScreencast: async tabId => { await record(tabId, 'disarm'); },
      save: async (tabId, mimeType, data) => {
        const value = await record(tabId, 'save', { mimeType, path: data.path });
        return { id: str(value.id), tabId: str(value.tabId, tabId), path: str(value.path), mimeType: str(value.mimeType, mimeType), sizeBytes: num(value.sizeBytes), createdAt: str(value.createdAt) };
      },
    },
    getDisplayMedia: async (tabId, constraints) => {
      const value = await record(tabId, 'capture', { frameRate: constraints.video.frameRate.max });
      const settings = { width: num(value.width), height: num(value.height), frameRate: num(value.frameRate) };
      let stopped = false;
      const stream: RecordingStream = { getVideoTracks: () => [{ getSettings: () => settings }],
        getTracks: () => [{ getSettings: () => settings, stop: () => { if (stopped) return; stopped = true; void record(tabId, 'release').catch(() => undefined); } }] };
      return stream;
    },
    isTypeSupported: type => NATIVE_RECORDING_TYPES.has(type),
    createRecorder: (tabId, _stream, options) => new NativeRecorder(host, tabId, options),
    createCompositor: async (tabId, stream, options) => {
      if (!options.showKeyPresses && !options.showMousePresses) return null;
      const value = await record(tabId, 'decorate', { showKeyPresses: options.showKeyPresses, showMousePresses: options.showMousePresses, primaryColor: annotationTheme(host.scheme).primary });
      return value.active === true ? { stream, dispose: () => undefined } : null;
    },
    hydrateSettings: async () => undefined,
    settings: () => recordingSettings(client),
    // The module holds the page painting from `arm` until the stream is released (T3BrowserParking).
    acquireSurfaceActivity: () => () => undefined,
    // The panel reads the recordings at its next projection.
    publish: () => undefined,
    requestAnimationFrame: callback => { const id = ++frames; void sleep(host, 16).then(() => callback(0)); return id; },
    cancelAnimationFrame: () => undefined,
    setTimeout: (fn, ms) => { const handle = { cancelled: false }; void sleep(host, ms).then(() => { if (!handle.cancelled) fn(); }); return handle; },
    clearTimeout: handle => { if (handle && typeof handle === 'object') (handle as { cancelled: boolean }).cancelled = true; },
    now: () => new Date(composerNow(client) || 0).toISOString(),
  };
}

// ── The panel's projection (PreviewView, PreviewChromeRow) ─────────────────────────────────────────
export type BrowserCaptureView = {
  pickActive: boolean; pickDisabled: boolean; pickTip: string;
  captureDisabled: boolean; recording: boolean;
  floating: boolean; floatDisabled: boolean;
  separateWindow: boolean;
};
export const emptyCaptureView = (): BrowserCaptureView => ({ pickActive: false, pickDisabled: true, pickTip: 'Annotate elements, regions, and drawings', captureDisabled: true,
  recording: false, floating: false, floatDisabled: true, separateWindow: false });
const report = (client: T3Client, runtimeId: string): Obj => obj(obj(client.presentation.browserTabs)[runtimeId]);
/** The open thread's key in the floating player's store, the device player's own (`deviceThreadId`, activeThreadRef): a
 *  new thread's draft has its id once it opened a tab (addBrowserSurface allocates it), while `client.threadId` stays
 *  empty until the draft is sent. */
function playerThreadKey(client: T3Client): string { return deviceThreadId(client); } // a call, not an alias: the modules import each other
/** selectThreadPreviewMiniPlayerTabId for the open thread: the floating browser tab, if one floats. */
export function floatingTabOf(client: T3Client): string | null {
  const key = playerThreadKey(client), source = key ? miniStoreOf(client).get(key)?.source : undefined;
  return source?.kind === 'browser' ? source.tabId : null;
}

export function recordingOf(client: T3Client, ref: ScopedThreadRef, tabId: string, runtimeId: string): boolean {
  const recordings = captureHost(client).recordings;
  return recordings.readActiveBrowserRecordingTabIds().has(runtimeId) || recordings.findActiveBrowserRecordingRuntimeTabId(ref, tabId) !== null;
}

export function browserCaptureView(client: T3Client, ref: ScopedThreadRef, tabId: string, runtimeId: string, hasWebContents: boolean, failed: boolean): BrowserCaptureView {
  const tab = report(client, runtimeId), pickActive = obj(tab.pick).active === true;
  return {
    pickActive, pickDisabled: !tabId || failed,
    // PreviewChromeRow: the disabled reason, else Cancel while annotating.
    pickTip: failed ? 'Page didn’t load — pick unavailable until the page renders' : pickActive ? 'Cancel annotation (Esc)' : 'Annotate elements, regions, and drawings',
    captureDisabled: !hasWebContents || failed, recording: recordingOf(client, ref, tabId, runtimeId),
    floating: floatingTabOf(client) === tabId, floatDisabled: !hasWebContents || failed,
    separateWindow: tab.pip === true,
  };
}
/** Bumps when a ⌘Return annotation should send: the panel projects it (`shell.panel.annotationSend`) whatever it shows, so
 *  a pick finished in the floating player or with the panel closed sends at once, as onSendAnnotation does (review of
 *  2026-10-10: projected in the Browser tab's view only, such a send waited until a Browser tab showed again). */
export function annotationSendSerial(client: T3Client): number { return captureHost(client).sendSerial; }

// ── The chrome row's ops (`surface-browser-annotate`, `-capture`, `-float`, `-window`) ─────────────
export type CaptureTarget = { ref: ScopedThreadRef; tabId: string; runtimeId: string };
export async function captureLocal(client: T3Client, native: Native, state: PanelState, op: string, target: CaptureTarget, value: string): Promise<boolean> {
  const host = captureHost(client);
  host.native = native;
  switch (op) {
    case 'annotate': await toggleAnnotate(client, host, target, value); return true;
    case 'capture': await capture(client, host, target, value === 'record'); return true;
    case 'float': toggleFloat(client, state, target); return true;
    case 'window': await toggleSeparateWindow(client, host, target); return true;
    default: return false;
  }
}

/** handlePickElement: a second press cancels; the result arrives through the module's status. */
async function toggleAnnotate(client: T3Client, host: Host, target: CaptureTarget, scheme: string): Promise<void> {
  if (obj(report(client, target.runtimeId).pick).active === true) { await nativeCall(host, { op: 'browserAnnotate', tab: target.runtimeId, action: 'cancel' }).catch(error => { if (letGo(error)) throw error; }); return; }
  host.scheme = scheme === 'dark' ? 'dark' : 'light';
  await nativeCall(host, { op: 'browserAnnotate', tab: target.runtimeId, action: 'start', theme: annotationTheme(host.scheme) });
}

const errorText = (error: unknown) => (error instanceof Error && error.message ? error.message : 'An error occurred.');
/** handleCapture: a recording tab stops; Shift-click records; a click saves a screenshot. */
async function capture(client: T3Client, host: Host, target: CaptureTarget, record: boolean): Promise<void> {
  const recordings = host.recordings;
  const recordingId = recordings.readActiveBrowserRecordingTabIds().has(target.runtimeId) ? target.runtimeId : recordings.findActiveBrowserRecordingRuntimeTabId(target.ref, target.tabId);
  if (recordingId) {
    try {
      const artifact = await recordings.stopBrowserRecording(recordingId);
      if (artifact) recordingSavedToast(client, artifact);
    } catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Unable to stop recording', description: errorText(error) }); }
    return;
  }
  if (record) {
    try { await recordings.startBrowserRecording(target.runtimeId, target.ref, target.tabId); }
    catch (error) {
      if (letGo(error)) throw error;
      if (error instanceof Error && error.name === 'BrowserRecordingStartCancelledError') return;
      pushToast(client, { kind: 'error', title: 'Unable to start recording', description: errorText(error) });
    }
    return;
  }
  let artifact: RecordingArtifact;
  try {
    const value = await nativeCall(host, { op: 'browserScreenshot', tab: target.runtimeId });
    artifact = { id: str(value.id), tabId: target.runtimeId, path: str(value.path), mimeType: 'image/png', sizeBytes: num(value.sizeBytes), createdAt: str(value.createdAt) };
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to capture screenshot', description: errorText(error) });
    return;
  }
  screenshotSavedToast(client, artifact);
}

const artifactAction = (label: string, action: string, path: string): ToastAction => ({ label, op: `shelllocal:surface-browser-artifact-${action}`, id: path });
/** The stacked "Screenshot saved" toast: Copy path, Reveal in Finder, then Copy image (toast.tsx's order). */
function screenshotSavedToast(client: T3Client, artifact: RecordingArtifact): void {
  const id = pushToast(client, { kind: 'success', title: 'Screenshot saved', stacked: true, action: artifactAction('Copy image', 'copy-image', artifact.path),
    extra: artifactAction('Copy path', 'copy-path', artifact.path), secondary: artifactAction('Reveal in Finder', 'reveal', artifact.path), secondaryVariant: 'outline' });
  tagToast(client, id);
}
/** The stacked "Recording saved" toast: Copy path, then Reveal in Finder. */
function recordingSavedToast(client: T3Client, artifact: RecordingArtifact): void {
  const id = pushToast(client, { kind: 'success', title: 'Recording saved', stacked: true, action: artifactAction('Reveal in Finder', 'reveal', artifact.path),
    secondary: artifactAction('Copy path', 'copy-path', artifact.path), secondaryVariant: 'outline' });
  tagToast(client, id);
}
/** The buttons carry their toast's id, so a copy can say "Copied!" on its own button. */
function tagToast(client: T3Client, id: number): void {
  const toast = toasts(client).find(entry => entry.id === id);
  if (!toast) return;
  const tag = (action: ToastAction | null | undefined): ToastAction | null => (action ? { ...action, value: String(id) } : null);
  updateToast(client, id, { action: tag(toast.action), secondary: tag(toast.secondary), extra: tag(toast.extra) });
}

/** A toast's artifact buttons (`surface-browser-artifact-*`; id = the file, value = the toast). */
export async function artifactLocal(client: T3Client, native: Native, action: string, path: string, toastValue: string): Promise<string> {
  const host = captureHost(client);
  host.native = native;
  const toastId = Number(toastValue) || 0;
  try {
    await nativeCall(host, { op: 'browserArtifact', action, path });
    if (action !== 'reveal' && toastId > 0) markActionCopied(client, toastId, `shelllocal:surface-browser-artifact-${action}`);
  } catch (error) {
    if (letGo(error)) throw error;
    if (action === 'reveal') return ''; // `void bridge.revealArtifact(path)`: the reference says nothing when Reveal fails
    const recording = /browser-recording-/.test(path);
    const title = action === 'copy-image' ? 'Unable to copy screenshot' : recording ? 'Unable to copy recording path' : 'Unable to copy screenshot path';
    // updateScreenshotToast keeps the screenshot's three buttons; the recording's error toast keeps only Reveal (actionProps).
    if (toastId > 0) updateToast(client, toastId, { kind: 'error', title, description: errorText(error), ...(recording ? { secondary: null } : {}) });
    else pushToast(client, { kind: 'error', title, description: errorText(error) });
  }
  return '';
}

/** handlePictureInPicture: the floating player takes the tab and the panel closes; again closes the player. */
function toggleFloat(client: T3Client, state: PanelState, target: CaptureTarget): void {
  const key = playerThreadKey(client);
  if (!key) return;
  const store = miniStoreOf(client);
  if (floatingTabOf(client) === target.tabId) { store.close(key); return; }
  store.open(key, browserMiniPlayerSource(target.tabId));
  state.visible = false;
  client.diffOpen = false;
}

/** handleNativePictureInPicture: the separate preview window opens or closes. */
async function toggleSeparateWindow(client: T3Client, host: Host, target: Pick<CaptureTarget, 'runtimeId'>): Promise<void> {
  const open = report(client, target.runtimeId).pip === true;
  try { await nativeCall(host, { op: 'browserPip', tab: target.runtimeId, action: open ? 'close' : 'open' }); }
  catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Unable to update popped-out preview', description: errorText(error) }); }
}

// ── Annotate's results (the panel's effects) ───────────────────────────────────────────────────
/** Each tab's settled pick, applied once: the chip and the crop in the tab's composer, the dropped-crop toast, and a send. */
export async function applyCaptureResults(client: T3Client, native: Native, liveTabs: ReadonlyArray<{ runtimeId: string; threadKey: string }>): Promise<void> {
  const host = captureHost(client);
  host.native = native;
  for (const { runtimeId, threadKey } of liveTabs) {
    const pick = obj(report(client, runtimeId).pick), serial = num(pick.serial);
    if (pick.ready !== true || serial <= (host.applied.get(runtimeId) ?? 0)) continue;
    const reply = await nativeCall(host, { op: 'browserAnnotate', tab: runtimeId, action: 'take', serial });
    const result = obj(reply.result);
    const outcome = !reply.result || result.cancelled === true ? 'cancelled' : await applyAnnotation(client, native, host, threadKey, result);
    host.applied.set(runtimeId, serial);
    // The module's log says what became of each pick (`t3.browser: annotate applied <serial> <outcome>`).
    await nativeCall(host, { op: 'browserAnnotate', tab: runtimeId, action: 'applied', serial, outcome }).catch(error => { if (letGo(error)) throw error; });
  }
}

async function applyAnnotation(client: T3Client, native: Native, host: Host, threadKey: string, result: Obj): Promise<string> {
  const annotation = result.annotation;
  if (!isPreviewAnnotationPayload(annotation)) return 'invalid'; // PickedElementPayload's validator: a malformed pick is dropped silently
  // The open thread's composer (a new thread's draft included, whose tab names its allocated id), else that thread's draft.
  const ref = parseScopedThreadKey(threadKey), open = activeRef(client);
  const draftKey = !ref || (open && scopedThreadKey(open) === threadKey) ? client.draftKey : `${ref.environmentId}:${ref.threadId}`;
  const screenshot = obj(result.screenshot), imageId = str(screenshot.id);
  // The crop goes on the shelf as a draft image the send uploads; a full shelf drops it, as a failed crop does.
  const room = draftKey !== client.draftKey || reservedAttachments(client) < MAX_ATTACHMENTS;
  const image = imageId && /^[a-f0-9-]{36}$/i.test(imageId) && room ? { id: imageId, name: str(screenshot.name, `preview-annotation-${annotation.id}.png`), mimeType: 'image/png', sizeBytes: num(screenshot.sizeBytes) } : null;
  const cropDropped = result.screenshotFailed === true || (!!imageId && !image);
  savePreviewAnnotation(client, annotation, image?.id ?? '');
  if (image) client.local.snapshotDrafts[draftKey] = [...(client.local.snapshotDrafts[draftKey] ?? []).filter(entry => str(entry.id) !== image.id), image];
  const placed = await insertAnnotationChip(client, native, draftKey, annotation);
  if (cropDropped) pushToast(client, { kind: 'error', title: 'Could not capture the picked element', description: 'The annotation was kept without the screenshot.', stacked: true });
  if (result.submission === 'send' && draftKey === client.draftKey) { markAnnotationSend(client); host.sendSerial += 1; } // a foreground send (browser-annotation.ts)
  return `${placed}${image ? '+image' : ''}${cropDropped ? ' crop-dropped' : ''}${result.submission === 'send' ? ' send' : ''}`;
}

/** addPreviewAnnotation: the chip at the composer's caret (once), else at the end of the stored draft. */
async function insertAnnotationChip(client: T3Client, native: Native, draftKey: string, annotation: PreviewAnnotationPayload): Promise<string> {
  const link = previewAnnotationLink(annotation), id = link.slice(link.lastIndexOf('/') + 1, -1);
  const present = (text: string) => contextReferences(text).some(reference => reference.kind === 'preview-annotation' && reference.id === id);
  if (draftKey === client.draftKey) {
    const state = await bridgeReply(native, { op: 'editorState' }).catch(() => null);
    if (state?.ok && present(str(obj(state.value).text))) return 'chip';
    const inserted = await bridgeReply(native, { op: 'editorInsert', text: link }).catch(() => null);
    if (inserted?.ok && obj(inserted.value).applied === true) { await bridgeReply(native, { op: 'editorFocus' }).catch(() => null); return 'chip'; }
  }
  const prompt = client.local.drafts[draftKey] ?? '';
  if (present(prompt)) return 'draft';
  client.local.drafts[draftKey] = `${prompt}${prompt && !/\s$/.test(prompt) ? ' ' : ''}${link} `;
  return 'draft';
}

// ── The floating player's browser source (ThreadPreviewMiniPlayer BrowserMiniPlayer) ─────────────
export type BrowserMiniView = {
  show: boolean; key: string; sourceKey: string; tabId: string; runtimeId: string; url: string; profileId: string; environment: string;
  hasWebContents: boolean; recording: boolean; separateWindow: boolean; width: number; height: number;
};
export const emptyBrowserMini = (): BrowserMiniView => ({ show: false, key: '', sourceKey: '', tabId: '', runtimeId: '', url: '', profileId: 'default', environment: '',
  hasWebContents: false, recording: false, separateWindow: false, width: 1280, height: 800 });

/** The page's last laid-out size (resolveFittedBrowserViewport with the fill viewport): its box, else 1280×800. */
export function browserMiniSourceSize(client: T3Client, runtimeId: string): { width: number; height: number } {
  const raw = report(client, runtimeId).size, size = Array.isArray(raw) ? raw.map(Number) : [];
  return size.length === 2 && size[0]! > 1 && size[1]! > 1 ? { width: size[0]!, height: size[1]! } : { width: 1280, height: 800 };
}

/** The tab the floating player shows, unless the panel shows it (shouldRenderPreviewMiniPlayer). */
export function browserMiniView(client: T3Client, sessions: (ref: ScopedThreadRef) => { serverEpoch: string | null; tabs: Record<string, { url: string; profileId: string }> },
  shownTabId: string | null): BrowserMiniView {
  const tabId = floatingTabOf(client), ref = activeRef(client);
  if (!tabId || !ref || shownTabId === tabId) return emptyBrowserMini();
  const state = sessions(ref), session = state.tabs[tabId];
  if (!session) return emptyBrowserMini();
  const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, tabId), tab = report(client, runtimeId), size = browserMiniSourceSize(client, runtimeId);
  const key = previewMiniPlayerSourceKey(browserMiniPlayerSource(tabId));
  captureHost(client).miniSize = { key, ...size };
  return { show: true, key, sourceKey: key, tabId, runtimeId, url: str(tab.url) || session.url, profileId: session.profileId, environment: ref.environmentId,
    hasWebContents: Object.keys(tab).length > 0, recording: recordingOf(client, ref, tabId, runtimeId), separateWindow: tab.pip === true, ...size };
}

/** `surface-browser-mini-*` (id = the floating tab's runtime id): Open in right panel, Pop into separate window, Close. */
export async function browserMiniLocal(client: T3Client, native: Native, op: string, runtimeId: string, reopen: (tabId: string) => void): Promise<string> {
  const tabId = floatingTabOf(client), key = playerThreadKey(client);
  if (!tabId || !key) return '';
  const host = captureHost(client);
  host.native = native;
  if (op === 'close') miniStoreOf(client).close(key);
  else if (op === 'restore') { miniStoreOf(client).close(key); reopen(tabId); }
  else if (op === 'window' && runtimeId) await toggleSeparateWindow(client, host, { runtimeId });
  return '';
}

/** resolvePreviewMiniPlayerSourceSize for the floating tab (the fill viewport: the page's box), for the chat canvas. */
export function floatingBrowserSize(client: T3Client, key: string): { width: number; height: number } {
  const size = captureHost(client).miniSize;
  return size && size.key === key ? { width: size.width, height: size.height } : { width: 1280, height: 800 };
}
