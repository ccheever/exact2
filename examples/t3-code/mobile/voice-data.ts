// @ref llp/1106.008-mobile-voice.decision.md#answer-lifetime
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { activeInput, pendingRequests } from './shared/requests';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { letGoAware, letGo } from './shared/let-go';
import { voiceInputBlocksSubmission, type VoiceDraftSnapshot } from './voice-controller';
import { VoiceInputSession, createVoiceInputTarget } from './voice-session';
import { VoiceTranscriptionError, throwIfVoiceTranscriptionAborted } from './voice-transcription';
import { resolveVoiceComposerPresentation } from './voice-presentation';
import { normalizeVoiceInputDecibels } from './voice-metering';
import { mobileComposerTargetCurrent, mobileComposerTargetText, mobileComposerTargetWriteText,
  mobileComposerTargetPersist, mobileComposerTargetRequire, mobileComposerTargetRevision, type MobileComposerTarget } from './composer-target';

export interface VoiceStatus { available: boolean; locale: string; reason: string; session: string; event: number;
  eventKind: string; error: string; uri: string; elapsed: number; levels: number[]; phase: string }
export interface VoiceBar { id: number; height: number; compactHeight: number; opacity: number }
export interface VoiceSnapshot { available: boolean; owner: string; label: string; phase: string; error: string; errorAction: string;
  busy: boolean; focused: boolean; globalVisible: boolean; elapsed: number; elapsedLabel: string; statusLabel: string;
  confirmationEnabled: boolean; bars: VoiceBar[]; compactBars: VoiceBar[]; selectionOwner: string; selectionStart: number; selectionEnd: number; selectionRevision: number }
export interface VoiceResult { revision: number; message: string; data: VoiceSnapshot }
interface Scope { native: Native; storage: Files; pending: Promise<unknown>[]; failed: unknown }
interface Target { key: string; content: MobileComposerTarget; text: string | null; revision: number; contentRevision: number | null; observer?: () => void }
interface Runtime { client: T3Client; session: VoiceInputSession; scope: Scope | null; serial: number; id: string; uri: string | null;
  status: VoiceStatus; handledEvent: number; target: Target | null; selection: { owner: string; start: number; end: number; revision: number; content: MobileComposerTarget | null; text: string; contentRevision: number | null } }
