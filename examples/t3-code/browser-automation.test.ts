// browser-surface part 5. Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names:
// apps/web/src/components/preview/previewAutomationHostBudget.test.ts (10: its `it.each` rows count one each),
// previewAutomationOpenReadiness.test.ts (13), previewAutomationTarget.test.ts (4), agentBrowserCursorLogic.test.ts
// (3) and previewAutomationRequestConsumer.test.ts (11). The consumer's stream is the transport's inbox here
// (client.ts drain → `previewStreamEvent`) and its handler is the plan the module runs, so its first five tests
// drive the drain and the dispatch; four of its six serialization tests are the module's answers and run in
// the AppKit binary `macos/tests/browser-automation` (the recording, viewport, non-editable and sanitized
// handler failures), the other two here. Then the clone's own rows: request routing (`planRequest`), the
// session sync before a plan, adopting a tab the module opened, preview events, and the focus report.
// `vi.useFakeTimers` is the reference's; the budget tests use a manual clock (`Timers`).
import { describe, expect, it } from 'bun:test';
import {
  DEFAULT_PREVIEW_AUTOMATION_VIEWPORT, PREVIEW_HOST_RESPONSE_MARGIN_MS, PreviewAutomationRequestConsumer, agentBrowserCursorOpacity,
  explicitlySuppressesPreviewMiniPlayer, needsPreviewAutomationSessionSync, operationError, planRequest, previewAutomationDefaultViewport,
  previewAutomationOpenNeedsOverlay, readStreamEvent, resolveBrowserNavigationTarget, resolveHostWaitBudgetMs, resolvePreviewAutomationOpenTab,
  resolvePreviewAutomationTarget, shouldAutoShowPreviewForAutomationUse, shouldOpenPreviewMiniPlayer, targetUnavailableError, waitForHostReadiness,
  type OpenInput, type PlanContext, type PreviewAutomationRequest, type Timers,
} from './browser-automation-plan';
import {
  PREVIEW_AUTOMATION_KEY, PREVIEW_EVENTS_KEY, adoptAutomationTabs, automationFleetEvent, automationHost, automationPrepare, focusedLink, previewStreamEvent, readPreviewEvent,
  reportAutomationFocus,
} from './browser-automation';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { browserHost } from './browser-surface';
import { EMPTY_THREAD_PREVIEW_STATE, PreviewStateStore, previewRuntimeTabId, type PreviewSessionSnapshot, type ThreadPreviewState } from './browser-state';
import { surfaceStore } from './r4-surfaces-panel';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { obj, type Obj } from './domain';

const snapshotOf = (navStatus: PreviewSessionSnapshot['navStatus'], tabId = 'tab-1'): PreviewSessionSnapshot => ({
  threadId: 'thread-1', tabId, navStatus, canGoBack: false, canGoForward: false, updatedAt: '2026-06-26T00:00:00.000Z',
});

