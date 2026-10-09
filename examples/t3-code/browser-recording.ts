// browser-surface part 3 (capture): a Browser tab's recording lifecycle (MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975: apps/web/src/browser/browserRecording.ts, browserRecordingScope.ts, browserRecordingUpload.ts).
//
// The reference arms the desktop's capture of one tab (`startScreencast`), takes the tab's display-media stream
// in the renderer (`getDisplayMedia`, one grant at a time), decorates it on a detached canvas
// (recordingCompositor.ts) and encodes it with MediaRecorder; the stop saves the encoded file as an artifact.
// Here every half of that is the module's (T3BrowserRecorder.swift, X1 path B): `arm` installs the in-page
// cursor and input overlay and holds the page painting (startScreencast), `capture` polls `takeSnapshot` at the
// recording frame rate (the stream), `decorate` turns the native compositor on (the canvas), `begin`/`finish`
// run the H.264 encoder (MediaRecorder) and `save` moves the file into the artifact directory. This file keeps
// the reference's lifecycle around those seams unchanged: one recording per tab (BrowserRecordingConflictError),
// one grant hand-off at a time, a stop that waits for a start to settle, a stop shared by duplicate callers, and
// one upload per recording. The seams are injected (`RecordingHost`), so the ported tests run against fakes, as
// the reference's run against a mocked bridge and MediaRecorder; `nativeRecordingHost` is the module's.
//
// Data-module rules: no clock and no timers here (the build refuses them). Timeouts and paint waits go through
// the host's `setTimeout` (the module's sleep in production); the time comes from the host's `now`.

// ── Errors (Schema.TaggedError in the reference; `_tag` and the fields kept) ──────────────────────
export class BrowserRecordingError extends Error {
  readonly _tag: string;
  constructor(tag: string, message: string) { super(message); this._tag = tag; this.name = tag; }
}
export class BrowserRecordingUnavailableError extends BrowserRecordingError {
  constructor(readonly fields: { tabId: string }) { super('BrowserRecordingUnavailableError', `Browser recording is unavailable for tab ${fields.tabId}.`); }
  get tabId() { return this.fields.tabId; }
}
export class BrowserRecordingConflictError extends BrowserRecordingError {
  constructor(readonly fields: { requestedTabId: string; activeTabId: string }) {
    super('BrowserRecordingConflictError', `Cannot record tab ${fields.requestedTabId} while tab ${fields.activeTabId} is already being recorded.`);
  }
  get requestedTabId() { return this.fields.requestedTabId; }
  get activeTabId() { return this.fields.activeTabId; }
}
export class BrowserRecordingStartCancelledError extends BrowserRecordingError {
  constructor(readonly fields: { tabId: string }) { super('BrowserRecordingStartCancelledError', `Browser recording start was cancelled for tab ${fields.tabId}.`); }
  get tabId() { return this.fields.tabId; }
}
export class BrowserRecordingFormatUnavailableError extends BrowserRecordingError {
  constructor(readonly fields: { tabId: string }) { super('BrowserRecordingFormatUnavailableError', `The recorder did not report an output format for tab ${fields.tabId}.`); }
  get tabId() { return this.fields.tabId; }
}
export class BrowserRecordingCaptureTimeoutError extends BrowserRecordingError {
  constructor(readonly fields: { tabId: string; timeoutMs: number }) {
    super('BrowserRecordingCaptureTimeoutError', `Browser recording media capture for tab ${fields.tabId} did not settle within ${fields.timeoutMs}ms.`);
  }
  get tabId() { return this.fields.tabId; }
  get timeoutMs() { return this.fields.timeoutMs; }
}
export type BrowserRecordingOperation = 'initialize-media-recorder' | 'capture-media-stream' | 'start-media-recorder' | 'start-screencast' | 'stop-screencast'
  | 'wait-startup' | 'stop-media-recorder' | 'save-artifact' | 'cleanup';
