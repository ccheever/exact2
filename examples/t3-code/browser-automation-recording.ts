// browser-recording-and-downloads BR-1: the agent's `preview_recording_start` and `preview_recording_stop` on the
// previewAutomation host (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/
// PreviewAutomationHosts.tsx, the `recordingStart` and `recordingStop` cases; previewAutomationErrors.ts
// `PreviewAutomationRecordingNotActiveError`, `PreviewAutomationOperationError.fromCause`; apps/web/src/browser/
// browserRecording.ts, browserRecordingScope.ts, browserRecordingUpload.ts).
//
// The module's host waits for the tab and hands the request over (T3BrowserAutomationRecording.swift): it lists it in
// `presentation.browserAutomation.recordings` until an answer comes back. Each listed request runs here once, on part 3's
// lifecycle (browser-recording.ts, the client's one `BrowserRecordings`: the Capture button's own recordings, so a tab
// the agent records shows Stop recording and a person's stop ends the agent's recording, as in the reference), and its
// outcome goes back to the module (`browserAutomation` with `answer`), which answers `previewAutomation.respond`. The
// work outlives the answer that started it: every module call goes through the newest answer's native.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { letGo } from './let-go';
import { captureHost, captureNativeCall } from './browser-capture';
import {
  RECORDING_TRANSFER_ERROR_TAGS, resolveBrowserRecordingStopTarget, uploadBrowserRecording, type BrowserRecordings, type RecordingArtifact, type RecordingUploader,
} from './browser-recording';
import { operationError, targetUnavailableError, type ErrorContext, type HostErrorJson } from './browser-automation-plan';

/** One request the module handed over (T3BrowserAutomation `handOver`). */
export type RecordingRequest = {
  key: string; requestId: string; connectionId: string; operation: 'recordingStart' | 'recordingStop'; environmentId: string; threadId: string;
  /** The request's tab, else the thread's current one (`request.tabId ?? state.snapshot?.tabId`). */
  tabId: string | null;
  tabIdExplicit: boolean;
  /** The ready tab's page (recordingStart). */
  runtimeId: string | null;
  input: Obj; generation: number; fleet: string;
  /** The request's host deadline (its receipt plus `resolveHostWaitBudgetMs`), in epoch milliseconds. */
  deadlineMs: number;
};
export function readRecordingRequests(presentation: unknown): RecordingRequest[] {
  const list = obj(obj(presentation).browserAutomation).recordings;
  return (Array.isArray(list) ? list : []).flatMap(raw => {
    const entry = obj(raw), operation = str(entry.operation);
    if (!str(entry.key) || !str(entry.requestId) || (operation !== 'recordingStart' && operation !== 'recordingStop')) return [];
    return [{ key: str(entry.key), requestId: str(entry.requestId), connectionId: str(entry.connectionId), operation, environmentId: str(entry.environmentId),
      threadId: str(entry.threadId), tabId: str(entry.tabId) || null, tabIdExplicit: entry.tabIdExplicit === true, runtimeId: str(entry.runtimeId) || null,
      input: obj(entry.input), generation: Number(entry.generation) || 0, fleet: str(entry.fleet), deadlineMs: Number(entry.deadlineMs) || 0 }];
  });
}

/** PreviewAutomationRecordingNotActiveError (answered as PreviewAutomationExecutionError). */
export const recordingNotActiveError = (context: Omit<ErrorContext, 'operation'>): HostErrorJson => ({
  _tag: 'PreviewAutomationExecutionError',
  message: `Preview automation request ${context.requestId} found no active recording for tab ${context.tabId ?? 'unassigned'} on environment ${context.environmentId} thread ${context.threadId}.`,
  detail: { requestId: context.requestId, environmentId: context.environmentId, threadId: context.threadId, tabId: context.tabId },
});
class HostFailure extends Error { constructor(readonly json: HostErrorJson) { super(json.message); } }

/** PreviewAutomationOperationError.fromCause: a host error answers as it is (the recording transfer errors, no active
 *  recording, an unavailable target); any other cause (a conflict, a failed start) is the operation's failure. */
export function recordingHostError(error: unknown, context: ErrorContext): HostErrorJson {
  if (error instanceof HostFailure) return error.json;
  const tag = error && typeof error === 'object' ? str((error as { _tag?: unknown })._tag) : '';
  if (RECORDING_TRANSFER_ERROR_TAGS.has(tag)) return { _tag: tag, message: (error as Error).message, detail: { threadId: context.threadId } };
  return operationError(context);
}

