// The previewAutomation host's executor (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/components/preview/PreviewAutomationHosts.tsx `handleRequest`, previewNavigationReadiness.ts,
// previewAutomationErrors.ts, previewClickFocus.ts; apps/desktop/src/preview/Manager.ts `automationStatus` …
// `automationWaitFor`, `withControlSession`, `performAutomationClick`, `performAutomationPress`).
//
// browser-automation.ts plans each request from the thread's preview store (browser-automation-plan.ts) and queues it
// here as one piece of slot work (browser-automation-recording.ts `queueSlotWork`): it runs to its end in one of the
// window's queued sends, waiting on the tab's frame (browser-frames.ts), and answers `previewAutomation.respond` on the
// connection the request came from. The page is driven with `@exact/browser-automation` (LLP 1020.002 D9) in place of
// CDP: scripts run with `callFunction` in the host's sandbox (browser-automation-scripts.ts), `preview_evaluate` in the
// page's world; a click is `performActions`' pointer input, a key its key input (trusted on macOS); the screenshot is
// `captureScreenshot`; locators and the ARIA snapshot come from Playwright's injected script. The data module has no
// clock: a request's deadline counts the run's sleeps and the snapshot's wall time, whichever ran further.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { obj, str, type Json, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';
import { sleep as timer } from '../client/command-waits';
// @ts-ignore: the bake types the package from its exact.json (`__exact_modules`); a bare tsc sees no module.
import * as packageCalls from '@exact/browser-automation';
import { composerNow } from '../composer/composer-controls';
import { activeRef } from '../terminal/terminal-drawer-view';
import { adoptedFiles } from '../media/save-file';
import { toBase64 } from '../shared/base64';
import { shownBrowserTab } from '../panel/panel-surfaces';
import { captureNativeCall, floatingTabOf } from './browser-capture';
import { browserHost } from './browser-surface';
import { previewRuntimeTabId, type PreviewViewportSetting } from './browser-state';
import { frameByElement, frameEntry, frameNav, frames, navigateFrame, setFrame, type FrameEntry } from './browser-frames';
import { makePreviewAutomationKeySequence, type PressInput } from './browser-automation-keys';
import { nativeDeps, runRecordingRequest, type RecordingRequest } from './browser-automation-recording';
import type { BrowserController, HostErrorJson, PreviewAutomationOperation } from './browser-automation-plan';
import * as scripts from './browser-automation-scripts';

// ── The package's calls (injectable: a test hands its own) ───────────────────────────────────────
type Id = { id: string };
export type AutomationCalls = {
  evaluate(options: Id & { expression: string; sandbox?: string; awaitPromise?: boolean; userActivation?: boolean }): Promise<{ result?: unknown }>;
  callFunction(options: Id & { functionDeclaration: string; arguments?: unknown[]; sandbox?: string; awaitPromise?: boolean; userActivation?: boolean }): Promise<{ result?: unknown }>;
  addPreloadScript(options: Id & { functionDeclaration: string; sandbox?: string }): Promise<{ script?: string }>;
  performActions(options: Id & { actions: unknown[] }): Promise<unknown>;
  handleUserPrompt(options: Id & { accept?: boolean; userText?: string }): Promise<unknown>;
  setDownloadBehavior(options: Id & { destinationFolder?: string }): Promise<unknown>;
  captureScreenshot(options: Id & { path: string; clip?: { x: number; y: number; width: number; height: number } }): Promise<{ path?: string; width?: number; height?: number }>;
};
let calls = packageCalls as unknown as AutomationCalls;
export function useAutomationCalls(fake: AutomationCalls | null): void { calls = fake ?? packageCalls as unknown as AutomationCalls; }
/** The calls in use (Annotate and Capture make theirs through them too). */
export const automationCalls = (): AutomationCalls => calls;
/** A call's error code (`{code, message}`, LLP 1133.002 D1), else its name. */
const codeOf = (error: unknown): string => str(obj(error).code) || (error instanceof Error ? error.name : '');

// ── Per-tab state (the reference's PreviewManager tab facts) ─────────────────────────────────────
export const DIAGNOSTIC_LIMIT = 200;
const MAX_EVALUATION_BYTES = 64_000, MAX_SCREENSHOT_WIDTH = 1_280, AGENT_CURSOR_MOVE_MS = 160, AGENT_CURSOR_CLICK_LEAD_MS = 40;
/** Manager.ts READ_ONLY_CONTROL_ACTIONS: control actions that only read the page, so they do not make it agent-driven. */
const READ_ONLY_CONTROL_ACTIONS = new Set(['snapshot', 'waitFor']);
type Tab = {
  console: Obj[]; network: Obj[]; actions: Obj[];
  muted: boolean; audible: boolean;
  /** The agent acted on the page since it mounted: the person's input then hands it back (controller "human"). */
  agentEver: boolean;
  /** The frame's navigation ids seen, and the highest at which its scripts went in: a lower id is a new web view. */
  lastNavigation: number; installed: boolean; installing: boolean;
  downloads: boolean;
  /** The last poll of its media and diagnostics (the snapshot's wall time). */
  polled: number; polling: boolean;
  /** Agent requests running on the tab now: while one does, its page's dialogs are answered at once. */
  runs: number;
  /** The agent drove the tab since its thread's turn began; the turn's end lets it go (`agentDriving`). */
  agentTurn: boolean;
  /** Until when (the shell's clock) the tab stays held after the agent's last request or the end of its turn. */
  idleUntil: number;
};
type Host = { tabs: Map<string, Tab>; pointerSequence: number; actionSequence: number; shots: number };
const hosts = new WeakMap<object, Host>();
function host(client: object): Host {
  let value = hosts.get(client);
  if (!value) { value = { tabs: new Map(), pointerSequence: 0, actionSequence: 0, shots: 0 }; hosts.set(client, value); }
  return value;
}
function tabOf(client: object, runtimeId: string): Tab {
  const tabs = host(client).tabs;
  let tab = tabs.get(runtimeId);
  if (!tab) {
    tab = { console: [], network: [], actions: [], muted: false, audible: false, agentEver: false, lastNavigation: 0, installed: false, installing: false, downloads: false, polled: 0, polling: false, runs: 0, agentTurn: false, idleUntil: 0 };
    tabs.set(runtimeId, tab);
  }
  return tab;
}
const capped = (list: Obj[]) => { if (list.length > DIAGNOSTIC_LIMIT) list.splice(0, list.length - DIAGNOSTIC_LIMIT); };

