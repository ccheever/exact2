// browser-surface part 5: the previewAutomation hosts, one per connected environment (MIT reference, see LICENSE-T3,
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
import { bridgeReply, ClientError, type Native } from './protocol';
import { EnvironmentFleet, fleet, type FleetEntry } from './settings-b-fleet';
import { letGo } from './let-go';
import { subscriptionSerial } from './shell-vcs';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';
import { activeRef } from './terminal-drawer-view';
import { surfaceStore } from './r4-surfaces-panel';
import { browserHost, listPreviewSessions, openBrowserIn } from './browser-surface';
import { browserAutoShowFloatingPreview } from './browser-capture';
import { EMPTY_THREAD_PREVIEW_STATE, previewRuntimeTabId, readSnapshot, type PreviewEvent, type PreviewSessionSnapshot } from './browser-state';
import {
  PREVIEW_AUTOMATION_OPERATIONS, PreviewAutomationRequestConsumer, needsPreviewAutomationSessionSync, planRequest, previewAutomationClientId,
  readStreamEvent, resolveHostWaitBudgetMs, type PreviewAutomationRequest,
} from './browser-automation-plan';

export const PREVIEW_AUTOMATION_KEY = 'preview-automation';
export const PREVIEW_EVENTS_KEY = 'preview-events';

/** One environment's connection the host serves (PreviewAutomationHosts mounts one host per environment): the focused
 *  client's, or a background environment's fleet transport (settings-b-fleet.ts). Its state lives with its owner. */
export type Link = {
  owner: object; environmentId: string; generation: number; environmentUrl: string;
  /** The fleet transport's key; null for the focused connection. The module answers on the same transport. */
  fleet: string | null;
  /** A transport op on this connection, generation-checked (subscribe, request). */
  call: (request: Obj) => Promise<Obj>;
  /** One request identifier from the connection's module (`ids`). */
  id: () => Promise<string>;
};
export const focusedLink = (client: T3Client, native: Native): Link => ({
  owner: client, environmentId: client.environmentId, generation: client.generation, environmentUrl: client.origin, fleet: null,
  call: request => client.restAccess(native).call(request),
  id: async () => (await client.ids(native, 1))[0] ?? '',
});
export function fleetLink(entry: FleetEntry, native: Native): Link {
  return { owner: entry, environmentId: entry.environmentId, generation: entry.generation, environmentUrl: entry.origin, fleet: entry.key,
    call: async request => {
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { ...request, generation: entry.generation });
      if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
      if (reply.generation !== entry.generation) throw new ClientError('The connection changed.', 'stale');
      return obj(reply.value);
    },
    id: async () => {
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'ids', count: 1 });
      return Array.isArray(reply.value) && typeof reply.value[0] === 'string' ? reply.value[0] : '';
    } };
}

type Stream = { id: string; floor: number; maxSeen: number; tried: boolean };
type Pending = { request: PreviewAutomationRequest; connectionId: string; listed: boolean; sending: boolean };
type Host = {
  clientId: string; generation: number; automation: Stream; events: Stream; consumer: PreviewAutomationRequestConsumer;
  queue: Pending[]; suppressed: Map<string, Set<string>>; focusKey: string;
  /** A fleet transport's entries, kept by the fleet's drain until the next shell answer serves them. */
  inbox: Obj[];
};
const hosts = new WeakMap<object, Host>();
/** The tabs the module opened that this client adopted (by connection and request), and the window's focus. */
const adopted = new WeakMap<T3Client, Set<string>>();
const windows = new WeakMap<T3Client, boolean>();
/** The client a background environment's events wake (the window has one client; app.ts). */
let wakes: T3Client | null = null;
const stream = (previous?: Stream): Stream => ({ id: '', floor: previous?.maxSeen ?? 0, maxSeen: previous?.maxSeen ?? 0, tried: false });
export function automationHost(owner: object, generation = (owner as { generation?: number }).generation ?? 0): Host {
  let host = hosts.get(owner);
  if (!host) {
    host = { clientId: '', generation, automation: stream(), events: stream(), consumer: new PreviewAutomationRequestConsumer(), queue: [], suppressed: new Map(), focusKey: '', inbox: [] };
    hosts.set(owner, host);
  }
  if (host.generation !== generation) {
    // A new connection: new subscriptions (serials only grow, so the high-water marks carry over); the queued
    // requests of the old one can no longer be answered there.
    host.generation = generation; host.automation = stream(host.automation); host.events = stream(host.events);
    host.consumer = new PreviewAutomationRequestConsumer(); host.queue = []; host.focusKey = ''; host.inbox = [];
  }
  return host;
}