export type RecordingDeps = { recordings: BrowserRecordings; upload: RecordingUploader; remove: (attachmentId: string) => void };
export type RecordingAnswer = { ok: true; result: unknown } | { ok: false; error: HostErrorJson };

/** The two cases of `handleRequest`. */
export async function runRecordingRequest(request: RecordingRequest, deps: RecordingDeps): Promise<RecordingAnswer> {
  const ref = { environmentId: request.environmentId, threadId: request.threadId };
  let tabId = request.tabId;
  const context = (): ErrorContext => ({ requestId: request.requestId, operation: request.operation, environmentId: request.environmentId, threadId: request.threadId, tabId });
  try {
    if (request.operation === 'recordingStart') {
      if (!tabId || !request.runtimeId) throw new HostFailure(targetUnavailableError(context()));
      const startedAt = await deps.recordings.startBrowserRecording(request.runtimeId, ref, tabId);
      return { ok: true, result: { tabId, recording: true, startedAt } };
    }
    const activeRecordings = deps.recordings.readActiveBrowserRecordingTargets(ref);
    const stopTabId = resolveBrowserRecordingStopTarget(new Set(activeRecordings.map(recording => recording.serverTabId)), tabId, request.tabIdExplicit ? request.tabId ?? undefined : undefined);
    tabId = stopTabId ?? tabId;
    const stopRuntimeTabId = activeRecordings.find(recording => recording.serverTabId === stopTabId)?.runtimeTabId ?? null;
    const transferToEnvironment = request.input.transferToEnvironment === true;
    let artifact: (RecordingArtifact & { uploadedAttachmentId?: string }) | null = null;
    if (stopRuntimeTabId) {
      artifact = transferToEnvironment
        ? await deps.recordings.stopBrowserRecordingForUpload(stopRuntimeTabId, (saved, blob) => uploadBrowserRecording(ref, saved, blob, request.deadlineMs, deps.upload, deps.remove))
        : await deps.recordings.stopBrowserRecording(stopRuntimeTabId);
    }
    if (!artifact || !stopTabId) throw new HostFailure(recordingNotActiveError(context()));
    return { ok: true, result: { ...artifact, tabId: stopTabId } };
  } catch (error) { return { ok: false, error: recordingHostError(error, context()) }; }
}

/** The module's upload (`browserRecordingUpload`) on the request's own connection. */
function nativeDeps(client: T3Client, request: RecordingRequest): RecordingDeps {
  const connection = { generation: request.generation, fleet: request.fleet };
  return {
    recordings: captureHost(client).recordings,
    upload: async input => {
      const value = await captureNativeCall(client, { op: 'browserRecordingUpload', action: 'upload', ...connection, ...input });
      if (str(value.status) === 'uploaded' && str(value.attachmentId)) return { status: 'uploaded', attachmentId: str(value.attachmentId) };
      return { status: 'failed', attachmentId: str(value.attachmentId) || null, error: new Error(str(value.error, 'Recording upload failed.')), expired: value.expired === true };
    },
    remove: attachmentId => { void captureNativeCall(client, { op: 'browserRecordingUpload', action: 'remove', attachmentId, ...connection }).catch(() => undefined); },
  };
}

const started = new WeakMap<T3Client, Set<string>>();
/** automationPrepare: each handed-over request runs once; its answer goes back to the module. */
export function serveRecordingRequests(client: T3Client): void {
  const requests = readRecordingRequests(client.presentation);
  let seen = started.get(client);
  if (!seen) { seen = new Set(); started.set(client, seen); }
  const listed = new Set(requests.map(request => request.key));
  for (const key of [...seen]) if (!listed.has(key)) seen.delete(key); // settled: the module no longer lists it
  for (const request of requests) {
    if (seen.has(request.key)) continue;
    seen.add(request.key);
    void (async () => {
      let answer: RecordingAnswer;
      try { answer = await runRecordingRequest(request, nativeDeps(client, request)); }
      catch { answer = { ok: false, error: operationError({ requestId: request.requestId, operation: request.operation, environmentId: request.environmentId, threadId: request.threadId, tabId: request.tabId }) }; }
      // The newest answer's native: the one that saw the request may have been let go meanwhile.
      for (let attempt = 0; attempt < 3; attempt++) {
        try { await captureNativeCall(client, { op: 'browserAutomation', plan: { answer: request.key, ...answer } as Obj }); return; }
        catch (error) { if (!letGo(error)) return; }
      }
    })();
  }
}