/** The overlay fields the executor decides (usePreviewBridge's audio and controller): the frame's own `controller`, and
 *  "human" once a person took back a page the agent drove. */
export function automationControllerOf(client: object, runtimeId: string): BrowserController {
  const entry = frameEntry(client, runtimeId), tab = host(client).tabs.get(runtimeId);
  if (entry?.controller === 'agent') return 'agent';
  return tab?.agentEver ? 'human' : 'none';
}
export function automationAudio(client: object, runtimeId: string): { audioMuted: boolean; audible: boolean } | null {
  const tab = host(client).tabs.get(runtimeId);
  return tab ? { audioMuted: tab.muted, audible: tab.audible } : null;
}

/** DesktopBrowserHost's FOCUS_IDLE: how long a tab the agent used stays held after its last request or its turn. */
export const AGENT_FOCUS_IDLE_MS = 5_000;
/**
 * DesktopBrowserHost's focus hold (T3 Code 7c3b3b3605ef, apps/desktop/src/preview/DesktopBrowserHost.ts): a hidden tab
 * keeps running while an agent request runs on it, for FOCUS_IDLE after the last one, and while the agent that drove it
 * is still in its thread's turn, model thinking between steps included; FOCUS_IDLE after the turn ends it may throttle
 * again. The turn is read from the thread as the sidebar shows it (`turnRunning`): it ends when the root turn and its
 * subagents finish (RunExecutionService's agentTurnsSettled); commands left running in the background are no turn.
 */
export function agentDriving(client: object, runtimeId: string, turnRunning: (environmentId: string, threadId: string) => boolean,
  now = composerNow(client)): boolean {
  const tab = host(client).tabs.get(runtimeId);
  if (!tab) return false;
  if (tab.runs > 0) return true;
  if (tab.agentTurn) {
    let ref: unknown;
    try { ref = JSON.parse(runtimeId); } catch { ref = null; }
    const [environmentId, threadId] = Array.isArray(ref) ? ref.map(part => str(part)) : ['', ''];
    if (environmentId && threadId && turnRunning(environmentId, threadId)) return true;
    tab.agentTurn = false;
    tab.idleUntil = Math.max(tab.idleUntil, now + AGENT_FOCUS_IDLE_MS);
  }
  return now < tab.idleUntil;
}
/** Whether a hold ends on the clock (a request to finish, an idle wait to run out): the shell's clock must keep running.
 *  A turn's end needs no clock: it comes as a shell change, which `settleAgentHolds` reads. */
export function agentHoldWaiting(client: object, now: number): boolean {
  for (const tab of host(client).tabs.values()) if (tab.runs > 0 || now < tab.idleUntil) return true;
  return false;
}
/**
 * Each shell answer, before the clock is decided: a tab whose frame is gone (its tab, thread or environment went) is
 * forgotten, and a tab whose thread's turn ended starts its FOCUS_IDLE wait, shown or hidden alike.
 */
export function settleAgentHolds(client: object, now: number, live: (runtimeId: string) => boolean,
  turnRunning: (environmentId: string, threadId: string) => boolean): void {
  const tabs = host(client).tabs;
  for (const [runtimeId, tab] of tabs) {
    if (tab.runs > 0) continue;
    if (!live(runtimeId)) { tabs.delete(runtimeId); continue; }
    if (tab.agentTurn) agentDriving(client, runtimeId, turnRunning, now);
  }
}

// ── The page's scripts, media and diagnostics (before the agent needs them) ─────────────────────
/** Each document of a frame gets the diagnostics (page world) and media (sandbox) scripts with `callFunction` as it
 *  commits (the frame's `currententrychange` of a new navigation); both are idempotent. They go in as reads, not preload
 *  scripts: any other call makes the frame the agent's (LLP 1020.002 D9), and a person's page would then have its dialogs
 *  held and its downloads moved. So what a document logs before its commit reaches the app is not kept. Then, at most
 *  every two seconds of the snapshot's wall time, its media state and diagnostics are read: Mute is set again on a new
 *  document, and an audible change redraws the tab. Calls a let-go answer loses are made by the next one. */
export async function automationTick(client: T3Client): Promise<void> {
  const now = composerNow(client);
  for (const entry of frames(client).entries.values()) {
    if (!entry.record) continue;
    const tab = tabOf(client, entry.runtimeId), id = entry.frameId;
    if (entry.committedId < tab.lastNavigation) tab.downloads = false; // a new web view: its download place goes too
    if (entry.committedId && entry.committedId !== tab.lastNavigation) { tab.installed = false; tab.lastNavigation = entry.committedId; }
    if (!tab.installed && !tab.installing && entry.committedId) {
      tab.installing = true;
      try {
        await calls.callFunction({ id, functionDeclaration: scripts.DIAGNOSTICS });
        await calls.callFunction({ id, functionDeclaration: scripts.MEDIA, sandbox: scripts.SANDBOX });
        tab.installed = true;
      } catch (error) { if (letGo(error)) throw error; }
      finally { tab.installing = false; }
    }
    if (!tab.installed || tab.polling || (now > 0 && now - tab.polled < 2_000)) continue;
    tab.polling = true; tab.polled = now;
    try {
      await drainDiagnostics(client, entry);
      const state = obj((await calls.evaluate({ id, expression: scripts.MEDIA_STATE, sandbox: scripts.SANDBOX })).result);
      if (tab.muted && state.muted === false) await calls.evaluate({ id, expression: scripts.setMuted(true), sandbox: scripts.SANDBOX });
      const audible = state.audible === true;
      if (audible !== tab.audible) { tab.audible = audible; client.revision++; }
    } catch (error) { if (letGo(error)) throw error; }
    finally { tab.polling = false; }
  }
}
async function drainDiagnostics(client: object, entry: FrameEntry): Promise<void> {
  const tab = tabOf(client, entry.runtimeId), drained = (await calls.evaluate({ id: entry.frameId, expression: scripts.DRAIN_DIAGNOSTICS })).result;
  for (const raw of Array.isArray(drained) ? drained : []) {
    const item = obj(raw), list = item.kind === 'network' ? tab.network : item.kind === 'console' ? tab.console : null;
    if (list) { list.push(obj(item.entry)); capped(list); }
  }
}

/** RightPanelTabs `tabMuteMenuItem` (previewBridge.setAudioMuted): the page's media elements, muted or not. */
export async function toggleFrameMute(client: T3Client, _native: Native, runtimeId: string, muted: boolean): Promise<void> {
  const entry = frameEntry(client, runtimeId);
  if (!entry) return;
  const tab = tabOf(client, runtimeId);
  tab.muted = muted;
  client.revision++;
  try { await calls.evaluate({ id: entry.frameId, expression: scripts.setMuted(muted), sandbox: scripts.SANDBOX }); }
  catch (error) { if (letGo(error)) throw error; }
}