export class BrowserRecordingOperationError extends BrowserRecordingError {
  constructor(readonly fields: { operation: BrowserRecordingOperation; tabId: string; cause: unknown }) {
    super('BrowserRecordingOperationError', `Browser recording operation ${fields.operation} failed for tab ${fields.tabId}.`);
  }
  get operation() { return this.fields.operation; }
  get tabId() { return this.fields.tabId; }
  override get cause() { return this.fields.cause; }
}
const isOperationError = (error: unknown): error is BrowserRecordingOperationError => error instanceof BrowserRecordingOperationError;
export const isBrowserRecordingStartCancelledError = (error: unknown): error is BrowserRecordingStartCancelledError => error instanceof BrowserRecordingStartCancelledError;
/** AggregateError, which some runtimes lack: the errors and the first cause. */
class RecordingAggregateError extends Error {
  constructor(readonly errors: unknown[], message: string, options: { cause: unknown }) { super(message); this.name = 'AggregateError'; (this as { cause?: unknown }).cause = options.cause; }
}

// ── Seams ────────────────────────────────────────────────────────────────────────────────────
export type ScopedRecordingThreadRef = { environmentId: string; threadId: string };
/** DesktopPreviewRecordingArtifact. */
export type RecordingArtifact = { id: string; tabId: string; path: string; mimeType: string; sizeBytes: number; createdAt: string };
/** The encoded file the recorder produced (the reference's Blob): written natively, moved by `save`. */
export type RecordingBlob = { type: string; size: number; path: string };
export type RecordingTrack = { getSettings?: () => { width?: number; height?: number; frameRate?: number }; stop: () => void };
/** The captured stream (MediaStream): its video settings and tracks. */
export type RecordingStream = { getVideoTracks: () => Array<{ getSettings: () => { width?: number; height?: number; frameRate?: number } }>; getTracks: () => RecordingTrack[] };
/** The encoder (MediaRecorder): start, stop, its output format and the encoded file. */
export interface RecordingRecorder {
  readonly state: 'inactive' | 'recording';
  readonly mimeType: string;
  start(timeslice: number): void | Promise<void>;
  /** Resolves once the encoder has flushed (MediaRecorder's `stop` event). */
  stop(): Promise<void>;
  /** The encoded output so far (MediaRecorder's chunks). */
  output(): RecordingBlob | null;
}
export type RecordingCompositor = { stream: RecordingStream; dispose: () => void } | null;
export type RecordingSettings = { browserRecordingFrameRate: number; browserRecordingShowKeyPresses: boolean; browserRecordingShowMousePresses: boolean };
export interface RecordingHost {
  /** previewBridge.recording; null when this client has no browser. */
  bridge: {
    startScreencast(tabId: string): Promise<void>;
    stopScreencast(tabId: string): Promise<void>;
    save(tabId: string, mimeType: string, data: RecordingBlob): Promise<RecordingArtifact>;
  } | null;
  /** navigator.mediaDevices.getDisplayMedia for the armed tab. */
  getDisplayMedia(tabId: string, constraints: { audio: false; video: { frameRate: { ideal: number; max: number } } }): Promise<RecordingStream>;
  /** MediaRecorder.isTypeSupported and its constructor. */
  isTypeSupported(mimeType: string): boolean;
  createRecorder(tabId: string, stream: RecordingStream, options: { mimeType?: string; videoBitsPerSecond: number }): RecordingRecorder;
  /** createRecordingCompositor: null when both decorations are off (frames go straight through). */
  createCompositor(tabId: string, stream: RecordingStream, options: { showKeyPresses: boolean; showMousePresses: boolean; frameRate: number }): Promise<RecordingCompositor>;
  /** ensureClientSettingsHydrated, then getClientSettings. */
  hydrateSettings(): Promise<void>;
  settings(): RecordingSettings;
  /** acquireBrowserSurfaceActivity: keeps a hidden tab's page painting; returns its release. */
  acquireSurfaceActivity(tabId: string): () => void;
  /** The active recordings changed (the reference's atom): the panel draws the Stop state from it. */
  publish(tabIds: ReadonlySet<string>): void;
  requestAnimationFrame(callback: (time: number) => void): number;
  cancelAnimationFrame(id: number): void;
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
  /** The current time as an ISO string. */
  now(): string;
}

