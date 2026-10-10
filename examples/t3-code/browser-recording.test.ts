// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/browser/browserRecording.test.ts (24 tests, the
// it.each's 4 rows: 28) and browserRecordingScope.test.ts (5), under their own names.
// Substitutions: the mocked previewBridge, atom registry, settings hook and browser surface store are the injected
// RecordingHost (browser-recording.ts); `getDisplayMedia` and MediaRecorder are the host's capture and recorder seams
// (the module's snapshot polling and H.264 encoder in the app), so `getDisplayMedia` is asked with the tab and the
// constraints, and the recorder's encoded output is a file (RecordingBlob) rather than chunks; Vitest's fake timers
// and `vi.waitFor` are a small fake clock and a polling helper. `toHaveBeenCalledWith({ audio, video })` checks the
// constraints argument; the call-order check compares an event log instead of `invocationCallOrder`; a rejection that a later
// step settles is captured with `settled` (Bun's `expect(promise).rejects` waits for the promise where it stands).
import { beforeEach, describe, expect, it, mock } from 'bun:test';
import {
  BROWSER_RECORDING_PAINT_SETTLE_TIMEOUT_MS, BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS, BrowserRecordingCaptureTimeoutError, BrowserRecordingConflictError,
  BrowserRecordingFormatUnavailableError, BrowserRecordingStartCancelledError, BrowserRecordings, resolveBrowserRecordingStopTarget,
  type RecordingBlob, type RecordingHost, type RecordingRecorder, type RecordingStream,
} from './browser-recording';
import { previewRuntimeTabId } from './browser-state';

/** Bun's `expect(promise).rejects` waits for the promise at once; a rejection a later step settles is captured instead. */
const settled = (promise: Promise<unknown>): Promise<unknown> => promise.then(value => ({ resolved: value }), (error: unknown) => error);
const flush = async () => { for (let i = 0; i < 25; i++) await new Promise(resolve => setImmediate(resolve)); };
async function waitFor(assertion: () => void, timeoutMs = 2000): Promise<void> {
  const end = performance.now() + timeoutMs;
  for (;;) {
    try { assertion(); return; } catch (error) { if (performance.now() > end) throw error; }
    await new Promise(resolve => setTimeout(resolve, 2));
  }
}
/** vi.useFakeTimers / advanceTimersByTimeAsync over the host's timers. */
class FakeClock {
  now = 0;
  private seq = 0;
  private timers = new Map<number, { at: number; fn: () => void }>();
  setTimeout = (fn: () => void, ms: number) => { const id = ++this.seq; this.timers.set(id, { at: this.now + ms, fn }); return id; };
  clearTimeout = (id: unknown) => { this.timers.delete(id as number); };
  async advance(ms: number) {
    const end = this.now + ms;
    await flush();
    for (;;) {
      const due = [...this.timers.entries()].filter(([, timer]) => timer.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]); this.now = due[1].at; due[1].fn(); await flush();
    }
    this.now = end; await flush();
  }
}

class FakeMediaRecorder implements RecordingRecorder {
  static readonly instances: FakeMediaRecorder[] = [];
  static supportedTypes = new Set(['video/webm;codecs=vp9']);
  static outputMimeType: string | undefined;
  static stopError: unknown;
  state: 'inactive' | 'recording' = 'inactive';
  readonly mimeType: string;
  constructor(readonly stream: RecordingStream, readonly options: { mimeType?: string; videoBitsPerSecond: number } | undefined) {
    this.mimeType = FakeMediaRecorder.outputMimeType ?? options?.mimeType ?? 'video/browser-default';
    FakeMediaRecorder.instances.push(this);
  }
  start(): void { this.state = 'recording'; }
  async stop(): Promise<void> {
    if (FakeMediaRecorder.stopError !== undefined) throw FakeMediaRecorder.stopError;
    this.state = 'inactive';
  }
  output(): RecordingBlob { return { type: this.mimeType, size: 0, path: '/tmp/encoded.mp4' }; }
}

const events: string[] = [];
const clientSettings = { browserRecordingFrameRate: 30 as 30 | 60, browserRecordingShowKeyPresses: false, browserRecordingShowMousePresses: false };
const stream = (stopTrack = mock(() => undefined), settings?: { width?: number; height?: number; frameRate?: number }): RecordingStream =>
  ({ getVideoTracks: () => (settings ? [{ getSettings: () => settings }] : []), getTracks: () => [{ stop: stopTrack }] });
