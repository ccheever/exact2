// The agent's `preview_recording_start` and `preview_recording_stop` on the
// previewAutomation host (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/
// PreviewAutomationHosts.tsx, the `recordingStart` and `recordingStop` cases; previewAutomationErrors.ts
// `PreviewAutomationRecordingNotActiveError`, `PreviewAutomationOperationError.fromCause`; apps/web/src/browser/
// browserRecording.ts, browserRecordingScope.ts, browserRecordingUpload.ts).
//
// The module's host waits for the tab and hands the request over (T3BrowserAutomationRecording.swift): it lists it in
// `presentation.browserAutomation.recordings` until an answer comes back. Each listed request runs here once, on the
// lifecycle (browser-recording.ts, the client's one `BrowserRecordings`: the Capture button's own recordings, so a tab
// the agent records shows Stop recording and a person's stop ends the agent's recording, as in the reference), and its
// outcome goes back to the module (`browserAutomation` with `answer`), which answers `previewAutomation.respond`. The
// work runs in one of the window's queued sends for it (below), not in the shell's answer that saw the request.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';
import { adoptCaptureNative, captureHost, captureNativeCall } from './browser-capture';
import type { Native } from '../client/protocol';
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

/** The module's upload (`browserRecordingUpload`) on the request's own connection. The connection goes under its own key:
 *  a module request with a top-level `fleet` is the fleet's (T3Module `later`), even an empty one. */
export function nativeDeps(client: T3Client, request: RecordingRequest): RecordingDeps {
  const connection = { connection: { generation: request.generation, fleet: request.fleet } };
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

// ── The window's queued sends (app.contract `automationRecording`) ──────────────────────────────
// A native call belongs to the answer whose continuation makes it: one an answer makes after a newer one let it go is
// refused, and one made after its answer was given waits for the next answer to settle. So the lifecycle cannot run
// detached from the shell's answers, which come and go with every status. It runs in a send of its own: the panel
// projects a serial that grows with each request the module hands over and names a send none of them runs on
// (`automationRecordingView`), the window's task sends `shelllocal:surface-browser-automation-recording` on that queued
// mutation (no send replaces another), and that send's answer runs every request not yet started to its end
// (`serveRecordingRequests`), as the Capture button's send runs its own recording. There are four sends: a queued send
// waits for the reply before it, and with one, a start listed during another thread's stop waited out that stop's
// upload, where the reference handles each request on its own (`handleRequest`; review of 2026-10-12). Only a fifth
// request in flight at once waits, behind one of the four (a Contract mutation's sends replace or queue: LLP 1092).
export const RECORDING_SENDS = 4;
type Served = { announced: Set<string>; serial: number; running: Map<string, Promise<void>>; done: Set<string>; busy: Set<number> };
const served = new WeakMap<T3Client, Served>();
function servedOf(client: T3Client): Served {
  let state = served.get(client);
  if (!state) { state = { announced: new Set(), serial: 0, running: new Map(), done: new Set(), busy: new Set() }; served.set(client, state); }
  return state;
}
export type AutomationRecordingView = { serial: number; slot: number };
/** The panel's projection (`shell.panel.automationRecording`): `serial` bumps once for each request the module newly
 *  lists; `slot` is the send the window's task sends on, one whose answer runs nothing (else one in turn). */
export function automationRecordingView(client: T3Client): AutomationRecordingView {
  const state = servedOf(client), listed = new Set(readRecordingRequests(client.presentation).map(request => request.key));
  for (const key of listed) if (!state.announced.has(key)) { state.announced.add(key); state.serial++; }
  for (const key of [...state.announced]) if (!listed.has(key)) state.announced.delete(key); // settled: no longer listed
  const free = Array.from({ length: RECORDING_SENDS }, (_, slot) => slot).find(slot => !state.busy.has(slot));
  return { serial: state.serial, slot: free ?? state.serial % RECORDING_SENDS };
}

/** Send `slot`'s answer: every listed request not yet started runs once, to its end, and its outcome goes to the module.
 *  The answer waits for the requests it started, never for another send's. */
export async function serveRecordingRequests(client: T3Client, native: Native, slot: number): Promise<void> {
  adoptCaptureNative(client, native); // this send's answer lives until its work ends: its native carries the calls
  const state = servedOf(client), requests = readRecordingRequests(client.presentation);
  const listed = new Set(requests.map(request => request.key));
  for (const key of [...state.done]) if (!listed.has(key)) state.done.delete(key);
  const started: Promise<void>[] = [];
  for (const request of requests) {
    if (state.running.has(request.key) || state.done.has(request.key)) continue;
    const run = (async () => {
      let answer: RecordingAnswer;
      try { answer = await runRecordingRequest(request, nativeDeps(client, request)); }
      catch { answer = { ok: false, error: operationError({ requestId: request.requestId, operation: request.operation, environmentId: request.environmentId, threadId: request.threadId, tabId: request.tabId }) }; }
      try { await captureNativeCall(client, { op: 'browserAutomation', plan: { answer: request.key, ...answer } as Obj }); }
      catch (error) { if (letGo(error)) throw error; }
      finally { state.running.delete(request.key); state.done.add(request.key); }
    })();
    state.running.set(request.key, run);
    started.push(run);
  }
  if (!started.length) return;
  state.busy.add(slot);
  try { await Promise.all(started); } finally { state.busy.delete(slot); }
}