// ── The lifecycle (browserRecording.ts) ──────────────────────────────────────────────────────
interface StartingLifecycle {
  readonly phase: 'starting';
  queuedForGrant: boolean | null;
  grantStarted: boolean;
  stopRequestedBeforeGrant: boolean;
  cancelledBeforeGrant: boolean;
  readonly cancelledBeforeGrantSignal: Promise<void>;
  readonly cancelBeforeGrant: () => void;
  readonly setQueuedForGrant: (queued: boolean) => void;
}
type Lifecycle = StartingLifecycle | { readonly phase: 'recording' } | { readonly phase: 'stopping'; readonly stopPromise: Promise<RecordingArtifact | null> };
interface ActiveRecording {
  /** Desktop-scoped identity used by the native capture lease. */
  readonly tabId: string;
  /** Server-local identity returned by preview automation tools. */
  readonly serverTabId: string;
  readonly threadRef: ScopedRecordingThreadRef | null;
  readonly startedAt: string;
  readonly startupSettled: Promise<void>;
  releaseSurfaceActivity: (() => void) | null;
  stream: RecordingStream | null;
  recorder: RecordingRecorder | null;
  compositor: RecordingCompositor;
  savedBlob?: RecordingBlob;
  uploadPromise?: Promise<string>;
  lifecycle: Lifecycle;
}
export interface ActiveBrowserRecordingTarget { readonly runtimeTabId: string; readonly serverTabId: string }

export const BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS = 5_000;
export const BROWSER_RECORDING_PAINT_SETTLE_TIMEOUT_MS = 250;
export const PREFERRED_RECORDING_MIME_TYPES = [
  'video/mp4;codecs=avc1', 'video/mp4;codecs=avc1.640028', 'video/mp4;codecs=avc1.42e01e', 'video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm',
] as const;

/** createMediaRecorder's budget: captured pixels × frames × 0.05, between 2.5 and 50 Mbit/s. */
export function recordingBitsPerSecond(settings: { width?: number; height?: number; frameRate?: number } | undefined): number {
  return Math.round(Math.min(50_000_000, Math.max(2_500_000, (settings?.width ?? 1920) * (settings?.height ?? 1080) * (settings?.frameRate ?? 30) * 0.05)));
}

/** resolveBrowserRecordingStopTarget (browserRecordingScope.ts). */
export function resolveBrowserRecordingStopTarget(activeTabIds: ReadonlySet<string>, implicitTabId: string | null, explicitTabId?: string): string | null {
  if (explicitTabId !== undefined) return activeTabIds.has(explicitTabId) ? explicitTabId : null;
  if (implicitTabId !== null && activeTabIds.has(implicitTabId)) return implicitTabId;
  if (activeTabIds.size !== 1) return null;
  return activeTabIds.values().next().value ?? null;
}

/** The recordings of one client (the reference's module state). */
export class BrowserRecordings {
  private readonly activeRecordings = new Map<string, ActiveRecording>();
  private grantTail: Promise<void> = Promise.resolve();
  private grantQueueDepth = 0;
  private readonly pendingCaptures = new Map<string, { start: () => void }>();

  constructor(private readonly host: RecordingHost) {}

  readActiveBrowserRecordingTabIds(threadRef?: ScopedRecordingThreadRef): ReadonlySet<string> {
    const tabIds = new Set<string>();
    for (const recording of this.activeRecordings.values()) {
      if (threadRef === undefined || (recording.threadRef?.environmentId === threadRef.environmentId && recording.threadRef.threadId === threadRef.threadId)) tabIds.add(recording.tabId);
    }
    return tabIds;
  }
  readActiveBrowserRecordingTargets(threadRef: ScopedRecordingThreadRef): ReadonlyArray<ActiveBrowserRecordingTarget> {
    return Array.from(this.activeRecordings.values()).flatMap(recording =>
      recording.threadRef?.environmentId === threadRef.environmentId && recording.threadRef.threadId === threadRef.threadId
        ? [{ runtimeTabId: recording.tabId, serverTabId: recording.serverTabId }] : []);
  }
  findActiveBrowserRecordingRuntimeTabId(threadRef: ScopedRecordingThreadRef, serverTabId: string): string | null {
    return this.readActiveBrowserRecordingTargets(threadRef).find(recording => recording.serverTabId === serverTabId)?.runtimeTabId ?? null;
  }
  /** The tab's lifecycle phase, for the chrome row: '' when it is not recording. */
  phaseOf(tabId: string): string { return this.activeRecordings.get(tabId)?.lifecycle.phase ?? ''; }

