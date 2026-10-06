// Composer selection, follow-up dispatch and primary-action state (lane
// composer-controls), adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// apps/web/src/composerDraftStore.ts (sticky model memory, applyStickyState),
// components/ChatView.tsx (onProviderModelSelect, persistThreadSettingsForNextTurn,
// onSend, onResume), packages/client-runtime/src/state/composerDispatch.ts,
// packages/client-runtime/src/operations/commands.ts (startThreadTurn) and
// components/chat/ComposerPrimaryActions.tsx.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, activeRun, providerAvailable } from './protocol';
import type { T3Client } from './client';

/** Next-turn choices made in a started thread while a turn runs (the reference keeps them in the draft). */
export type StagedSelection = { providerId: string; modelId: string; options: Obj[]; runtimeMode: string; interactionMode: string };
export type ComposerControlsPrefs = {
  stickyProvider: string;
  stickyByProvider: Record<string, { model: string; options: Obj[] }>;
  stickyOptionsByModel: Record<string, Record<string, Obj[]>>;
  staged: Record<string, StagedSelection>;
  /** `${environment}:${thread}` → the woke timestamp the user acknowledged (markThreadVisited). */
  wokeSeen: Record<string, string>;
  /** Draft key → its workspace mode and branch (composer-controls-branch.ts). */
  contexts: Record<string, { envMode: string; branch: string; worktreePath: string }>;
  /** r7-handoff: draft key → the thread id the draft launches as (a hand-off's setup terminal belongs to it). */
  draftThreads?: Record<string, string>;
};
export function emptyComposerControls(): ComposerControlsPrefs {
  return { stickyProvider: '', stickyByProvider: {}, stickyOptionsByModel: {}, staged: {}, wokeSeen: {}, contexts: {} };
}
const options = (value: unknown): Obj[] => arr(value).filter(option => typeof option.id === 'string'
  && (typeof option.value === 'string' || typeof option.value === 'boolean')).map(option => ({ id: option.id, value: option.value })).slice(0, 32);
export function decodeComposerControls(saved: unknown): ComposerControlsPrefs {
  const value = obj(saved), next = emptyComposerControls();
  next.stickyProvider = str(value.stickyProvider);
  for (const [id, entry] of Object.entries(obj(value.stickyByProvider)).slice(0, 64)) {
    const model = str(obj(entry).model);
    if (model) next.stickyByProvider[id] = { model, options: options(obj(entry).options) };
  }
  for (const [id, models] of Object.entries(obj(value.stickyOptionsByModel)).slice(0, 64)) {
    const remembered: Record<string, Obj[]> = {};
    for (const [model, entry] of Object.entries(obj(models)).slice(0, 128)) remembered[model] = options(entry);
    next.stickyOptionsByModel[id] = remembered;
  }
  for (const [key, entry] of Object.entries(obj(value.staged)).slice(0, 200)) {
    const staged = obj(entry);
    if (str(staged.providerId) && str(staged.modelId)) next.staged[key] = { providerId: str(staged.providerId), modelId: str(staged.modelId),
      options: options(staged.options), runtimeMode: str(staged.runtimeMode, 'approval-required'), interactionMode: str(staged.interactionMode, 'default') };
  }
  for (const [key, at] of Object.entries(obj(value.wokeSeen)).slice(-500)) if (typeof at === 'string') next.wokeSeen[key] = at;
  for (const [key, entry] of Object.entries(obj(value.contexts)).slice(-200)) {
    const context = obj(entry);
    if (context.envMode === 'local' || context.envMode === 'worktree') next.contexts[key] = { envMode: context.envMode, branch: str(context.branch).slice(0, 256), worktreePath: str(context.worktreePath).slice(0, 4096) };
  }
  for (const [key, id] of Object.entries(obj(value.draftThreads)).slice(-200)) if (typeof id === 'string' && /^[\w-]{1,128}$/.test(id)) (next.draftThreads ??= {})[key] = id; // r7-handoff
  return next;
}
// Data sources have no Date.now(): the snapshot's wall time (wallTime + elapsed) is the composer's clock.
const clocks = new WeakMap<object, number>();
export function noteNow(client: object, now: number): void { if (Number.isFinite(now) && now > 0) clocks.set(client, Math.max(clocks.get(client) ?? 0, now)); }
/** The newest wall time a snapshot saw for this client (0 before the first). */
export function composerNow(client: object): number { return clocks.get(client) ?? 0; }
function prefs(client: T3Client): ComposerControlsPrefs {
  const local = client.local as { composerControls?: ComposerControlsPrefs };
  return local.composerControls ??= emptyComposerControls();
}

