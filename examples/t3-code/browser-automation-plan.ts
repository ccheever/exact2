// browser-surface part 5: the previewAutomation host's decisions, without the client (MIT reference, see
// LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/previewAutomationHostBudget.ts,
// previewAutomationOpenReadiness.ts, previewAutomationTarget.ts, previewAutomationRequestConsumer.ts,
// previewAutomationErrors.ts, previewAutomationClientId.ts, agentBrowserCursorLogic.ts, the request handler of
// PreviewAutomationHosts.tsx; apps/web/src/browser/browserTargetResolver.ts; packages/contracts/src/previewAutomation.ts).
//
// `planRequest` is the reference's `handleRequest` up to the point where it waits on the page: it resolves the
// target tab from the thread's preview store, decides reuse, presentation and readiness, and names everything the
// module needs. The module (T3BrowserAutomation.swift) does the waiting and answers `previewAutomation.respond`.
import type { PreviewSessionSnapshot, PreviewViewportSetting, ThreadPreviewState } from './browser-state';
import { previewRuntimeTabId } from './browser-state';
import { normalizePreviewUrl } from './browser-url';
import { isLocalLoopbackHost, isPrivateNetworkHost } from './host-classification';
import { pressPlan, type PressInput } from './browser-automation-keys';
import type { ScopedThreadRef } from './terminal-ui-state';

// ── contracts previewAutomation.ts ───────────────────────────────────────────────────────────────
export const PREVIEW_AUTOMATION_V1_OPERATIONS = ['status', 'open', 'navigate', 'snapshot', 'click', 'type', 'press', 'scroll', 'evaluate', 'waitFor', 'recordingStart', 'recordingStop'] as const;
/** Advertised by current desktop hosts for mixed-version routing; this host serves every one of them. */
export const PREVIEW_AUTOMATION_OPERATIONS = [...PREVIEW_AUTOMATION_V1_OPERATIONS, 'resize', 'setColorScheme'] as const;
export type PreviewAutomationOperation = (typeof PREVIEW_AUTOMATION_OPERATIONS)[number];
export type PreviewAutomationRequest = { requestId: string; threadId: string; tabId?: string; tabIdExplicit?: boolean; operation: PreviewAutomationOperation; input: Record<string, unknown>; timeoutMs: number };
export type PreviewAutomationStreamEvent = { type: 'connected'; connectionId: string } | { type: 'request'; connectionId: string; request: PreviewAutomationRequest };
export type HostErrorJson = { _tag: string; message: string; detail?: Record<string, unknown> };

const text = (value: unknown): string => (typeof value === 'string' ? value : '');
/** A stream value read defensively (PreviewAutomationStreamEvent); anything malformed is null. */
export function readStreamEvent(value: unknown): PreviewAutomationStreamEvent | null {
  if (!value || typeof value !== 'object') return null;
  const raw = value as Record<string, unknown>, connectionId = text(raw.connectionId).trim();
  if (!connectionId) return null;
  if (raw.type === 'connected') return { type: 'connected', connectionId };
  if (raw.type !== 'request' || !raw.request || typeof raw.request !== 'object') return null;
  const request = raw.request as Record<string, unknown>;
  const operation = text(request.operation) as PreviewAutomationOperation;
  const timeoutMs = Number(request.timeoutMs);
  if (!text(request.requestId).trim() || !text(request.threadId).trim() || !PREVIEW_AUTOMATION_OPERATIONS.includes(operation) || !Number.isInteger(timeoutMs) || timeoutMs <= 0) return null;
  return { type: 'request', connectionId, request: {
    requestId: text(request.requestId), threadId: text(request.threadId), ...(text(request.tabId) ? { tabId: text(request.tabId) } : {}),
    ...(typeof request.tabIdExplicit === 'boolean' ? { tabIdExplicit: request.tabIdExplicit } : {}), operation,
    input: request.input && typeof request.input === 'object' ? request.input as Record<string, unknown> : {}, timeoutMs,
  } };
}

