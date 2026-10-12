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
import { readFileSync } from 'node:fs';
import { automationRecordingView, nativeDeps, readRecordingRequests, RECORDING_SENDS, recordingHostError, runRecordingRequest, serveRecordingRequests, type RecordingRequest } from './browser-automation-recording';
import { surfaceLocal, surfaceStore } from './r4-surfaces-panel';
import { adoptCaptureNative } from './browser-capture';
import type { T3Client } from './client';
import type { Native } from './protocol';

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

describe('the window\'s send for the handed-over requests', () => {
  const nativeLog = (log: unknown[]): Native => ({ available: true, watch() {}, later: async (value: unknown) => { log.push(value); return { ok: true, generation: 0, value: { accepted: true } }; } } as unknown as Native);
  it('the panel\'s serial bumps once for each request the module newly lists', () => {
    const client = { presentation: {}, local: {}, revision: 0 } as unknown as T3Client & { presentation: unknown };
    expect(automationRecordingView(client)).toEqual({ serial: 0, slot: 0 });
    client.presentation = { browserAutomation: { recordings: [request('recordingStart')] } };
    expect(automationRecordingView(client).serial).toBe(1);
    expect(automationRecordingView(client).serial).toBe(1); // projected again: no new request
    client.presentation = { browserAutomation: { recordings: [request('recordingStart'), request('recordingStop')] } };
    expect(automationRecordingView(client).serial).toBe(2);
    client.presentation = { browserAutomation: { recordings: [] } };
    expect(automationRecordingView(client)).toEqual({ serial: 2, slot: 0 });
  });

  it('a request listed while another send\'s request runs starts at once on a free send (handleRequest per request)', async () => {
    // Thread A's request is still running (its answer to the module is held, as a stop's upload would hold it); thread
    // B's start, listed meanwhile, runs on another send and answers without waiting for A (review of 2026-10-12).
    const a = request('recordingStop', { key: 'a', requestId: 'a', threadId: 'thread-a' }), b = request('recordingStart', { key: 'b', requestId: 'b', threadId: 'thread-b', runtimeId: null });
    const client = { presentation: { browserAutomation: { recordings: [a] } }, local: {}, revision: 0 } as unknown as T3Client & { presentation: unknown };
    let release = () => {};
    const held = new Promise<void>(resolve => { release = resolve; });
    const answered: string[] = [];
    const native = (hold: boolean): Native => ({ available: true, watch() {}, later: async (value: { op?: string; plan?: { answer?: string } }) => {
      if (value.op === 'browserAutomation') { if (hold) await held; answered.push(String(value.plan?.answer)); }
      return { ok: true, generation: 0, value: {} };
    } } as unknown as Native);
    expect(automationRecordingView(client)).toEqual({ serial: 1, slot: 0 });
    let aDone = false;
    const first = serveRecordingRequests(client, native(true), 0).then(() => { aDone = true; });
    await new Promise(resolve => setTimeout(resolve, 1));
    client.presentation = { browserAutomation: { recordings: [a, b] } };
    expect(automationRecordingView(client)).toEqual({ serial: 2, slot: 1 }); // send 0 runs A
    await serveRecordingRequests(client, native(false), 1);
    expect(answered).toEqual(['b']);
    expect(aDone).toBe(false);
    release();
    await first;
    expect(answered).toEqual(['b', 'a']);
    expect(automationRecordingView(client).slot).toBe(0); // both sends free again
  });

  it('with every send running a request, a new one waits behind one of them in turn', async () => {
    const client = { presentation: {}, local: {}, revision: 0 } as unknown as T3Client & { presentation: unknown };
    let release = () => {};
    const held = new Promise<void>(resolve => { release = resolve; });
    const holding = { available: true, watch() {}, later: async () => { await held; return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    const running: Promise<void>[] = [], listed: RecordingRequest[] = [];
    for (let slot = 0; slot < RECORDING_SENDS; slot++) {
      listed.push(request('recordingStart', { key: `k${slot}`, requestId: `r${slot}`, runtimeId: null }));
      client.presentation = { browserAutomation: { recordings: [...listed] } };
      expect(automationRecordingView(client)).toEqual({ serial: slot + 1, slot });
      running.push(serveRecordingRequests(client, holding, slot));
      await new Promise(resolve => setTimeout(resolve, 1));
    }
    client.presentation = { browserAutomation: { recordings: [...listed, request('recordingStart', { key: 'k4', requestId: 'r4', runtimeId: null })] } };
    expect(automationRecordingView(client)).toEqual({ serial: 5, slot: 5 % RECORDING_SENDS });
    release();
    await Promise.all(running);
  });

  it('its answer runs each listed request once, to its end, and sends the outcome to the module through its own native', async () => {
    const stop = request('recordingStop');
    const client = { presentation: { browserAutomation: { recordings: [stop] } }, local: {}, revision: 0 } as unknown as T3Client;
    const first: unknown[] = [], second: unknown[] = [];
    await serveRecordingRequests(client, nativeLog(first), 0); // awaited: the send's answer lives until the request settled
    await serveRecordingRequests(client, nativeLog(second), 1); // listed again before the module's status caught up: not run twice
    expect(first.filter(sent => (sent as { op?: string }).op === 'browserAutomation')).toEqual([{ op: 'browserAutomation', plan: { answer: stop.key, ok: false, error: {
      _tag: 'PreviewAutomationExecutionError',
      message: 'Preview automation request preview-recordingStop found no active recording for tab tab_1 on environment env-1 thread thread-1.',
      detail: { requestId: 'preview-recordingStop', environmentId: 'env-1', threadId: 'thread-1', tabId: 'tab_1' } } } }]);
    expect(second).toEqual([]);
  });

  it('the upload goes to the module with its connection under its own key, never a top-level fleet (T3Module routes those to the fleet)', async () => {
    const client = { presentation: {}, local: {}, revision: 0 } as unknown as T3Client;
    const sent: Array<Record<string, unknown>> = [];
    adoptCaptureNative(client, { available: true, watch() {}, later: async (value: Record<string, unknown>) => {
      sent.push(value);
      return { ok: true, generation: 0, value: value.action === 'upload' ? { status: 'uploaded', attachmentId: 'pending-1-mp4' } : {} };
    } } as unknown as Native);
    const deps = nativeDeps(client, request('recordingStop', { generation: 9, fleet: '' }));
    expect(await deps.upload({ name: 'a.mp4', mimeType: 'video/mp4', sizeBytes: 3, path: '/artifacts/a.mp4', deadlineMs: 5 })).toEqual({ status: 'uploaded', attachmentId: 'pending-1-mp4' });
    deps.remove('pending-1-mp4');
    await new Promise(resolve => setTimeout(resolve, 5));
    expect(sent).toEqual([
      { op: 'browserRecordingUpload', action: 'upload', connection: { generation: 9, fleet: '' }, name: 'a.mp4', mimeType: 'video/mp4', sizeBytes: 3, path: '/artifacts/a.mp4', deadlineMs: 5 },
      { op: 'browserRecordingUpload', action: 'remove', attachmentId: 'pending-1-mp4', connection: { generation: 9, fleet: '' } },
    ]);
    expect(sent.every(value => !('fleet' in value) && !('generation' in value))).toBe(true);
  });

  it('is one of the window\'s four queued sends, the one the panel names, asked whenever the panel\'s serial passes the one it handled', () => {
    const app = readFileSync(new URL('./app.contract', import.meta.url), 'utf8').split('\n');
    const at = (text: string) => app.findIndex(line => line.trimStart().startsWith(text));
    for (let slot = 0; slot < RECORDING_SENDS; slot++) expect(app[at(`mutation automationRecordingServed${slot} `)]).toContain('as shape Change queue refreshes data');
    expect(at(`mutation automationRecordingServed${RECORDING_SENDS} `)).toBe(-1);
    expect(app[at('task automationRecording when')]).toBe('  task automationRecording when shell.panel.automationRecording.serial > automationRecordingHandled key=shell.panel.automationRecording.serial');
    const action = app.slice(at('action automationRecordingNow'), at('action automationRecordingNow') + 10).map(line => line.trim());
    expect(action.slice(0, 2)).toEqual(['action automationRecordingNow', 'automationRecordingHandled = shell.panel.automationRecording.serial']);
    for (let slot = 0; slot < RECORDING_SENDS; slot++) expect(action).toContain(`send automationRecordingServed${slot} = command("shelllocal:surface-browser-automation-recording", "${slot}", "", 0)`);
    expect(action).toContain('if shell.panel.automationRecording.slot == 1');
    expect(readFileSync(new URL('./r4-surfaces-shapes.contract', import.meta.url), 'utf8')).toContain('  automationRecording: R4AutomationRecording');
  });

  it('is not the person\'s use of the panel: the viewed thread\'s panel is left as it was (no userRevision, no state)', async () => {
    // An agent's request served before the viewed thread's saved panel was restored must not make restoreRightPanel skip
    // it (review of 2026-10-12): the op runs before surfaceLocal reads the panel.
    const stop = request('recordingStop');
    const client = { environmentId: 'env', threadId: 't1', projectId: 'p1', draftKey: 'env:new:p1', ready: false, generation: 1, revision: 0, diffOpen: false,
      presentation: { browserAutomation: { recordings: [stop] } }, local: {}, shell: { projects: [], threads: [] } } as unknown as T3Client;
    const sent: unknown[] = [];
    expect(await surfaceLocal(client, nativeLog(sent), 'browser-automation-recording', '2', '')).toBe('');
    expect(sent.filter(value => (value as { op?: string }).op === 'browserAutomation').length).toBe(1);
    expect(surfaceStore(client).panels.size).toBe(0);
  });
});