  /** The capture trigger the armed module calls back (DESKTOP_PREVIEW_RECORDING_CAPTURE_TRIGGER): starts the pending stream. */
  triggerTabMediaCapture(tabId: unknown): boolean {
    if (typeof tabId !== 'string') return false;
    const pending = this.pendingCaptures.get(tabId);
    if (!pending) return false;
    this.pendingCaptures.delete(tabId);
    pending.start();
    return true;
  }

  private publish(): void { this.host.publish(new Set(this.activeRecordings.keys())); }

  private makeStartingLifecycle(): StartingLifecycle {
    let signal!: () => void;
    const cancelledBeforeGrantSignal = new Promise<void>(resolve => { signal = resolve; });
    const lifecycle: StartingLifecycle = {
      phase: 'starting', queuedForGrant: null, grantStarted: false, stopRequestedBeforeGrant: false, cancelledBeforeGrant: false, cancelledBeforeGrantSignal,
      cancelBeforeGrant: () => {
        // Queue position is unknown during paint/settings warmup. Keep the stop request so a start that later turns out
        // to be contended can still be cancelled before native capture.
        lifecycle.stopRequestedBeforeGrant = true;
        if (lifecycle.queuedForGrant && !lifecycle.grantStarted && !lifecycle.cancelledBeforeGrant) { lifecycle.cancelledBeforeGrant = true; signal(); }
      },
      setQueuedForGrant: queued => {
        lifecycle.queuedForGrant = queued;
        if (queued && lifecycle.stopRequestedBeforeGrant) lifecycle.cancelBeforeGrant();
      },
    };
    return lifecycle;
  }

  private queueGrant<T>(useGrant: () => Promise<T>): { readonly queued: boolean; readonly result: Promise<T> } {
    const queued = this.grantQueueDepth > 0;
    this.grantQueueDepth += 1;
    const result = this.grantTail.then(useGrant);
    const settle = () => { this.grantQueueDepth -= 1; };
    this.grantTail = result.then(settle, settle);
    return { queued, result };
  }

  private prepareTabMediaCapture(tabId: string, frameRate: number) {
    let accept = true;
    let captured: RecordingStream | null = null;
    let resolveCapture!: (stream: RecordingStream | PromiseLike<RecordingStream>) => void;
    let rejectCapture!: (cause: unknown) => void;
    const capturePromise = new Promise<RecordingStream>((resolve, reject) => { resolveCapture = resolve; rejectCapture = reject; }).then(stream => {
      captured = stream;
      if (!accept) { stopStream(stream); captured = null; }
      return stream;
    });
    const pending = { start: () => {
      try { resolveCapture(this.host.getDisplayMedia(tabId, { audio: false, video: { frameRate: { ideal: frameRate, max: frameRate } } })); }
      catch (cause) { rejectCapture(cause); }
    } };
    this.pendingCaptures.set(tabId, pending);
    return {
      capturePromise,
      cancel: () => {
        accept = false;
        if (captured) { stopStream(captured); captured = null; }
        if (this.pendingCaptures.get(tabId) === pending) this.pendingCaptures.delete(tabId);
        void capturePromise.catch(() => undefined);
      },
    };
  }

  private async captureWithTimeout(tabId: string, capturePromise: Promise<RecordingStream>): Promise<RecordingStream> {
    let accept = true;
    let timeout: unknown = null;
    const streamPromise = capturePromise.then(stream => { if (!accept) stopStream(stream); return stream; });
    try {
      return await Promise.race([streamPromise, new Promise<never>((_, reject) => {
        timeout = this.host.setTimeout(() => reject(new BrowserRecordingCaptureTimeoutError({ tabId, timeoutMs: BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS })), BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS);
      })]);
    } finally {
      accept = false;
      if (timeout !== null) this.host.clearTimeout(timeout);
    }
  }

  private clearActiveRecording(recording: ActiveRecording): void {
    recording.compositor?.dispose();
    recording.compositor = null;
    recording.releaseSurfaceActivity?.();
    recording.releaseSurfaceActivity = null;
    if (this.activeRecordings.get(recording.tabId) !== recording) return;
    this.activeRecordings.delete(recording.tabId);
    this.publish();
  }