const states = new WeakMap<T3Client, Runtime>();
const emptyStatus = (): VoiceStatus => ({ available: false, locale: '', reason: '', session: '', event: 0, eventKind: '', error: '', uri: '', elapsed: 0, levels: [], phase: 'idle' });
function stateFor(client: T3Client): Runtime {
  const existing = states.get(client); if (existing) return existing;
  const runtime = { client, scope: null, serial: 0, id: '', uri: null, status: emptyStatus(), handledEvent: 0, target: null,
    selection: { owner: '', start: 0, end: 0, revision: 0, content: null, text: '', contentRevision: null } } as unknown as Runtime;
  const request = (action: string, fields: object = {}) => invoke(runtime, action, fields);
  const enqueue = (operation: Promise<unknown>) => {
    const scope = runtime.scope; if (!scope) throw new Error('Voice operation has no active answer.');
    scope.pending.push(operation.catch(error => { scope.failed ??= error; }));
  };
  runtime.session = new VoiceInputSession({
    recorder: {
      get uri() { return runtime.uri; },
      async prepareToRecordAsync() { runtime.uri = str((await request('recorder-prepare')).uri) || null; },
      record({ forDuration }) { enqueue(request('record', { seconds: forDuration })); },
      async stop() { const result = await request('stop'); runtime.uri = str(result.uri) || runtime.uri; },
    },
    getTranscriber: () => runtime.status.available ? { async prepare({ signal }) {
      throwIfVoiceTranscriptionAborted(signal);
      const prepared = await request('prepare'); throwIfVoiceTranscriptionAborted(signal);
      const locale = str(prepared.locale);
      return { locale, async transcribe(uri, { signal }) {
        throwIfVoiceTranscriptionAborted(signal);
        const result = await request('transcribe', { uri, locale }); throwIfVoiceTranscriptionAborted(signal);
        return str(result.transcript);
      } };
    } } : null,
    async requestPermission() { const result = await request('permission'); return { granted: result.granted === true, canAskAgain: result.canAskAgain === true }; },
    async configureRecording() { await request('configure'); },
    async releaseRecording() { await request('release'); },
    deleteRecording(uri) { enqueue(request('delete', { uri })); },
    onStateChange(state) {
      // A status invalidation projects an in-flight phase without retaining the answer.
      if (runtime.scope) enqueue(request('publish', { phase: state.phase }));
    },
  });
  states.set(client, runtime); return runtime;
}
async function invoke(runtime: Runtime, action: string, fields: object = {}) {
  const scope = runtime.scope; if (!scope) throw new Error('Voice operation has no active answer.');
  const reply = await bridgeReply(scope.native, { op: 'mobileVoice', action, session: runtime.id, generation: runtime.client.generation, ...fields }).catch(error => {
    scope.failed ??= error;
    if (letGo(error)) {
      const cleanup = runtime.session.controller.cancel();
      if (cleanup) scope.pending.push(cleanup.catch(failure => { scope.failed ??= failure; }));
    }
    throw error;
  });
  if (!reply.ok) {
    const error = reply.error!;
    if (error.kind === 'superseded') throw new ClientError(error.message, error.kind);
    throw new VoiceTranscriptionError(error.kind === 'unsupported-locale' ? 'unsupported-locale' : 'transcription-failed', error.message);
  }
  return obj(reply.value);
}
async function drain(scope: Scope) { for (let index = 0; index < scope.pending.length; index++) await scope.pending[index]; }
function readTarget(runtime: Runtime): VoiceDraftSnapshot | null {
  const target = runtime.target; if (!target) return null;
  // Navigation preserves a captured target; an ended edit never falls back to an ordinary draft.
  const text = mobileComposerTargetText(runtime.client, target.content);
  if (text === null) return null;
  const revision = target.content.kind === 'queued-edit' ? mobileComposerTargetRevision(runtime.client, target.content) : target.revision;
  if (revision === null) return null;
  return { ownerKey: target.key, text, revision, selection: { start: 0, end: 0 } };
}
/** Call synchronously after every shared draft mutation, including clear/send, before another mutation can revert it. */
export function mobileVoiceObserveDraft(client: T3Client = mobileClient) {
  const runtime = states.get(client); if (!runtime) return;
  const target = runtime.target;
  if (target) {
    const next = mobileComposerTargetText(client, target.content), revision = mobileComposerTargetRevision(client, target.content);
    if (next !== target.text || revision !== target.contentRevision) {
      target.text = next; target.contentRevision = revision; target.revision++; target.observer?.();
    }
  }
  // Picker commits also own a caret, even if dictation has never started or now targets another draft.
  const selection = runtime.selection;
  if (selection.owner && selection.content && (mobileComposerTargetText(client, selection.content) !== selection.text
    || mobileComposerTargetRevision(client, selection.content) !== selection.contentRevision)) {
    runtime.selection = { ...selection, owner: '', revision: selection.revision + 1 };
  }
}
/** One caller answer stages native caret state using the same revision projected by the root hook.
 * Persistence belongs to the producer. No promise, Native or Files handle survives this call. */
