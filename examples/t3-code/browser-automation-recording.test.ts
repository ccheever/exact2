// browser-recording-and-downloads BR-1: the previewAutomation host's `recordingStart` and `recordingStop` (T3 Code
// 1e2ecbd975, MIT, see LICENSE-T3: apps/web/src/components/preview/PreviewAutomationHosts.tsx, previewAutomationErrors.ts,
// apps/web/src/browser/browserRecordingUpload.ts; the server's claimPreviewRecording for what the answer must carry).
// The clone's own rows: the reference has no host tests for these cases. The lifecycle is part 3's BrowserRecordings
// (browser-recording.test.ts ports its tests) over a fake RecordingHost; the module's hand-off and the native upload are
// the AppKit binary `macos/tests/browser-automation` (recording.swift).
import { describe, expect, it } from 'bun:test';
import {
  BrowserRecordings, PreviewAutomationRecordingDeadlineExpiredError, PreviewAutomationRecordingTooLargeError, PreviewAutomationRecordingTransferError,
  RECORDING_UPLOAD_MAX_BYTES, uploadBrowserRecording, type RecordingHost, type RecordingUploader,
} from './browser-recording';
import { readRecordingRequests, recordingHostError, runRecordingRequest, type RecordingRequest } from './browser-automation-recording';

const STARTED_AT = '2026-10-11T12:00:00.000Z';
function fakeRecordings(options: { size?: number } = {}) {
  const calls: string[] = [];
  const host: RecordingHost = {
    bridge: {
      startScreencast: async tabId => { calls.push(`arm ${tabId}`); queueMicrotask(() => recordings.triggerTabMediaCapture(tabId)); },
      stopScreencast: async tabId => { calls.push(`disarm ${tabId}`); },
      save: async (tabId, mimeType, data) => {
        calls.push(`save ${tabId}`);
        return { id: 'browser-recording-mg3k2a1b', tabId, path: '/lane/userdata/browser-artifacts/browser-recording-mg3k2a1b.mp4', mimeType, sizeBytes: data.size, createdAt: STARTED_AT };
      },
    },
    getDisplayMedia: async () => ({ getVideoTracks: () => [{ getSettings: () => ({ width: 1280, height: 800, frameRate: 30 }) }], getTracks: () => [{ stop: () => undefined }] }),
    isTypeSupported: type => type === 'video/mp4;codecs=avc1',
    createRecorder: (tabId, _stream, recorderOptions) => {
      let state: 'inactive' | 'recording' = 'inactive';
      return {
        get state() { return state; },
        mimeType: recorderOptions.mimeType ?? '',
        start: () => { state = 'recording'; calls.push(`begin ${tabId}`); },
        stop: async () => { state = 'inactive'; calls.push(`finish ${tabId}`); },
        output: () => ({ type: 'video/mp4', size: options.size ?? 4096, path: '/tmp/encoded.mp4' }),
      };
    },
    createCompositor: async () => null,
    hydrateSettings: async () => undefined,
    settings: () => ({ browserRecordingFrameRate: 30, browserRecordingShowKeyPresses: false, browserRecordingShowMousePresses: false }),
    acquireSurfaceActivity: () => () => undefined,
    publish: () => undefined,
    requestAnimationFrame: callback => { queueMicrotask(() => callback(0)); return 1; },
    cancelAnimationFrame: () => undefined,
    setTimeout: (fn, ms) => setTimeout(fn, ms),
    clearTimeout: handle => clearTimeout(handle as ReturnType<typeof setTimeout>),
    now: () => STARTED_AT,
  };
  const recordings = new BrowserRecordings(host);
  return { recordings, calls };
}

const RUNTIME = '["env-1","thread-1","epoch-1","tab_1"]';
const request = (operation: RecordingRequest['operation'], extra: Partial<RecordingRequest> = {}): RecordingRequest => ({
  key: `\u0000connection-1\u0000preview-${operation}`, requestId: `preview-${operation}`, connectionId: 'connection-1', operation, environmentId: 'env-1', threadId: 'thread-1',
  tabId: 'tab_1', tabIdExplicit: false, runtimeId: operation === 'recordingStart' ? RUNTIME : null, input: operation === 'recordingStop' ? { transferToEnvironment: true } : {},
  generation: 3, fleet: '', deadlineMs: 2_000_000_000_000, ...extra,
});
const uploaded: RecordingUploader = async () => ({ status: 'uploaded', attachmentId: 'pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4' });