// ── previewAutomationHostBudget.ts ───────────────────────────────────────────────────────────────
/** Allow time to deliver an overlay failure before the broker times out. */
export const PREVIEW_HOST_RESPONSE_MARGIN_MS = 1_500;
const HOST_RESPONSE_MARGIN_FRACTION = 0.2;
export function resolveHostWaitBudgetMs(requestTimeoutMs: number): number {
  if (!Number.isFinite(requestTimeoutMs) || requestTimeoutMs <= 0) return 0;
  const reservedMs = Math.min(PREVIEW_HOST_RESPONSE_MARGIN_MS, Math.ceil(requestTimeoutMs * HOST_RESPONSE_MARGIN_FRACTION));
  return Math.max(0, requestTimeoutMs - reservedMs);
}
export type Timers = { now: () => number; setTimeout: (fn: () => void, ms: number) => unknown; clearTimeout: (handle: unknown) => void };
/** Both readiness probes and polling delays share the request's host deadline. A data source has no clock and no
 *  timers (LLP 1027.000), so the module runs this rule (T3BrowserAutomation.waitForHostReadiness, from the budget it
 *  is handed); this port, over an injected clock, is the rule's tested statement. */
export async function waitForHostReadiness(deadlineMs: number, isReady: () => Promise<boolean>, timers: Timers): Promise<boolean> {
  while (timers.now() < deadlineMs) {
    let timeout: unknown;
    let ready: boolean | null;
    try {
      ready = await Promise.race([isReady(), new Promise<null>(resolve => { timeout = timers.setTimeout(() => resolve(null), Math.max(0, deadlineMs - timers.now())); })]);
    } finally { timers.clearTimeout(timeout); }
    if (ready) return true;
    if (ready === null || timers.now() >= deadlineMs) return false;
    await new Promise<void>(resolve => { timers.setTimeout(resolve, Math.min(50, deadlineMs - timers.now())); });
  }
  return false;
}

// ── previewAutomationOpenReadiness.ts ────────────────────────────────────────────────────────────
export type OpenInput = { tabId?: string; url?: string; open?: boolean; show?: boolean; reuseExistingTab?: boolean };
export const FILL_PREVIEW_VIEWPORT: PreviewViewportSetting = { _tag: 'fill' };
/** The viewport an agent-opened tab falls back to without a configured default (agents screenshot what they open). */
export const DEFAULT_PREVIEW_AUTOMATION_VIEWPORT = { _tag: 'freeform', width: 1280, height: 800 } as const satisfies PreviewViewportSetting;
/** An explicit `open`/`show` outranks the preference; the setting decides only when the agent said nothing. */
export const shouldOpenPreviewMiniPlayer = (input: OpenInput, autoShowFloatingPreview = true): boolean => input.open ?? input.show ?? autoShowFloatingPreview;
export const shouldAutoShowPreviewForAutomationUse = (input: { operation: PreviewAutomationOperation; autoShowFloatingPreview: boolean; presentationSuppressed: boolean }): boolean =>
  input.operation !== 'open' && input.autoShowFloatingPreview && !input.presentationSuppressed;
export const explicitlySuppressesPreviewMiniPlayer = (input: OpenInput): boolean => (input.open ?? input.show) === false;
export const previewAutomationOpenNeedsOverlay = (input: OpenInput, snapshot: Pick<PreviewSessionSnapshot, 'navStatus'>): boolean => input.url !== undefined || snapshot.navStatus._tag !== 'Idle';
/** Whether a freshly opened automation tab still needs a viewport applied: fill means no stated preference. */
export function previewAutomationDefaultViewport(reusedExistingTab: boolean, snapshot: Pick<PreviewSessionSnapshot, 'viewport'>): PreviewViewportSetting | null {
  const viewport = snapshot.viewport ?? FILL_PREVIEW_VIEWPORT;
  return !reusedExistingTab && viewport._tag === 'fill' ? DEFAULT_PREVIEW_AUTOMATION_VIEWPORT : null;
}