let getDisplayMedia = mock(async (_tabId: string, _constraints: unknown): Promise<RecordingStream> => stream());
let save = mock(async (tabId: string, _mimeType: string, _data: RecordingBlob) => {
  events.push('save');
  return { id: 'recording-test', tabId, path: '/tmp/recording-test.webm', mimeType: 'video/webm', sizeBytes: 0, createdAt: '2026-06-26T00:00:00.000Z' };
});
let startScreencast = mock(async (_tabId: string) => { events.push('start-screencast'); });
let stopScreencast = mock(async (_tabId: string): Promise<void> => undefined);
let requestDisplayMediaCapture = mock((_tabId: string) => undefined as void);
let hydrateSettings = mock(async () => undefined as void);
let activityByTabId: Record<string, number> = {};
let animationFrameCount = 0;
let requestAnimationFrame: (callback: (time: number) => void) => number;
let cancelAnimationFrame = mock((_id: number) => undefined as void);
let clock: FakeClock | null = null;
let recordings: BrowserRecordings;

function makeRecordings(): BrowserRecordings {
  const host: RecordingHost = {
    bridge: {
      startScreencast: async tabId => { await startScreencast(tabId); requestDisplayMediaCapture(tabId); },
      stopScreencast: tabId => stopScreencast(tabId),
      save: (tabId, mimeType, data) => save(tabId, mimeType, data),
    },
    getDisplayMedia: (tabId, constraints) => getDisplayMedia(tabId, constraints),
    isTypeSupported: type => FakeMediaRecorder.supportedTypes.has(type),
    createRecorder: (_tabId, source, options) => new FakeMediaRecorder(source, options),
    createCompositor: async () => null,
    hydrateSettings: () => hydrateSettings(),
    settings: () => clientSettings,
    acquireSurfaceActivity: tabId => {
      activityByTabId = { ...activityByTabId, [tabId]: (activityByTabId[tabId] ?? 0) + 1 };
      let released = false;
      return () => {
        if (released) return;
        released = true;
        const count = (activityByTabId[tabId] ?? 1) - 1;
        const { [tabId]: _gone, ...rest } = activityByTabId;
        activityByTabId = count > 0 ? { ...rest, [tabId]: count } : rest;
      };
    },
    publish: tabIds => { events.push(tabIds.size === 0 ? 'clear' : `publish:${Array.from(tabIds).join(',')}`); },
    requestAnimationFrame: callback => requestAnimationFrame(callback),
    cancelAnimationFrame: id => cancelAnimationFrame(id),
    setTimeout: (fn, ms) => (clock ? clock.setTimeout(fn, ms) : setTimeout(fn, ms)),
    clearTimeout: handle => (clock ? clock.clearTimeout(handle) : clearTimeout(handle as ReturnType<typeof setTimeout>)),
    now: () => '2026-06-26T00:00:00.000Z',
  };
  return new BrowserRecordings(host);
}
const useFakeTimers = () => { clock = new FakeClock(); return clock; };