describe('the previewAutomation host\'s recording operations', () => {
  it('reads the requests the module handed over and drops malformed ones', () => {
    const entry = { ...request('recordingStop'), tabId: null };
    const presentation = { browserAutomation: { recordings: [entry, { key: 'k', requestId: 'r', operation: 'click' }, { operation: 'recordingStart' }, null] } };
    expect(readRecordingRequests(presentation)).toEqual([{ ...entry, tabId: null }]);
    expect(readRecordingRequests({})).toEqual([]);
  });

  it('recordingStart starts the tab\'s recording (the Capture button\'s) and answers PreviewAutomationRecordingStatus', async () => {
    const { recordings, calls } = fakeRecordings();
    const answer = await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    expect(answer).toEqual({ ok: true, result: { tabId: 'tab_1', recording: true, startedAt: STARTED_AT } });
    expect(calls).toEqual([`arm ${RUNTIME}`, `begin ${RUNTIME}`]);
    // The chrome row reads the same recordings: the tab shows Stop recording.
    expect(recordings.findActiveBrowserRecordingRuntimeTabId({ environmentId: 'env-1', threadId: 'thread-1' }, 'tab_1')).toBe(RUNTIME);
    // An already running recording answers its own start time (startBrowserRecording).
    expect(await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined })).toEqual(answer);
  });

  it('recordingStop stops the thread\'s recording, saves it and answers the artifact with its uploaded attachment once', async () => {
    const { recordings, calls } = fakeRecordings();
    await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    const uploads: Array<Parameters<RecordingUploader>[0]> = [];
    const answer = await runRecordingRequest(request('recordingStop', { tabId: null }), { recordings, remove: () => undefined,
      upload: async input => { uploads.push(input); return uploaded(input); } });
    expect(answer).toEqual({ ok: true, result: {
      id: 'browser-recording-mg3k2a1b', tabId: 'tab_1', path: '/lane/userdata/browser-artifacts/browser-recording-mg3k2a1b.mp4', mimeType: 'video/mp4;codecs=avc1',
      sizeBytes: 4096, createdAt: STARTED_AT, uploadedAttachmentId: 'pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4',
    } });
    // The one active recording of the thread is the target with no tab named (resolveBrowserRecordingStopTarget).
    expect(calls.slice(2)).toEqual([`disarm ${RUNTIME}`, `finish ${RUNTIME}`, `save ${RUNTIME}`]);
    expect(uploads).toEqual([{ name: 'browser-recording-mg3k2a1b.mp4', mimeType: 'video/mp4;codecs=avc1', sizeBytes: 4096, path: '/lane/userdata/browser-artifacts/browser-recording-mg3k2a1b.mp4', deadlineMs: 2_000_000_000_000 }]);
    expect(recordings.readActiveBrowserRecordingTabIds().size).toBe(0);
  });

  it('a stop the server does not ask to transfer answers the local artifact', async () => {
    const { recordings } = fakeRecordings();
    await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    const answer = await runRecordingRequest(request('recordingStop', { input: {} }), { recordings, upload: uploaded, remove: () => undefined });
    expect(answer.ok && (answer.result as { uploadedAttachmentId?: string }).uploadedAttachmentId).toBe(undefined);
    expect(answer.ok && (answer.result as { tabId: string }).tabId).toBe('tab_1');
  });

  it('a stop with no active recording, or one of another tab named explicitly, answers PreviewAutomationRecordingNotActiveError', async () => {
    const { recordings } = fakeRecordings();
    expect(await runRecordingRequest(request('recordingStop'), { recordings, upload: uploaded, remove: () => undefined })).toEqual({ ok: false, error: {
      _tag: 'PreviewAutomationExecutionError',
      message: 'Preview automation request preview-recordingStop found no active recording for tab tab_1 on environment env-1 thread thread-1.',
      detail: { requestId: 'preview-recordingStop', environmentId: 'env-1', threadId: 'thread-1', tabId: 'tab_1' },
    } });
    await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    const other = await runRecordingRequest(request('recordingStop', { tabId: 'tab_2', tabIdExplicit: true }), { recordings, upload: uploaded, remove: () => undefined });
    expect(other.ok).toBe(false);
    expect(!other.ok && other.error.message).toContain('found no active recording for tab tab_2');
    expect(recordings.readActiveBrowserRecordingTabIds().size).toBe(1);
  });

  it('a recording conflict is the operation\'s failure; a start with no ready tab is an unavailable target', async () => {
    const { recordings } = fakeRecordings();
    await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    // The same server tab under another runtime id (a new server epoch): BrowserRecordingConflictError.
    const conflict = await runRecordingRequest(request('recordingStart', { runtimeId: '["env-1","thread-1","epoch-2","tab_1"]' }), { recordings, upload: uploaded, remove: () => undefined });
    expect(conflict).toEqual({ ok: false, error: {
      _tag: 'PreviewAutomationExecutionError',
      message: 'Preview automation recordingStart request preview-recordingStart failed on environment env-1 thread thread-1 (tab tab_1).',
      detail: { requestId: 'preview-recordingStart', operation: 'recordingStart', environmentId: 'env-1', threadId: 'thread-1', tabId: 'tab_1' },
    } });
    const unavailable = await runRecordingRequest(request('recordingStart', { runtimeId: null }), { recordings, upload: uploaded, remove: () => undefined });
    expect(!unavailable.ok && unavailable.error._tag).toBe('PreviewAutomationTabNotFoundError');
  });

  it('a failed transfer answers the contracts\' transfer error, removes the pending upload and keeps the saved copy', async () => {
    const { recordings } = fakeRecordings();
    await runRecordingRequest(request('recordingStart'), { recordings, upload: uploaded, remove: () => undefined });
    const removed: string[] = [];
    const answer = await runRecordingRequest(request('recordingStop'), { recordings, remove: id => removed.push(id),
      upload: async () => ({ status: 'failed', attachmentId: 'pending-1-mp4', error: new Error('Recording upload rejected (413).') }) });
    expect(answer).toEqual({ ok: false, error: { _tag: 'PreviewAutomationRecordingTransferError',
      message: 'Preview recording could not be saved to the agent environment. The saved copy remains on the desktop.', detail: { threadId: 'thread-1' } } });
    expect(removed).toEqual(['pending-1-mp4']);
  });
});