  private async waitForPaint(): Promise<void> {
    let first: number | null = null, second: number | null = null, timeout: unknown = null;
    const painted = new Promise<void>(resolve => {
      first = this.host.requestAnimationFrame(() => {
        first = null;
        second = this.host.requestAnimationFrame(() => { second = null; resolve(); });
      });
    });
    const timedOut = new Promise<void>(resolve => { timeout = this.host.setTimeout(resolve, BROWSER_RECORDING_PAINT_SETTLE_TIMEOUT_MS); });
    try { await Promise.race([painted, timedOut]); }
    finally {
      if (timeout !== null) this.host.clearTimeout(timeout);
      if (first !== null) this.host.cancelAnimationFrame(first);
      if (second !== null) this.host.cancelAnimationFrame(second);
    }
  }

  private async cleanupFailedStart(bridge: NonNullable<RecordingHost['bridge']>, recording: ActiveRecording): Promise<unknown | undefined> {
    const errors: unknown[] = [];
    try { await bridge.stopScreencast(recording.tabId); } catch (error) { errors.push(error); }
    try { await stopRecorder(recording.recorder); } catch (error) { errors.push(error); }
    try { stopStream(recording.stream); } catch (error) { errors.push(error); } finally { this.clearActiveRecording(recording); }
    if (errors.length === 0) return undefined;
    if (errors.length === 1) return errors[0];
    return new RecordingAggregateError(errors, `Browser recording startup cleanup failed for tab ${recording.tabId}.`, { cause: errors[0] });
  }

  private startupCancelled(recording: ActiveRecording, cause: unknown = new Error(`Browser recording startup was cancelled for tab ${recording.tabId}.`)): BrowserRecordingOperationError {
    return new BrowserRecordingOperationError({ operation: 'start-screencast', tabId: recording.tabId, cause });
  }
  private isStarting(recording: ActiveRecording): boolean { return this.activeRecordings.get(recording.tabId) === recording && recording.lifecycle.phase === 'starting'; }

  private async waitForStartupToSettle(recording: ActiveRecording): Promise<void> {
    let timeout: unknown = null;
    try {
      await Promise.race([recording.startupSettled, new Promise<void>((_, reject) => {
        timeout = this.host.setTimeout(() => reject(new Error(`Browser recording startup did not settle for tab ${recording.tabId}.`)), BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS);
      })]);
    } catch (cause) {
      throw new BrowserRecordingOperationError({ operation: 'wait-startup', tabId: recording.tabId, cause });
    } finally { if (timeout !== null) this.host.clearTimeout(timeout); }
  }