// ── Sticky model memory ────────────────────────────────────────────────────

/** setStickyModelSelection: a picked model becomes the next draft's model; its own remembered traits come back with it. */
export function rememberModel(client: T3Client, providerId: string, modelId: string): Obj[] {
  const state = prefs(client);
  const remembered = state.stickyOptionsByModel[providerId]?.[modelId] ?? [];
  state.stickyByProvider[providerId] = { model: modelId, options: remembered };
  state.stickyProvider = providerId;
  return remembered.map(option => ({ ...option }));
}
/** setProviderModelOptions(..., persistSticky): traits stick to the sticky model and are remembered for this model. */
export function rememberOptions(client: T3Client, providerId: string, modelId: string, chosen: Obj[]): void {
  const state = prefs(client);
  const base = state.stickyByProvider[providerId]?.model || modelId;
  state.stickyByProvider[providerId] = { model: base, options: chosen.map(option => ({ ...option })) };
  state.stickyOptionsByModel[providerId] = { ...state.stickyOptionsByModel[providerId], [modelId]: chosen.map(option => ({ ...option })) };
  state.stickyProvider = providerId;
}
/** applyStickyState: a new draft opens on the last picked provider/model, ahead of the configured default. */
export function applySticky(client: T3Client): void {
  const state = prefs(client), id = state.stickyProvider, entry = state.stickyByProvider[id];
  if (!id || !entry) return;
  const provider = arr(client.config.providers).find(candidate => candidate.instanceId === id);
  if (!provider || !providerAvailable(provider)) return;
  const models = arr(provider.models);
  const model = models.find(candidate => candidate.slug === entry.model);
  if (model) { client.providerId = id; client.modelId = entry.model; client.modelOptions = entry.options.map(option => ({ ...option })); return; }
  const fallback = models.find(candidate => candidate.isDefault === true && candidate.isCustom !== true) ?? models.find(candidate => candidate.isCustom !== true) ?? models[0];
  if (fallback) { client.providerId = id; client.modelId = str(fallback.slug); client.modelOptions = []; }
}

// ── Next-turn staging in started threads ───────────────────────────────────

/** A started thread stages its next-turn choices while a turn runs, or once a staged choice exists. */
export function stagesChanges(client: T3Client): boolean {
  return !!client.threadId && (!!activeRun(client.projection) || !!prefs(client).staged[client.draftKey]);
}
export function stage(client: T3Client, change: Partial<StagedSelection>): void {
  const state = prefs(client);
  const current = state.staged[client.draftKey] ?? { providerId: client.providerId, modelId: client.modelId,
    options: client.modelOptions.map(option => ({ ...option })), runtimeMode: client.runtimeMode, interactionMode: client.interactionMode };
  const next = { ...current, ...change };
  const thread = obj(client.projection.thread), selection = obj(thread.modelSelection);
  const same = next.providerId === str(selection.instanceId) && next.modelId === str(selection.model)
    && JSON.stringify(next.options) === JSON.stringify(arr(selection.options)) && next.runtimeMode === str(thread.runtimeMode, 'approval-required')
    && next.interactionMode === str(thread.interactionMode, 'default');
  if (same) delete state.staged[client.draftKey]; else state.staged[client.draftKey] = next;
  client.providerId = next.providerId; client.modelId = next.modelId; client.modelOptions = next.options.map(option => ({ ...option }));
  client.runtimeMode = next.runtimeMode; client.interactionMode = next.interactionMode;
}
/** After the thread's own selection is read, a staged choice still owns the composer. */
export function applyStaged(client: T3Client): void {
  const staged = prefs(client).staged[client.draftKey];
  if (!staged) return;
  client.providerId = staged.providerId; client.modelId = staged.modelId; client.modelOptions = staged.options.map(option => ({ ...option }));
  client.runtimeMode = staged.runtimeMode; client.interactionMode = staged.interactionMode;
}
export function stagedFor(client: T3Client, key = client.draftKey): StagedSelection | undefined { return prefs(client).staged[key]; }
export function clearStaged(client: T3Client, key: string): void { delete prefs(client).staged[key]; }
/** persistThreadSettingsForNextTurn: runtime and interaction mode commands that precede the next message. */
export function nextTurnCommands(thread: Obj, staged: StagedSelection | undefined, interactionOverride = ''): Obj[] {
  const commands: Obj[] = [];
  const runtimeMode = staged?.runtimeMode ?? str(thread.runtimeMode, 'approval-required');
  const interactionMode = interactionOverride || (staged?.interactionMode ?? str(thread.interactionMode, 'default'));
  if (runtimeMode !== str(thread.runtimeMode, 'approval-required')) commands.push({ type: 'thread.runtime-mode.set', runtimeMode });
  if (interactionMode !== str(thread.interactionMode, 'default')) commands.push({ type: 'thread.interaction-mode.set', interactionMode });
  return commands;
}