// ── previewAutomationTarget.ts ───────────────────────────────────────────────────────────────────
type SessionIndex = Pick<ThreadPreviewState, 'snapshot' | 'sessions'>;
export const needsPreviewAutomationSessionSync = (state: SessionIndex, requestedTabId: string | undefined): boolean =>
  Object.keys(state.sessions).length === 0 || requestedTabId === undefined || state.sessions[requestedTabId] === undefined;
export function resolvePreviewAutomationTarget(state: SessionIndex, requestedTabId: string | null): { tabId: string | null; snapshot: PreviewSessionSnapshot | null } {
  const snapshot = requestedTabId ? (state.sessions[requestedTabId] ?? null) : state.snapshot;
  return { tabId: snapshot?.tabId ?? null, snapshot };
}
export function resolvePreviewAutomationOpenTab(state: SessionIndex, requestedTabId: string | undefined, reuseExistingTab: boolean): string | null {
  if (!reuseExistingTab) return null;
  if (requestedTabId !== undefined) return state.sessions[requestedTabId]?.tabId ?? null;
  return state.snapshot?.tabId ?? null;
}

// ── agentBrowserCursorLogic.ts, previewAutomationClientId.ts ─────────────────────────────────────
export type BrowserController = 'human' | 'agent' | 'none';
export function agentBrowserCursorOpacity(active: boolean, controller: BrowserController): number {
  if (active) return 1;
  return controller === 'human' ? 0.18 : 0.35;
}
/** `preview-<32 hex>`, from 16 random bytes (the module's UUIDs here: Hermes has no Web Crypto). */
export const previewAutomationClientId = (uuid: string): string => `preview-${uuid.replace(/-/g, '').toLowerCase()}`;

// ── previewAutomationRequestConsumer.ts: the stream's connection ─────────────────────────────────
/** The reference's consumer atom as a value: a replacement stream is acknowledged before its requests are
 *  consumed; late requests of an older connection are dropped once a `connected` event named the new one. */
export class PreviewAutomationRequestConsumer {
  activeConnectionId: string | null = null;
  private explicit = false;
  /** The request to handle, or null (a `connected` event, or a request of a retired connection). */
  consume(event: PreviewAutomationStreamEvent): PreviewAutomationRequest | null {
    if (event.type === 'connected') { this.activeConnectionId = event.connectionId; this.explicit = true; return null; }
    if (this.activeConnectionId === null) this.activeConnectionId = event.connectionId;
    else if (this.activeConnectionId !== event.connectionId) {
      if (this.explicit) return null;
      this.activeConnectionId = event.connectionId;
    }
    return event.request;
  }
}

// ── previewAutomationErrors.ts: the host's errors the data module answers ────────────────────────
export type ErrorContext = { requestId: string; operation: PreviewAutomationOperation; environmentId: string; threadId: string; tabId: string | null };
/** PreviewAutomationTargetUnavailableError (answered as PreviewAutomationTabNotFoundError). */
export const targetUnavailableError = (context: ErrorContext): HostErrorJson => ({
  _tag: 'PreviewAutomationTabNotFoundError',
  message: `Preview automation target for ${context.operation} request ${context.requestId} is unavailable on environment ${context.environmentId} thread ${context.threadId} (tab ${context.tabId ?? 'unassigned'}, bridge available).`,
  detail: { requestId: context.requestId, operation: context.operation, environmentId: context.environmentId, threadId: context.threadId, tabId: context.tabId, bridgeAvailable: true },
});
/** PreviewAutomationOperationError (answered as PreviewAutomationExecutionError): the cause stays here. */
export const operationError = (context: ErrorContext): HostErrorJson => ({
  _tag: 'PreviewAutomationExecutionError',
  message: `Preview automation ${context.operation} request ${context.requestId} failed on environment ${context.environmentId} thread ${context.threadId} (tab ${context.tabId ?? 'unassigned'}).`,
  detail: { requestId: context.requestId, operation: context.operation, environmentId: context.environmentId, threadId: context.threadId, tabId: context.tabId },
});