describe('uploadBrowserRecording', () => {
  const ref = { environmentId: 'env-1', threadId: 'thread-1' };
  const artifact = { id: 'browser-recording-a', tabId: RUNTIME, path: '/artifacts/browser-recording-a.mp4', mimeType: 'video/mp4', sizeBytes: 10, createdAt: STARTED_AT };
  it('refuses a recording over 50 MiB before minting anything', async () => {
    let minted = 0;
    const error = await uploadBrowserRecording(ref, artifact, { type: 'video/mp4', size: RECORDING_UPLOAD_MAX_BYTES + 1, path: artifact.path }, 0, async () => { minted++; return uploaded({} as never); }, () => undefined).catch(cause => cause);
    expect(error).toBeInstanceOf(PreviewAutomationRecordingTooLargeError);
    expect((error as Error).message).toBe('The recording exceeds 50 MiB. The saved copy remains on the desktop.');
    expect(minted).toBe(0);
  });
  it('a transfer that ran out of the request\'s budget is the deadline error, any other the transfer error', async () => {
    const blob = { type: 'video/mp4', size: 10, path: artifact.path };
    const expired = await uploadBrowserRecording(ref, artifact, blob, 5, async () => ({ status: 'failed', attachmentId: null, error: new Error('Recording transfer deadline expired.'), expired: true }), () => undefined).catch(cause => cause);
    expect(expired).toBeInstanceOf(PreviewAutomationRecordingDeadlineExpiredError);
    expect((expired as Error).message).toBe('The recording transfer deadline expired. The saved copy remains on the desktop.');
    const failed = await uploadBrowserRecording(ref, artifact, blob, 5, async () => ({ status: 'failed', error: new Error('offline') }), () => undefined).catch(cause => cause);
    expect(failed).toBeInstanceOf(PreviewAutomationRecordingTransferError);
    expect(recordingHostError(expired, { requestId: 'r', operation: 'recordingStop', environmentId: 'env-1', threadId: 'thread-1', tabId: 'tab_1' })).toEqual({
      _tag: 'PreviewAutomationRecordingDeadlineExpiredError', message: 'The recording transfer deadline expired. The saved copy remains on the desktop.', detail: { threadId: 'thread-1' } });
  });
});