  /** startBrowserRecording: the startedAt of the tab's recording (an already running one's too). */
  async startBrowserRecording(tabId: string, threadRef: ScopedRecordingThreadRef | null = null, serverTabId = tabId): Promise<string> {
    const bridge = this.host.bridge;
    if (!bridge) throw new BrowserRecordingUnavailableError({ tabId });
    const active = this.activeRecordings.get(tabId);
    if (active) {
      if (active.lifecycle.phase === 'recording') return active.startedAt;
      throw new BrowserRecordingConflictError({ requestedTabId: tabId, activeTabId: active.tabId });
    }
    const logical = threadRef === null ? null : this.findActiveBrowserRecordingRuntimeTabId(threadRef, serverTabId);
    if (logical !== null) throw new BrowserRecordingConflictError({ requestedTabId: tabId, activeTabId: logical });
    const startedAt = this.host.now();
    let settleStartup: (() => void) | undefined;
    const startupSettled = new Promise<void>(resolve => { settleStartup = resolve; });
    const starting = this.makeStartingLifecycle();
    const recording: ActiveRecording = { tabId, serverTabId, threadRef, startedAt, startupSettled, releaseSurfaceActivity: this.host.acquireSurfaceActivity(tabId),
      stream: null, recorder: null, compositor: null, lifecycle: starting };
    this.activeRecordings.set(tabId, recording);
    this.publish();
    try {
      await this.host.hydrateSettings().catch((cause: unknown) => { this.clearActiveRecording(recording); throw cause; });
      const settings = this.host.settings();
      const frameRate = settings.browserRecordingFrameRate;
      await this.waitForPaint();
      const throwIfStartupCancelled = async (): Promise<void> => {
        // Once a grant starts, a stop lets startup finish so the caller receives an artifact. Only a contended start can
        // be cancelled before it reaches native capture.
        if (this.activeRecordings.get(tabId) === recording) return;
        try { await bridge.stopScreencast(tabId); }
        catch (cause) {
          throw this.startupCancelled(recording, new RecordingAggregateError([new Error(`Browser recording startup was cancelled for tab ${tabId}.`), cause],
            `Browser recording startup cancellation failed for tab ${tabId}.`, { cause }));
        }
        throw this.startupCancelled(recording);
      };
      // The module hands out one capture grant at a time. Keep only the arm-to-capture handoff exclusive; acquired
      // streams can record concurrently.
      const grant = this.queueGrant(async () => {
        if (starting.cancelledBeforeGrant) throw new BrowserRecordingStartCancelledError({ tabId });
        starting.grantStarted = true;
        await throwIfStartupCancelled();
        const capture = this.prepareTabMediaCapture(tabId, frameRate);
        try { await bridge.startScreencast(tabId); }
        catch (cause) {
          capture.cancel();
          if (!this.isStarting(recording)) throw this.startupCancelled(recording, cause);
          this.clearActiveRecording(recording);
          throw new BrowserRecordingOperationError({ operation: 'start-screencast', tabId, cause });
        }
        try { await throwIfStartupCancelled(); } catch (cause) { capture.cancel(); throw cause; }
        try {
          recording.stream = await this.captureWithTimeout(tabId, capture.capturePromise);
          return recording.stream;
        } catch (cause) {
          const cleanup = await this.cleanupFailedStart(bridge, recording);
          if (cause instanceof BrowserRecordingCaptureTimeoutError && cleanup === undefined) throw cause;
          throw new BrowserRecordingOperationError({ operation: 'capture-media-stream', tabId,
            cause: cleanup === undefined ? cause : new RecordingAggregateError([cause, cleanup], `Browser media capture and cleanup failed for tab ${tabId}.`, { cause }) });
        }
      });
      starting.setQueuedForGrant(grant.queued);
      const stream = await Promise.race([grant.result, starting.cancelledBeforeGrantSignal.then(() => { throw new BrowserRecordingStartCancelledError({ tabId }); })]);
      await throwIfStartupCancelled();
      let recorder: RecordingRecorder;
      try {
        recording.compositor = await this.host.createCompositor(tabId, stream, { showKeyPresses: settings.browserRecordingShowKeyPresses, showMousePresses: settings.browserRecordingShowMousePresses, frameRate });
        const source = recording.compositor?.stream ?? stream;
        const mimeType = PREFERRED_RECORDING_MIME_TYPES.find(candidate => this.host.isTypeSupported(candidate));
        recorder = this.host.createRecorder(tabId, source, { ...(mimeType ? { mimeType } : {}), videoBitsPerSecond: recordingBitsPerSecond(source.getVideoTracks()[0]?.getSettings()) });
        recording.recorder = recorder;
      } catch (cause) {
        const cleanup = await this.cleanupFailedStart(bridge, recording);
        throw new BrowserRecordingOperationError({ operation: 'initialize-media-recorder', tabId,
          cause: cleanup === undefined ? cause : new RecordingAggregateError([cause, cleanup], `Browser recording initialization and cleanup failed for tab ${tabId}.`, { cause }) });
      }
      try { await recorder.start(1_000); }
      catch (cause) {
        const cleanup = await this.cleanupFailedStart(bridge, recording);
        throw new BrowserRecordingOperationError({ operation: 'start-media-recorder', tabId,
          cause: cleanup === undefined ? cause : new RecordingAggregateError([cause, cleanup], `Browser media recorder start and cleanup failed for tab ${tabId}.`, { cause }) });
      }
      if (recording.lifecycle.phase === 'starting') recording.lifecycle = { phase: 'recording' };
      return startedAt;
    } finally { settleStartup?.(); }
  }

