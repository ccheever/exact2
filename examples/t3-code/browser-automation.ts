// browser-surface part 5: the previewAutomation host on the focused connection (MIT reference, see LICENSE-T3,
// T3 Code 1e2ecbd975: apps/web/src/components/preview/PreviewAutomationHosts.tsx, usePreviewSession.ts,
// previewAutomationRequestConsumer.ts; packages/contracts/src/rpc.ts `previewAutomation.connect`, `.respond`,
// `.focusHost`, `subscribePreviewEvents`).
//
// The streams are the Swift transport's subscriptions (X21: until #126's runner-owned streams), drained by
// client.ts into `previewStreamEvent`: a request joins this client's queue, a preview event updates the thread's
// preview store (an agent-opened or agent-navigated tab appears). `automationPrepare` (browserPrepare, every shell
// answer) subscribes, hands each queued request's plan (browser-automation-plan.ts) to the module once
// (`browserAutomation`), adopts the tabs the module opened and reports the window's focus and live tabs
// (`previewAutomation.focusHost`). The module answers `previewAutomation.respond` itself, so a request outlives
// the data-source answer that planned it (a let-go answer's replies are dropped).
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { subscriptionSerial } from './shell-vcs';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';
import { activeRef } from './terminal-drawer-view';
import { surfaceStore } from './r4-surfaces-panel';
import { browserHost, listPreviewSessions, openBrowserIn } from './browser-surface';
import { EMPTY_THREAD_PREVIEW_STATE, previewRuntimeTabId, readSnapshot, type PreviewEvent } from './browser-state';
import {
  PREVIEW_AUTOMATION_OPERATIONS, PreviewAutomationRequestConsumer, needsPreviewAutomationSessionSync, planRequest, previewAutomationClientId,
  readStreamEvent, resolveHostWaitBudgetMs, waitForHostReadiness, type PreviewAutomationRequest,
} from './browser-automation-plan';

export const PREVIEW_AUTOMATION_KEY = 'preview-automation';
export const PREVIEW_EVENTS_KEY = 'preview-events';

type Stream = { id: string; floor: number; maxSeen: number; tried: boolean };
type Pending = { request: PreviewAutomationRequest; connectionId: string; deadline: number; expires: number; listed: boolean; sending: boolean };
type Host = {
  clientId: string; generation: number; automation: Stream; events: Stream; consumer: PreviewAutomationRequestConsumer;
  queue: Pending[]; suppressed: Map<string, Set<string>>; adopted: Set<string>; focusKey: string; windowFocused: boolean;
};
const hosts = new WeakMap<T3Client, Host>();
const stream = (previous?: Stream): Stream => ({ id: '', floor: previous?.maxSeen ?? 0, maxSeen: previous?.maxSeen ?? 0, tried: false });
export function automationHost(client: T3Client): Host {
  let host = hosts.get(client);
  if (!host) {
    host = { clientId: '', generation: client.generation, automation: stream(), events: stream(), consumer: new PreviewAutomationRequestConsumer(),
      queue: [], suppressed: new Map(), adopted: new Set(), focusKey: '', windowFocused: true };
    hosts.set(client, host);
  }
  if (host.generation !== client.generation) {
    // A new connection: new subscriptions (serials only grow, so the high-water marks carry over); the queued
    // requests of the old one can no longer be answered there.
    host.generation = client.generation; host.automation = stream(host.automation); host.events = stream(host.events);
    host.consumer = new PreviewAutomationRequestConsumer(); host.queue = []; host.focusKey = '';
  }
  return host;
}

/** shell.ts: the window's `exactPage()` focus and visibility (the reference's `document.hasFocus()` and visibility). */
export function noteAutomationWindow(client: T3Client, focused: boolean): void { automationHost(client).windowFocused = focused; }

/** One entry of a subscription (the newest subscription's entries count; an ended one subscribes again). */
function current(state: Stream, entry: Obj): Obj | null {
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  state.maxSeen = Math.max(state.maxSeen, serial);
  if (serial <= state.floor || (state.id && serial < subscriptionSerial(state.id))) return null;
  state.id = id;
  const value = obj(entry.value);
  if (value._retryDue || value._streamEnded || value._transportError) { state.id = ''; state.tried = false; return null; }
  return value;
}

/** client.ts drain: the `preview-automation` and `preview-events` entries of the focused connection. */
export function previewStreamEvent(client: T3Client, entry: Obj): void {
  const host = automationHost(client);
  if (str(entry.key) === PREVIEW_AUTOMATION_KEY) {
    const value = current(host.automation, entry), event = value ? readStreamEvent(value) : null;
    const request = event ? host.consumer.consume(event) : null;
    if (!request || !event || host.queue.some(pending => pending.request.requestId === request.requestId && pending.connectionId === event.connectionId)) return;
    // Session sync and tab creation consume the same budget as the page's readiness.
    const now = Date.now();
    host.queue.push({ request, connectionId: event.connectionId, deadline: now + resolveHostWaitBudgetMs(request.timeoutMs), expires: now + request.timeoutMs, listed: false, sending: false });
    client.revision++;
    return;
  }
  const value = current(host.events, entry), event = value ? readPreviewEvent(value) : null;
  if (!event || !client.environmentId) return;
  applyPreviewEvent(client, { environmentId: client.environmentId, threadId: event.threadId }, event);
}