// ── Follow-up dispatch ─────────────────────────────────────────────────────

export type DispatchMode = 'auto' | 'queue' | 'steer';
/** resolveComposerDispatchMode: the alternate swaps queue and steer relative to the configured behavior. */
export function resolveDispatchMode(running: boolean, followUp: string, alternate: boolean): DispatchMode {
  if (!running) return 'auto';
  const preferred: DispatchMode = followUp === 'steer' ? 'steer' : 'queue';
  if (alternate) return preferred === 'queue' ? 'steer' : 'queue';
  return preferred;
}
export function followUpBehavior(client: T3Client): string {
  return str((client.local as { clientSettings?: Obj }).clientSettings?.followUpBehavior, 'queue') === 'steer' ? 'steer' : 'queue';
}
/** startThreadTurn with server-resolved command context: queue waits behind the active run, everything else names its intent. */
export function withDispatchMode(payload: Obj, mode: DispatchMode, selection?: Obj): Obj {
  const next: Obj = { ...payload, ...(selection ? { modelSelection: selection } : {}) };
  if (mode === 'queue') { delete next.deliveryIntent; next.dispatchMode = { type: 'queue_after_active' }; }
  else { next.deliveryIntent = mode; next.dispatchMode = { type: 'start_immediately' }; }
  return next;
}

/**
 * The keyboard or pointer gesture that pressed Send (T3ComposerIntent.swift):
 * ⌘/Ctrl alone is the alternate (composer.sendAlternate while a turn runs, or
 * a ⌘-click), ⌥⌘ in a draft starts it in the background (composer.sendBackground).
 */
export function submissionIntent(gesture: Obj, running: boolean, draft: boolean): 'foreground' | 'alternate' | 'background' {
  const modifiers = str(gesture.modifiers), age = Number(gesture.ageMs);
  if (!Number.isFinite(age) || age < 0 || age > 2000) return 'foreground';
  const meta = /\b(meta|control)\b/.test(modifiers), alt = /\balt\b/.test(modifiers), shift = /\bshift\b/.test(modifiers);
  if (meta && alt && !shift && draft && gesture.source === 'key') return 'background';
  if (meta && !alt && !shift && running) return 'alternate';
  return 'foreground';
}

// ── Plan follow-up ─────────────────────────────────────────────────────────