/** An agent-driven page's own `alert`, `confirm`, `prompt` or `beforeunload` (the frame's `userpromptopened`, which comes
 *  only while the frame is the agent's). While an agent request runs on the tab no dialog shows, so the host answers it at
 *  once, an alert and a leave-page prompt with OK, a confirm and a prompt with Cancel, and lists it among the page's
 *  console entries for the agent's next snapshot. Otherwise the agent is done with the page and the person answers it:
 *  the layer shows it over the page (browser-frames.contract BrowserFrame) as Chromium's tab-modal dialog words it. */
export function adoptFramePrompt(client: T3Client, frameId: string, packed: string): void {
  const entry = frameByElement(client, frameId);
  if (!entry) return;
  const q = new URLSearchParams(packed), type = q.get('type') ?? 'alert', message = q.get('message') ?? '';
  const tab = tabOf(client, entry.runtimeId);
  if (tab.runs === 0) {
    let host = ''; try { host = new URL(entry.record?.url ?? '').host; } catch { /* no page URL */ }
    entry.prompt = type === 'beforeunload'
      ? { type, title: 'Leave site?', message: 'Changes that you made may not be saved.', defaultValue: '' }
      : { type, title: `${host || 'This page'} says`, message, defaultValue: q.get('default') ?? '' };
    client.revision++;
    return;
  }
  const accept = type === 'alert' || type === 'beforeunload';
  tab.console.push({ level: 'info', text: `Dialog (${type}) answered ${accept ? 'OK' : 'Cancel'}: ${message}`, source: 'dialog', timestamp: new Date(composerNow(client) || 0).toISOString() });
  capped(tab.console);
  void calls.handleUserPrompt({ id: frameId, accept }).catch(() => undefined); // a call a let-go answer loses leaves the dialog held until a person's click shows its sheet
}
/** The person's answer to a held dialog (`accept`, and a prompt's `text`). */
export async function answerFramePrompt(client: T3Client, frameId: string, packed: string): Promise<void> {
  const entry = frameByElement(client, frameId), prompt = entry?.prompt;
  if (!entry || !prompt) return;
  const q = new URLSearchParams(packed), accept = q.get('accept') === 'true';
  entry.prompt = null;
  client.revision++;
  try { await calls.handleUserPrompt({ id: frameId, accept, ...(accept && prompt.type === 'prompt' ? { userText: q.get('text') ?? '' } : {}) }); }
  catch (error) { if (letGo(error)) throw error; }
}

// ── Errors (previewAutomationErrors.ts `serializePreviewAutomationHostError`) ────────────────────
type Failure =
  | { kind: 'planned'; json: HostErrorJson } | { kind: 'targetUnavailable' } | { kind: 'overlayTimeout'; ms: number }
  | { kind: 'navigationTimeout'; readiness: string; ms: number } | { kind: 'viewportTimeout'; ms: number }
  | { kind: 'notEditable'; selectorKind: string; length: number | null } | { kind: 'operation'; cause: string };
class HostError extends Error { constructor(readonly failure: Failure) { super(failure.kind); } }
const fail = (failure: Failure): never => { throw new HostError(failure); };
const operation = (cause: string): never => fail({ kind: 'operation', cause });

/** The public tag and message the server classifies: the message and the context fields as `detail`, never a cause. */
export function hostErrorJson(failure: Failure, context: Context): HostErrorJson {
  if (failure.kind === 'planned') return failure.json;
  const tab = context.tabId ?? 'unassigned', where = `environment ${context.environmentId} thread ${context.threadId}`;
  const base = { requestId: context.requestId, environmentId: context.environmentId, threadId: context.threadId };
  switch (failure.kind) {
    case 'targetUnavailable': return { _tag: 'PreviewAutomationTabNotFoundError', detail: { ...base, operation: context.operation, tabId: context.tabId, bridgeAvailable: true },
      message: `Preview automation target for ${context.operation} request ${context.requestId} is unavailable on ${where} (tab ${tab}, bridge available).` };
    case 'overlayTimeout': return { _tag: 'PreviewAutomationTimeoutError', detail: { ...base, timeoutMs: failure.ms },
      message: `Preview webview for request ${context.requestId} on ${where} did not register within ${failure.ms}ms.` };
    case 'navigationTimeout': return { _tag: 'PreviewAutomationTimeoutError', detail: { ...base, tabId: tab, readiness: failure.readiness, timeoutMs: failure.ms },
      message: `Preview navigation for request ${context.requestId} on ${where} tab ${tab} did not reach ${failure.readiness} readiness within ${failure.ms}ms.` };
    case 'viewportTimeout': return { _tag: 'PreviewAutomationTimeoutError', detail: { ...base, tabId: tab, timeoutMs: failure.ms },
      message: `Preview viewport for request ${context.requestId} on ${where} tab ${tab} was not rendered within ${failure.ms}ms.` };
    case 'notEditable': return { _tag: 'PreviewAutomationTargetNotEditableError',
      detail: { ...base, operation: context.operation, tabId: context.tabId, selectorKind: failure.selectorKind, ...(failure.length === null ? {} : { selectorLength: failure.length }) },
      message: `Preview automation ${context.operation} request ${context.requestId} requires an editable target in tab ${tab}.` };
    case 'operation': return { _tag: 'PreviewAutomationExecutionError', detail: { ...base, operation: context.operation, tabId: context.tabId },
      message: `Preview automation ${context.operation} request ${context.requestId} failed on ${where} (tab ${tab}).` };
  }
}

// ── A request's run ──────────────────────────────────────────────────────────────────────────────
export type Context = {
  plan: Obj; requestId: string; connectionId: string; clientId: string; operation: PreviewAutomationOperation; environmentId: string; threadId: string;
  tabId: string | null; runtimeId: string | null; input: Obj; timeoutMs: number; generation: number; budgetMs: number; fleet: string;
  /** The run's sleeps so far, and the snapshot's wall time when the run started. */
  slept: number; startedAt: number;
};
export function readContext(plan: Obj, now: number): Context {
  const timeoutMs = Number(plan.timeoutMs) || 15_000;
  return {
    plan, requestId: str(plan.requestId), connectionId: str(plan.connectionId), clientId: str(plan.clientId), operation: str(plan.operation) as PreviewAutomationOperation,
    environmentId: str(plan.environmentId), threadId: str(plan.threadId), tabId: str(plan.tabId) || null, runtimeId: str(plan.runtimeId) || null, input: obj(plan.input),
    timeoutMs, generation: Number(plan.generation) || 0, budgetMs: typeof plan.budgetMs === 'number' ? plan.budgetMs : timeoutMs, fleet: str(plan.fleet), slept: 0, startedAt: now,
  };
}
/** What the run needs from its slot: the send's native, an RPC on the request's connection, and the host's adoption of a
 *  tab it opened (into the thread's store, shown or suppressed, with a frame). */