// ── browserTargetResolver.ts ─────────────────────────────────────────────────────────────────────
export type BrowserNavigationTarget = { kind: 'url'; url: string } | { kind: 'environment-port'; port: number; protocol?: 'http' | 'https'; path?: string };
/** resolveBrowserNavigationTarget: a URL as it is; an environment port at the environment's own host (`localhost` for a
 *  loopback environment, so a dev server bound to ::1 or 127.0.0.1 is reached). */
export function resolveBrowserNavigationTarget(environmentUrl: string, target: BrowserNavigationTarget): string {
  if (target.kind === 'url') return target.url;
  const url = new URL(environmentUrl), host = url.hostname.replace(/^\[|\]$/g, '');
  if (!isPrivateNetworkHost(host)) throw new Error('This environment port needs the planned authenticated preview gateway; its server address is not directly private-network reachable.');
  const path = target.path?.startsWith('/') ? target.path : `/${target.path ?? ''}`;
  const resolvedHost = isLocalLoopbackHost(host) ? 'localhost' : host.includes(':') ? `[${host}]` : host;
  return new URL(path, `${target.protocol ?? 'http'}://${resolvedHost}:${target.port}`).toString();
}

// ── The plan (PreviewAutomationHosts.tsx `handleRequest`) ────────────────────────────────────────
export type PlanContext = {
  request: PreviewAutomationRequest; connectionId: string; clientId: string; environmentId: string; environmentUrl: string; generation: number;
  /** The thread's preview state after any session sync the request needed. */
  state: ThreadPreviewState;
  /** The request's host wait budget (`resolveHostWaitBudgetMs(timeoutMs)`): the module's deadline is its receipt plus this. */
  budgetMs: number;
  /** Tabs whose presentation the agent suppressed (`open: false`) in this thread. */
  suppressed: ReadonlySet<string>;
  /** The floating-preview preference (`browserAutoShowFloatingPreview`, Settings › Integrations › Browser, part 4). */
  autoShowFloatingPreview: boolean;
  /** The configured open defaults (part 4's browserDefaults): a tab an agent creates without a size gets them, as a
   *  hand-opened one does. Fill and Default when absent. */
  openDefaults?: { viewport: PreviewViewportSetting; profileId: string };
};
export type Plan = Record<string, unknown> & { requestId: string; operation: PreviewAutomationOperation };
/** What the data module does besides sending the plan: show a tab, record or clear a suppression. */
export type PlanEffects = { present: string | null; suppress: string | null; unsuppress: string | null };

const READY_OPERATIONS = new Set<PreviewAutomationOperation>(['navigate', 'resize', 'setColorScheme', 'snapshot', 'click', 'type', 'press', 'scroll', 'evaluate', 'waitFor', 'recordingStart']);