export const PLAN_IMPLEMENTATION_PROMPT_PREFIX = 'PLEASE IMPLEMENT THIS PLAN:\n';
/** proposedPlanTitle: the first Markdown heading, else null. */
export function proposedPlanTitle(markdown: string): string {
  const heading = markdown.split('\n').map(line => line.trim()).find(line => /^#{1,6}\s+\S/.test(line));
  return heading ? heading.replace(/^#{1,6}\s+/, '').trim() : '';
}
/** findLatestProposedPlan + hasActionableProposedPlan: the latest run's plan artifact, when still active. */
export function latestProposedPlan(projection: Obj): { id: string; markdown: string; runId: string } | null {
  const plans = arr(projection.plans).filter(plan => plan.kind === 'proposed_plan');
  const latestRunId = str(latestExecutedRun(projection)?.id);
  const candidates = latestRunId && plans.some(plan => plan.runId === latestRunId) ? plans.filter(plan => plan.runId === latestRunId) : plans;
  const time = (id: unknown) => str(arr(projection.turnItems).filter(item => (item.type === 'proposed_plan' || item.type === 'todo_list') && item.planId === id).pop()?.updatedAt, str(projection.updatedAt));
  const plan = [...candidates].sort((a, b) => time(a.id).localeCompare(time(b.id)) || str(a.id).localeCompare(str(b.id))).pop();
  return plan && plan.status === 'active' ? { id: str(plan.id), markdown: str(plan.markdown), runId: str(plan.runId) } : null;
}
export function planFollowUp(client: T3Client): { title: string; markdown: string; planId: string } | null {
  if (!client.threadId || client.interactionMode !== 'plan') return null;
  const projection = client.projection;
  if (activeRun(projection)) return null;
  if (arr(projection.runtimeRequests).some(request => request.status === 'pending')) return null;
  if (client.snapshotDrafts.length) return null;
  const plan = latestProposedPlan(projection);
  return plan ? { title: proposedPlanTitle(plan.markdown), markdown: plan.markdown, planId: plan.id } : null;
}
export function resolvePlanSubmission(draft: string, markdown: string): { text: string; interactionMode: 'default' | 'plan' } {
  const text = draft.trim();
  return text ? { text, interactionMode: 'plan' } : { text: `${PLAN_IMPLEMENTATION_PROMPT_PREFIX}${markdown.trim()}`, interactionMode: 'default' };
}

// ── Resume ─────────────────────────────────────────────────────────────────

/**
 * runRanAfter: the run that ended last ran last (an unfinished run is the
 * latest); equal end times fall back to the ordinal. A queued run resumed
 * after a restart continuation can have a lower ordinal than one that ended.
 */
export function runRanAfter(run: Obj, other: Obj): boolean {
  const end = (candidate: Obj) => { const at = str(candidate.completedAt); if (!at) return Number.POSITIVE_INFINITY; const ms = Date.parse(at); return Number.isFinite(ms) ? ms : Number.POSITIVE_INFINITY; };
  return end(run) === end(other) ? Number(run.ordinal) > Number(other.ordinal) : end(run) > end(other);
}
/** latestExecutedRun: the started run that ran last. */
export function latestExecutedRun(projection: Obj): Obj | undefined {
  let latest: Obj | undefined;
  for (const run of arr(projection.runs)) {
    if (run.status === 'queued' || run.status === 'cancelled' && !run.startedAt) continue;
    if (!latest || runRanAfter(run, latest)) latest = run;
  }
  return latest;
}
/** resumableRunId / hasHeldQueuedRuns. */
export function resumeState(projection: Obj): { runId: string; heldQueue: boolean } {
  const run = latestExecutedRun(projection);
  const heldQueue = arr(projection.runs).some(entry => entry.status === 'queued' && entry.queueHeld === true);
  const thread = obj(projection.thread);
  const usageLimit = run?.status === 'failed' && (thread.lastErrorClass === 'usage_limit'
    || arr(projection.turnItems).some(item => item.type === 'error' && item.runId === run.id && obj(item.failure).class === 'usage_limit'));
  return { runId: run && (run.status === 'interrupted' || usageLimit) ? str(run.id) : '', heldQueue };
}

/** Composer gating for a request (the native client refuses rather than guessing). */
export function requireProvider(client: T3Client, providerId = client.providerId, modelId = client.modelId): Obj {
  const provider = arr(client.config.providers).find(entry => entry.instanceId === providerId);
  if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
  if (!arr(provider.models).some(model => model.slug === modelId)) throw new ClientError('Choose one of the models advertised by T3.');
  return provider;
}