  private async finalize(bridge: NonNullable<RecordingHost['bridge']>, recording: ActiveRecording): Promise<RecordingArtifact | null> {
    const { tabId } = recording;
    let result: { _tag: 'Success'; artifact: RecordingArtifact | null } | { _tag: 'Failure'; error: unknown };
    try {
      await this.waitForStartupToSettle(recording);
      try { await bridge.stopScreencast(tabId); }
      catch (cause) { throw new BrowserRecordingOperationError({ operation: 'stop-screencast', tabId, cause }); }
      if (!recording.recorder) result = { _tag: 'Success', artifact: null };
      else {
        try { await stopRecorder(recording.recorder); }
        catch (cause) { throw new BrowserRecordingOperationError({ operation: 'stop-media-recorder', tabId, cause }); }
        recording.compositor?.dispose();
        recording.compositor = null;
        // Encoding has flushed; release native capture before saving the file.
        stopStream(recording.stream);
        recording.stream = null;
        const output = recording.recorder.output();
        const mimeType = recording.recorder.mimeType || output?.type;
        if (!mimeType) throw new BrowserRecordingFormatUnavailableError({ tabId });
        try {
          const blob: RecordingBlob = { type: mimeType, size: output?.size ?? 0, path: output?.path ?? '' };
          const artifact = await bridge.save(tabId, mimeType, blob);
          recording.savedBlob = { ...blob, path: artifact.path, size: artifact.sizeBytes || blob.size };
          result = { _tag: 'Success', artifact };
        } catch (cause) { throw new BrowserRecordingOperationError({ operation: 'save-artifact', tabId, cause }); }
      }
    } catch (error) { result = { _tag: 'Failure', error }; }

    if (result._tag === 'Failure' && isStartupWaitTimeout(result.error)) {
      // Do not clear `active` yet. The start promise can still resolve later, and its cancellation path stops the
      // screencast. Keeping the slot reserved keeps a newer recording of this tab from being stopped by that late cleanup.
      throw result.error;
    }
    const cleanupErrors: unknown[] = [];
    try { await stopRecorder(recording.recorder); } catch (cause) { cleanupErrors.push(cause); }
    try { stopStream(recording.stream); } catch (cause) { cleanupErrors.push(cause); } finally { this.clearActiveRecording(recording); }
    const cleanupError = cleanupErrors.length === 0 ? undefined : new BrowserRecordingOperationError({ operation: 'cleanup', tabId,
      cause: cleanupErrors.length === 1 ? cleanupErrors[0] : new RecordingAggregateError(cleanupErrors, `Browser recording media cleanup failed for tab ${tabId}.`, { cause: cleanupErrors[0] }) });
    if (result._tag === 'Failure') {
      if (cleanupError) throw new BrowserRecordingOperationError({ operation: 'cleanup', tabId,
        cause: new RecordingAggregateError([result.error, cleanupError], `Browser recording stop and cleanup failed for tab ${tabId}.`, { cause: result.error }) });
      throw result.error;
    }
    if (cleanupError) throw cleanupError;
    return result.artifact;
  }

  private async discard(bridge: NonNullable<RecordingHost['bridge']>, recording: ActiveRecording): Promise<null> {
    try {
      await bridge.stopScreencast(recording.tabId).catch(() => undefined);
      await stopRecorder(recording.recorder).catch(() => undefined);
      stopStream(recording.stream);
      return null;
    } finally { this.clearActiveRecording(recording); }
  }

  /** stopBrowserRecording: the saved artifact, or null when nothing was recording (or the start was cancelled). */
  stopBrowserRecording(tabId: string): Promise<RecordingArtifact | null> {
    const bridge = this.host.bridge, recording = this.activeRecordings.get(tabId);
    if (!bridge || !recording) return Promise.resolve(null);
    if (recording.lifecycle.phase === 'stopping') return recording.lifecycle.stopPromise;
    if (recording.lifecycle.phase === 'starting') recording.lifecycle.cancelBeforeGrant();
    const stopPromise = Promise.resolve().then(() => this.finalize(bridge, recording)).catch(error => {
      if (isStartupWaitTimeout(error) && this.activeRecordings.get(recording.tabId) === recording) {
        const cleanupAfterStartup = recording.startupSettled.then(() => this.discard(bridge, recording));
        recording.lifecycle = { phase: 'stopping', stopPromise: cleanupAfterStartup };
        void cleanupAfterStartup.catch(() => undefined);
      }
      throw error;
    });
    recording.lifecycle = { phase: 'stopping', stopPromise };
    return stopPromise;
  }