export function planRequest(context: PlanContext): { plan: Plan; effects: PlanEffects } {
  const { request, state } = context;
  const ref: ScopedThreadRef = { environmentId: context.environmentId, threadId: request.threadId };
  const effects: PlanEffects = { present: null, suppress: null, unsuppress: null };
  const runtimeOf = (tabId: string) => previewRuntimeTabId(ref, state.serverEpoch, tabId);
  const tabId = request.tabId ?? state.snapshot?.tabId ?? null;
  const base: Plan = {
    requestId: request.requestId, connectionId: context.connectionId, clientId: context.clientId, operation: request.operation, input: request.input,
    environmentId: context.environmentId, threadId: request.threadId, timeoutMs: request.timeoutMs, generation: context.generation, budgetMs: context.budgetMs,
    tabId, runtimeId: tabId && state.sessions[tabId] ? runtimeOf(tabId) : null,
    ...(request.tabIdExplicit ? { tabIdExplicit: true } : {}),
  };
  const errorContext: ErrorContext = { requestId: request.requestId, operation: request.operation, environmentId: context.environmentId, threadId: request.threadId, tabId };
  const fail = (error: HostErrorJson) => ({ plan: { ...base, failure: error }, effects });
  if (request.operation === 'status') {
    const target = resolvePreviewAutomationTarget(state, tabId);
    const nav = target.snapshot?.navStatus;
    return { plan: { ...base, tabId: target.tabId, runtimeId: target.tabId ? runtimeOf(target.tabId) : null,
      ...(target.snapshot ? { viewportSetting: target.snapshot.viewport ?? FILL_PREVIEW_VIEWPORT } : {}),
      fallback: { url: nav && nav._tag !== 'Idle' ? nav.url : null, title: nav && nav._tag !== 'Idle' ? nav.title : null, loading: nav?._tag === 'Loading' } }, effects };
  }
  if (request.operation === 'open') {
    const input = request.input as OpenInput;
    const activeTabId = resolvePreviewAutomationOpenTab(state, request.tabId, input.reuseExistingTab ?? true);
    const present = shouldOpenPreviewMiniPlayer(input, context.autoShowFloatingPreview), suppressed = explicitlySuppressesPreviewMiniPlayer(input);
    let url: string | undefined;
    if (input.url !== undefined) { try { url = normalizePreviewUrl(input.url); } catch { return fail(operationError({ ...errorContext, tabId: activeTabId })); } }
    if (!activeTabId) {
      const viewport = context.openDefaults?.viewport ?? FILL_PREVIEW_VIEWPORT;
      return { plan: { ...base, tabId: null, runtimeId: null, open: {
        create: { threadId: request.threadId, ...(input.url !== undefined ? { url: input.url } : {}), viewport, profileId: context.openDefaults?.profileId ?? 'default' },
        epoch: state.serverEpoch, present, suppress: suppressed,
        // Part 2 (the device toolbar) renders it; T3BrowserViewport is the hook.
        defaultViewport: previewAutomationDefaultViewport(false, { viewport }),
      } }, effects };
    }
    const snapshot = state.sessions[activeTabId] ?? state.snapshot;
    const runtimeId = runtimeOf(activeTabId);
    if (suppressed) effects.suppress = runtimeId;
    else if (present) effects.unsuppress = runtimeId;
    if (present) effects.present = activeTabId;
    return { plan: { ...base, tabId: activeTabId, runtimeId, open: {
      tabId: activeTabId, runtimeId, ...(url ? { url } : {}), present, needsOverlay: !!snapshot && previewAutomationOpenNeedsOverlay(input, snapshot),
    }, ...(snapshot ? { viewportSetting: snapshot.viewport ?? FILL_PREVIEW_VIEWPORT } : {}) }, effects };
  }
  if (request.operation === 'recordingStop') return { plan: base, effects };
  // requireReadyTab: a tab to act on, shown unless the agent asked for background work.
  if (!tabId || !state.sessions[tabId]) return fail(targetUnavailableError(errorContext));
  const runtimeId = runtimeOf(tabId);
  if (READY_OPERATIONS.has(request.operation) && shouldAutoShowPreviewForAutomationUse({ operation: request.operation, autoShowFloatingPreview: context.autoShowFloatingPreview, presentationSuppressed: context.suppressed.has(runtimeId) })) effects.present = tabId;
  const plan: Plan = { ...base, tabId, runtimeId, viewportSetting: state.sessions[tabId]?.viewport ?? FILL_PREVIEW_VIEWPORT };
  switch (request.operation) {
    case 'navigate': {
      const input = request.input as { url?: string; target?: BrowserNavigationTarget };
      try { plan.navigate = { url: normalizePreviewUrl(resolveBrowserNavigationTarget(context.environmentUrl, input.target ?? { kind: 'url', url: input.url ?? '' })) }; }
      catch { return fail(operationError(errorContext)); }
      break;
    }
    case 'resize': {
      const input = request.input as { mode?: string; width?: number; height?: number; preset?: string; orientation?: string };
      plan.viewport = input.mode === 'freeform' ? { _tag: 'freeform', width: input.width, height: input.height }
        : input.mode === 'preset' ? { _tag: 'preset', presetId: input.preset, ...(input.orientation ? { orientation: input.orientation } : {}) } : FILL_PREVIEW_VIEWPORT;
      break;
    }
    case 'press': plan.key = pressPlan(request.input as PressInput); break;
  }
  return { plan, effects };
}
