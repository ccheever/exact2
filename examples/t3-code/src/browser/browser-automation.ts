// The previewAutomation hosts, one per connected environment (MIT reference, see LICENSE-T3,
// T3 Code 9c161462c9: apps/web/src/components/preview/PreviewAutomationHosts.tsx, usePreviewSession.ts (74541bab17 for
// `useDesktopHostedPreviewSessions`),
// previewAutomationRequestConsumer.ts; packages/contracts/src/rpc.ts `previewAutomation.connect`, `.respond`,
// `.focusHost`, `subscribePreviewEvents`).
//
// A server that reports the `serverBrowser` capability runs the agent's browser itself and has no
// previewAutomation RPCs (9c161462c9, ac8e9453ca): its link subscribes to preview events only, so no
// request reaches this client's web views. Agent control on such a server is not ported.
//
// The streams are the transport's subscriptions, drained by
// client.ts into `previewStreamEvent`: a request joins this client's queue, a preview event updates the thread's
// preview store (an agent-opened or agent-navigated tab appears). `automationPrepare` (browserPrepare, every shell
// answer) subscribes, plans each queued request once (browser-automation-plan.ts) and queues it as slot work for the
// executor (browser-automation-run.ts), which runs it in one of the window's queued sends, so a request outlives the
// data-source answer that planned it, and answers `previewAutomation.respond` itself. It also adopts the tabs the
// executor opened, keeps the pages' media and diagnostics read (`automationTick`) and reports the window's focus and
// live tabs (`previewAutomation.focusHost`).
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { EnvironmentFleet, fleet, type FleetEntry } from '../connections/settings-b-fleet';
import { letGo } from '../client/let-go';
import { subscriptionSerial } from '../details/shell-vcs';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from '../terminal/terminal-ui-state';
import { activeRef } from '../terminal/terminal-drawer-view';
import { surfaceStore } from '../panel/panel-surfaces';
import { browserHost, listPreviewSessions, openBrowserIn, syncNativeSessions } from './browser-surface';
import { automationAudio, automationControllerOf, automationTick, frameVisible, runAutomation } from './browser-automation-run';
import { queueSlotWork } from './browser-automation-recording';
import { resolveBrowserOpenDefaults } from './browser-defaults';
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
  /** The server runs the agent's browser itself (`environment.capabilities.serverBrowser`). */
  serverBrowser: boolean;
};
const runsServerBrowser = (config: Obj | undefined) => obj(obj(obj(config).environment).capabilities).serverBrowser === true;
export const focusedLink = (client: T3Client, native: Native): Link => ({
  owner: client, environmentId: client.environmentId, generation: client.generation, environmentUrl: client.origin, fleet: null,
  call: request => client.restAccess(native).call(request),
  id: async () => (await client.ids(native, 1))[0] ?? '',
  serverBrowser: runsServerBrowser(client.config),
});
export function fleetLink(entry: FleetEntry, native: Native): Link {
  return { owner: entry, environmentId: entry.environmentId, generation: entry.generation, environmentUrl: entry.origin, fleet: entry.key, serverBrowser: runsServerBrowser(entry.config),
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
/** A tab the executor opened (`preview_open`), for the host to adopt. */
type OpenedNote = { connectionId: string; requestId: string; environmentId: string; threadId: string; fleet: string; snapshot: unknown; present: boolean; suppress: boolean };
type Host = {
  clientId: string; generation: number; automation: Stream; events: Stream; consumer: PreviewAutomationRequestConsumer;
  queue: Pending[]; suppressed: Map<string, Set<string>>; focusKey: string;
  /** The requests queued for the executor, by connection and id, newest last (at most 64): a request delivered again
   *  runs once. */
  answered: string[];
  /** The plans queued, newest last (at most 16): what a drive's state and the tests read. */
  planned: Obj[];
  /** A fleet transport's entries, kept by the fleet's drain until the next shell answer serves them. */
  inbox: Obj[];
  /** Threads a preview event made known, whose earlier tabs the next prepare lists (useDesktopHostedPreviewSessions). */
  unlisted: ScopedThreadRef[];
};
const hosts = new WeakMap<object, Host>();
/** The tabs the executor opened, not yet adopted, and the window's focus. */
const opened = new WeakMap<T3Client, OpenedNote[]>();
const windows = new WeakMap<T3Client, boolean>();
/** The client a background environment's events wake (the window has one client; app.ts). */
let wakes: T3Client | null = null;
const stream = (previous?: Stream): Stream => ({ id: '', floor: previous?.maxSeen ?? 0, maxSeen: previous?.maxSeen ?? 0, tried: false });
export function automationHost(owner: object, generation = (owner as { generation?: number }).generation ?? 0): Host {
  let host = hosts.get(owner);
  if (!host) {
    host = { clientId: '', generation, automation: stream(), events: stream(), consumer: new PreviewAutomationRequestConsumer(), queue: [], suppressed: new Map(), focusKey: '', answered: [], planned: [], inbox: [], unlisted: [] };
    hosts.set(owner, host);
  }
  if (host.generation !== generation) {
    // A new connection: new subscriptions (serials only grow, so the high-water marks carry over); the queued
    // requests of the old one can no longer be answered there.
    host.generation = generation; host.automation = stream(host.automation); host.events = stream(host.events);
    host.consumer = new PreviewAutomationRequestConsumer(); host.queue = []; host.focusKey = ''; host.inbox = []; host.unlisted = [];
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
    if (!request || !event || host.queue.some(pending => pending.request.requestId === request.requestId && pending.connectionId === event.connectionId)
      || host.answered.includes(`${event.connectionId}\u0000${request.requestId}`)) return;
    host.queue.push({ request, connectionId: event.connectionId, listed: false, sending: false });
    client.revision++;
    return;
  }
  const value = current(host.events, entry), event = value ? readPreviewEvent(value) : null;
  if (!event || !environmentId) return;
  const ref = { environmentId, threadId: event.threadId };
  applyPreviewEvent(client, ref, event, () => host.unlisted.push(ref));
}

/** usePreviewSession's event handling: an event of another server process asks for the list again. Events count for
 *  threads this client keeps preview state for and the shown one, and any event but `closed` makes its thread one of
 *  them (useDesktopHostedPreviewSessions, 74541bab17): an agent's tab in a background thread gets its web view
 *  (`liveSessions`) without that thread's chat being open. */
export function applyPreviewEvent(client: T3Client, ref: ScopedThreadRef, event: PreviewEvent, discovered?: () => void): boolean {
  const store = browserHost(client).store, state = store.read(ref), active = activeRef(client);
  const seen = state !== EMPTY_THREAD_PREVIEW_STATE || (active && scopedThreadKey(active) === scopedThreadKey(ref));
  if (!seen && event.type === 'closed') return false;
  // A thread this event makes known is listed too, so its tabs from before the event get their web views.
  if (!seen) discovered?.();
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

/** rightPanelStore.openBrowser for the request's thread (the floating preview is browser-capture.ts's; the panel shows it). */
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

/** The tabs the executor opened for `preview_open`: into the thread's store (applyPreviewServerSnapshot), shown or
 *  suppressed as the request asked, each with its frame. Runs before the panel reconciles, and at once from the run. */
export function adoptAutomationTabs(client: T3Client): void {
  const notes = opened.get(client)?.splice(0) ?? [];
  for (const note of notes) {
    const snapshot = readSnapshot(note.snapshot);
    if (!snapshot || !note.environmentId) continue;
    const ref = { environmentId: note.environmentId, threadId: note.threadId }, store = browserHost(client).store;
    const owner = note.fleet ? fleet.entries.get(note.fleet) ?? null : client;
    store.applyServerSnapshot(ref, snapshot);
    const runtimeId = previewRuntimeTabId(ref, store.read(ref).serverEpoch, snapshot.tabId);
    const suppressed = owner ? suppressions(automationHost(owner), ref.threadId) : new Set<string>();
    if (note.suppress) suppressed.add(runtimeId);
    if (note.present) { suppressed.delete(runtimeId); present(client, ref, snapshot.tabId); }
    client.revision++;
  }
  if (notes.length) syncNativeSessions(client);
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
      for (const ref of host.unlisted.splice(0)) {
        // A let-go answer leaves the thread for the next prepare; a failed list keeps what the events gave it.
        try { await listOn(client, native, link, ref); } catch (error) { if (letGo(error)) { host.unlisted.push(ref); throw error; } }
      }
      if (link.serverBrowser) continue;
      await subscribe(link, host.automation, PREVIEW_AUTOMATION_KEY, 'previewAutomation.connect',
        { clientId: host.clientId, environmentId: link.environmentId, supportedOperations: [...PREVIEW_AUTOMATION_OPERATIONS] });
      await serveAutomation(client, native, link);
      await reportAutomationFocus(client, link);
    } catch (error) { if (letGo(error)) throw error; }
  }
  await automationTick(client);
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

/** Each queued request, planned once and queued for the executor, which runs it in one of the window's queued sends on
 *  that send's native (the request's own connection: the focused one, or its fleet transport). */
export async function serveAutomation(client: T3Client, native: Native, link: Link = focusedLink(client, native)): Promise<void> {
  if (link.serverBrowser) return;
  const host = automationHost(link.owner, link.generation);
  for (const pending of [...host.queue]) {
    if (pending.sending) continue;
    pending.sending = true;
    try {
      const ref = { environmentId: link.environmentId, threadId: pending.request.threadId }, store = browserHost(client).store;
      if (!pending.listed && needsPreviewAutomationSessionSync(store.read(ref), pending.request.tabId)) {
        // registry.refresh(previewEnvironment.list): an authoritative list (the transport bounds the read).
        await listOn(client, native, link, ref);
        pending.listed = true;
      }
      // Part 4: the configured defaults, refused (left queued) while the settings are unread, as the reference's
      // resolveBrowserDefaults refuses the request.
      const defaults = resolveBrowserOpenDefaults(client);
      const { plan, effects } = planRequest({
        request: pending.request, connectionId: pending.connectionId, clientId: host.clientId, environmentId: link.environmentId, environmentUrl: link.environmentUrl,
        generation: link.generation, state: store.read(ref), budgetMs: resolveHostWaitBudgetMs(pending.request.timeoutMs), suppressed: suppressions(host, ref.threadId),
        autoShowFloatingPreview: defaults.autoShowFloatingPreview, openDefaults: { viewport: defaults.viewport, profileId: defaults.profileId },
      });
      if (effects.suppress) suppressions(host, ref.threadId).add(effects.suppress);
      if (effects.unsuppress) suppressions(host, ref.threadId).delete(effects.unsuppress);
      if (effects.present) present(client, ref, effects.present);
      const key = `${pending.connectionId}\u0000${pending.request.requestId}`, fleetKey = link.fleet;
      queueSlotWork(client, `automation\u0000${fleetKey ?? ''}\u0000${key}`, async slotNative => {
        const entry = fleetKey ? fleet.entries.get(fleetKey) : undefined;
        if (fleetKey && !entry) return; // the environment went: its request cannot be answered there
        const on = entry ? fleetLink(entry, slotNative) : focusedLink(client, slotNative);
        await runAutomation(client, (fleetKey ? { ...plan, fleet: fleetKey } : plan) as unknown as Obj, {
          native: slotNative, rpc: (method, payload) => on.call({ op: 'request', method, payload }), origin: on.environmentUrl,
          adopt: note => {
            const notes = opened.get(client) ?? [];
            notes.push({ connectionId: pending.connectionId, requestId: pending.request.requestId, environmentId: link.environmentId, threadId: pending.request.threadId, fleet: fleetKey ?? '', ...note });
            opened.set(client, notes);
            adoptAutomationTabs(client);
          },
        });
      });
      host.planned.push((fleetKey ? { ...plan, fleet: fleetKey } : plan) as unknown as Obj);
      if (host.planned.length > 16) host.planned.splice(0, host.planned.length - 16);
      host.answered.push(key);
      if (host.answered.length > 64) host.answered.splice(0, host.answered.length - 64);
      host.queue = host.queue.filter(entry => entry !== pending);
    } catch (error) { if (letGo(error)) throw error; }
    finally { pending.sending = false; }
  }
}

/** The host's focus and its live tabs (PreviewAutomationHost's focus effect), whenever they change. */
export async function reportAutomationFocus(client: T3Client, link: Link): Promise<void> {
  if (link.serverBrowser) return;
  const host = automationHost(link.owner, link.generation), connectionId = host.consumer.activeConnectionId;
  if (!connectionId) return;
  const liveTabs: Array<{ threadId: string; tabId: string; visible: boolean }> = [], focused = windowFocused(client);
  for (const [key, state] of browserHost(client).store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref || ref.environmentId !== link.environmentId) continue;
    for (const snapshot of Object.values(state.sessions)) {
      if (!state.desktopByTabId[snapshot.tabId]?.hasWebContents) continue;
      liveTabs.push({ threadId: ref.threadId, tabId: snapshot.tabId, visible: frameVisible(client, previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId)) && focused });
    }
  }
  const input = { clientId: host.clientId, environmentId: link.environmentId, connectionId, focused, liveTabs };
  const key = JSON.stringify(input);
  if (host.focusKey === key) return;
  host.focusKey = key;
  try { await link.call({ op: 'request', method: 'previewAutomation.focusHost', payload: input }); }
  catch (error) { if (host.focusKey === key) host.focusKey = ''; if (letGo(error)) throw error; }
}

/** The overlay fields the executor decides (usePreviewBridge's audio and controller). */
export function automationOverlay(client: T3Client, runtimeId: string): Partial<{ audioMuted: boolean; audible: boolean; controller: 'human' | 'agent' | 'none' }> {
  return { ...(automationAudio(client, runtimeId) ?? {}), controller: automationControllerOf(client, runtimeId) };
}

export { adoptFramePrompt, answerFramePrompt, toggleFrameMute } from './browser-automation-run';