export type RunDeps = {
  native: Native;
  rpc: (method: string, payload: Obj) => Promise<Obj>;
  /** The connection's server origin (a recording's upload URL is under it). */
  origin: string;
  adopt: (note: { snapshot: Obj; present: boolean; suppress: boolean }) => void;
};
type Run = { client: T3Client; context: Context; deps: RunDeps };
const elapsed = (run: Run): number => Math.max(run.context.slept, composerNow(run.client) > 0 && run.context.startedAt > 0 ? composerNow(run.client) - run.context.startedAt : 0);
const remaining = (run: Run): number => run.context.budgetMs - elapsed(run);
async function sleep(run: Run, ms: number): Promise<void> {
  const wait = Math.max(0, Math.round(ms));
  run.context.slept += wait;
  await timer(wait);
}
const iso = (run: Run): string => new Date((run.context.startedAt || 0) + elapsed(run)).toISOString();

/** One request, run to its end and answered. */
export async function runAutomation(client: T3Client, plan: Obj, deps: RunDeps): Promise<{ ok: boolean; result?: unknown; error?: HostErrorJson }> {
  const run: Run = { client, context: readContext(plan, composerNow(client)), deps };
  const tab = run.context.runtimeId ? tabOf(client, run.context.runtimeId) : null;
  let outcome: { ok: boolean; result?: unknown; error?: HostErrorJson };
  if (tab) tab.runs++;
  // A recording streams the page itself, which holds it drawing (heldFrame); any other request drives it.
  const driving = run.context.operation !== 'recordingStart' && run.context.operation !== 'recordingStop';
  if (tab && driving) tab.agentTurn = true;
  try { outcome = await execute(run); }
  catch (error) {
    if (letGo(error)) throw error;
    outcome = { ok: false, error: hostErrorJson(error instanceof HostError ? error.failure : { kind: 'operation', cause: String(error) }, run.context) };
  }
  finally {
    if (tab) tab.runs--;
    if (tab && driving) tab.idleUntil = Math.max(tab.idleUntil, composerNow(client) + AGENT_FOCUS_IDLE_MS);
  }
  const payload: Obj = { clientId: run.context.clientId, connectionId: run.context.connectionId, requestId: run.context.requestId, ok: outcome.ok };
  if (outcome.ok && outcome.result !== undefined) payload.result = outcome.result as Json;
  if (!outcome.ok) payload.error = outcome.error as unknown as Json;
  try { await deps.rpc('previewAutomation.respond', payload); } catch (error) { if (letGo(error)) throw error; }
  return outcome;
}

async function execute(run: Run): Promise<{ ok: boolean; result?: unknown; error?: HostErrorJson }> {
  const { context } = run;
  if (context.plan.failure) return { ok: false, error: context.plan.failure as HostErrorJson };
  switch (context.operation) {
    case 'status': return { ok: true, result: await status(run, context.tabId, context.runtimeId) };
    case 'open': return { ok: true, result: await open(run) };
    case 'navigate': {
      const entry = await requireReady(run), url = str(obj(context.plan.navigate).url);
      if (!url) operation('no navigation URL');
      await navigateAndWait(run, entry, url, str(context.input.readiness) || 'load', Number(context.input.timeoutMs) || context.timeoutMs);
      return { ok: true, result: await status(run, context.tabId, context.runtimeId) };
    }
    case 'resize': return { ok: true, result: await resize(run) };
    case 'setColorScheme': {
      const entry = await requireReady(run), scheme = str(context.input.colorScheme) || 'system';
      setFrame(run.client, entry.runtimeId, { colorScheme: scheme });
      run.client.revision++;
      return { ok: true, result: { tabId: context.tabId ?? '', colorScheme: scheme } };
    }
    case 'snapshot': return { ok: true, result: await controlled(run, 'snapshot', entry => snapshot(run, entry)) };
    case 'click': await controlled(run, 'click', entry => click(run, entry)); return { ok: true, result: {} };
    case 'type': await controlled(run, 'type', entry => type(run, entry)); return { ok: true, result: {} };
    case 'press': await controlled(run, 'press', entry => press(run, entry)); return { ok: true, result: {} };
    case 'scroll': await controlled(run, 'scroll', entry => scroll(run, entry)); return { ok: true, result: {} };
    case 'evaluate': { const value = await controlled(run, 'evaluate', entry => evaluate(run, entry)); return value === undefined ? { ok: true } : { ok: true, result: value }; }
    case 'waitFor': await controlled(run, 'waitFor', entry => waitFor(run, entry)); return { ok: true, result: {} };
    case 'recordingStart': case 'recordingStop': return recording(run);
    default: return operation(`unsupported operation ${context.operation}`);
  }
}