  /** stopBrowserRecordingForUpload: joins local stops and shares one upload among concurrent callers. */
  async stopBrowserRecordingForUpload(tabId: string, upload: (artifact: RecordingArtifact, blob: RecordingBlob) => Promise<string>): Promise<(RecordingArtifact & { uploadedAttachmentId: string }) | null> {
    const recording = this.activeRecordings.get(tabId);
    if (!recording) return null;
    const artifact = await this.stopBrowserRecording(tabId);
    if (!artifact || !recording.savedBlob) return null;
    recording.uploadPromise ??= upload(artifact, recording.savedBlob);
    return { ...artifact, uploadedAttachmentId: await recording.uploadPromise };
  }
}

const isStartupWaitTimeout = (error: unknown): error is BrowserRecordingOperationError => isOperationError(error) && error.operation === 'wait-startup';
async function stopRecorder(recorder: RecordingRecorder | null): Promise<void> {
  if (!recorder || recorder.state === 'inactive') return;
  await recorder.stop();
}
function stopStream(stream: RecordingStream | null): void { for (const track of stream?.getTracks() ?? []) track.stop(); }

/** recordingFileExtension (Manager.ts): the subtype without `x-` and punctuation, else "video". */
export function recordingFileExtension(mimeType: string): string {
  const subtype = mimeType.split(';', 1)[0]?.trim().toLowerCase().split('/')[1] ?? '';
  return subtype.replace(/^x-/, '').replace(/[^a-z0-9]/g, '') || 'video';
}

// ── Upload (browserRecordingUpload.ts): the encoded file once, as an attachment ─────────────────
/** PROVIDER_SEND_TURN_MAX_FILE_BYTES. */
export const RECORDING_UPLOAD_MAX_BYTES = 50 * 1024 * 1024;
export class PreviewAutomationRecordingTooLargeError extends BrowserRecordingError {
  constructor(readonly fields: { threadId: string }) { super('PreviewAutomationRecordingTooLargeError', 'The recording is too large to attach.'); }
}
export class PreviewAutomationRecordingTransferError extends BrowserRecordingError {
  constructor(readonly fields: { threadId: string; cause?: unknown }) { super('PreviewAutomationRecordingTransferError', 'The recording could not be transferred.'); }
}
export class PreviewAutomationRecordingDeadlineExpiredError extends BrowserRecordingError {
  constructor(readonly fields: { threadId: string; cause?: unknown }) { super('PreviewAutomationRecordingDeadlineExpiredError', 'The recording transfer deadline expired.'); }
}
/** The attachment upload cycle the clone's composer uses (createUploadUrl, POST, finish). */
export type RecordingUploader = (input: { name: string; mimeType: string; sizeBytes: number; path: string; remainingMs: number }) => Promise<{ status: 'uploaded'; attachmentId: string } | { status: 'failed' | 'cancelled'; attachmentId?: string; error?: unknown }>;
/** uploadBrowserRecording: sends the finished file once; frames never cross the environment connection. */
export async function uploadBrowserRecording(ref: ScopedRecordingThreadRef, artifact: RecordingArtifact, blob: RecordingBlob, deadlineMs: number, nowMs: number,
  upload: RecordingUploader, remove: (attachmentId: string) => void): Promise<string> {
  if (blob.size > RECORDING_UPLOAD_MAX_BYTES) throw new PreviewAutomationRecordingTooLargeError({ threadId: ref.threadId });
  // Encoding, saving and minting consume the same request budget. Leave time to reply.
  const remainingMs = deadlineMs - nowMs - 1_000;
  const result = remainingMs <= 0 ? { status: 'failed' as const, error: new Error('Recording transfer deadline expired.') }
    : await upload({ name: artifact.path.split(/[\\/]/).at(-1) ?? artifact.id, mimeType: artifact.mimeType, sizeBytes: blob.size, path: blob.path || artifact.path, remainingMs });
  if (result.status !== 'uploaded') {
    if (result.attachmentId) remove(result.attachmentId);
    const cause = result.status === 'failed' ? result.error : undefined;
    if (remainingMs <= 0) throw new PreviewAutomationRecordingDeadlineExpiredError({ threadId: ref.threadId, cause });
    throw new PreviewAutomationRecordingTransferError({ threadId: ref.threadId, cause });
  }
  return result.attachmentId;
}