/** usePreviewSession's event handling: an event of another server process asks for the list again. Events
 *  count for threads this client keeps preview state for (and the shown one), as the reference's per-thread sync. */
export function applyPreviewEvent(client: T3Client, ref: ScopedThreadRef, event: PreviewEvent): boolean {
  const store = browserHost(client).store, state = store.read(ref), active = activeRef(client);
  const known = state !== EMPTY_THREAD_PREVIEW_STATE || (active && scopedThreadKey(active) === scopedThreadKey(ref));
  if (!known) return false;
  if (state.serverEpoch !== null && state.serverEpoch !== event.serverEpoch) { browserHost(client).listed.delete(scopedThreadKey(ref)); client.revision++; return false; }
  const changed = store.applyServerEvent(ref, event);
  if (changed) client.revision++;
  return changed;
}

/** PreviewEvent read defensively; anything malformed is null. */
export function readPreviewEvent(value: unknown): PreviewEvent | null {
  const raw = obj(value), type = str(raw.type), base = { threadId: str(raw.threadId), tabId: str(raw.tabId), createdAt: str(raw.createdAt), serverEpoch: str(raw.serverEpoch), revision: Number(raw.revision) };
  if (!base.threadId || !base.tabId || !base.serverEpoch || !Number.isInteger(base.revision) || base.revision <= 0) return null;
  if (type === 'opened' || type === 'navigated' || type === 'resized') {
    const snapshot = readSnapshot(raw.snapshot);
    return snapshot ? { ...base, type, snapshot } : null;
  }
  if (type === 'failed') return { ...base, type, url: str(raw.url), title: str(raw.title), code: Number(raw.code) || 0, description: str(raw.description) };
  return type === 'closed' ? { ...base, type } : null;
}

/** rightPanelStore.openBrowser for the request's thread (the floating preview is part 3's; the panel shows it). */
function present(client: T3Client, ref: ScopedThreadRef, tabId: string): void {
  const store = surfaceStore(client), key = `${ref.environmentId}:${ref.threadId}`;
  let state = store.panels.get(key);
  if (!state) { state = { surfaces: [], active: '', visible: false, userRevision: 0 }; store.panels.set(key, state); }
  const active = activeRef(client);
  if (active && scopedThreadKey(active) === scopedThreadKey(ref)) client.diffOpen = false;
  openBrowserIn(state, tabId, scopedThreadKey(ref));
  client.revision++;
}
function suppressions(host: Host, threadId: string): Set<string> {
  let set = host.suppressed.get(threadId);
  if (!set) { set = new Set(); host.suppressed.set(threadId, set); }
  return set;
}

/** The tabs the module opened for `preview_open` (`presentation.browserAutomation.opened`): into the thread's
 *  store (applyPreviewServerSnapshot), shown or suppressed as the request asked. Runs before the panel reconciles. */
export function adoptAutomationTabs(client: T3Client): void {
  const host = automationHost(client), notes = Array.isArray(obj(obj(client.presentation).browserAutomation).opened) ? obj(obj(client.presentation).browserAutomation).opened as unknown[] : [];
  for (const raw of notes) {
    const note = obj(raw), key = `${str(note.connectionId)}\u0000${str(note.requestId)}`, snapshot = readSnapshot(note.snapshot);
    if (host.adopted.has(key) || !snapshot || str(note.environmentId) !== client.environmentId) continue;
    host.adopted.add(key);
    const ref = { environmentId: str(note.environmentId), threadId: str(note.threadId) }, store = browserHost(client).store;
    store.applyServerSnapshot(ref, snapshot);
    const runtimeId = previewRuntimeTabId(ref, store.read(ref).serverEpoch, snapshot.tabId);
    if (note.suppress === true) suppressions(host, ref.threadId).add(runtimeId);
    if (note.present === true) { suppressions(host, ref.threadId).delete(runtimeId); present(client, ref, snapshot.tabId); }
    client.revision++;
  }
}

async function subscribe(client: T3Client, native: Native, state: Stream, key: string, method: string, payload: Obj): Promise<void> {
  if (state.id || state.tried) return;
  state.tried = true; state.floor = state.maxSeen;
  try {
    const reply = await client.restAccess(native).call({ op: 'subscribe', key, method, payload });
    const serial = subscriptionSerial(str(reply.id));
    state.maxSeen = Math.max(state.maxSeen, serial);
    if (serial > state.floor && (!state.id || serial > subscriptionSerial(state.id))) state.id = str(reply.id);
  } catch (error) { state.tried = false; if (letGo(error)) throw error; }
}

