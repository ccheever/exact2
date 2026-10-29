import { requestComposerField } from '../composer/composer-field-request';
// The chrome row's Annotate, Capture (Shift-click records) and Float preview
// buttons, the More menu's separate window, their toasts and the floating player's browser source (MIT reference,
// see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/PreviewView.tsx `handlePickElement`,
// `handleCapture`, `handlePictureInPicture`, `handleNativePictureInPicture`; PreviewChromeRow.tsx;
// ThreadPreviewMiniPlayer.tsx BrowserMiniPlayer; ChatView.tsx `closePreviewPanel` and `shouldRenderPreviewMiniPlayer`;
// apps/web/src/browser/annotationTheme.ts; T3 Code 74541bab17: apps/desktop/src/preview/captureErrors.ts, which words a
// failed screenshot or recording start).
//
// Annotate is data code over the tab's frame (browser-annotate.ts); a screenshot is the frame's `captureScreenshot`,
// moved into the artifact directory by the module; the recording and the separate window are the module's, over the web
// view the frame hands its hatch (T3BrowserPage.swift). This file asks them, keeps the client's recordings
// (browser-recording.ts) and turns the results into the composer's chip, the toasts and the panel's projection. The
// recording's native calls go through the newest answer's native (`host.native`): the reference's lifecycle outlives the
// answer that started it, and a stop sent while the start runs must not wait on a native that was let go.
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { arr, obj, str, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';
import { pushToast, toasts, updateToast, type ToastAction } from '../shell/toast';
import { markActionCopied } from '../shell/shell';
import { composerNow } from '../composer/composer-controls';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from '../terminal/terminal-ui-state';
import { activeRef } from '../terminal/terminal-drawer-view';
import { serverStreamUrl, serverStreamFrame } from './server-browser-runtime';
import { previewRuntimeTabId } from './browser-state';
import { readWindowZoom } from './browser-viewport';
import { BrowserRecordings, PreviewCaptureFailure, type PreviewCaptureFailureStage, type RecordingArtifact, type RecordingBlob, type RecordingHost, type RecordingRecorder, type RecordingStream } from './browser-recording';
import { isPreviewAnnotationPayload, previewAnnotationLink, savePreviewAnnotation, type PreviewAnnotationPayload } from './browser-annotation';
import { composerFieldOwner } from '../composer/auto-balance-owner';
import { composerFieldCurrent, composerFieldInputSerial } from '../composer/composer-field-bridge';
import type { ComposerFieldSnapshot } from '../composer/composer-field-commands';
import { composerFieldReplacement } from '../composer/composer-field-edit';
import { composerSourceSession } from '../composer/composer-rich-state';
import { finishAttachmentEffect, prepareAttachmentEffect } from '../composer/composer-attachment-receipts';
import { browserMiniPlayerSource, previewMiniPlayerSourceKey } from '../device/previewMiniPlayerStore';
import { deviceThreadId, miniStoreOf } from '../device/device-session'; // the thread's floating player (previewMiniPlayerStore)
import { contextReferences } from '../composer/composer-editor-menu';
import { MAX_ATTACHMENTS } from '../composer/composer-editor-files';
import { reservedAttachments } from '../composer/composer-editor-attach';
import type { PanelState } from '../panel/panel-surfaces';
import { previewPdfAssetUrl } from './browser-url';
import { appFiles } from '../media/save-file';
import { copyImageFile } from '../media/media-copy-image';
import { frameEntry, frameIdOf } from './browser-frames';
import { agentDriving } from './browser-automation-run';
import { fleet } from '../connections/settings-b-fleet';
import { shellRuntime, sidebarStatus } from '../sidebar/sidebar-model';
import { activePicks, annotateState, cancelAnnotate, startAnnotate, type ApplyPick } from './browser-annotate';
import { automationCalls } from './browser-automation-run';

// ── The annotation theme (annotationTheme.ts readPreviewAnnotationTheme; Manager.ts DEFAULT_ANNOTATION_THEME) ──────
/** The app's tokens for the overlay, in sRGB hex (the recording compositor parses no oklch()). */
export function annotationTheme(scheme: string): Record<string, string> {
  const dark = scheme === 'dark';
  return {
    colorScheme: dark ? 'dark' : 'light', radius: '0.5rem', background: dark ? '#000000' : '#ffffff', foreground: dark ? '#f5f5f5' : '#18181b',
    popover: dark ? '#171717' : '#ffffff', popoverForeground: dark ? '#f5f5f5' : '#18181b', primary: dark ? '#346bf1' : '#1b4ed8', primaryForeground: '#ffffff',
    muted: dark ? 'rgb(255 255 255 / 4%)' : 'rgb(0 0 0 / 4%)', mutedForeground: dark ? '#a1a1aa' : '#71717b', accent: dark ? 'rgb(255 255 255 / 4%)' : 'rgb(0 0 0 / 4%)',
    accentForeground: dark ? '#f5f5f5' : '#18181b', border: dark ? 'rgb(255 255 255 / 8%)' : 'rgb(0 0 0 / 8%)', input: dark ? 'rgb(255 255 255 / 10%)' : 'rgb(0 0 0 / 10%)',
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
  sendTarget: AnnotationDispatch | null;
  /** Runtime tab → the settled-pick serial whose active pick was cancelled because the panel stopped showing the tab. */
  cancelled: Map<string, number>;
  /** The scheme the overlay was last started in (the recording's decorations use its primary colour). */
  scheme: string;
  /** The floating tab's page size, as the panel last measured it (the chat canvas sizes the player from it). */
  miniSize: { key: string; width: number; height: number } | null;
  /** Screenshots taken, which name each capture's file under app:/tmp/browser-shots. */
  shots: number;
};
const hosts = new WeakMap<T3Client, Host>();
export function captureHost(client: T3Client): Host {
  let host = hosts.get(client);
  if (!host) {
    const fresh: Host = { native: null, recordings: null as unknown as BrowserRecordings, applied: new Map(), sendSerial: 0, sendTarget: null, cancelled: new Map(), scheme: 'light', miniSize: null, shots: 0 };
    fresh.recordings = new BrowserRecordings(nativeRecordingHost(client, fresh));
    hosts.set(client, fresh);
    host = fresh;
  }
  return host;
}
/** Every answer that can reach the browser hands its native over; calls in flight keep theirs. */
export function adoptCaptureNative(client: T3Client, native: Native): void { captureHost(client).native = native; }
/** A module op through the newest answer's native (the agent's recordings, browser-automation-recording.ts). */
export const captureNativeCall = (client: T3Client, request: Obj): Promise<Obj> => nativeCall(captureHost(client), request);

async function nativeCall(host: Host, request: Obj, stage?: PreviewCaptureFailureStage): Promise<Obj> {
  const native = host.native;
  // The page ops name the tab's frame (T3BrowserPage.swift keys a page by its element id).
  if (typeof request.tab === 'string' && (request.op === 'browserRecord' || request.op === 'browserPip')) request = { ...request, frame: frameIdOf(request.tab) };
  if (!native?.available) {
    if (stage) throw new PreviewCaptureFailure('renderer-unavailable', stage);
    throw new ClientError('The browser is unavailable in this window.');
  }
  const reply = await bridgeReply(native, request);
  if (!reply.ok) {
    if (stage) throw captureFailure(str(obj(reply.error).kind), stage);
    throw new ClientError(str(obj(reply.error).message, 'The browser could not do that.'));
  }
  return obj(reply.value);
}
/** captureErrors.ts normalizeCaptureFailure over the module's refusals (T3BrowserCapture.swift): a tab that is gone, a
 *  snapshot WebKit did not give (`CapturePage`), or a failure at the stage the call names. */
export function captureFailure(kind: string, stage: PreviewCaptureFailureStage): PreviewCaptureFailure {
  if (kind === 'NotFound') return new PreviewCaptureFailure('renderer-unavailable', stage);
  if (kind === 'CapturePage') return new PreviewCaptureFailure('capture-failed', 'capture-page');
  return new PreviewCaptureFailure('capture-failed', stage);
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

/** The recording defaults (contracts settings.ts names and defaults; their Settings rows are browser-defaults.ts's). */
export function browserRecordingSettings(client: T3Client): { browserRecordingFrameRate: 30 | 60; browserRecordingShowKeyPresses: boolean; browserRecordingShowMousePresses: boolean } {
  const prefs = obj((client.local as unknown as { clientSettings?: Obj } | undefined)?.clientSettings);
  return { browserRecordingFrameRate: prefs.browserRecordingFrameRate === 60 ? 60 : 30, browserRecordingShowKeyPresses: prefs.browserRecordingShowKeyPresses === true,
    browserRecordingShowMousePresses: prefs.browserRecordingShowMousePresses === true };
}
/** browserDefaults.ts autoShowFloatingPreview (default on): whether an agent's preview floats (read by browser-automation.ts). */
export function browserAutoShowFloatingPreview(client: T3Client): boolean {
  return obj((client.local as unknown as { clientSettings?: Obj } | undefined)?.clientSettings).browserAutoShowFloatingPreview !== false;
}
const recordingSettings = browserRecordingSettings;

function nativeRecordingHost(client: T3Client, host: Host): RecordingHost {
  const record = (tabId: string, action: string, extra: Obj = {}) => nativeCall(host, { op: 'browserRecord', tab: tabId, action, ...extra });
  return {
    bridge: {
      startScreencast: async tabId => {
        const settings = recordingSettings(client);
        await nativeCall(host, { op: 'browserRecord', tab: tabId, action: 'arm', showKeyPresses: settings.browserRecordingShowKeyPresses,
          showMousePresses: settings.browserRecordingShowMousePresses, theme: annotationTheme(host.scheme), controller: 'none' }, 'recording-start');
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
    // A hidden tab's frame stays mounted, and its snapshots still paint (LLP 1020.002 stage 1).
    acquireSurfaceActivity: () => () => undefined,
    // The panel reads the recordings at its next projection.
    publish: () => undefined,
    // The recorder's answer's own timers (LLP 1027.000 D1). A data module has no frame callbacks: a frame is 16 ms.
    requestAnimationFrame: callback => setTimeout(() => callback(0), 16) as unknown as number,
    cancelAnimationFrame: id => clearTimeout(id),
    setTimeout: (fn, ms) => setTimeout(fn, ms),
    clearTimeout: handle => clearTimeout(handle as ReturnType<typeof setTimeout>),
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
/** The tab's page as its hatch reports it (T3BrowserPage.swift `presentation.browserPages`): its size and separate window. */
const report = (client: T3Client, runtimeId: string): Obj => obj(obj(client.presentation.browserPages)[frameIdOf(runtimeId)]);
/** The open thread's key in the floating player's store, the device player's own (`deviceThreadId`, activeThreadRef): a
 *  new thread's draft has its id once it opened a tab (addBrowserSurface allocates it), while `client.threadId` stays
 *  empty until the draft is sent. */
function playerThreadKey(client: T3Client): string { return deviceThreadId(client); } // a call, not an alias: the modules import each other
/** selectThreadPreviewMiniPlayerTabId for the open thread: the floating browser tab, if one floats. */
export function floatingTabOf(client: T3Client): string | null {
  const key = playerThreadKey(client), source = key ? miniStoreOf(client).get(key)?.source : undefined;
  return source?.kind === 'browser' ? source.tabId : null;
}

/** A hidden tab's page that must keep drawing (browser-frames.ts FrameHold): the hatch records it or shows it in its
 *  separate window, an agent's recording holds it, or the agent drives it in its turn. */
export function heldFrame(client: T3Client, runtimeId: string): { width: number; height: number } | null {
  const tab = report(client, runtimeId), recording = obj(tab.recording);
  const held = tab.pip === true || recording.armed === true || recording.capturing === true || recording.recording === true
    || captureHost(client).recordings.readActiveBrowserRecordingTabIds().has(runtimeId)
    || agentDriving(client, runtimeId, (environmentId, threadId) => threadTurnRunning(client, environmentId, threadId));
  return held ? browserMiniSourceSize(client, runtimeId) : null;
}

/**
 * The thread's agent turn runs (the reference's PreviewAgentActivity, ended by RunExecutionService's agentTurnsSettled):
 * the root turn works, waits on the person or on another thread (t3_thread_wait), or its run is parked on subagents, the
 * child turns agentTurnsSettled waits for. A parked monitor or background task is no turn: it can be a dev server's
 * watchdog.
 */
export function threadTurnRunning(client: T3Client, environmentId: string, threadId: string): boolean {
  const shell = environmentId === client.environmentId ? client.shell : [...fleet.entries.values()].find(entry => entry.environmentId === environmentId)?.shell;
  const thread = shell?.threads.find(entry => entry.id === threadId);
  if (!thread) return false;
  const status = sidebarStatus(thread);
  if (status === 'working' || status === 'approval' || status === 'input') return true;
  if (status !== 'waiting') return false;
  return shellRuntime(thread)?.status === 'running' || arr(thread.pendingBackgroundTasks).some(task => str(task.kind) === 'subagent');
}

export function recordingOf(client: T3Client, ref: ScopedThreadRef, tabId: string, runtimeId: string): boolean {
  const recordings = captureHost(client).recordings;
  return recordings.readActiveBrowserRecordingTabIds().has(runtimeId) || recordings.findActiveBrowserRecordingRuntimeTabId(ref, tabId) !== null;
}

export function browserCaptureView(client: T3Client, ref: ScopedThreadRef, tabId: string, runtimeId: string, hasWebContents: boolean, failed: boolean): BrowserCaptureView {
  const tab = report(client, runtimeId), pickActive = annotateState(client, runtimeId).active;
  return {
    pickActive, pickDisabled: !tabId || failed,
    // PreviewChromeRow: the disabled reason, else Cancel while annotating.
    pickTip: failed ? 'Page didn\'t load. Pick is unavailable until the page renders' : pickActive ? 'Cancel annotation (Esc)' : 'Annotate elements, regions, and drawings',
    captureDisabled: !hasWebContents || failed, recording: recordingOf(client, ref, tabId, runtimeId),
    floating: floatingTabOf(client) === tabId, floatDisabled: !hasWebContents || failed,
    separateWindow: tab.pip === true,
  };
}
/** Bumps when a ⌘Return annotation should send: the panel projects it (`shell.panel.annotationSend`) whatever it shows, so
 *  a pick finished in the floating player or with the panel closed sends at once, as onSendAnnotation does (review of
 *  2026-10-10: projected in the Browser tab's view only, such a send waited until a Browser tab showed again). */
export function annotationSendSerial(client: T3Client): number { return captureHost(client).sendSerial; }
export type AnnotationDispatch = ComposerFieldSnapshot & { serial: number; draftKey: string; inputSerial: number };
export function commitAnnotationSend(client: T3Client): void {
  const current = composerFieldCurrent(client, composerFieldOwner(client));
  if (!current || current.composing) return;
  const host = captureHost(client);
  host.sendTarget = { ...current, draftKey: client.draftKey, serial: ++host.sendSerial, inputSerial: composerFieldInputSerial(client) };
}
export function annotationDispatchCurrent(client: T3Client, target: AnnotationDispatch): boolean {
  const current = composerFieldCurrent(client, composerFieldOwner(client));
  return !!current && !current.composing && target.generation === client.generation && target.snapshotOwner === client.snapshotOwner
    && target.owner === current.owner && target.draftKey === client.draftKey && target.value === current.value && target.inputSerial === composerFieldInputSerial(client);
}
export function takeAnnotationDispatch(client: T3Client, serial: number): AnnotationDispatch | null {
  const host = captureHost(client), target = host.sendTarget;
  if (!target || target.serial !== serial) return null;
  host.sendTarget = null;
  return annotationDispatchCurrent(client, target) ? { ...target } : null;
}
export function annotationDispatchView(client: T3Client) {
  const target = captureHost(client).sendTarget;
  return { annotationOwner: target?.owner ?? '', annotationValue: target?.value ?? '', annotationSnapshot: target?.snapshotOwner ?? '', annotationGeneration: target?.generation ?? -1 };
}

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

/** handlePickElement: a second press cancels; the settled pick goes to the tab's thread's composer. */
async function toggleAnnotate(client: T3Client, host: Host, target: CaptureTarget, scheme: string): Promise<void> {
  if (annotateState(client, target.runtimeId).active) { await cancelAnnotate(client, target.runtimeId); return; }
  host.scheme = scheme === 'dark' ? 'dark' : 'light';
  const threadKey = scopedThreadKey(target.ref);
  await startAnnotate(client, target.runtimeId, annotationTheme(host.scheme), () => pickApplier(client, threadKey));
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
    // Manager.ts captureScreenshot: the frame's picture at its own pixels, then into the artifact directory.
    const entry = frameEntry(client, target.runtimeId);
    if (!entry) throw new PreviewCaptureFailure('renderer-unavailable', 'screenshot');
    const path = `app:/tmp/browser-shots/${entry.frameId}-${++host.shots}.png`;
    await appFiles().fs.mkdir('app:/tmp/browser-shots', { recursive: true });
    try { await automationCalls().captureScreenshot({ id: entry.frameId, path }); }
    catch (error) { if (letGo(error)) throw error; throw new PreviewCaptureFailure('capture-failed', 'capture-page'); }
    const value = await nativeCall(host, { op: 'browserArtifact', action: 'adopt', path, url: entry.record?.url ?? '' }, 'screenshot');
    artifact = { id: str(value.id), tabId: target.runtimeId, path: str(value.path), mimeType: 'image/png', sizeBytes: num(value.sizeBytes), createdAt: str(value.createdAt) };
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to capture screenshot', description: errorText(error) });
    return;
  }
  screenshotSavedToast(client, artifact);
}

/** An agent-driven tab's finished download (the frame's `download` record, `completed` under setDownloadBehavior's folder):
 *  the file moves into the artifact directory beside the screenshots, as the reference's download path is that directory. */
export async function adoptAgentDownload(client: T3Client, native: Native, packed: string): Promise<void> {
  const q = new URLSearchParams(packed), path = q.get('path') ?? '';
  if (q.get('state') !== 'completed' || !/(^app:\/tmp\/|\/)browser-downloads\//.test(path)) return;
  const host = captureHost(client);
  host.native = native;
  try { await nativeCall(host, { op: 'browserArtifact', action: 'adopt', path }); } catch (error) { if (letGo(error)) throw error; }
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
    // copyArtifactToClipboard: any artifact by its path, as a PNG through @exact/clipboard (media-copy-image.ts). Data
    // code reads only the app's own files and the artifact directory is the local server's, so the module copies the
    // artifact into them first (`stage`).
    if (action === 'copy-image') await copyImageFile(str((await nativeCall(host, { op: 'browserArtifact', action: 'stage', path })).path));
    else await nativeCall(host, { op: 'browserArtifact', action, path });
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
export function toggleFloat(client: T3Client, state: PanelState, target: CaptureTarget): void {
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

// ── Annotate's results ─────────────────────────────────────────────────────────────────────────
/** A settled pick's applier: the chip and the crop in the tab's thread's composer as the window shows it now, the
 *  dropped-crop toast, and a send. */
function pickApplier(client: T3Client, threadKey: string): ApplyPick {
  const ref = parseScopedThreadKey(threadKey), open = activeRef(client);
  const key = !ref || open && scopedThreadKey(open) === threadKey ? client.draftKey : `${ref.environmentId}:${ref.threadId}`;
  const origin = { generation: client.generation, environmentId: client.environmentId, snapshotOwner: client.snapshotOwner, fieldOwner: composerFieldOwner(client), key, foreground: key === client.draftKey };
  return async (result, native) => {
    const outcome = await applyAnnotation(client, native, origin, result);
    client.revision++;
    return outcome;
  };
}

/** PreviewView's unmount cleanup (`cancelPickElement`): the reference's PreviewView unmounts when the panel stops showing
 *  its tab (Float closes the panel, the panel closes, another tab or thread shows), and its pick ends with it; the
 *  floating player's page is never annotated. One that settled with ⌘Return first still sends (`annotationSendSerial`). */
export async function cancelHiddenPicks(client: T3Client, shownRuntimeId: string | null): Promise<void> {
  for (const runtimeId of activePicks(client)) if (runtimeId !== shownRuntimeId) await cancelAnnotate(client, runtimeId);
}

type AnnotationOrigin = { generation: number; environmentId: string; snapshotOwner: string; fieldOwner: string; key: string; foreground: boolean };
async function applyAnnotation(client: T3Client, native: Native, origin: AnnotationOrigin, result: Obj): Promise<string> {
  const annotation = result.annotation;
  if (!isPreviewAnnotationPayload(annotation)) return 'invalid'; // PickedElementPayload's validator: a malformed pick is dropped silently
  if (origin.generation !== client.generation || origin.environmentId !== client.environmentId) return 'refused';
  const draftKey = origin.key;
  const screenshot = obj(result.screenshot), imageId = str(screenshot.id);
  // The crop goes on the shelf as a draft image the send uploads; a full shelf drops it, as a failed crop does.
  const room = draftKey !== client.draftKey || reservedAttachments(client) < MAX_ATTACHMENTS;
  const image = imageId && /^[a-f0-9-]{36}$/i.test(imageId) && room ? { id: imageId, name: str(screenshot.name, `preview-annotation-${annotation.id}.png`), mimeType: 'image/png', sizeBytes: num(screenshot.sizeBytes) } : null;
  const cropDropped = result.screenshotFailed === true || (!!imageId && !image);
  savePreviewAnnotation(client, annotation, image?.id ?? '');
  if (image) client.local.snapshotDrafts[draftKey] = [...(client.local.snapshotDrafts[draftKey] ?? []).filter(entry => str(entry.id) !== image.id), image];
  const placed = await insertAnnotationChip(client, native, origin, annotation, result.submission === 'send');
  if (cropDropped) pushToast(client, { kind: 'error', title: 'Could not capture the picked element', description: 'The annotation was kept without the screenshot.', stacked: true });
  return `${placed}${image ? '+image' : ''}${cropDropped ? ' crop-dropped' : ''}${result.submission === 'send' && placed === 'chip' ? ' send' : ''}`;
}

/** addPreviewAnnotation: the chip at the composer's caret (once), else at the end of the stored draft. */
async function insertAnnotationChip(client: T3Client, native: Native, origin: AnnotationOrigin, annotation: PreviewAnnotationPayload, send: boolean): Promise<string> {
  const link = previewAnnotationLink(annotation), id = link.slice(link.lastIndexOf('/') + 1, -1);
  const present = (text: string) => contextReferences(text).some(reference => reference.kind === 'preview-annotation' && reference.id === id);
  const refuse = () => {
    if (origin.generation === client.generation && origin.snapshotOwner === client.snapshotOwner && origin.fieldOwner === composerFieldOwner(client) && origin.key === client.draftKey)
      pushToast(client, { kind: 'info', title: 'Annotation was not added', description: 'The prompt changed. The annotation was kept; insert it again when the prompt is ready.', hideCopy: true });
    return 'refused';
  };
  if (origin.foreground) {
    if (origin.snapshotOwner !== client.snapshotOwner || origin.fieldOwner !== composerFieldOwner(client) || origin.key !== client.draftKey) return 'refused';
    const snapshot = composerFieldCurrent(client, origin.fieldOwner), session = composerSourceSession(client);
    if (!snapshot || snapshot.composing) return refuse();
    if (present(snapshot.value)) { if (send) commitAnnotationSend(client); return 'chip'; }
    const request = { op: 'editorInsert', text: link, focus: true, value: snapshot.value, snapshotOwner: snapshot.snapshotOwner, fieldOwner: snapshot.owner };
    const edit = composerFieldReplacement(request, { snapshot, draftOwner: client.draftKey, mode: session.mode, chips: session.chips });
    if (edit.kind === 'refused') return refuse();
    const expected = edit.edit.value.slice(0, edit.edit.start) + edit.edit.text + edit.edit.value.slice(edit.edit.end);
    const completion = send ? prepareAttachmentEffect(client, snapshot, expected, { kind: 'annotation-send', draftKey: origin.key }) : 0;
    if (send && !completion) return refuse();
    const inserted = await requestComposerField(native, { ...request, completion }).catch(() => null);
    if (completion && !inserted?.queued) finishAttachmentEffect(client, { ...edit.edit, completion }, inserted?.applied === true, composerFieldCurrent(client, origin.fieldOwner));
    return inserted?.queued ? 'queued' : inserted?.applied ? 'chip' : refuse();
  }
  if (origin.key === client.draftKey) return refuse();
  const prompt = client.local.drafts[origin.key] ?? '';
  if (present(prompt)) return 'draft';
  client.local.drafts[origin.key] = `${prompt}${prompt && !/\s$/.test(prompt) ? ' ' : ''}${link} `;
  return 'draft';
}

// ── The floating player's browser source (ThreadPreviewMiniPlayer BrowserMiniPlayer) ─────────────
export type BrowserMiniView = {
  show: boolean; key: string; sourceKey: string; tabId: string; runtimeId: string; server: boolean; streamUrl: string; pdfUrl: string; url: string; profileId: string; environment: string;
  hasWebContents: boolean; recording: boolean; separateWindow: boolean; width: number; height: number;
  /** The main window's zoom. */
  windowZoom: number;
};
export const emptyBrowserMini = (): BrowserMiniView => ({ show: false, key: '', sourceKey: '', tabId: '', runtimeId: '', server: false, streamUrl: '', pdfUrl: '', url: '', profileId: 'default', environment: '',
  hasWebContents: false, recording: false, separateWindow: false, width: 1280, height: 800, windowZoom: 1 });

/** The page's last laid-out size (resolveFittedBrowserViewport with the fill viewport): its box, else 1280×800. */
export function browserMiniSourceSize(client: T3Client, runtimeId: string): { width: number; height: number } {
  const raw = report(client, runtimeId).size, size = Array.isArray(raw) ? raw.map(Number) : [];
  return size.length === 2 && size[0]! > 1 && size[1]! > 1 ? { width: size[0]!, height: size[1]! } : { width: 1280, height: 800 };
}

/** The tab the floating player shows, unless the panel shows it (shouldRenderPreviewMiniPlayer). */
export function browserMiniView(client: T3Client, sessions: (ref: ScopedThreadRef) => { serverEpoch: string | null; tabs: Record<string, { url: string; profileId: string; server?: boolean }> },
  shownTabId: string | null): BrowserMiniView {
  const tabId = floatingTabOf(client), ref = activeRef(client);
  if (!tabId || !ref || shownTabId === tabId) return emptyBrowserMini();
  const state = sessions(ref), session = state.tabs[tabId];
  if (!session) return emptyBrowserMini();
  const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, tabId), tab = report(client, runtimeId), size = browserMiniSourceSize(client, runtimeId);
  const key = previewMiniPlayerSourceKey(browserMiniPlayerSource(tabId));
  if (session.server) { const frame = serverStreamFrame(client, runtimeId); if (frame.width > 0 && frame.height > 0) { size.width = frame.width; size.height = frame.height; } }
  captureHost(client).miniSize = { key, ...size };
  return { show: true, key, sourceKey: key, tabId, runtimeId, server: session.server === true, streamUrl: session.server ? serverStreamUrl(client, runtimeId) : '',
    pdfUrl: session.server ? previewPdfAssetUrl(session.url, ref.environmentId === client.environmentId ? client.origin : null) ?? '' : '', url: session.url, profileId: session.profileId, environment: ref.environmentId,
    hasWebContents: session.server === true || frameEntry(client, runtimeId) !== null, recording: recordingOf(client, ref, tabId, runtimeId), separateWindow: tab.pip === true, ...size,
    windowZoom: readWindowZoom(client.presentation) };
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