describe('browser recording', () => {
  beforeEach(() => {
    events.length = 0;
    FakeMediaRecorder.instances.length = 0;
    FakeMediaRecorder.supportedTypes = new Set(['video/webm;codecs=vp9']);
    FakeMediaRecorder.outputMimeType = undefined;
    FakeMediaRecorder.stopError = undefined;
    clientSettings.browserRecordingFrameRate = 30;
    animationFrameCount = 0;
    clock = null;
    requestAnimationFrame = callback => { animationFrameCount += 1; callback(animationFrameCount); return animationFrameCount; };
    cancelAnimationFrame = mock((_id: number) => undefined as void);
    getDisplayMedia = mock(async (_tabId: string, _constraints: unknown): Promise<RecordingStream> => stream());
    save = mock(async (tabId: string, _mimeType: string, _data: RecordingBlob) => {
      events.push('save');
      return { id: 'recording-test', tabId, path: '/tmp/recording-test.webm', mimeType: 'video/webm', sizeBytes: 0, createdAt: '2026-06-26T00:00:00.000Z' };
    });
    startScreencast = mock(async (_tabId: string) => { events.push('start-screencast'); });
    stopScreencast = mock(async (_tabId: string): Promise<void> => undefined);
    hydrateSettings = mock(async () => undefined as void);
    activityByTabId = {};
    recordings = makeRecordings();
    requestDisplayMediaCapture = mock((tabId: string) => {
      if (recordings.triggerTabMediaCapture(tabId) !== true) throw new Error(`No pending display-media capture for ${tabId}.`);
    });
  });

  it('starts recording for a visible tab', async () => {
    await recordings.startBrowserRecording('recording-tab');
    const startupEvents = [...events];
    await recordings.stopBrowserRecording('recording-tab');
    expect(startupEvents).toEqual(['publish:recording-tab', 'start-screencast']);
  });

  it('routes gesture-free starts through the desktop capture trigger', async () => {
    await recordings.startBrowserRecording('automation-recording-tab');
    expect(requestDisplayMediaCapture).toHaveBeenCalledWith('automation-recording-tab');
    expect(getDisplayMedia).toHaveBeenCalledTimes(1);
    await recordings.stopBrowserRecording('automation-recording-tab');
  });

  it('saves locally and releases capture before transferring the encoded recording once', async () => {
    const stopTrack = mock(() => undefined);
    getDisplayMedia.mockResolvedValue(stream(stopTrack));
    await recordings.startBrowserRecording('transfer-tab');
    let finishUpload!: () => void;
    const uploaded = new Promise<void>(resolve => { finishUpload = resolve; });
    const transfer = mock(async (artifact: { path: string }, blob: RecordingBlob) => {
      expect(save).toHaveBeenCalledTimes(1);
      expect(stopTrack).toHaveBeenCalled();
      expect(artifact.path).toBe('/tmp/recording-test.webm');
      expect(blob.type).toBe('video/webm;codecs=vp9');
      await uploaded;
      return 'uploaded-recording';
    });
    const localStop = recordings.stopBrowserRecording('transfer-tab');
    const firstStop = recordings.stopBrowserRecordingForUpload('transfer-tab', transfer);
    const secondStop = recordings.stopBrowserRecordingForUpload('transfer-tab', transfer);
    finishUpload();
    expect(await firstStop).toEqual(await secondStop);
    expect((await firstStop)?.uploadedAttachmentId).toBe('uploaded-recording');
    expect((await localStop)?.path).toBe('/tmp/recording-test.webm');
    expect(transfer).toHaveBeenCalledTimes(1);
  });

  it('keeps the saved desktop file and releases the recording when transfer fails', async () => {
    await recordings.startBrowserRecording('failed-transfer-tab');
    await expect(recordings.stopBrowserRecordingForUpload('failed-transfer-tab', async () => { throw new Error('Connection interrupted'); })).rejects.toThrow('Connection interrupted');
    expect(save).toHaveBeenCalledTimes(1);
    expect(recordings.readActiveBrowserRecordingTabIds().has('failed-transfer-tab')).toBe(false);
    await recordings.startBrowserRecording('failed-transfer-tab');
    await recordings.stopBrowserRecording('failed-transfer-tab');
  });

  it('paints and holds a hidden browser surface for the recording lifetime', async () => {
    startScreencast.mockImplementationOnce(async (tabId: string) => {
      expect(animationFrameCount).toBe(2);
      expect(activityByTabId[tabId]).toBe(1);
    });
    getDisplayMedia.mockImplementationOnce(async () => {
      expect(animationFrameCount).toBe(2);
      expect(activityByTabId['background-tab']).toBe(1);
      return stream();
    });
    await recordings.startBrowserRecording('background-tab');
    expect(activityByTabId['background-tab']).toBe(1);
    await recordings.stopBrowserRecording('background-tab');
    expect(activityByTabId['background-tab']).toBeUndefined();
  });

  it('bounds compositor warmup when animation frames are paused', async () => {
    const timers = useFakeTimers();
    requestAnimationFrame = () => 42;
    const startPromise = recordings.startBrowserRecording('hidden-window-tab');
    await timers.advance(BROWSER_RECORDING_PAINT_SETTLE_TIMEOUT_MS);
    await startPromise;
    expect(cancelAnimationFrame).toHaveBeenCalledWith(42);
    await recordings.stopBrowserRecording('hidden-window-tab');
  });

  for (const settings of [
    { width: 1280, height: 720, frameRate: 60, bitrate: 2_764_800 },
    { width: 320, height: 240, frameRate: 30, bitrate: 2_500_000 },
    { width: 3840, height: 2160, frameRate: 60, bitrate: 24_883_200 },
    { width: 7680, height: 4320, frameRate: 60, bitrate: 50_000_000 },
  ]) {
    it(`records the native ${settings.width} x ${settings.height} stream at ${settings.frameRate} fps`, async () => {
      const stopTrack = mock(() => { events.push('stop-track'); });
      const source = stream(stopTrack, settings);
      getDisplayMedia.mockResolvedValueOnce(source);
      await recordings.startBrowserRecording('recording-tab');
      expect(getDisplayMedia.mock.calls[0]?.[1]).toEqual({ audio: false, video: { frameRate: { ideal: 30, max: 30 } } });
      expect(FakeMediaRecorder.instances[0]?.stream).toBe(source);
      expect(FakeMediaRecorder.instances[0]?.options?.videoBitsPerSecond).toBe(settings.bitrate);
      await recordings.stopBrowserRecording('recording-tab');
      expect(stopTrack).toHaveBeenCalledTimes(1);
      expect(events.indexOf('stop-track')).toBeLessThan(events.indexOf('save'));
    });
  }

  it('uses the configured recording frame rate', async () => {
    clientSettings.browserRecordingFrameRate = 60;
    await recordings.startBrowserRecording('recording-tab');
    expect(getDisplayMedia.mock.calls[0]?.[1]).toEqual({ audio: false, video: { frameRate: { ideal: 60, max: 60 } } });
    await recordings.stopBrowserRecording('recording-tab');
  });

  it('clears a failed settings read before retrying recording', async () => {
    const tabId = 'settings-read-failure-tab';
    const error = new Error('Settings read failed');
    hydrateSettings.mockRejectedValueOnce(error);
    await expect(recordings.startBrowserRecording(tabId)).rejects.toBe(error);
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set());
    expect(activityByTabId[tabId]).toBeUndefined();
    expect(animationFrameCount).toBe(0);
    expect(startScreencast).not.toHaveBeenCalled();
    expect(stopScreencast).not.toHaveBeenCalled();
    expect(getDisplayMedia).not.toHaveBeenCalled();
    expect(FakeMediaRecorder.instances).toHaveLength(0);
    clientSettings.browserRecordingFrameRate = 60;
    await recordings.startBrowserRecording(tabId);
    expect(getDisplayMedia.mock.calls[0]?.[1]).toEqual({ audio: false, video: { frameRate: { ideal: 60, max: 60 } } });
    await recordings.stopBrowserRecording(tabId);
    expect(startScreencast).toHaveBeenCalledTimes(1);
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set());
    expect(activityByTabId[tabId]).toBeUndefined();
  });

  it('stops the native stream when MediaRecorder cleanup fails', async () => {
    const stopTrack = mock(() => undefined);
    getDisplayMedia.mockResolvedValueOnce(stream(stopTrack));
    await recordings.startBrowserRecording('recording-tab');
    FakeMediaRecorder.stopError = new Error('stop failed');
    await expect(recordings.stopBrowserRecording('recording-tab')).rejects.toMatchObject({ operation: 'cleanup', tabId: 'recording-tab' });
    expect(stopTrack).toHaveBeenCalledTimes(1);
  });

  it("uses the best supported encoder and saves the recorder's actual format", async () => {
    FakeMediaRecorder.supportedTypes = new Set(['video/mp4;codecs=avc1', 'video/mp4;codecs=avc1.42e01e', 'video/webm;codecs=vp9', 'video/webm;codecs=av1']);
    FakeMediaRecorder.outputMimeType = 'video/webm;codecs=av01';
    await recordings.startBrowserRecording('recording-tab');
    await recordings.stopBrowserRecording('recording-tab');
    expect(FakeMediaRecorder.instances[0]?.options).toEqual({ mimeType: 'video/mp4;codecs=avc1', videoBitsPerSecond: 3_110_400 });
    expect(save.mock.calls[0]?.slice(0, 2)).toEqual(['recording-tab', 'video/webm;codecs=av01']);
  });

  it('lets the browser select the format when no preferred encoding is supported', async () => {
    FakeMediaRecorder.supportedTypes = new Set();
    FakeMediaRecorder.outputMimeType = 'video/platform-default';
    await recordings.startBrowserRecording('recording-tab');
    await recordings.stopBrowserRecording('recording-tab');
    expect(FakeMediaRecorder.instances[0]?.options).toEqual({ videoBitsPerSecond: 3_110_400 });
    expect(save.mock.calls[0]?.slice(0, 2)).toEqual(['recording-tab', 'video/platform-default']);
  });

  it('reports when MediaRecorder provides no output format', async () => {
    FakeMediaRecorder.supportedTypes = new Set();
    FakeMediaRecorder.outputMimeType = '';
    const recorderOutput = FakeMediaRecorder.prototype.output;
    FakeMediaRecorder.prototype.output = () => ({ type: '', size: 0, path: '' });
    try {
      await recordings.startBrowserRecording('recording-tab');
      await expect(recordings.stopBrowserRecording('recording-tab')).rejects.toBeInstanceOf(BrowserRecordingFormatUnavailableError);
      expect(save).not.toHaveBeenCalled();
      expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set());
    } finally { FakeMediaRecorder.prototype.output = recorderOutput; }
  });

  it('releases the native capture lease when stream acquisition fails', async () => {
    getDisplayMedia.mockRejectedValueOnce(new Error('capture failed'));
    await expect(recordings.startBrowserRecording('recording-tab')).rejects.toMatchObject({ operation: 'capture-media-stream', tabId: 'recording-tab' });
    expect(stopScreencast).toHaveBeenCalledWith('recording-tab');
    expect(events.at(-1)).toBe('clear');
  });

  it('times out stalled stream acquisition and stops a late stream', async () => {
    const timers = useFakeTimers();
    let finishCapture!: (value: RecordingStream) => void;
    const stopTrack = mock(() => undefined);
    getDisplayMedia.mockImplementationOnce(() => new Promise<RecordingStream>(resolve => { finishCapture = resolve; }));
    const startPromise = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(getDisplayMedia).toHaveBeenCalledTimes(1));
    const rejection = settled(startPromise);
    await timers.advance(BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS);
    expect(await rejection).toMatchObject({ _tag: 'BrowserRecordingCaptureTimeoutError', tabId: 'recording-tab', timeoutMs: BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS });
    await expect(startPromise).rejects.toBeInstanceOf(BrowserRecordingCaptureTimeoutError);
    expect(stopScreencast).toHaveBeenCalledWith('recording-tab');
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set());
    expect(activityByTabId['recording-tab']).toBeUndefined();
    finishCapture(stream(stopTrack));
    await timers.advance(0);
    expect(stopTrack).toHaveBeenCalledTimes(1);
  });

  it('records separate tabs concurrently', async () => {
    const firstThreadRef = { environmentId: 'environment-recording', threadId: 'thread-recording-first' };
    const secondThreadRef = { environmentId: 'environment-recording', threadId: 'thread-recording-second' };
    await Promise.all([recordings.startBrowserRecording('recording-tab', firstThreadRef), recordings.startBrowserRecording('recording-tab-2', secondThreadRef)]);
    expect(startScreencast).toHaveBeenCalledTimes(2);
    expect(events).toContain('publish:recording-tab,recording-tab-2');
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set(['recording-tab', 'recording-tab-2']));
    expect(recordings.readActiveBrowserRecordingTabIds(firstThreadRef)).toEqual(new Set(['recording-tab']));
    expect(recordings.readActiveBrowserRecordingTabIds(secondThreadRef)).toEqual(new Set(['recording-tab-2']));
    await recordings.stopBrowserRecording('recording-tab');
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set(['recording-tab-2']));
    await recordings.stopBrowserRecording('recording-tab-2');
    expect(recordings.readActiveBrowserRecordingTabIds()).toEqual(new Set());
    expect(save).toHaveBeenCalledTimes(2);
  });

  it('serializes display media grants for concurrent recording starts', async () => {
    let finishFirstCapture!: (value: RecordingStream) => void;
    const source = stream();
    getDisplayMedia.mockImplementationOnce(() => new Promise<RecordingStream>(resolve => { finishFirstCapture = resolve; })).mockResolvedValueOnce(source);
    const firstStart = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(getDisplayMedia).toHaveBeenCalledTimes(1));
    const secondStart = recordings.startBrowserRecording('recording-tab-2');
    await waitFor(() => expect(recordings.readActiveBrowserRecordingTabIds().size).toBe(2));
    expect(startScreencast).toHaveBeenCalledTimes(1);
    finishFirstCapture(source);
    await Promise.all([firstStart, secondStart]);
    expect(startScreencast.mock.calls).toEqual([['recording-tab'], ['recording-tab-2']]);
    expect(getDisplayMedia).toHaveBeenCalledTimes(2);
    await Promise.all([recordings.stopBrowserRecording('recording-tab'), recordings.stopBrowserRecording('recording-tab-2')]);
  });

  it('cancels a queued recording when stopped before its media grant', async () => {
    let finishFirstCapture!: (value: RecordingStream) => void;
    const source = stream();
    getDisplayMedia.mockImplementationOnce(() => new Promise<RecordingStream>(resolve => { finishFirstCapture = resolve; }));
    const firstStart = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(getDisplayMedia).toHaveBeenCalledTimes(1));
    const secondStart = recordings.startBrowserRecording('recording-tab-2');
    await waitFor(() => expect(recordings.readActiveBrowserRecordingTabIds().size).toBe(2));
    const secondStop = recordings.stopBrowserRecording('recording-tab-2');
    await expect(secondStart).rejects.toBeInstanceOf(BrowserRecordingStartCancelledError);
    await expect(secondStop).resolves.toBeNull();
    expect(startScreencast).toHaveBeenCalledTimes(1);
    finishFirstCapture(source);
    await firstStart;
    await recordings.stopBrowserRecording('recording-tab');
    expect(getDisplayMedia).toHaveBeenCalledTimes(1);
  });

  it('latches a stop that arrives before the start becomes queued', async () => {
    let releaseDelayedPaint!: (time: number) => void;
    let frameId = 0;
    requestAnimationFrame = callback => { frameId += 1; if (frameId === 1) releaseDelayedPaint = callback; else callback(frameId); return frameId; };
    let finishBlockingCapture!: (value: RecordingStream) => void;
    const source = stream();
    getDisplayMedia.mockImplementationOnce(() => new Promise<RecordingStream>(resolve => { finishBlockingCapture = resolve; }));
    const delayedStart = recordings.startBrowserRecording('delayed-tab');
    await waitFor(() => expect(recordings.readActiveBrowserRecordingTabIds().has('delayed-tab')).toBe(true));
    const blockingStart = recordings.startBrowserRecording('blocking-tab');
    await waitFor(() => expect(getDisplayMedia).toHaveBeenCalledTimes(1));
    const delayedStop = recordings.stopBrowserRecording('delayed-tab');
    releaseDelayedPaint(1);
    await expect(delayedStart).rejects.toBeInstanceOf(BrowserRecordingStartCancelledError);
    await expect(delayedStop).resolves.toBeNull();
    expect(startScreencast).toHaveBeenCalledTimes(1);
    finishBlockingCapture(source);
    await blockingStart;
    await recordings.stopBrowserRecording('blocking-tab');
  });

  it('finishes an uncontended pre-grant start before stopping', async () => {
    const animationFrames: ((time: number) => void)[] = [];
    requestAnimationFrame = callback => { animationFrames.push(callback); return animationFrames.length; };
    const startPromise = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(recordings.readActiveBrowserRecordingTabIds().has('recording-tab')).toBe(true));
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    expect(startScreencast).not.toHaveBeenCalled();
    animationFrames.shift()?.(1);
    animationFrames.shift()?.(2);
    await startPromise;
    await expect(stopPromise).resolves.toMatchObject({ tabId: 'recording-tab' });
  });

  it('keeps a recording reachable through its runtime id after a server epoch changes', async () => {
    const threadRef = { environmentId: 'environment-recording', threadId: 'thread-recording-scoped' };
    const runtimeTabId = previewRuntimeTabId(threadRef, 'epoch-a', 'tab_1');
    await recordings.startBrowserRecording(runtimeTabId, threadRef, 'tab_1');
    expect(startScreencast).toHaveBeenCalledWith(runtimeTabId);
    expect(recordings.readActiveBrowserRecordingTabIds(threadRef)).toEqual(new Set([runtimeTabId]));
    expect(recordings.readActiveBrowserRecordingTargets(threadRef)).toEqual([{ runtimeTabId, serverTabId: 'tab_1' }]);
    expect(recordings.findActiveBrowserRecordingRuntimeTabId(threadRef, 'tab_1')).toBe(runtimeTabId);
    const replacementRuntimeTabId = previewRuntimeTabId(threadRef, 'epoch-b', 'tab_1');
    await expect(recordings.startBrowserRecording(replacementRuntimeTabId, threadRef, 'tab_1')).rejects.toBeInstanceOf(BrowserRecordingConflictError);
    expect(startScreencast).toHaveBeenCalledTimes(1);
    await recordings.stopBrowserRecording(runtimeTabId);
  });

  it('does not report success for a second start while the first is still starting', async () => {
    let finishStartingScreencast: (() => void) | undefined;
    startScreencast.mockImplementationOnce(async () => { events.push('start-screencast'); await new Promise<void>(resolve => { finishStartingScreencast = resolve; }); });
    const firstStart = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(startScreencast).toHaveBeenCalledTimes(1));
    await expect(recordings.startBrowserRecording('recording-tab')).rejects.toBeInstanceOf(BrowserRecordingConflictError);
    finishStartingScreencast?.();
    await firstStart;
    await recordings.stopBrowserRecording('recording-tab');
  });

  it('does not report success for a start while the recording is stopping', async () => {
    let finishStoppingScreencast: (() => void) | undefined;
    stopScreencast.mockImplementationOnce(async () => { await new Promise<void>(resolve => { finishStoppingScreencast = resolve; }); });
    await recordings.startBrowserRecording('recording-tab');
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    await waitFor(() => expect(stopScreencast).toHaveBeenCalledTimes(1));
    await expect(recordings.startBrowserRecording('recording-tab')).rejects.toBeInstanceOf(BrowserRecordingConflictError);
    finishStoppingScreencast?.();
    await stopPromise;
  });

  it('shares an in-progress stop with duplicate callers', async () => {
    let finishStoppingScreencast: (() => void) | undefined;
    stopScreencast.mockImplementationOnce(async () => { await new Promise<void>(resolve => { finishStoppingScreencast = resolve; }); });
    await recordings.startBrowserRecording('recording-tab');
    const firstStop = recordings.stopBrowserRecording('recording-tab');
    await waitFor(() => expect(stopScreencast).toHaveBeenCalledTimes(1));
    const duplicateStop = recordings.stopBrowserRecording('recording-tab');
    finishStoppingScreencast?.();
    const [firstArtifact, duplicateArtifact] = await Promise.all([firstStop, duplicateStop]);
    expect(duplicateArtifact).toEqual(firstArtifact);
    expect(stopScreencast).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it('finishes startup before stopping so an active recording yields an artifact', async () => {
    let finishStartingScreencast: (() => void) | undefined;
    startScreencast.mockImplementationOnce(async () => { events.push('start-screencast'); await new Promise<void>(resolve => { finishStartingScreencast = resolve; }); });
    const startPromise = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(startScreencast).toHaveBeenCalledTimes(1));
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    expect(stopScreencast).not.toHaveBeenCalled();
    finishStartingScreencast?.();
    await startPromise;
    await expect(stopPromise).resolves.toMatchObject({ tabId: 'recording-tab' });
    expect(stopScreencast).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledTimes(1);
    expect(events.at(-1)).toBe('clear');
  });

  it('does not release the recording slot until a cancelled start settles', async () => {
    let finishStartingScreencast: (() => void) | undefined;
    startScreencast.mockImplementationOnce(async () => { events.push('start-screencast'); await new Promise<void>(resolve => { finishStartingScreencast = resolve; }); });
    const firstStart = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(startScreencast).toHaveBeenCalledTimes(1));
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    const restartAfterStop = stopPromise.then(() => recordings.startBrowserRecording('recording-tab'));
    await new Promise(resolve => setTimeout(resolve, 0));
    const startCallsBeforeFirstSettled = startScreencast.mock.calls.length;
    finishStartingScreencast?.();
    await firstStart;
    await stopPromise;
    await restartAfterStop;
    await recordings.stopBrowserRecording('recording-tab');
    expect(startCallsBeforeFirstSettled).toBe(1);
  });

  it('keeps the recording slot while a failed stop waits for startup', async () => {
    let finishStartingScreencast: (() => void) | undefined;
    startScreencast.mockImplementationOnce(async () => { events.push('start-screencast'); await new Promise<void>(resolve => { finishStartingScreencast = resolve; }); });
    stopScreencast.mockRejectedValueOnce(new Error('initial stop failed'));
    const firstStart = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(startScreencast).toHaveBeenCalledTimes(1));
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    const rejectedStop = settled(stopPromise);
    expect(stopScreencast).not.toHaveBeenCalled();
    await expect(recordings.startBrowserRecording('recording-tab')).rejects.toBeInstanceOf(BrowserRecordingConflictError);
    finishStartingScreencast?.();
    await firstStart;
    expect(await rejectedStop).toMatchObject({ operation: 'stop-screencast', tabId: 'recording-tab' });
    expect(stopScreencast).toHaveBeenCalledTimes(1);
    await recordings.startBrowserRecording('recording-tab');
    await recordings.stopBrowserRecording('recording-tab');
  });

  it('fails a stop that waits too long for startup without freeing the recording slot', async () => {
    const timers = useFakeTimers();
    let finishStartingScreencast: (() => void) | undefined;
    startScreencast.mockImplementationOnce(async () => { events.push('start-screencast'); await new Promise<void>(resolve => { finishStartingScreencast = resolve; }); });
    const startPromise = recordings.startBrowserRecording('recording-tab');
    await waitFor(() => expect(startScreencast).toHaveBeenCalledTimes(1));
    const stopPromise = recordings.stopBrowserRecording('recording-tab');
    await timers.advance(0);
    expect(stopScreencast).not.toHaveBeenCalled();
    const rejection = settled(stopPromise);
    await timers.advance(BROWSER_RECORDING_STARTUP_SETTLE_TIMEOUT_MS);
    expect(await rejection).toMatchObject({ operation: 'wait-startup', tabId: 'recording-tab' });
    expect(save).not.toHaveBeenCalled();
    await expect(recordings.startBrowserRecording('recording-tab')).rejects.toBeInstanceOf(BrowserRecordingConflictError);
    finishStartingScreencast?.();
    await timers.advance(32);
    await startPromise;
    const cleanupResult = await recordings.stopBrowserRecording('recording-tab');
    expect(cleanupResult).toBeNull();
    expect(stopScreencast).toHaveBeenCalledTimes(1);
    expect(save).not.toHaveBeenCalled();
    expect(events.at(-1)).toBe('clear');
  });
});