export async function mobileComposerStageSelection(client: T3Client, target: MobileComposerTarget, text: string,
  start: number, end: number, native: Native): Promise<void> {
  const runtime = stateFor(client); mobileVoiceObserveDraft(client);
  const contentRevision = mobileComposerTargetRevision(client, target);
  if (mobileComposerTargetText(client, target) !== text || contentRevision === null || !Number.isFinite(start) || !Number.isFinite(end))
    throw new ClientError('The draft changed before its selection could be restored.', 'superseded');
  const lower = Math.max(0, Math.min(text.length, Math.floor(start))), upper = Math.max(lower, Math.min(text.length, Math.floor(end)));
  const revision = runtime.selection.revision + 1;
  runtime.selection = { owner: target.editorOwner, start: lower, end: upper, revision, content: target, text, contentRevision };
  try {
    const reply = await bridgeReply(letGoAware(native), { op: 'mobileVoice', action: 'selection-commit', generation: target.generation,
      owner: target.editorOwner, text, start: lower, end: upper, revision });
    mobileVoiceObserveDraft(client);
    if (runtime.selection.revision !== revision || runtime.selection.owner !== target.editorOwner)
      throw new ClientError('The draft selection changed.', 'superseded');
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  } catch (error) {
    if (runtime.selection.revision === revision) runtime.selection = { ...runtime.selection, owner: '', revision: revision + 1 };
    throw error;
  }
}
export function mobileVoiceSnapshot(focusedOwner = '', client: T3Client = mobileClient, waveformWidth = 220): VoiceSnapshot {
  const runtime = stateFor(client); mobileVoiceObserveDraft(client);
  const state = runtime.session.controller.currentState;
  const busy = voiceInputBlocksSubmission(state), focused = !!focusedOwner && focusedOwner === runtime.session.ownerKey && !!readTarget(runtime);
  const elapsed = runtime.status.elapsed, presentation = resolveVoiceComposerPresentation(state, elapsed);
  const bars = Array.from({ length: 64 }, (_, id) => { const level = normalizeVoiceInputDecibels(runtime.status.levels[id]);
    return { id, height: 2 + level * 30, compactHeight: 2 + level * 12, opacity: .22 + level * .78 }; });
  const slotEligible = !activeInput(client) && pendingRequests(client.projection).approvals.length === 0;
  return { available: runtime.status.available && slotEligible && (!busy || focused), owner: runtime.session.ownerKey ?? '', label: runtime.session.label ?? 'Draft',
    phase: state.phase, error: state.error ?? '', errorAction: state.errorAction ?? '', busy, focused,
    globalVisible: !!presentation.statusLabel && !focused, elapsed, elapsedLabel: `${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, '0')}`,
    statusLabel: presentation.statusLabel ?? '', confirmationEnabled: presentation.confirmationEnabled, bars: bars.slice(-Math.max(1, Math.min(64, Math.floor(waveformWidth / 5)))), compactBars: bars.slice(-8),
    selectionOwner: runtime.selection.owner, selectionStart: runtime.selection.start, selectionEnd: runtime.selection.end, selectionRevision: runtime.selection.revision };
}
/** Status reads normally only project. Background during in-flight preparation invalidates that active controller scope immediately; other events use root reconciliation. */
export async function mobileVoiceStatus(native: Native, client: T3Client = mobileClient): Promise<VoiceStatus> {
  native.watch('t3.mobile-voice');
  const reply = await bridgeReply(native, { op: 'mobileVoice', action: 'status', generation: client.generation });
  const value = obj(reply.value), runtime = stateFor(client);
  runtime.status = { available: reply.ok && value.available === true, locale: str(value.locale), reason: str(value.reason), session: str(value.session),
    event: Number(value.event) || 0, eventKind: str(value.eventKind), error: str(value.error), uri: str(value.uri), elapsed: Number(value.elapsed) || 0,
    levels: Array.isArray(value.levels) ? value.levels.map(Number) : [], phase: str(value.phase, 'idle') };
  mobileVoiceObserveDraft(client);
  if (runtime.scope && runtime.status.session === runtime.id && runtime.status.event > runtime.handledEvent && runtime.status.eventKind === 'background' && runtime.session.controller.currentState.phase === 'preparing') {
    runtime.handledEvent = runtime.status.event; runtime.session.controller.appMovedToBackground();
  }
  return runtime.status;
}
async function reconcileNativeEvent(runtime: Runtime) {
  const status = runtime.status;
  if (status.session !== runtime.id || status.event <= runtime.handledEvent) return;
  runtime.handledEvent = status.event;
  if (status.eventKind === 'background') await runtime.session.controller.appMovedToBackground();
  else if (status.eventKind === 'finished' || status.eventKind === 'interrupted') await runtime.session.controller.handleRecorderStatus({
    isFinished: status.eventKind === 'finished', hasError: status.eventKind === 'interrupted', error: status.error || null, url: status.uri || null });
}
/** One answer owns every native request; only the recorder/analyzer survives natively between answers. */
export async function mobileVoiceAction(op: string, start: number, end: number, label: string, focusedOwner: string,
  nativeInput: Native, storage: Files, client: T3Client = mobileClient): Promise<VoiceResult> {
  const runtime = stateFor(client), native = letGoAware(nativeInput);
  const result = (message = '') => ({ revision: client.revision, message, data: mobileVoiceSnapshot(focusedOwner, client) });
  if (runtime.scope) {
    // Cancel is the only concurrent action. The original answer keeps its own scope until cancellation settles.
    if (op === 'cancel') {
      runtime.session.controller.cancel();
      await bridgeReply(native, { op: 'mobileVoice', action: 'cancel', session: runtime.id, generation: client.generation });
      return result();
    }
    return result('Voice input is still finishing.');
  }
  const scope: Scope = { native, storage, pending: [], failed: null }; runtime.scope = scope;
  try {
    mobileVoiceObserveDraft(client);
    if (op === 'start') {
      if (voiceInputBlocksSubmission(runtime.session.controller.currentState)) return result();
      if (activeInput(client) || pendingRequests(client.projection).approvals.length) return result('Voice input is unavailable for this request answer.');
      if (client.busy || client.pending) return result('Wait for the current submission to finish before starting dictation.');
      if (!client.projectId || !client.environmentId) return result('This draft is no longer available.');
      const content = mobileComposerTargetRequire(client);
      if (focusedOwner !== content.editorOwner) return result('This draft is no longer available.');
      await mobileVoiceStatus(native, client);
      if (!mobileComposerTargetCurrent(client, content)) return result('This draft is no longer available.');
      if (!runtime.status.available) return result(runtime.status.reason || 'Voice transcription is not available.');
      const key = content.editorOwner, text = mobileComposerTargetText(client, content), contentRevision = mobileComposerTargetRevision(client, content);
      if (text === null) return result('This draft is no longer available.');
      if (start < 0 || end < 0) {
        const selected = await invoke(runtime, 'selection', { owner: key, text });
        if (!mobileComposerTargetCurrent(client, content) || mobileComposerTargetText(client, content) !== text || mobileComposerTargetRevision(client, content) !== contentRevision) return result('The draft changed before voice input could start.');
        start = Number(selected.start); end = Number(selected.end);
        if (!Number.isFinite(start) || !Number.isFinite(end)) return result('The text selection is unavailable.');
      }
      runtime.selection = { ...runtime.selection, owner: '', revision: runtime.selection.revision + 1 };
      runtime.id = `voice-${++runtime.serial}`; runtime.uri = null;
      const target: Target = { key, content, text, revision: 0, contentRevision };
      runtime.target = target;
      const selection = { start: Math.max(0, Math.min(text.length, start)), end: Math.max(0, Math.min(text.length, end)) };
      const sourceTarget = createVoiceInputTarget(key, () => readTarget(runtime)?.text ?? null, (value, selected) => {
        // The copied controller checks text/revision and owner immediately before this synchronous write.
        if (!readTarget(runtime) || !mobileComposerTargetWriteText(client, content, value)) throw new Error('This draft is no longer available.');
        const commitScope = runtime.scope; if (!commitScope) throw new Error('Voice commit has no active answer.');
        commitScope.pending.push(mobileComposerStageSelection(client, content, value, selected.start, selected.end, commitScope.native).catch(error => { commitScope.failed ??= error; }));
        commitScope.pending.push(mobileComposerTargetPersist(client, content, commitScope.native, commitScope.storage).catch(error => { commitScope.failed ??= error; }));
      }, selection, changed => { target.observer = changed; return () => { target.observer = undefined; }; });
      // Queued content has its own monotonic revision. Read it directly so an edit/revert
      // between voice observations cannot evade the copied controller's comparison.
      await runtime.session.start({ ...sourceTarget, label, ...(content.kind === 'queued-edit' ? { readDraft: () => {
        const current = readTarget(runtime); return current ? { ...current, selection } : null;
      } } : {}) });
    } else if (op === 'stop') await runtime.session.controller.stop();
    else if (op === 'cancel') {
      await runtime.session.controller.cancel();
      await invoke(runtime, 'cancel');
    } else if (op === 'retry') {
      runtime.id = `voice-${++runtime.serial}`; runtime.uri = null;
      await runtime.session.retry();
    } else if (op === 'settings') {
      await runtime.session.controller.cancel(); await invoke(runtime, 'settings');
    } else if (op === 'reconcile') {
      await reconcileNativeEvent(runtime);
    } else return result('Unknown voice action.');
    await drain(scope);
    await reconcileNativeEvent(runtime);
    await drain(scope);
    if (scope.failed) {
      await runtime.session.controller.interruptRecording('Could not start voice recording.'); await drain(scope);
      throw scope.failed;
    }
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    return result(error instanceof Error ? error.message : 'Voice input failed.');
  } finally {
    // All fire-and-drain recorder/cleanup work settles within the originating answer.
    await drain(scope); if (runtime.scope === scope) runtime.scope = null;
  }
}