/** PreviewAutomationHosts: the streams, the queued requests, the focus report. */
export async function automationPrepare(client: T3Client, native: Native): Promise<void> {
  if (!native.available || client.connection !== 'connected' || !client.ready || !client.environmentId) return;
  const host = automationHost(client);
  if (!host.clientId) {
    const [uuid] = await client.ids(native, 1);
    host.clientId ||= previewAutomationClientId(uuid ?? '');
  }
  await subscribe(client, native, host.events, PREVIEW_EVENTS_KEY, 'subscribePreviewEvents', {});
  await subscribe(client, native, host.automation, PREVIEW_AUTOMATION_KEY, 'previewAutomation.connect',
    { clientId: host.clientId, environmentId: client.environmentId, supportedOperations: [...PREVIEW_AUTOMATION_OPERATIONS] });
  await serveAutomation(client, native);
  await reportAutomationFocus(client, native);
}

/** Each queued request's plan, to the module once; a request whose time ran out is dropped (the broker timed it out). */
export async function serveAutomation(client: T3Client, native: Native): Promise<void> {
  const host = automationHost(client);
  for (const pending of [...host.queue]) {
    if (pending.sending) continue;
    if (Date.now() > pending.expires) { host.queue = host.queue.filter(entry => entry !== pending); continue; }
    pending.sending = true;
    try {
      const ref = { environmentId: client.environmentId, threadId: pending.request.threadId }, store = browserHost(client).store;
      if (!pending.listed && pending.request.operation !== 'recordingStop' && needsPreviewAutomationSessionSync(store.read(ref), pending.request.tabId)) {
        // registry.refresh(previewEnvironment.list): an authoritative list, within the request's host deadline.
        browserHost(client).listed.delete(scopedThreadKey(ref));
        await waitForHostReadiness(pending.deadline, async () => { await listPreviewSessions(client, native, ref); return true; });
        pending.listed = true;
      }
      const { plan, effects } = planRequest({
        request: pending.request, connectionId: pending.connectionId, clientId: host.clientId, environmentId: client.environmentId, environmentUrl: client.origin,
        generation: client.generation, state: store.read(ref), deadline: pending.deadline, suppressed: suppressions(host, ref.threadId), autoShowFloatingPreview: true,
      });
      if (effects.suppress) suppressions(host, ref.threadId).add(effects.suppress);
      if (effects.unsuppress) suppressions(host, ref.threadId).delete(effects.unsuppress);
      if (effects.present) present(client, ref, effects.present);
      const reply = await client.raw(native, { op: 'browserAutomation', plan });
      if (reply.ok) host.queue = host.queue.filter(entry => entry !== pending);
    } catch (error) { if (letGo(error)) throw error; }
    finally { pending.sending = false; }
  }
}

/** The host's focus and its live tabs (PreviewAutomationHost's focus effect), whenever they change. */
export async function reportAutomationFocus(client: T3Client, native: Native): Promise<void> {
  const host = automationHost(client), connectionId = host.consumer.activeConnectionId;
  if (!connectionId) return;
  const tabs = obj(obj(client.presentation).browserTabs), liveTabs: Array<{ threadId: string; tabId: string; visible: boolean }> = [];
  for (const [key, state] of browserHost(client).store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref || ref.environmentId !== client.environmentId) continue;
    for (const snapshot of Object.values(state.sessions)) {
      if (!state.desktopByTabId[snapshot.tabId]?.hasWebContents) continue;
      const tab = obj(tabs[previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId)]);
      liveTabs.push({ threadId: ref.threadId, tabId: snapshot.tabId, visible: tab.attached === true && host.windowFocused });
    }
  }
  const input = { clientId: host.clientId, environmentId: client.environmentId, connectionId, focused: host.windowFocused, liveTabs };
  const key = JSON.stringify(input);
  if (host.focusKey === key) return;
  host.focusKey = key;
  try { await client.rpc(native, 'previewAutomation.focusHost', input, true); }
  catch (error) { if (host.focusKey === key) host.focusKey = ''; if (letGo(error)) throw error; }
}

/** The module's per-tab automation facts (`presentation.browserAutomation.tabs`). */
export type AutomationTab = { audible: boolean; muted: boolean; colorScheme: 'system' | 'light' | 'dark'; controller: 'human' | 'agent' | 'none' };
export function automationTabs(client: T3Client): Record<string, AutomationTab> {
  const tabs: Record<string, AutomationTab> = {};
  for (const [id, raw] of Object.entries(obj(obj(obj(client.presentation).browserAutomation).tabs))) {
    const value = obj(raw), scheme = str(value.colorScheme), controller = str(value.controller);
    tabs[id] = { audible: value.audible === true, muted: value.muted === true, colorScheme: scheme === 'light' || scheme === 'dark' ? scheme : 'system',
      controller: controller === 'agent' || controller === 'human' ? controller : 'none' };
  }
  return tabs;
}
/** The overlay fields the module's automation state decides (usePreviewBridge's audio, appearance and controller). */
export function automationOverlay(client: T3Client, runtimeId: string): Partial<{ audioMuted: boolean; audible: boolean; colorScheme: AutomationTab['colorScheme']; controller: AutomationTab['controller'] }> {
  const tab = automationTabs(client)[runtimeId];
  return tab ? { audioMuted: tab.muted, audible: tab.audible, colorScheme: tab.colorScheme, controller: tab.controller } : {};
}