/** A manual clock for `waitForHostReadiness` (the reference's `vi.useFakeTimers()` + `setSystemTime(0)`). */
function manualTimers() {
  let now = 0, next = 1;
  const pending = new Map<number, { at: number; fn: () => void }>();
  const timers: Timers = {
    now: () => now,
    setTimeout: (fn, ms) => { const id = next++; pending.set(id, { at: now + Math.max(0, ms), fn }); return id; },
    clearTimeout: id => { pending.delete(id as number); },
  };
  const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
  /** advanceTimersByTimeAsync */
  const advance = async (ms: number) => {
    const end = now + ms;
    for (;;) {
      await flush();
      const due = [...pending.entries()].filter(([, timer]) => timer.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      pending.delete(due[0]); now = Math.max(now, due[1].at); due[1].fn();
    }
    now = end; await flush();
  };
  return { timers, advance, count: () => pending.size, now: () => now };
}

describe('resolveHostWaitBudgetMs', () => {
  it('reserves the full response margin once the request budget allows it', () => {
    expect(resolveHostWaitBudgetMs(15_000)).toBe(15_000 - PREVIEW_HOST_RESPONSE_MARGIN_MS);
    expect(resolveHostWaitBudgetMs(60_000)).toBe(60_000 - PREVIEW_HOST_RESPONSE_MARGIN_MS);
  });
  it('keeps most of a short request budget instead of collapsing it', () => {
    expect(resolveHostWaitBudgetMs(1_000)).toBe(800);
    expect(resolveHostWaitBudgetMs(100)).toBe(80);
  });
  it('returns a non-negative budget for invalid input', () => {
    for (const invalid of [0, -5, Number.NaN, Number.POSITIVE_INFINITY]) expect(resolveHostWaitBudgetMs(invalid)).toBeGreaterThanOrEqual(0);
  });
});

describe('waitForHostReadiness', () => {
  for (const requestTimeoutMs of [1, 10, 100, 250, 1_000, 15_000]) {
    it(`stops unavailable-overlay polling before a ${requestTimeoutMs}ms broker timeout`, async () => {
      const clock = manualTimers();
      const deadlineMs = clock.now() + resolveHostWaitBudgetMs(requestTimeoutMs);
      let finishedAt: number | undefined;
      const result = waitForHostReadiness(deadlineMs, async () => false, clock.timers).then(ready => { finishedAt = clock.now(); return ready; });
      await clock.advance(requestTimeoutMs);
      expect(await result).toBe(false);
      expect(finishedAt).toBeLessThan(requestTimeoutMs);
      expect(clock.count()).toBe(0);
    });
  }
  it('includes session setup in the deadline instead of starting a fresh wait budget', async () => {
    const clock = manualTimers();
    const deadlineMs = clock.now() + resolveHostWaitBudgetMs(15_000);
    await clock.advance(2_000);
    let finished = false;
    const result = waitForHostReadiness(deadlineMs, async () => false, clock.timers).then(ready => { finished = true; return ready; });
    await clock.advance(11_499);
    expect(finished).toBe(false);
    await clock.advance(1);
    expect(await result).toBe(false);
    expect(clock.now()).toBe(deadlineMs);
    expect(clock.count()).toBe(0);
  });
  it('skips readiness probes when setup has already exhausted the deadline', async () => {
    const clock = manualTimers();
    const deadlineMs = clock.now() + resolveHostWaitBudgetMs(100);
    await clock.advance(100);
    let probes = 0;
    expect(await waitForHostReadiness(deadlineMs, async () => { probes++; return false; }, clock.timers)).toBe(false);
    expect(probes).toBe(0);
    expect(clock.count()).toBe(0);
  });
  it('returns as soon as the overlay is ready and clears its timeout', async () => {
    const clock = manualTimers();
    const answers = [false, true];
    let probes = 0;
    const result = waitForHostReadiness(800, async () => answers[probes++] ?? true, clock.timers);
    await clock.advance(50);
    expect(await result).toBe(true);
    expect(probes).toBe(2);
    expect(clock.count()).toBe(0);
  });
  for (const ready of [false, true]) {
    it(`keeps a probe result of ${ready} that wins the deadline race`, async () => {
      const clock = manualTimers();
      let probes = 0;
      const result = waitForHostReadiness(80, () => { probes++; return new Promise<boolean>(resolve => { clock.timers.setTimeout(() => resolve(ready), 80); }); }, clock.timers);
      await clock.advance(80);
      expect(await result).toBe(ready);
      expect(probes).toBe(1);
      expect(clock.count()).toBe(0);
    });
  }
  it('bounds a stalled status probe and ignores a late ready result', async () => {
    const clock = manualTimers();
    let completeProbe!: (ready: boolean) => void, probes = 0;
    const result = waitForHostReadiness(80, () => { probes++; return new Promise<boolean>(resolve => { completeProbe = resolve; }); }, clock.timers);
    await clock.advance(80);
    expect(await result).toBe(false);
    expect(clock.count()).toBe(0);
    completeProbe(true);
    await clock.advance(1_000);
    expect(probes).toBe(1);
    expect(clock.count()).toBe(0);
  });
  it('preserves probe failures and clears the pending timeout', async () => {
    const clock = manualTimers(), error = new Error('Preview target was replaced');
    await expect(waitForHostReadiness(800, async () => { throw error; }, clock.timers)).rejects.toBe(error);
    expect(clock.count()).toBe(0);
  });
});

describe('preview automation open readiness', () => {
  it('opens the inline preview by default', () => { expect(shouldOpenPreviewMiniPlayer({} as OpenInput)).toBe(true); });
  it('supports explicit opt-out and the legacy show alias', () => {
    expect(shouldOpenPreviewMiniPlayer({ open: false })).toBe(false);
    expect(shouldOpenPreviewMiniPlayer({ show: false })).toBe(false);
    expect(shouldOpenPreviewMiniPlayer({ open: true, show: false })).toBe(true);
  });
  it('does not wait for a desktop overlay when opening an empty tab', () => {
    expect(previewAutomationOpenNeedsOverlay({}, snapshotOf({ _tag: 'Idle' }))).toBe(false);
  });
  it('waits when an empty tab is immediately given a URL', () => {
    expect(previewAutomationOpenNeedsOverlay({ url: 'https://example.com' }, snapshotOf({ _tag: 'Idle' }))).toBe(true);
  });
  it('waits for existing tabs that already have rendered content', () => {
    expect(previewAutomationOpenNeedsOverlay({}, snapshotOf({ _tag: 'Success', url: 'https://example.com/', title: 'Example' }))).toBe(true);
  });
  it('gives newly-created automation tabs a stable desktop viewport', () => {
    expect(previewAutomationDefaultViewport(false, snapshotOf({ _tag: 'Idle' }))).toEqual(DEFAULT_PREVIEW_AUTOMATION_VIEWPORT);
  });
  it('preserves reused and already-fixed browser viewports', () => {
    expect(previewAutomationDefaultViewport(true, snapshotOf({ _tag: 'Idle' }))).toBeNull();
    expect(previewAutomationDefaultViewport(false, { ...snapshotOf({ _tag: 'Idle' }), viewport: { _tag: 'freeform', width: 900, height: 600 } })).toBeNull();
  });
});

describe('shouldOpenPreviewMiniPlayer with the floating-preview preference', () => {
  it('honours the preference when the agent said nothing either way', () => {
    expect(shouldOpenPreviewMiniPlayer({}, false)).toBe(false);
    expect(shouldOpenPreviewMiniPlayer({}, true)).toBe(true);
  });
  it('lets an explicit request outrank the preference in both directions', () => {
    expect(shouldOpenPreviewMiniPlayer({ open: true }, false)).toBe(true);
    expect(shouldOpenPreviewMiniPlayer({ open: false }, true)).toBe(false);
    expect(shouldOpenPreviewMiniPlayer({ show: true }, false)).toBe(true);
  });
  it('distinguishes an explicit background request from a disabled preference', () => {
    expect(explicitlySuppressesPreviewMiniPlayer({ open: false })).toBe(true);
    expect(explicitlySuppressesPreviewMiniPlayer({ show: false })).toBe(true);
    expect(explicitlySuppressesPreviewMiniPlayer({})).toBe(false);
  });
});

describe('auto-show for existing automation tabs', () => {
  it('records floating-preview intent whenever an agent uses the tab', () => {
    expect(shouldAutoShowPreviewForAutomationUse({ operation: 'navigate', autoShowFloatingPreview: true, presentationSuppressed: false })).toBe(true);
  });
  it('leaves explicit opens and background-only tabs alone', () => {
    expect(shouldAutoShowPreviewForAutomationUse({ operation: 'open', autoShowFloatingPreview: true, presentationSuppressed: false })).toBe(false);
    expect(shouldAutoShowPreviewForAutomationUse({ operation: 'click', autoShowFloatingPreview: true, presentationSuppressed: true })).toBe(false);
  });
  it('honours the auto-show preference for reused tabs', () => {
    expect(shouldAutoShowPreviewForAutomationUse({ operation: 'snapshot', autoShowFloatingPreview: false, presentationSuppressed: false })).toBe(false);
  });
});

describe('preview automation target selection', () => {
  const snapshot = (tabId: string) => snapshotOf({ _tag: 'Idle' }, tabId);
  it('refreshes authoritative sessions whenever the caller relies on the active tab', () => {
    const active = snapshot('tab-active');
    expect(needsPreviewAutomationSessionSync({ snapshot: active, sessions: { [active.tabId]: active } }, undefined)).toBe(true);
  });
  it('refreshes an explicit tab only when it is absent locally', () => {
    const active = snapshot('tab-active'), state = { snapshot: active, sessions: { [active.tabId]: active } };
    expect(needsPreviewAutomationSessionSync(state, active.tabId)).toBe(false);
    expect(needsPreviewAutomationSessionSync(state, 'tab-missing')).toBe(true);
  });
  it('does not report the active tab under an unknown requested tab id', () => {
    const active = snapshot('tab-active');
    expect(resolvePreviewAutomationTarget({ snapshot: active, sessions: { [active.tabId]: active } }, 'tab-missing')).toEqual({ tabId: null, snapshot: null });
  });
  it("reuses the provider session's pinned tab instead of the mutable UI tab", () => {
    const uiActive = snapshot('tab-ui-active'), agentTab = snapshot('tab-opened-by-agent');
    const state = { snapshot: uiActive, sessions: { [uiActive.tabId]: uiActive, [agentTab.tabId]: agentTab } };
    expect(resolvePreviewAutomationOpenTab(state, agentTab.tabId, true)).toBe(agentTab.tabId);
    expect(resolvePreviewAutomationOpenTab(state, undefined, true)).toBe(uiActive.tabId);
    expect(resolvePreviewAutomationOpenTab(state, agentTab.tabId, false)).toBeNull();
  });
});

describe('agentBrowserCursorOpacity', () => {
  it('keeps active movement fully visible', () => {
    expect(agentBrowserCursorOpacity(true, 'agent')).toBe(1);
    expect(agentBrowserCursorOpacity(true, 'human')).toBe(1);
  });
  it('settles to a visible idle state', () => {
    expect(agentBrowserCursorOpacity(false, 'none')).toBe(0.35);
    expect(agentBrowserCursorOpacity(false, 'agent')).toBe(0.35);
  });
  it('dims further while the human controls the page', () => { expect(agentBrowserCursorOpacity(false, 'human')).toBe(0.18); });
});

// ── The client: drain, queue, plan, dispatch ──────────────────────────────────────────────────────
type Sent = Obj;
/** A focused client with a fake transport (`call`, `rpc`) and module (`raw`), as browser-surface.test.ts's. */
function fakeClient(options: { list?: (payload: Obj) => Obj; presentation?: Obj } = {}) {
  const sent: Sent[] = [], rpcs: Array<{ method: string; payload: Obj }> = [];
  let serial = 0;
  const client = {
    environmentId: 'env-1', threadId: 'thread-1', projectId: 'p1', generation: 3, connection: 'connected', ready: true, origin: 'http://127.0.0.1:16750',
    draftKey: 'env-1:thread-1', presentation: options.presentation ?? {}, revision: 0, diffOpen: true, local: { composerControls: false }, shell: { threads: [], projects: [] },
    async ids() { return ['0123abcd-4567-89ab-cdef-0123456789ab']; },
    restAccess() { return { call: async (request: Obj) => { sent.push(request); return request.op === 'request' ? (rpcs.push({ method: String(request.method), payload: obj(request.payload) }), {}) : { id: `3-${++serial}` }; } }; },
    async rpc(_native: Native, method: string, payload: Obj) {
      rpcs.push({ method, payload });
      if (method === 'preview.list') return options.list?.(payload) ?? { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
      return {};
    },
    async raw(_native: Native, request: Obj) { sent.push(request); return { ok: true, generation: 3, value: { accepted: true } }; },
  } as unknown as T3Client & { revision: number; diffOpen: boolean; presentation: Obj };
  return { client, sent, rpcs };
}
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
const entry = (value: Obj, subscriptionId = '3-2', key = PREVIEW_AUTOMATION_KEY): Obj => ({ key, subscriptionId, value, generation: 3 });
const requestOf = (requestId: string, overrides: Partial<PreviewAutomationRequest> = {}): PreviewAutomationRequest => ({ requestId, threadId: 'thread-1', operation: 'status', input: {}, timeoutMs: 15_000, ...overrides });
const requestEntry = (requestId: string, overrides: Partial<PreviewAutomationRequest> = {}, connectionId = 'connection-1') => entry({ type: 'request', connectionId, request: requestOf(requestId, overrides) });
const plans = (sent: Sent[]) => sent.filter(request => request.op === 'browserAutomation').map(request => request.plan as Obj);
/** The client subscribed once (the second subscription id is the automation stream's: events subscribe first). */
async function subscribed(options: Parameters<typeof fakeClient>[0] = {}) {
  const fake = fakeClient(options);
  await automationPrepare(fake.client, module);
  return fake;
}

describe('previewAutomationRequestConsumer', () => {
  it('acknowledges a replacement stream before consuming requests from it', async () => {
    const { client, sent } = await subscribed();
    previewStreamEvent(client, entry({ type: 'connected', connectionId: 'connection-1' }));
    previewStreamEvent(client, requestEntry('request-after-connect'));
    expect(automationHost(client).consumer.activeConnectionId).toBe('connection-1');
    await automationPrepare(client, module);
    expect(plans(sent).map(plan => [plan.requestId, plan.connectionId])).toEqual([['request-after-connect', 'connection-1']]);
  });
  it('drops late requests from an older stream generation', async () => {
    const { client, sent } = await subscribed();
    previewStreamEvent(client, entry({ type: 'connected', connectionId: 'connection-2' }));
    previewStreamEvent(client, requestEntry('request-stale', {}, 'connection-1'));
    await automationPrepare(client, module);
    expect(automationHost(client).consumer.activeConnectionId).toBe('connection-2');
    expect(plans(sent)).toEqual([]);
  });
  it('consumes every request emitted before React can render', async () => {
    const { client, sent } = await subscribed();
    previewStreamEvent(client, requestEntry('request-1'));
    previewStreamEvent(client, requestEntry('request-2'));
    await automationPrepare(client, module);
    expect(plans(sent).map(plan => plan.requestId)).toEqual(['request-1', 'request-2']);
  });
  it('uses the latest request handler without rebuilding the stream consumer', async () => {
    // The clone's "handler" is the plan from the store as it is when the request is served, not when it came.
    const { client, sent } = await subscribed({ list: () => ({ sessions: [snapshotOf({ _tag: 'Success', url: 'https://a.test/', title: 'A' })], serverEpoch: 'epoch-1', revision: 2 }) });
    previewStreamEvent(client, requestEntry('request-first'));
    await automationPrepare(client, module);
    browserHost(client).store.applyServerSnapshot({ environmentId: 'env-1', threadId: 'thread-1' }, { ...snapshotOf({ _tag: 'Success', url: 'https://b.test/', title: 'B' }), updatedAt: '2026-06-27T00:00:00.000Z' });
    previewStreamEvent(client, requestEntry('request-second'));
    await automationPrepare(client, module);
    expect(plans(sent).map(plan => (plan.fallback as Obj).url)).toEqual(['https://a.test/', 'https://b.test/']);
  });
  it('consumes a request that arrived immediately before the consumer mounted', async () => {
    const { client, sent } = fakeClient();
    previewStreamEvent(client, requestEntry('request-ready'));
    await automationPrepare(client, module);
    expect(plans(sent)).toMatchObject([{ requestId: 'request-ready', connectionId: 'connection-1', clientId: 'preview-0123abcd456789abcdef0123456789ab' }]);
  });
  it('preserves tagged automation errors and their structured diagnostics', () => {
    expect(targetUnavailableError({ requestId: 'request-1', operation: 'click', environmentId: 'environment-1', threadId: 'thread-1', tabId: 'tab-1' })).toEqual({
      _tag: 'PreviewAutomationTabNotFoundError',
      message: 'Preview automation target for click request request-1 is unavailable on environment environment-1 thread thread-1 (tab tab-1, bridge available).',
      detail: { requestId: 'request-1', operation: 'click', environmentId: 'environment-1', threadId: 'thread-1', tabId: 'tab-1', bridgeAvailable: true },
    });
  });
  it('correlates unexpected failures without exposing cause details', () => {
    const response = operationError({ requestId: 'request-2', operation: 'snapshot', environmentId: 'environment-1', threadId: 'thread-1', tabId: 'tab-1' });
    expect(response).toEqual({
      _tag: 'PreviewAutomationExecutionError',
      message: 'Preview automation snapshot request request-2 failed on environment environment-1 thread thread-1 (tab tab-1).',
      detail: { requestId: 'request-2', operation: 'snapshot', environmentId: 'environment-1', threadId: 'thread-1', tabId: 'tab-1' },
    });
  });
});

// ── The clone's routing rows ──────────────────────────────────────────────────────────────────────
const ref = { environmentId: 'env-1', threadId: 'thread-1' };
function context(request: PreviewAutomationRequest, state: ThreadPreviewState, extra: Partial<PlanContext> = {}): PlanContext {
  return { request, connectionId: 'connection-1', clientId: 'preview-client', environmentId: 'env-1', environmentUrl: 'http://127.0.0.1:16750', generation: 3, state, budgetMs: 13_500, suppressed: new Set(), autoShowFloatingPreview: true, ...extra };
}
function stateWith(...snapshots: PreviewSessionSnapshot[]): ThreadPreviewState {
  const store = new PreviewStateStore();
  store.reconcileServerSessions(ref, { sessions: snapshots, serverEpoch: 'epoch-1', revision: 1 });
  return store.read(ref);
}
const loaded = (tabId = 'tab-1') => snapshotOf({ _tag: 'Success', url: 'http://127.0.0.1:16751/', title: 'Fixture' }, tabId);

describe('planRequest (the request handler up to the page)', () => {
  it('targets the thread’s current tab by its runtime id, shown unless the agent said otherwise', () => {
    const { plan, effects } = planRequest(context(requestOf('r1', { operation: 'click', input: { locator: 'text=Go' } }), stateWith(loaded())));
    expect(plan).toMatchObject({ operation: 'click', tabId: 'tab-1', runtimeId: previewRuntimeTabId(ref, 'epoch-1', 'tab-1'), budgetMs: 13_500, generation: 3, viewportSetting: { _tag: 'fill' } });
    expect(effects.present).toBe('tab-1');
    expect(planRequest(context(requestOf('r2', { operation: 'click' }), stateWith(loaded()), { suppressed: new Set([previewRuntimeTabId(ref, 'epoch-1', 'tab-1')]) })).effects.present).toBeNull();
  });
  it('answers a tab it cannot find with the reference’s TabNotFound error', () => {
    const { plan } = planRequest(context(requestOf('r1', { operation: 'snapshot', tabId: 'tab-9' }), stateWith(loaded())));
    expect((plan.failure as Obj)._tag).toBe('PreviewAutomationTabNotFoundError');
    expect(planRequest(context(requestOf('r2', { operation: 'evaluate' }), EMPTY_THREAD_PREVIEW_STATE)).plan.failure).toMatchObject({ detail: { tabId: null } });
  });
  it('status falls back to the snapshot and names no tab under an unknown id', () => {
    const status = planRequest(context(requestOf('r1'), stateWith(loaded()))).plan;
    expect(status).toMatchObject({ tabId: 'tab-1', fallback: { url: 'http://127.0.0.1:16751/', title: 'Fixture', loading: false }, viewportSetting: { _tag: 'fill' } });
    expect(planRequest(context(requestOf('r2', { tabId: 'tab-9' }), stateWith(loaded()))).plan).toMatchObject({ tabId: null, runtimeId: null });
  });
  it('open creates a tab when there is none to reuse, or reuses and navigates the pinned one', () => {
    const created = planRequest(context(requestOf('r1', { operation: 'open', input: { url: 'localhost:5173' } }), stateWith())).plan;
    expect(created.open).toEqual({ create: { threadId: 'thread-1', url: 'localhost:5173', viewport: { _tag: 'fill' }, profileId: 'default' }, epoch: 'epoch-1', present: true, suppress: false, defaultViewport: DEFAULT_PREVIEW_AUTOMATION_VIEWPORT });
    const fresh = planRequest(context(requestOf('r2', { operation: 'open', input: { reuseExistingTab: false } }), stateWith(loaded()))).plan;
    expect((fresh.open as Obj).create).toBeDefined();
    const reused = planRequest(context(requestOf('r3', { operation: 'open', input: { url: 'localhost:5173', open: false } }), stateWith(loaded())));
    expect(reused.plan.open).toEqual({ tabId: 'tab-1', runtimeId: previewRuntimeTabId(ref, 'epoch-1', 'tab-1'), url: 'http://localhost:5173/', present: false, needsOverlay: true });
    expect(reused.effects).toEqual({ present: null, suppress: previewRuntimeTabId(ref, 'epoch-1', 'tab-1'), unsuppress: null });
  });
  it('navigate resolves an environment port at the environment’s host and refuses a bad URL', () => {
    const port = planRequest(context(requestOf('r1', { operation: 'navigate', input: { target: { kind: 'environment-port', port: 5173, path: 'settings?tab=1' } } }), stateWith(loaded()))).plan;
    expect(port.navigate).toEqual({ url: 'http://localhost:5173/settings?tab=1' });
    expect(planRequest(context(requestOf('r2', { operation: 'navigate', input: { url: 't3.chat' } }), stateWith(loaded()))).plan.navigate).toEqual({ url: 'https://t3.chat/' });
    expect((planRequest(context(requestOf('r3', { operation: 'navigate', input: { url: 'ftp://x' } }), stateWith(loaded()))).plan.failure as Obj)._tag).toBe('PreviewAutomationExecutionError');
    expect(() => resolveBrowserNavigationTarget('https://remote.example.com', { kind: 'environment-port', port: 3000 })).toThrow('preview gateway');
  });
  it('press carries the native chord, its text, and the macOS editing commands', () => {
    expect(planRequest(context(requestOf('r1', { operation: 'press', input: { key: 'Enter' } }), stateWith(loaded()))).plan.key).toMatchObject({ chord: 'Enter', text: '\r', commands: [], editing: '' });
    const selectAll = planRequest(context(requestOf('r2', { operation: 'press', input: { key: 'a', modifiers: ['Meta'] } }), stateWith(loaded()))).plan.key as Obj;
    expect(selectAll).toMatchObject({ chord: 'Meta+a', text: '', commands: ['selectAll'] });
    expect(String(selectAll.editing)).toContain('__T3_PREVIEW_CLIPBOARD__');
    expect(planRequest(context(requestOf('r3', { operation: 'resize', input: { mode: 'freeform', width: 800, height: 600 } }), stateWith(loaded()))).plan.viewport).toEqual({ _tag: 'freeform', width: 800, height: 600 });
  });
  it('reads stream events defensively', () => {
    expect(readStreamEvent({ type: 'connected', connectionId: ' c ' })).toEqual({ type: 'connected', connectionId: 'c' });
    expect(readStreamEvent({ type: 'request', connectionId: 'c', request: { requestId: 'r', threadId: 't', operation: 'nope', input: {}, timeoutMs: 1 } })).toBeNull();
    expect(readStreamEvent({ type: 'request', connectionId: 'c', request: { requestId: 'r', threadId: 't', operation: 'click', timeoutMs: 0 } })).toBeNull();
  });
});

describe('the host on the client', () => {
  it('subscribes the request stream once per connection with every operation', async () => {
    const { client, sent } = await subscribed();
    const subscriptions = sent.filter(request => request.op === 'subscribe');
    expect(subscriptions.map(request => request.method)).toEqual(['subscribePreviewEvents', 'previewAutomation.connect']);
    expect(subscriptions[1]?.payload).toEqual({ clientId: 'preview-0123abcd456789abcdef0123456789ab', environmentId: 'env-1',
      supportedOperations: ['status', 'open', 'navigate', 'snapshot', 'click', 'type', 'press', 'scroll', 'evaluate', 'waitFor', 'recordingStart', 'recordingStop', 'resize', 'setColorScheme'] });
    await automationPrepare(client, module);
    expect(sent.filter(request => request.op === 'subscribe')).toHaveLength(2);
    previewStreamEvent(client, entry({ _streamEnded: true }));
    await automationPrepare(client, module);
    expect(sent.filter(request => request.op === 'subscribe').map(request => request.method).at(-1)).toBe('previewAutomation.connect');
  });
  it('lists the thread’s sessions before planning a request that relies on its current tab', async () => {
    const { client, sent, rpcs } = await subscribed({ list: () => ({ sessions: [loaded()], serverEpoch: 'epoch-1', revision: 4 }) });
    previewStreamEvent(client, requestEntry('r1', { operation: 'snapshot' }));
    await automationPrepare(client, module);
    expect(rpcs.map(call => call.method)).toContain('preview.list');
    expect(plans(sent)[0]).toMatchObject({ tabId: 'tab-1', runtimeId: previewRuntimeTabId(ref, 'epoch-1', 'tab-1') });
    expect(surfaceStore(client).panels.get('env-1:thread-1')).toMatchObject({ active: 'browser:tab-1', visible: true });
    expect(client.diffOpen).toBe(false);
  });
  it('sends a plan once, with the request’s host wait budget (the module’s deadline starts at its receipt)', async () => {
    const { client, sent } = await subscribed();
    previewStreamEvent(client, requestEntry('r-new', { timeoutMs: 1_000 }));
    previewStreamEvent(client, requestEntry('r-new', { timeoutMs: 1_000 }));
    await automationPrepare(client, module);
    await automationPrepare(client, module);
    expect(plans(sent).map(plan => [plan.requestId, plan.budgetMs])).toEqual([['r-new', 800]]);
  });
  it('adopts a tab the module opened: into the thread’s store, shown as the agent asked', () => {
    const presentation = { browserAutomation: { opened: [{ requestId: 'r1', connectionId: 'connection-1', environmentId: 'env-1', threadId: 'thread-2', epoch: 'epoch-1', present: true, snapshot: { ...loaded('tab-7'), threadId: 'thread-2' } }] } };
    const { client } = fakeClient({ presentation });
    adoptAutomationTabs(client);
    adoptAutomationTabs(client);
    expect(Object.keys(browserHost(client).store.read({ environmentId: 'env-1', threadId: 'thread-2' }).sessions)).toEqual(['tab-7']);
    expect(surfaceStore(client).panels.get('env-1:thread-2')?.surfaces.map(surface => surface.id)).toEqual(['browser:tab-7']);
  });
  it('applies preview events to the threads it keeps state for', () => {
    const { client } = fakeClient();
    const opened = { type: 'opened', threadId: 'thread-1', tabId: 'tab-3', createdAt: '2026-10-09T00:00:00Z', serverEpoch: 'epoch-1', revision: 2, snapshot: loaded('tab-3') };
    expect(readPreviewEvent({ ...opened, revision: 0 })).toBeNull();
    previewStreamEvent(client, entry(opened, '3-1', PREVIEW_EVENTS_KEY));
    expect(Object.keys(browserHost(client).store.read(ref).sessions)).toEqual(['tab-3']);
    previewStreamEvent(client, entry({ ...opened, threadId: 'thread-elsewhere', revision: 3, snapshot: { ...loaded('tab-4'), threadId: 'thread-elsewhere' } }, '3-1', PREVIEW_EVENTS_KEY));
    expect(browserHost(client).store.read({ environmentId: 'env-1', threadId: 'thread-elsewhere' })).toBe(EMPTY_THREAD_PREVIEW_STATE);
    previewStreamEvent(client, entry({ type: 'closed', threadId: 'thread-1', tabId: 'tab-3', createdAt: '2026-10-09T00:00:01Z', serverEpoch: 'epoch-1', revision: 4 }, '3-1', PREVIEW_EVENTS_KEY));
    expect(browserHost(client).store.read(ref).sessions).toEqual({});
  });
  it('reports focus and live tabs once per change', async () => {
    const runtime = previewRuntimeTabId(ref, 'epoch-1', 'tab-1');
    const { client, rpcs } = await subscribed({ presentation: { browserTabs: { [runtime]: { kind: 'Success', attached: true } } } });
    previewStreamEvent(client, entry({ type: 'connected', connectionId: 'connection-1' }));
    browserHost(client).store.reconcileServerSessions(ref, { sessions: [loaded()], serverEpoch: 'epoch-1', revision: 1 });
    browserHost(client).store.applyDesktopState(ref, 'tab-1', { hasWebContents: true, canGoBack: false, canGoForward: false, loading: false, zoomFactor: 1, pictureInPicture: false, colorScheme: 'system', audioMuted: false, audible: false, controller: 'none', favicon: null });
    await reportAutomationFocus(client, focusedLink(client, module));
    await reportAutomationFocus(client, focusedLink(client, module));
    const focus = rpcs.filter(call => call.method === 'previewAutomation.focusHost');
    expect(focus).toHaveLength(1);
    expect(focus[0]?.payload).toEqual({ clientId: 'preview-0123abcd456789abcdef0123456789ab', environmentId: 'env-1', connectionId: 'connection-1', focused: true, liveTabs: [{ threadId: 'thread-1', tabId: 'tab-1', visible: true }] });
  });
});

describe('a background environment’s host (PreviewAutomationHosts mounts one per environment)', () => {
  it('serves a request that came on a fleet transport, on that transport, under that environment', async () => {
    const { client, sent } = fakeClient();
    const key = 'http://127.0.0.1:16760\nenv-2';
    const entry = { key, origin: 'http://127.0.0.1:16760', environmentId: 'env-2', phase: 'connected', message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0,
      subscriptions: {}, config: {}, shell: { projects: [], threads: [] }, scopes: [], error: '', requested: true } as unknown as FleetEntry;
    fleet.entries.set(key, entry);
    const fleetCalls: Obj[] = [];
    let serial = 0;
    const native = { available: true, watch() {}, async later(request: unknown) {
      const op = obj(request);
      if (op.op === 'browserAutomation') { sent.push(op); return { ok: true, generation: 3, value: { accepted: true } }; }
      fleetCalls.push(op);
      if (op.op === 'ids') return { ok: true, generation: 7, value: ['fffffffe-0000-0000-0000-000000000001'] };
      if (op.op === 'subscribe') return { ok: true, generation: 7, value: { id: `7-${++serial}` } };
      if (op.op === 'request' && op.method === 'preview.list') return { ok: true, generation: 7, value: { sessions: [{ ...loaded('tab-5'), threadId: 'thread-b' }], serverEpoch: 'epoch-2', revision: 3 } };
      return { ok: true, generation: 7, value: {} };
    } } as Native;
    try {
      await automationPrepare(client, native);
      expect(fleetCalls.filter(op => op.op === 'subscribe').map(op => [op.fleet, op.method])).toEqual([[key, 'subscribePreviewEvents'], [key, 'previewAutomation.connect']]);
      expect(automationFleetEvent(entry, { key: PREVIEW_AUTOMATION_KEY, subscriptionId: '7-2', generation: 7, value: { type: 'request', connectionId: 'c-2', request: requestOf('r-b', { threadId: 'thread-b', operation: 'snapshot' }) } })).toBe(true);
      expect(automationFleetEvent(entry, { key: 'shell', generation: 7, value: {} })).toBe(false);
      await automationPrepare(client, native);
      const plan = plans(sent).at(-1) ?? {};
      expect(plan).toMatchObject({ requestId: 'r-b', connectionId: 'c-2', environmentId: 'env-2', fleet: key, generation: 7, tabId: 'tab-5',
        runtimeId: previewRuntimeTabId({ environmentId: 'env-2', threadId: 'thread-b' }, 'epoch-2', 'tab-5'), clientId: 'preview-fffffffe000000000000000000000001' });
      expect(fleetCalls.some(op => op.op === 'request' && op.method === 'preview.list' && op.fleet === key)).toBe(true);
    } finally { fleet.entries.delete(key); }
  });
});