/** shell.ts: the window's `exactPage()` focus and visibility (the reference's `document.hasFocus()` and visibility). */
export function noteAutomationWindow(client: T3Client, focused: boolean): void { windows.set(client, focused); }
const windowFocused = (client: T3Client) => windows.get(client) ?? true;

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
  streamEvent(client, client, client.environmentId, client.generation, entry);
}
/** settings-b-fleet.ts drain: a background environment's entries wait for the next shell answer (which has the client). */
export function automationFleetEvent(entry: FleetEntry, event: Obj): boolean {
  const key = str(event.key);
  if (key !== PREVIEW_AUTOMATION_KEY && key !== PREVIEW_EVENTS_KEY) return false;
  if (Number(event.generation) === entry.generation) { automationHost(entry, entry.generation).inbox.push(event); if (wakes) wakes.revision++; }
  return true;
}
function streamEvent(client: T3Client, owner: object, environmentId: string, generation: number, entry: Obj): void {
  const host = automationHost(owner, generation);
  if (str(entry.key) === PREVIEW_AUTOMATION_KEY) {
    const value = current(host.automation, entry), event = value ? readStreamEvent(value) : null;
    const request = event ? host.consumer.consume(event) : null;
    if (!request || !event || host.queue.some(pending => pending.request.requestId === request.requestId && pending.connectionId === event.connectionId)) return;
    host.queue.push({ request, connectionId: event.connectionId, listed: false, sending: false });
    client.revision++;
    return;
  }
  const value = current(host.events, entry), event = value ? readPreviewEvent(value) : null;
  if (!event || !environmentId) return;
  applyPreviewEvent(client, { environmentId, threadId: event.threadId }, event);
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
  let seen = adopted.get(client);
  if (!seen) { seen = new Set(); adopted.set(client, seen); }
  const notes = obj(obj(client.presentation).browserAutomation).opened;
  for (const raw of Array.isArray(notes) ? notes : []) {
    const note = obj(raw), key = `${str(note.connectionId)}\u0000${str(note.requestId)}`, snapshot = readSnapshot(note.snapshot);
    if (seen.has(key) || !snapshot || !str(note.environmentId)) continue;
    seen.add(key);
    const ref = { environmentId: str(note.environmentId), threadId: str(note.threadId) }, store = browserHost(client).store;
    const owner = str(note.fleet) ? fleet.entries.get(str(note.fleet)) ?? null : client;
    store.applyServerSnapshot(ref, snapshot);
    const runtimeId = previewRuntimeTabId(ref, store.read(ref).serverEpoch, snapshot.tabId);
    const suppressed = owner ? suppressions(automationHost(owner), ref.threadId) : new Set<string>();
    if (note.suppress === true) suppressed.add(runtimeId);
    if (note.present === true) { suppressed.delete(runtimeId); present(client, ref, snapshot.tabId); }
    client.revision++;
  }
}
/** The notes this client has adopted, of those the module still lists (it keeps 16): `browserSync` carries them, and
 *  the module answers an open only once its note is among them, so the agent's next request is planned with the tab
 *  adopted and its suppression recorded (T3BrowserAutomation `open`). */
export function adoptedAutomationNotes(client: T3Client): string[] {
  const seen = adopted.get(client), notes = obj(obj(client.presentation).browserAutomation).opened;
  if (!seen || !Array.isArray(notes)) return [];
  return notes.map(raw => { const note = obj(raw); return `${str(note.connectionId)}\u0000${str(note.requestId)}`; }).filter(key => seen.has(key));
}

async function subscribe(link: Link, state: Stream, key: string, method: string, payload: Obj): Promise<void> {
  if (state.id || state.tried) return;
  state.tried = true; state.floor = state.maxSeen;
  try {
    const reply = await link.call({ op: 'subscribe', key, method, payload });
    const serial = subscriptionSerial(str(reply.id));
    state.maxSeen = Math.max(state.maxSeen, serial);
    if (serial > state.floor && (!state.id || serial > subscriptionSerial(state.id))) state.id = str(reply.id);
  } catch (error) { state.tried = false; if (letGo(error)) throw error; }
}

/** PreviewAutomationHosts: one host per connected environment (the focused one and each background one), each with
 *  its streams, its queued requests and its focus report. */