describe('resolveBrowserRecordingStopTarget', () => {
  it('stops the only active recording when the implicit browser target changed', () => {
    expect(resolveBrowserRecordingStopTarget(new Set(['tab-recording']), 'tab-browsing')).toBe('tab-recording');
  });
  it('prefers an implicit target that is actively recording', () => {
    expect(resolveBrowserRecordingStopTarget(new Set(['tab-recording-a', 'tab-recording-b']), 'tab-recording-b')).toBe('tab-recording-b');
  });
  it('does not guess when multiple recordings are active and the implicit target is not one', () => {
    expect(resolveBrowserRecordingStopTarget(new Set(['tab-recording-a', 'tab-recording-b']), 'tab-browsing')).toBeNull();
  });
  it('only stops an explicitly requested tab when that tab is recording', () => {
    const activeTabIds = new Set(['tab-recording']);
    expect(resolveBrowserRecordingStopTarget(activeTabIds, 'tab-browsing', 'tab-recording')).toBe('tab-recording');
    expect(resolveBrowserRecordingStopTarget(activeTabIds, 'tab-recording', 'tab-browsing')).toBe(null);
  });
  it('returns null when no matching recording is active', () => {
    expect(resolveBrowserRecordingStopTarget(new Set(), 'tab-browsing')).toBeNull();
  });
});