// ── Readiness (requireReadyTab, waitForDesktopOverlay, waitForNavigationReadiness) ──────────────
/** previewAutomationHostBudget `waitForHostReadiness`: probes and polling delays share the deadline. */
async function waitUntil(run: Run, ms: number, ready: () => Promise<boolean> | boolean): Promise<boolean> {
  const end = elapsed(run) + ms;
  while (elapsed(run) < end) {
    if (await ready()) return true;
    const wait = Math.min(50, end - elapsed(run));
    if (wait <= 0) break;
    await sleep(run, wait);
  }
  return false;
}
/** The tab's frame, once the layer has mounted it (a sandbox probe answers), within the request's host deadline. */
async function requireReady(run: Run, tabId = run.context.tabId, runtimeId = run.context.runtimeId): Promise<FrameEntry> {
  if (!runtimeId || !tabId) return fail({ kind: 'targetUnavailable' });
  const budget = Math.max(0, Math.round(remaining(run)));
  const ready = await waitUntil(run, budget, async () => {
    const entry = frameEntry(run.client, runtimeId);
    if (!entry) return false;
    try { await calls.evaluate({ id: entry.frameId, expression: '1', sandbox: scripts.SANDBOX }); return true; }
    catch (error) { if (letGo(error)) throw error; return false; }
  });
  const entry = frameEntry(run.client, runtimeId);
  if (!ready || !entry) return fail({ kind: 'overlayTimeout', ms: budget });
  return entry;
}
/** `assertPreviewRuntimeCurrent`: the frame is still the tab's. */
function current(run: Run, entry: FrameEntry): void {
  if (frameEntry(run.client, entry.runtimeId) !== entry) fail({ kind: 'targetUnavailable' });
}
async function navigateAndWait(run: Run, entry: FrameEntry, url: string, readiness: string, timeoutMs: number): Promise<void> {
  // The frame reports the navigation it starts (a new navigation id); until then its record still shows the page before.
  const before = entry.record?.navigationId ?? 0;
  if (!navigateFrame(run.client, entry.runtimeId, url)) operation('the URL is not a web page');
  run.client.revision++;
  await waitUntil(run, Math.min(timeoutMs, 5_000), () => (entry.record?.navigationId ?? 0) > before || entry.failure !== null || frameEntry(run.client, entry.runtimeId) !== entry);
  current(run, entry);
  if (readiness === 'none') return;
  const reached = await waitUntil(run, timeoutMs, async () => {
    current(run, entry);
    if (readiness !== 'domContentLoaded') return frameNav(entry).kind !== 'Loading';
    try { const state = (await calls.evaluate({ id: entry.frameId, expression: scripts.READY_STATE, sandbox: scripts.SANDBOX })).result; return state === 'interactive' || state === 'complete'; }
    catch (error) { if (letGo(error)) throw error; return false; }
  });
  if (!reached) fail({ kind: 'navigationTimeout', readiness, ms: timeoutMs });
}

// ── Status and open (currentStatus, the "open" case) ─────────────────────────────────────────────
/** The page shows: the panel's stage or the floating player holds the tab of the open thread. */
export function frameVisible(client: T3Client, runtimeId: string): boolean {
  const ref = activeRef(client);
  if (!ref) return false;
  const epoch = browserHost(client).store.read(ref).serverEpoch;
  return [shownBrowserTab(client), floatingTabOf(client)].some(tabId => !!tabId && previewRuntimeTabId(ref, epoch, tabId) === runtimeId);
}
async function measure(entry: FrameEntry): Promise<{ width: number; height: number } | null> {
  try {
    const value = obj((await calls.evaluate({ id: entry.frameId, expression: scripts.VIEWPORT, sandbox: scripts.SANDBOX })).result);
    const width = Math.round(Number(value.width)), height = Math.round(Number(value.height));
    return width > 0 && height > 0 ? { width, height } : null;
  } catch (error) { if (letGo(error)) throw error; return null; }
}
async function status(run: Run, tabId: string | null, runtimeId: string | null): Promise<Obj> {
  const value: Obj = { tabId: tabId ?? null };
  if (tabId && run.context.plan.viewportSetting) value.viewportSetting = run.context.plan.viewportSetting;
  const entry = runtimeId ? frameEntry(run.client, runtimeId) : null;
  if (entry) {
    const visible = frameVisible(run.client, entry.runtimeId), nav = frameNav(entry);
    Object.assign(value, { available: true, visible, url: nav.url || entry.record?.url || null, title: nav.title || entry.record?.title || null, loading: nav.kind === 'Loading' });
    if (visible) { const viewport = await measure(entry); if (viewport) value.viewport = viewport; }
    return value;
  }
  const fallback = obj(run.context.plan.fallback);
  return { ...value, available: false, visible: false, url: fallback.url ?? null, title: fallback.title ?? null, loading: fallback.loading === true };
}
const sizeOf = (setting: unknown): { width: number; height: number } | null => {
  const value = obj(setting), width = Number(value.width), height = Number(value.height);
  return value._tag !== 'fill' && width > 0 && height > 0 ? { width, height } : null;
};
/** isPreviewViewportReady: Fill is whatever the panel gives; a fixed size matches within a pixel. */
export const viewportMatches = (setting: unknown, viewport: { width: number; height: number }): boolean => {
  const size = sizeOf(setting);
  return !size || (Math.abs(viewport.width - size.width) <= 1 && Math.abs(viewport.height - size.height) <= 1);
};

async function open(run: Run): Promise<Obj> {
  const { context } = run, plan = obj(context.plan.open), ref = { environmentId: context.environmentId, threadId: context.threadId };
  let tabId = str(plan.tabId) || null, runtimeId = str(plan.runtimeId) || null;
  const reused = tabId !== null;
  let needsOverlay = plan.needsOverlay === true, applied: unknown = null, adopted: Obj | null = null;
  if (!reused) {
    const create = obj(plan.create);
    if (!plan.create) operation('no tab to reuse and nothing to create');
    let snapshot: Obj;
    try { snapshot = await run.deps.rpc('preview.open', create); } catch (error) { if (letGo(error)) throw error; return operation(`preview.open failed: ${String(error)}`); }
    const created = str(snapshot.tabId);
    if (!created) operation('preview.open answered no tab');
    tabId = created;
    needsOverlay = create.url !== undefined || str(obj(snapshot.navStatus)._tag, 'Idle') !== 'Idle'; // previewAutomationOpenNeedsOverlay
    // previewAutomationDefaultViewport: a new tab on Fill takes the agent default (1280×800) before its frame mounts.
    adopted = snapshot;
    let failure: string | null = null;
    if (plan.defaultViewport && str(obj(snapshot.viewport)._tag, 'fill') === 'fill') {
      try { adopted = await run.deps.rpc('preview.resize', { threadId: context.threadId, tabId: created, viewport: plan.defaultViewport as Json }); applied = plan.defaultViewport; }
      catch (error) { if (letGo(error)) throw error; failure = `preview.resize failed: ${String(error)}`; }
    }
    run.deps.adopt({ snapshot: adopted ?? snapshot, present: plan.present === true, suppress: plan.suppress === true }); // `open: false` keeps it out of view
    runtimeId = previewRuntimeTabId(ref, browserHost(run.client).store.read(ref).serverEpoch, created);
    if (failure) operation(failure);
  }
  if (needsOverlay) await requireReady(run, tabId, runtimeId);
  if (plan.present === true && runtimeId) {
    // waitForPreviewPresentation: settle briefly so an active-thread open reports visible=true, then let a fixed size render.
    await waitUntil(run, Math.min(500, remaining(run)), () => frameVisible(run.client, runtimeId!));
    const fixed = applied ?? (reused ? context.plan.viewportSetting : null), entry = frameEntry(run.client, runtimeId);
    if (fixed && sizeOf(fixed) && entry) await waitUntil(run, Math.min(2_000, remaining(run)), async () => !frameVisible(run.client, runtimeId!) || viewportMatches(fixed, await measure(entry) ?? { width: 0, height: 0 }));
  }
  if (reused && str(plan.url) && runtimeId) {
    const entry = await requireReady(run, tabId, runtimeId);
    await navigateAndWait(run, entry, str(plan.url), 'load', context.timeoutMs);
  }
  const value = await status(run, tabId, runtimeId);
  if (adopted) value.viewportSetting = adopted.viewport ?? { _tag: 'fill' }; // the new tab's, as currentStatus reads the store
  return value;
}