export async function automationPrepare(client: T3Client, native: Native): Promise<void> {
  if (!native.available) return;
  wakes = client;
  const links: Link[] = [];
  if (client.connection === 'connected' && client.ready && client.environmentId) links.push(focusedLink(client, native));
  for (const entry of fleet.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || !entry.environmentId || entry.environmentId === client.environmentId) continue;
    const host = automationHost(entry, entry.generation), inbox = host.inbox;
    host.inbox = [];
    for (const event of inbox) streamEvent(client, entry, entry.environmentId, entry.generation, event);
    links.push(fleetLink(entry, native));
  }
  for (const link of links) {
    const host = automationHost(link.owner, link.generation);
    try {
      if (!host.clientId) {
        const id = await link.id();
        if (id) host.clientId ||= previewAutomationClientId(id);
      }
      if (!host.clientId) continue;
      await subscribe(link, host.events, PREVIEW_EVENTS_KEY, 'subscribePreviewEvents', {});
      await subscribe(link, host.automation, PREVIEW_AUTOMATION_KEY, 'previewAutomation.connect',
        { clientId: host.clientId, environmentId: link.environmentId, supportedOperations: [...PREVIEW_AUTOMATION_OPERATIONS] });
      await serveAutomation(client, native, link);
      await reportAutomationFocus(client, link);
    } catch (error) { if (letGo(error)) throw error; }
  }
}

/** `preview.list` on the request's connection, as an authoritative reconcile. */
async function listOn(client: T3Client, native: Native, link: Link, ref: ScopedThreadRef): Promise<void> {
  const host = browserHost(client);
  host.listed.delete(scopedThreadKey(ref));
  if (!link.fleet) return listPreviewSessions(client, native, ref);
  const result = await link.call({ op: 'request', method: 'preview.list', payload: { threadId: ref.threadId } });
  const sessions = (Array.isArray(result.sessions) ? result.sessions : []).map(readSnapshot).filter((entry): entry is PreviewSessionSnapshot => !!entry);
  if (str(result.serverEpoch)) host.store.reconcileServerSessions(ref, { sessions, serverEpoch: str(result.serverEpoch), revision: Number(result.revision) || 0 });
}

/** Each queued request's plan, to the module once (a data source has no clock: the module keeps the request's time). */
export async function serveAutomation(client: T3Client, native: Native, link: Link = focusedLink(client, native)): Promise<void> {
  const host = automationHost(link.owner, link.generation);
  for (const pending of [...host.queue]) {
    if (pending.sending) continue;
    pending.sending = true;
    try {
      const ref = { environmentId: link.environmentId, threadId: pending.request.threadId }, store = browserHost(client).store;
      if (!pending.listed && pending.request.operation !== 'recordingStop' && needsPreviewAutomationSessionSync(store.read(ref), pending.request.tabId)) {
        // registry.refresh(previewEnvironment.list): an authoritative list (the transport bounds the read).
        await listOn(client, native, link, ref);
        pending.listed = true;
      }
      const { plan, effects } = planRequest({
        request: pending.request, connectionId: pending.connectionId, clientId: host.clientId, environmentId: link.environmentId, environmentUrl: link.environmentUrl,
        generation: link.generation, state: store.read(ref), budgetMs: resolveHostWaitBudgetMs(pending.request.timeoutMs), suppressed: suppressions(host, ref.threadId), autoShowFloatingPreview: browserAutoShowFloatingPreview(client),
      });
      if (effects.suppress) suppressions(host, ref.threadId).add(effects.suppress);
      if (effects.unsuppress) suppressions(host, ref.threadId).delete(effects.unsuppress);
      if (effects.present) present(client, ref, effects.present);
      const reply = await client.raw(native, { op: 'browserAutomation', plan: link.fleet ? { ...plan, fleet: link.fleet } : plan });
      if (reply.ok) host.queue = host.queue.filter(entry => entry !== pending);
    } catch (error) { if (letGo(error)) throw error; }
    finally { pending.sending = false; }
  }
}

/** The host's focus and its live tabs (PreviewAutomationHost's focus effect), whenever they change. */
export async function reportAutomationFocus(client: T3Client, link: Link): Promise<void> {
  const host = automationHost(link.owner, link.generation), connectionId = host.consumer.activeConnectionId;
  if (!connectionId) return;
  const tabs = obj(obj(client.presentation).browserTabs), liveTabs: Array<{ threadId: string; tabId: string; visible: boolean }> = [], focused = windowFocused(client);
  for (const [key, state] of browserHost(client).store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref || ref.environmentId !== link.environmentId) continue;
    for (const snapshot of Object.values(state.sessions)) {
      if (!state.desktopByTabId[snapshot.tabId]?.hasWebContents) continue;
      const tab = obj(tabs[previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId)]);
      liveTabs.push({ threadId: ref.threadId, tabId: snapshot.tabId, visible: tab.attached === true && focused });
    }
  }
  const input = { clientId: host.clientId, environmentId: link.environmentId, connectionId, focused, liveTabs };
  const key = JSON.stringify(input);
  if (host.focusKey === key) return;
  host.focusKey = key;
  try { await link.call({ op: 'request', method: 'previewAutomation.focusHost', payload: input }); }
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
