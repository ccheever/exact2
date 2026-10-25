// The agent's `preview_recording_start` and `preview_recording_stop` on the
// previewAutomation host (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/
// PreviewAutomationHosts.tsx, the `recordingStart` and `recordingStop` cases; previewAutomationErrors.ts
// `PreviewAutomationRecordingNotActiveError`, `PreviewAutomationOperationError.fromCause`; apps/web/src/browser/
// browserRecording.ts, browserRecordingScope.ts, browserRecordingUpload.ts), and the window's queued sends every
// long-lived piece of browser work runs in.
//
// A recording request runs on the lifecycle (browser-recording.ts, the client's one `BrowserRecordings`: the Capture
// button's own recordings, so a tab the agent records shows Stop recording and a person's stop ends the agent's
// recording, as in the reference); the executor (browser-automation-run.ts) runs it in one of the queued sends below and
// answers `previewAutomation.respond` with its outcome.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';
import { adoptCaptureNative, captureHost, captureNativeCall } from './browser-capture';
import type { Native } from '../client/protocol';
import {
  RECORDING_TRANSFER_ERROR_TAGS, resolveBrowserRecordingStopTarget, uploadBrowserRecording, type BrowserRecordings, type RecordingArtifact, type RecordingUploader,
} from './browser-recording';
import { operationError, targetUnavailableError, type ErrorContext, type HostErrorJson } from './browser-automation-plan';

/** One recording request, as the executor builds it from the request's plan. */
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

/** The request's connection, as the executor has it: an RPC there, the server's origin and the request's budget left. */
export type RecordingLink = { rpc: (method: string, payload: Obj) => Promise<Obj>; origin: string; remaining: () => number };
/** runAttachmentUploadCycle on the request's own connection: `attachments.createUploadUrl` and `attachments.delete` go out on
 *  this module's socket; the bytes go from the module (`browserRecordingUpload`, T3BrowserPage.swift), since the saved
 *  recording is in the artifact directory, outside the app's files this module can read. The transfer keeps the
 *  deadline on the module's clock; the mint keeps it on the run's (`expired` is the reference's
 *  `Date.now() >= deadlineMs - 1_000`). */
export function nativeDeps(client: T3Client, link: RecordingLink): RecordingDeps {
  const expired = () => link.remaining() <= 1000;
  return {
    recordings: captureHost(client).recordings,
    upload: async input => {
      // Encoding, saving and minting consume the same request budget. Leave time to reply.
      if (expired()) return { status: 'failed', attachmentId: null, error: new Error('Recording transfer deadline expired.'), expired: true };
      let minted: Obj;
      try { minted = await link.rpc('attachments.createUploadUrl', { type: 'file', name: input.name.slice(0, 255), mimeType: input.mimeType, sizeBytes: input.sizeBytes }); }
      catch (error) { if (letGo(error)) throw error; return { status: 'failed', attachmentId: null, error, expired: expired() }; }
      const attachmentId = str(minted.attachmentId), relativeUrl = str(minted.relativeUrl);
      if (!attachmentId || !relativeUrl) return { status: 'failed', attachmentId: attachmentId || null, error: new Error('The upload could not be prepared.'), expired: expired() };
      let value: Obj;
      try { value = await captureNativeCall(client, { op: 'browserRecordingUpload', origin: link.origin, relativeUrl, path: input.path, mimeType: input.mimeType, deadlineMs: input.deadlineMs }); }
      catch (error) {
        // A let-go call returns to no caller that would remove the minted attachment, so it goes here.
        if (letGo(error)) { void link.rpc('attachments.delete', { attachmentId }).catch(() => undefined); throw error; }
        return { status: 'failed', attachmentId, error, expired: expired() };
      }
      if (str(value.status) === 'uploaded') return { status: 'uploaded', attachmentId };
      return { status: 'failed', attachmentId, error: new Error(str(value.error, 'Recording upload failed.')), expired: value.expired === true };
    },
    remove: attachmentId => { void link.rpc('attachments.delete', { attachmentId }).catch(() => undefined); },
  };
}

// ── The window's queued sends (app.contract `automationRecording`) ──────────────────────────────
// A native call belongs to the answer whose continuation makes it: one an answer makes after a newer one let it go is
// refused, and one made after its answer was given waits for the next answer to settle. So work that outlives the
// shell's answers (an agent's request that waits on its page, a recording, a pick the person finishes) runs in a send of
// its own: `queueSlotWork` queues a keyed piece of work and bumps a serial the panel projects with a send none of the
// work runs on (`automationRecordingView`), the window's task sends `shelllocal:surface-browser-automation-recording` on
// that queued mutation (no send replaces another), and that send's answer runs every piece not yet started to its end
// with its own native (`serveRecordingRequests`). There are four sends: a queued send waits for the reply before it, and
// with one, a start listed during another thread's stop waited out that stop's upload, where the reference handles each
// request on its own (`handleRequest`; review of 2026-10-12). Only a fifth piece in flight at once waits, behind one of
// the four (a Contract mutation's sends replace or queue: LLP 1092).
export const RECORDING_SENDS = 4;
type Work = { key: string; run: (native: Native) => Promise<void> };
type Served = { queued: Work[]; running: Set<string>; serial: number; busy: Set<number> };
const served = new WeakMap<object, Served>();
function servedOf(client: object): Served {
  let state = served.get(client);
  if (!state) { state = { queued: [], running: new Set(), serial: 0, busy: new Set() }; served.set(client, state); }
  return state;
}
/** Queues `work` under `key` for the next queued send (a key already queued or running is left as it is). */
export function queueSlotWork(client: T3Client, key: string, work: (native: Native) => Promise<void>): void {
  const state = servedOf(client);
  if (state.running.has(key) || state.queued.some(entry => entry.key === key)) return;
  state.queued.push({ key, run: work });
  state.serial++;
  client.revision++;
}
/** Whether work under `key` is queued or running. */
export function slotWorkRunning(client: object, key: string): boolean {
  const state = servedOf(client);
  return state.running.has(key) || state.queued.some(entry => entry.key === key);
}
export type AutomationRecordingView = { serial: number; slot: number };
/** The panel's projection (`shell.panel.automationRecording`): `serial` bumps once for each piece of work queued; `slot`
 *  is the send the window's task sends on, one whose answer runs nothing (else one in turn). */
export function automationRecordingView(client: object): AutomationRecordingView {
  const state = servedOf(client);
  const free = Array.from({ length: RECORDING_SENDS }, (_, slot) => slot).find(slot => !state.busy.has(slot));
  return { serial: state.serial, slot: free ?? state.serial % RECORDING_SENDS };
}

/** Send `slot`'s answer: every queued piece of work runs once, to its end, with this send's native. The answer waits for
 *  the work it started, never for another send's. */
export async function serveRecordingRequests(client: T3Client, native: Native, slot: number): Promise<void> {
  adoptCaptureNative(client, native); // this send's answer lives until its work ends: its native carries the calls
  const state = servedOf(client), work = state.queued.splice(0);
  if (!work.length) return;
  const started = work.map(async entry => {
    state.running.add(entry.key);
    try { await entry.run(native); }
    catch (error) { if (letGo(error)) throw error; }
    finally { state.running.delete(entry.key); }
  });
  state.busy.add(slot);
  try { await Promise.all(started); } finally { state.busy.delete(slot); }
}