// ── Resize (the "resize" case; a fixed viewport is the device toolbar's) ───────────────────────
async function resize(run: Run): Promise<Obj> {
  const { context } = run, entry = await requireReady(run), setting = obj(context.plan.viewport) as Obj;
  if (!setting._tag) operation('no viewport setting');
  if (setting._tag !== 'fill' && !sizeOf(setting)) operation(`a ${str(setting._tag)} viewport without its size`);
  const ref = { environmentId: context.environmentId, threadId: context.threadId };
  const apply = async (viewport: Json) => {
    const answer = await run.deps.rpc('preview.resize', { threadId: context.threadId, tabId: context.tabId ?? '', viewport });
    if (str(answer.tabId)) browserHost(run.client).store.updateServerSnapshot(ref, answer as never);
    run.client.revision++;
  };
  try { await apply(setting as Json); } catch (error) { if (letGo(error)) throw error; operation('preview.resize failed'); }
  const timeoutMs = Number(context.input.timeoutMs) || context.timeoutMs;
  let measured: { width: number; height: number } | null = null;
  // A tab no stage or player shows is laid out by nothing: its size is the setting's once the server took it.
  await waitUntil(run, timeoutMs, async () => {
    current(run, entry);
    if (!frameVisible(run.client, entry.runtimeId)) { measured = sizeOf(setting) ?? await measure(entry); return measured !== null || setting._tag === 'fill'; }
    const viewport = await measure(entry);
    if (viewport && viewportMatches(setting, viewport)) { measured = viewport; return true; }
    return false;
  });
  if (!measured && setting._tag !== 'fill') {
    // shouldRollbackPreviewViewport: back to the tab's previous size when it differs.
    const previous = context.plan.viewportSetting as PreviewViewportSetting | undefined;
    if (previous && JSON.stringify(previous) !== JSON.stringify(setting)) { try { await apply(previous as unknown as Json); } catch (error) { if (letGo(error)) throw error; } }
    fail({ kind: 'viewportTimeout', ms: timeoutMs });
  }
  return { tabId: context.tabId ?? '', setting, viewport: measured ?? { width: 0, height: 0 } };
}

// ── The control session (withControlSession): the action timeline, downloads ─────────────────────
async function controlled<A>(run: Run, action: string, body: (entry: FrameEntry) => Promise<A>): Promise<A> {
  const entry = await requireReady(run), state = host(run.client), tab = tabOf(run.client, entry.runtimeId);
  const record: Obj = { id: `action-${++state.actionSequence}`, action, status: 'running', startedAt: iso(run) };
  tab.actions.push(record); capped(tab.actions);
  if (!READ_ONLY_CONTROL_ACTIONS.has(action)) {
    tab.agentEver = true;
    // Manager.ts withControlSession: an action that changes the page makes it agent-driven, so its downloads go to the
    // agent's folder without a dialog (installDownloadHandler); reading it (snapshot, wait for) does not.
    if (!tab.downloads) {
      try { await calls.setDownloadBehavior({ id: entry.frameId, destinationFolder: AGENT_DOWNLOADS }); tab.downloads = true; }
      catch (error) { if (letGo(error)) throw error; }
    }
    run.client.revision++;
  }
  try {
    const value = await body(entry);
    Object.assign(record, { status: 'succeeded', completedAt: iso(run) });
    return value;
  } catch (error) {
    Object.assign(record, { status: 'failed', completedAt: iso(run), error: error instanceof HostError ? hostErrorJson(error.failure, run.context)._tag : 'PreviewAutomationExecutionError' });
    throw error;
  } finally { run.client.revision++; }
}
/** Where an agent-driven tab's downloads go (app.ts grants). */
export const AGENT_DOWNLOADS = 'app:/tmp/browser-downloads';

// ── Page scripts ─────────────────────────────────────────────────────────────────────────────────
/** A function body in the sandbox (or the page's world), its value as JSON. A value whose JSON is longer than a call
 *  carries (64 KB) is left in the sandbox and read in slices. */
async function script(entry: FrameEntry, body: string, world: 'sandbox' | 'page' = 'sandbox'): Promise<unknown> {
  const sandbox = world === 'sandbox' ? { sandbox: scripts.SANDBOX } : {};
  const declaration = `async () => {\n const __t3Value = await (async () => {${body}\n})();\n const text = JSON.stringify(__t3Value === undefined ? null : __t3Value);\n if (text.length <= 16000) return { text };\n globalThis.__t3Out = text;\n return { slices: Math.ceil(text.length / 8000) };\n}`;
  let answer: Obj;
  try { answer = obj((await calls.callFunction({ id: entry.frameId, functionDeclaration: declaration, awaitPromise: true, ...sandbox })).result); }
  catch (error) { if (letGo(error)) throw error; return operation(`script: ${codeOf(error)} ${String(obj(error).message ?? error)}`); }
  let text = str(answer.text);
  for (let index = 0; index < (Number(answer.slices) || 0); index++) {
    const part = (await calls.evaluate({ id: entry.frameId, expression: `globalThis.__t3Out.slice(${index * 8000}, ${(index + 1) * 8000})`, ...sandbox })).result;
    text += str(part);
  }
  return text ? JSON.parse(text) : null;
}
/** ensurePlaywrightInjected: the injected script in the sandbox, once per document. */
async function ensurePlaywright(entry: FrameEntry): Promise<void> {
  try {
    if ((await calls.callFunction({ id: entry.frameId, functionDeclaration: scripts.PLAYWRIGHT_INSTALLED, sandbox: scripts.SANDBOX })).result === true) return;
    await calls.callFunction({ id: entry.frameId, functionDeclaration: scripts.PLAYWRIGHT_INSTALL, sandbox: scripts.SANDBOX });
  } catch (error) { if (letGo(error)) throw error; operation(`Playwright install: ${codeOf(error)}`); }
}
const locatorOf = (input: Obj): string | null => typeof input.locator === 'string' ? input.locator : typeof input.selector === 'string' ? `css=${input.selector}` : null;
const selectorDiagnostics = (input: Obj): { selectorKind: string; length: number | null } =>
  typeof input.locator === 'string' ? { selectorKind: 'locator', length: input.locator.length } : typeof input.selector === 'string' ? { selectorKind: 'selector', length: input.selector.length } : { selectorKind: 'focused-element', length: null };
function located(result: unknown): Obj {
  const value = obj(result);
  if (value.invalidSelector) operation('invalid selector');
  if (value.notFound) operation('target not found');
  return value;
}

// ── Snapshot ─────────────────────────────────────────────────────────────────────────────────────
async function snapshot(run: Run, entry: FrameEntry): Promise<Obj> {
  try { await ensurePlaywright(entry); } catch (error) { if (!(error instanceof HostError)) throw error; } // the ARIA snapshot only
  const page = obj(await script(entry, scripts.SNAPSHOT));
  try { await drainDiagnostics(run.client, entry); } catch (error) { if (letGo(error)) throw error; }
  const shot = await screenshot(run, entry), tab = tabOf(run.client, entry.runtimeId);
  return {
    url: str(page.url), title: str(page.title), loading: page.loading === true, visibleText: str(page.visibleText),
    interactiveElements: Array.isArray(page.interactiveElements) ? page.interactiveElements : [], accessibilityTree: page.accessibilityTree ?? null,
    consoleEntries: [...tab.console], networkEntries: [...tab.network], actionTimeline: [...tab.actions], screenshot: shot,
  };
}
type ScaleFiles = { fs: { readFile(path: string): Promise<ArrayBuffer>; compressImage?(from: string, to: string, options: { maxDimension: number; maxBytes: number; type: 'image/png' }): Promise<{ path?: string }> } };
/** The page as a PNG at most 1,280 pixels wide (`MAX_SCREENSHOT_WIDTH`): `captureScreenshot` into app:/tmp/browser-shots,
 *  scaled by the storage's `compressImage` when it is wider. */
async function screenshot(run: Run, entry: FrameEntry): Promise<Obj> {
  const name = `agent-${entry.frameId}-${++host(run.client).shots % 8}`, path = `app:/tmp/browser-shots/${name}.png`;
  let shot: { path?: string; width?: number; height?: number };
  try { shot = await calls.captureScreenshot({ id: entry.frameId, path }); }
  catch (error) { if (letGo(error)) throw error; return operation(`captureScreenshot: ${codeOf(error)}`); }
  const files = adoptedFiles() as unknown as ScaleFiles | null;
  if (!files) return operation('no app files');
  const width = Number(shot.width) || 0, height = Number(shot.height) || 0;
  let from = shot.path || path;
  if (width > MAX_SCREENSHOT_WIDTH && files.fs.compressImage) {
    const longest = Math.round(Math.max(MAX_SCREENSHOT_WIDTH, MAX_SCREENSHOT_WIDTH * height / Math.max(1, width)));
    const scaled = await files.fs.compressImage(from, `app:/tmp/browser-shots/${name}-scaled.png`, { maxDimension: longest, maxBytes: 32 * 1024 * 1024, type: 'image/png' });
    from = scaled.path || `app:/tmp/browser-shots/${name}-scaled.png`;
  }
  const bytes = new Uint8Array(await files.fs.readFile(from));
  const ihdr = bytes.length >= 24 ? new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) : null; // PNG IHDR: width and height at 16 and 20
  return { mimeType: 'image/png', data: toBase64(bytes), width: ihdr ? ihdr.getUint32(16) : width, height: ihdr ? ihdr.getUint32(20) : height };
}

// ── Click (performAutomationClick) ───────────────────────────────────────────────────────────────
async function cursor(run: Run, entry: FrameEntry, phase: string, x: number, y: number): Promise<void> {
  const sequence = ++host(run.client).pointerSequence;
  try { await captureNativeCall(run.client, { op: 'browserPage', frame: entry.frameId, phase, x, y, sequence, controller: automationControllerOf(run.client, entry.runtimeId) }); }
  catch (error) { if (letGo(error)) throw error; }
}
async function click(run: Run, entry: FrameEntry): Promise<void> {
  const input = run.context.input, locator = locatorOf(input);
  let x: number, y: number;
  if (locator !== null) {
    await ensurePlaywright(entry);
    const point = located(await script(entry, scripts.clickPoint(locator)));
    x = Number(point.x) || 0; y = Number(point.y) || 0;
  } else { x = Number(input.x) || 0; y = Number(input.y) || 0; }
  const viewport = await measure(entry);
  if (!viewport) return operation('no viewport');
  if (x < 0 || y < 0 || x > viewport.width || y > viewport.height) operation('coordinates outside the viewport');
  await cursor(run, entry, 'move', x, y);
  await sleep(run, AGENT_CURSOR_MOVE_MS);
  await cursor(run, entry, 'click', x, y);
  await sleep(run, AGENT_CURSOR_CLICK_LEAD_MS);
  current(run, entry);
  const px = Math.min(x, viewport.width - 0.5), py = Math.min(y, viewport.height - 0.5);
  try {
    await calls.performActions({ id: entry.frameId, actions: [{ type: 'pointer', id: 't3-agent-mouse', parameters: { pointerType: 'mouse' }, actions: [
      { type: 'pointerMove', x: px, y: py, origin: 'viewport' }, { type: 'pointerDown', button: 0 }, { type: 'pointerUp', button: 0 }] }] });
  } catch (error) {
    if (letGo(error) || codeOf(error) === 'NotFoundError' || codeOf(error) === 'NotAllowedError') throw error;
    if (obj(await script(entry, scripts.syntheticClick(x, y), 'page')).missed) operation('nothing at the point');
  }
}

// ── Type (typeIntoAutomationTarget: editing in the page, as the reference does for hidden guests too) ──
async function type(run: Run, entry: FrameEntry): Promise<void> {
  const input = run.context.input, locator = locatorOf(input);
  if (locator !== null) await ensurePlaywright(entry);
  const result = located(await script(entry, scripts.typeInto(locator, str(input.text), input.clear === true)));
  if (result.notEditable) fail({ kind: 'notEditable', ...selectorDiagnostics(input) });
}

// ── Press (performAutomationPress) ───────────────────────────────────────────────────────────────
/** WebDriver's key values for the named keys the arm sends natively (BiDi `input.performActions`). */
const WEBDRIVER_KEYS: Record<string, string> = {
  Backspace: '', Tab: '', Enter: '', Escape: '', Space: ' ', ' ': ' ', PageUp: '', PageDown: '', End: '', Home: '',
  ArrowLeft: '', ArrowUp: '', ArrowRight: '', ArrowDown: '', Delete: '',
  F1: '', F2: '', F3: '', F4: '', F5: '', F6: '', F7: '', F8: '', F9: '', F10: '', F11: '', F12: '',
};
const MODIFIER_KEYS: Record<string, string> = { Shift: '', Control: '', Alt: '', Meta: '' };
/** A key and its modifiers as one key source's actions, or null for a key the arm has no native value for. */
export function keyActions(input: PressInput): unknown[] | null {
  const named = WEBDRIVER_KEYS[input.key];
  const value = named ?? (Array.from(input.key).length === 1 ? makePreviewAutomationKeySequence(input, { isMac: true }).keyDown.key : null);
  if (!value) return null;
  const modifiers = (input.modifiers ?? []).map(modifier => MODIFIER_KEYS[modifier]).filter((key): key is string => !!key);
  return [{ type: 'key', id: 't3-agent-keyboard', actions: [
    ...modifiers.map(key => ({ type: 'keyDown', value: key })), { type: 'keyDown', value }, { type: 'keyUp', value }, ...modifiers.reverse().map(key => ({ type: 'keyUp', value: key })),
  ] }];
}
async function press(run: Run, entry: FrameEntry): Promise<void> {
  const key = obj(run.context.plan.key), input = run.context.input as unknown as PressInput, commands = Array.isArray(key.commands) ? key.commands.map(String) : [];
  // macOS editing shortcuts edit the focused element in the page without native focus; a paste is the platform's own
  // key, which reads the system clipboard (the data module reads no clipboard). Each acts (`userActivation`), so the frame
  // is the agent's again after a person took it back, as Manager.ts's control session makes it.
  if (str(key.editing) && !commands.includes('paste')) {
    try { await calls.evaluate({ id: entry.frameId, expression: str(key.editing), userActivation: true }); }
    catch (error) { if (letGo(error)) throw error; operation(`editing: ${codeOf(error)}`); }
    return;
  }
  const actions = keyActions(input);
  if (actions) {
    try { await calls.performActions({ id: entry.frameId, actions }); return; }
    catch (error) { if (letGo(error) || codeOf(error) === 'NotFoundError' || codeOf(error) === 'NotAllowedError') throw error; }
  }
  if (str(key.dom)) await script(entry, str(key.dom), 'page');
}

// ── Scroll, evaluate, wait for ───────────────────────────────────────────────────────────────────
async function scroll(run: Run, entry: FrameEntry): Promise<void> {
  const input = run.context.input, locator = locatorOf(input);
  if (locator !== null) await ensurePlaywright(entry);
  located(await script(entry, scripts.scrollBy(locator, Number(input.deltaX) || 0, Number(input.deltaY) || 0)));
}
const utf8Length = (text: string): number => { let bytes = 0; for (const c of text) { const code = c.codePointAt(0)!; bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4; } return bytes; };
async function evaluate(run: Run, entry: FrameEntry): Promise<unknown> {
  const expression = str(run.context.input.expression), awaitPromise = run.context.input.awaitPromise !== false;
  let text: unknown;
  try { text = (await calls.callFunction({ id: entry.frameId, functionDeclaration: scripts.evaluateExpression(expression, awaitPromise), awaitPromise: true, userActivation: true })).result; }
  catch (error) {
    if (letGo(error)) throw error;
    // A script that is no expression is a SyntaxError in an expression position: run it as a script.
    if (codeOf(error) !== 'SyntaxError') return operation(`evaluation: ${codeOf(error)}`);
    try { text = (await calls.callFunction({ id: entry.frameId, functionDeclaration: scripts.evaluateScript(expression), userActivation: true })).result; }
    catch (again) { if (letGo(again)) throw again; return operation(`evaluation: ${codeOf(again)}`); }
  }
  if (typeof text !== 'string' || text === scripts.UNDEFINED_MARK) return undefined;
  if (utf8Length(text) > MAX_EVALUATION_BYTES) operation(`result larger than ${MAX_EVALUATION_BYTES} bytes`);
  try { return JSON.parse(text); } catch { return undefined; }
}
async function waitFor(run: Run, entry: FrameEntry): Promise<void> {
  const input = run.context.input, locator = locatorOf(input), timeoutMs = Number(input.timeoutMs) || 15_000;
  const body = scripts.waitFor(locator, typeof input.text === 'string' ? input.text : null, typeof input.urlIncludes === 'string' ? input.urlIncludes : null);
  const end = elapsed(run) + timeoutMs;
  while (elapsed(run) <= end) {
    current(run, entry);
    if (locator !== null) { try { await ensurePlaywright(entry); } catch (error) { if (!(error instanceof HostError)) throw error; } } // a navigation replaced the document
    let result: Obj = {};
    try { result = obj(await script(entry, body)); } catch (error) { if (!(error instanceof HostError)) throw error; }
    if (result.invalidSelector) operation('invalid selector');
    if (result.matched === true) return;
    await sleep(run, 100);
  }
  operation(`wait timed out after ${timeoutMs}ms`);
}

// ── Recording (the recordingStart and recordingStop cases) ───────────────────────────────────────
async function recording(run: Run): Promise<{ ok: boolean; result?: unknown; error?: HostErrorJson }> {
  const { context } = run;
  // requireReadyTab, then startBrowserRecording; a stop resolves the recording to stop itself.
  const runtimeId = context.operation === 'recordingStart' ? (await requireReady(run)).runtimeId : null;
  const request: RecordingRequest = {
    key: `${context.fleet}\u0000${context.connectionId}\u0000${context.requestId}`, requestId: context.requestId, connectionId: context.connectionId,
    operation: context.operation as 'recordingStart' | 'recordingStop', environmentId: context.environmentId, threadId: context.threadId, tabId: context.tabId,
    tabIdExplicit: context.plan.tabIdExplicit === true, runtimeId, input: context.input, generation: context.generation, fleet: context.fleet,
    deadlineMs: (context.startedAt || 0) + context.budgetMs,
  };
  const answer = await runRecordingRequest(request, nativeDeps(run.client, { rpc: run.deps.rpc, origin: run.deps.origin, remaining: () => remaining(run) }));
  return answer.ok ? { ok: true, result: answer.result } : { ok: false, error: answer.error };
}

