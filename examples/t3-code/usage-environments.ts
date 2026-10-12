// The Usage page across environments (lane usage-pooled-view), T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3): apps/web/src/state/usage.ts (EnvironmentUsageStatus, useUsage, mergeAnsweredUsage),
// apps/web/src/components/usage/UsagePage.tsx (the environment filter, refreshWindow, refreshLimits,
// the automatic limits refresh, the limits clock `limitsNow`, CursorEnableButton) and
// UsageLimits.tsx useResetCredit for a pooled segment. Changes:
// - the environments are the focused connection (T3Client) and every background environment of
//   EnvironmentFleet; one that is not connected and synchronized is listed with its phase and
//   contributes nothing (the reference's presentations atom);
// - React state and effects are one per-client state stepped by each Usage page answer
//   (`prepareUsage`): the time is the answer's `now` (X19), a press of refresh is a changed counter,
//   the automatic limits check runs when Limits shows or the connected set changes;
// - every per-environment request (summary, rates, provider check, redeem) is detached: it is queued,
//   sent by the next answer on that environment's transport (composer-replies.ts for the focused
//   one, usage-replies.ts for a background one), and its reply settles in the drain that hears it,
//   so a slow environment never holds the window or the other environments.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { fleet as appFleet, type EnvironmentFleet } from './settings-b-fleet';
import { startDetached, type DetachedReply } from './composer-replies';
import { onUsageFleetChange, settleFleetWaiters, startFleetDetached } from './usage-replies';
import { needsCursorKeychainAccess, refreshUsage, refreshUsageLimits, sessionUnavailable, type UsageRefreshPort } from './usage-refresh';
import { REDEEM_IDLE, redeemStep, type RedeemEvent, type RedeemState } from './reset-credits';
import type { LimitAccount } from './usage-limits-pools';

export type UsageWindowInput = { sinceDay: string; untilDay: string; timeZone: string; resolution: 'day' | 'hour'; sinceTime?: string; untilTime?: string };
const PHASE_TEXT: Record<string, string> = { available: 'Disconnected', connecting: 'Connecting…', reconnecting: 'Reconnecting…', error: 'Unavailable', unsupported: 'Update required',
  disconnected: 'Disconnected', connected: 'Synchronizing…' };

/** One environment the page can read: its config while connected and synchronized, else its phase. */
export type UsageEnvironment = {
  readonly id: string; readonly label: string; readonly connected: boolean; readonly phase: string;
  readonly config: Obj | null; readonly focused: boolean; readonly fleet: string; readonly generation: number;
  /** The transport is connected (a stream failure may still be resynchronizing it): its replies still come. */
  readonly live: boolean;
};

/** The focused environment, then each background one, in the catalog's order. */
export function usageEnvironments(client: T3Client, source: EnvironmentFleet = appFleet): UsageEnvironment[] {
  const list: UsageEnvironment[] = [];
  if (client.environmentId) {
    const connected = client.connection === 'connected' && client.ready;
    list.push({ id: client.environmentId, label: str(obj(client.config.environment).label, 'This environment'), connected,
      phase: connected ? 'connected' : client.connection, config: connected ? client.config : null, focused: true, fleet: '', generation: client.generation,
      live: client.connection === 'connected' });
  }
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || entry.environmentId === client.environmentId || list.some(item => item.id === entry.environmentId)) continue;
    const connected = entry.phase === 'connected' && entry.synchronized === entry.generation;
    const saved = source.saved.find(item => str(item.environmentId) === entry.environmentId);
    const label = str(obj(entry.config.environment).label) || str(saved?.label) || (entry.primary ? 'This machine' : entry.origin.replace(/^\w+:\/\//, ''));
    list.push({ id: entry.environmentId, label, connected, phase: connected ? 'connected' : entry.phase, config: connected ? entry.config : null,
      focused: false, fleet: entry.key, generation: entry.generation, live: entry.phase === 'connected' });
  }
  return list;
}

type Summary = { windowKey: string; summary: Obj | null; error: string; pending: boolean; stale: boolean; token: number; generation: number; readAt: number };
type Outgoing = { environmentId: string; method: string; payload: Obj; resolve: (reply: DetachedReply) => void };
export type UsageState = {
  selected: Set<string> | null; envs: UsageEnvironment[]; clock: number;
  open: boolean; metric: string; windowDays: number; window: UsageWindowInput | null; refreshSeen: number | null;
  summaries: Map<string, Summary>; outbox: Outgoing[]; listeners: Map<string, Set<() => void>>;
  /** refreshingRef / isRefreshing: one press at a time, for either view. */
  refreshing: boolean;
  limitsNow: number; autoKey: string;
  /**
   * The redeem of each account's window (useResetCredit lives in that segment's popover, keyed by the
   * account in the reference), and the confirm that is open: the segment it was asked from and the
   * target captured then, so a redraw that reorders the accounts never redirects a credit.
   */
  redeems: Map<string, RedeemState>;
  confirm: string;
  asked: { redeemKey: string; target: NonNullable<LimitAccount['redeem']> } | null;
  /** The segments last drawn: each one's account, redeem key and name, by its (positional) id. */
  segments: Map<string, { account: LimitAccount; redeemKey: string; name: string }>;
  cursorPending: Set<string>;
  /** The external usage pages last drawn: the only addresses `open` may hand to the system. */
  links: string[];
};
const states = new WeakMap<object, UsageState>();
export function usageState(client: object): UsageState {
  let state = states.get(client);
  if (!state) states.set(client, state = { selected: null, envs: [], clock: 0, open: false, metric: '', windowDays: 0, window: null, refreshSeen: null,
    summaries: new Map(), outbox: [], listeners: new Map(), refreshing: false, limitsNow: 0, autoKey: '', redeems: new Map(), confirm: '', asked: null, segments: new Map(), cursorPending: new Set(), links: [] });
  return state;
}
/** Tests and a closed page start over. */
export function forgetUsageState(client: object): void { states.delete(client); }

export const isSelected = (state: UsageState, id: string) => state.selected === null || state.selected.has(id);
/** The environments whose data the page reads: selected, connected and synchronized. */
export const selectedConnected = (state: UsageState) => state.envs.filter(env => env.connected && isSelected(state, env.id));

/**
 * UsageEnvironmentFilter: "All environments" checks or clears every one; an environment toggles
 * alone, and a selection that covers every listed environment is "all" again.
 */
export function toggleEnvironment(state: UsageState, id: string): void {
  if (!id) { state.selected = state.selected === null ? new Set() : null; return; }
  const listed = state.envs.filter(env => env.connected);
  const next = new Set(listed.filter(env => isSelected(state, env.id)).map(env => env.id));
  if (next.has(id)) next.delete(id); else next.add(id);
  state.selected = next.size === listed.length ? null : next;
}

// ── Requests ───────────────────────────────────────────────────────────────

/** Queues a request for an environment; the next answer sends it (`flush`). */
export function request(state: UsageState, environmentId: string, method: string, payload: Obj): Promise<DetachedReply> {
  return new Promise(resolve => { state.outbox.push({ environmentId, method, payload, resolve }); });
}
const disconnected = (): DetachedReply => ({ ok: false, error: new ClientError('The environment is not connected.', 'Disconnected'), interrupted: true });
const turns = async (count = 8) => { for (let index = 0; index < count; index++) await Promise.resolve(); };

/** Sends the queued requests on their transports, letting each continuation queue the next (bounded). */
export async function flush(client: T3Client, native: Native, state: UsageState): Promise<void> {
  for (let pass = 0; pass < 6; pass++) {
    // A refresh queues its request a turn after it starts (refreshUsageLimits runs `refresh` in a then).
    await turns();
    if (!state.outbox.length) break;
    const batch = state.outbox.splice(0);
    await Promise.all(batch.map(async item => {
      const env = state.envs.find(candidate => candidate.id === item.environmentId);
      // The focus may have moved since the page last drew: a request never goes to another environment.
      const moved = env?.focused && (client.environmentId !== env.id || client.generation !== env.generation);
      if (!env?.connected || moved) { item.resolve(disconnected()); return; }
      const sent = env.focused ? await startDetached(client, native, item.method, item.payload)
        : await startFleetDetached(native, { key: env.fleet, generation: env.generation }, item.method, item.payload);
      void sent.reply.then(item.resolve);
    }));
  }
}

// ── Summaries (useUsage) ───────────────────────────────────────────────────

export const windowKeyOf = (window: UsageWindowInput) => JSON.stringify([window.sinceDay, window.untilDay, window.timeZone, window.resolution, window.sinceTime ?? '', window.untilTime ?? '']);
function windowPayload(window: UsageWindowInput): Obj {
  const payload: Obj = { sinceDay: window.sinceDay, untilDay: window.untilDay, timeZone: window.timeZone, resolution: window.resolution };
  if (window.sinceTime) { payload.sinceTime = window.sinceTime; payload.untilTime = window.untilTime; }
  return payload;
}
function summaryEntry(state: UsageState, id: string): Summary | null {
  if (!state.window) return null;
  const windowKey = windowKeyOf(state.window);
  let entry = state.summaries.get(id);
  if (!entry || entry.windowKey !== windowKey) state.summaries.set(id, entry = { windowKey, summary: null, error: '', pending: false, stale: true, token: 0, generation: -1, readAt: 0 });
  return entry;
}
/** A summary read for the current window; a read that an invalidation or a newer read replaced is dropped. */
function readSummary(state: UsageState, id: string): Promise<void> {
  const entry = summaryEntry(state, id);
  if (!entry || !state.window) return Promise.resolve();
  const token = ++entry.token;
  entry.pending = true; entry.stale = false;
  entry.generation = state.envs.find(env => env.id === id)?.generation ?? -1;
  return request(state, id, 'server.getUsageSummary', windowPayload(state.window)).then(reply => {
    if (entry.token !== token || state.summaries.get(id) !== entry) return;
    entry.pending = false;
    if (reply.ok) { entry.summary = reply.value; entry.error = ''; entry.readAt = state.clock; }
    else if (reply.interrupted) entry.stale = true; // read again once the environment is back
    else entry.error = 'This environment could not report usage.';
  });
}
/**
 * Every selected connected environment reads the current window once (the per-window query atoms).
 * As the reference's query depends on the session, a reconnected environment reads again (its last
 * summary and error give way to the new read); `reopened` is the query's 60 s stale time on a remount.
 */
function ensureSummaries(state: UsageState, reopened: boolean): void {
  for (const env of selectedConnected(state)) {
    const entry = summaryEntry(state, env.id);
    if (entry && !entry.pending && (entry.generation !== env.generation && (entry.summary || entry.error)
      || reopened && entry.summary && state.clock - entry.readAt > 60_000)) { entry.stale = true; entry.error = ''; }
    if (entry?.stale && !entry.pending) void readSummary(state, env.id);
  }
}

/** Prices or mappings changed: every summary reads again; the last one shows until then ("Refreshing…"). */
export function forgetSummaries(state: UsageState): void {
  for (const entry of state.summaries.values()) { entry.token++; entry.pending = false; entry.stale = true; }
}

/** EnvironmentUsageStatus for each listed environment with what it reported for the current window. */
export type EnvironmentUsageStatus = { environmentId: string; label: string; isPending: boolean; error: string | null; summary: Obj | null; needsCursorKeychainAccess: boolean; connected: boolean; phase: string };
export function environmentStatuses(state: UsageState): EnvironmentUsageStatus[] {
  const windowKey = state.window ? windowKeyOf(state.window) : '';
  return state.envs.map(env => {
    const entry = state.summaries.get(env.id);
    const current = entry && entry.windowKey === windowKey && env.connected ? entry : null;
    return { environmentId: env.id, label: env.label, connected: env.connected, phase: env.phase,
      isPending: !current || current.pending || current.stale, error: current?.error || null, summary: current?.summary ?? null,
      needsCursorKeychainAccess: needsCursorKeychainAccess(current?.summary ?? null, env.config ? arr(env.config.providers) : null) };
  });
}
export const phaseText = (phase: string) => PHASE_TEXT[phase] ?? 'Disconnected';

// ── Refreshes ──────────────────────────────────────────────────────────────

function connectivityChanged(state: UsageState): void {
  for (const [, listeners] of state.listeners) for (const listener of [...listeners]) listener();
}
/** refreshUsage's environment operations over this page's queue and summary reads. */
export function usagePort(state: UsageState): UsageRefreshPort {
  return {
    refreshRates: id => request(state, id, 'server.refreshUsageRates', {})
      .then(reply => reply.ok ? { ok: true as const } : { ok: false as const, sessionUnavailable: reply.interrupted || sessionUnavailable(reply.error.kind) }),
    invalidate: id => { const entry = state.summaries.get(id); if (entry) { entry.token++; entry.pending = false; entry.stale = true; } },
    read: (id, signal) => new Promise<void>(resolve => { signal.onabort = () => resolve(); void readSummary(state, id).then(resolve); }),
    connected: id => state.envs.some(env => env.id === id && env.connected),
    subscribe: (id, listener) => {
      let listeners = state.listeners.get(id);
      if (!listeners) state.listeners.set(id, listeners = new Set());
      listeners.add(listener);
      return () => { listeners!.delete(listener); };
    },
  };
}
/** refreshLimits: `server.refreshProviders` per selected connected environment (refreshUsageLimits), then the clock renews. */
export async function refreshLimits(state: UsageState, automatic = false, afterPending = false): Promise<void> {
  try {
    await Promise.all(selectedConnected(state).filter(env => env.config !== null).map(env =>
      refreshUsageLimits(env.id, () => request(state, env.id, 'server.refreshProviders', {}), () => state.clock, automatic, afterPending)));
  } finally {
    state.limitsNow = state.clock;
  }
}
/** refreshWindow: Limits re-checks the providers; cost and tokens move the window to now and rescan with fresh prices. */
function pressRefresh(state: UsageState, limits: boolean, now: number, makeWindow: (days: number, now: number) => UsageWindowInput): void {
  if (state.refreshing) return;
  state.refreshing = true;
  const done = () => { state.refreshing = false; };
  if (limits) { void refreshLimits(state).finally(done); return; }
  const next = makeWindow(state.windowDays, now);
  if (!state.window || windowKeyOf(next) !== windowKeyOf(state.window)) state.window = next;
  void refreshUsage(usagePort(state), state.envs.filter(env => isSelected(state, env.id)).map(env => env.id)).finally(done);
}

export type UsageAnswer = { open: boolean; metric: string; windowDays: number; refresh: number; now: number };
/**
 * One Usage page answer: adopt the environments and the clock, settle what a stopped environment
 * will never answer, run the refreshes the page's effects would, read the window, send the queue.
 */
export async function prepareUsage(client: T3Client, native: Native | null | undefined, input: UsageAnswer,
  makeWindow: (days: number, now: number) => UsageWindowInput, source: EnvironmentFleet = appFleet): Promise<UsageState> {
  const state = usageState(client);
  state.clock = Math.max(state.clock, input.now);
  const before = state.envs.map(env => `${env.id}:${env.connected}:${env.generation}`).join(',');
  state.envs = usageEnvironments(client, source);
  if (state.envs.map(env => `${env.id}:${env.connected}:${env.generation}`).join(',') !== before) connectivityChanged(state);
  settleFleetWaiters((key, generation) => state.envs.some(env => env.fleet === key && env.live && env.generation === generation));
  onUsageFleetChange(input.open ? () => { client.revision++; } : null);
  if (!input.open) { state.open = false; state.autoKey = ''; state.confirm = ''; state.asked = null; return state; }
  const opening = !state.open, limits = input.metric === 'limits';
  state.open = true;
  // selectMetric("limits") and the first mount read the time; the countdown advances only on refresh after that.
  if (limits && (opening || state.metric !== 'limits')) state.limitsNow = input.now;
  if (opening || !state.window || state.windowDays !== input.windowDays) { state.windowDays = input.windowDays; state.window = makeWindow(input.windowDays, input.now); }
  if (state.refreshSeen === null || opening) state.refreshSeen = input.refresh;
  else if (input.refresh !== state.refreshSeen) { state.refreshSeen = input.refresh; pressRefresh(state, limits, input.now, makeWindow); }
  state.metric = input.metric;
  // The automatic check: when Limits shows, and again whenever the connected selected set changes.
  const key = limits ? selectedConnected(state).filter(env => env.config !== null).map(env => env.id).sort().join(',') : '';
  if (key && key !== state.autoKey) void refreshLimits(state, true);
  state.autoKey = key;
  ensureSummaries(state, opening);
  if (native?.available) await flush(client, native, state);
  await turns();
  return state;
}

// ── Commands (pageslocal:usage-pool-*) ─────────────────────────────────────

/** consumeResetCredit's reply as the redeem's next step: a typed server error says its message. */
function redeemEvent(reply: DetachedReply): RedeemEvent {
  if (reply.ok) return { type: 'done', outcome: str(reply.value.outcome), ...(str(reply.value.warning) ? { warning: str(reply.value.warning) } : {}) };
  const typed = !reply.interrupted && ['ProviderSetupError', 'UsageLimitSourceError', 'EnvironmentAuthorizationError'].includes(reply.error.kind);
  return { type: 'failed', message: typed ? reply.error.message : null };
}

/**
 * The pooled view's commands: the environment filter (`env`), "Use reset" in a segment's popover
 * (`reset-ask`), the confirm's Cancel or Escape (`reset-cancel`) and "Use credit" (`reset-confirm`),
 * and Cursor's Keychain offer (`cursor`). A confirm returns the focus to its segment.
 */
export async function usagePoolLocal(client: T3Client, native: Native | null | undefined, op: string, id: string): Promise<string> {
  const state = usageState(client);
  if (op === 'env') { toggleEnvironment(state, id); return ''; }
  const segmentFocus = (segment: string) => (segment ? `focus:usage-seg-${segment}` : '');
  // The segment that now draws an account's window (the order can change while a confirm is open).
  const segmentOf = (redeemKey: string, fallback: string) => [...state.segments].find(([, entry]) => entry.redeemKey === redeemKey)?.[0] ?? fallback;
  if (op === 'reset-ask') {
    const shown = state.segments.get(id);
    if (!shown?.account.redeem || state.redeems.get(shown.redeemKey)?.busy) return '';
    state.redeems.set(shown.redeemKey, redeemStep(state.redeems.get(shown.redeemKey) ?? REDEEM_IDLE, { type: 'ask' }));
    state.confirm = id; state.asked = { redeemKey: shown.redeemKey, target: shown.account.redeem };
    return 'focus:reset-credit-cancel';
  }
  if (op === 'reset-cancel') {
    const asked = state.asked, segment = asked ? segmentOf(asked.redeemKey, state.confirm || id) : state.confirm || id;
    if (asked) state.redeems.set(asked.redeemKey, redeemStep(state.redeems.get(asked.redeemKey) ?? REDEEM_IDLE, { type: 'cancel' }));
    state.confirm = ''; state.asked = null;
    return segmentFocus(segment);
  }
  if (op === 'reset-confirm') {
    const asked = state.asked, segment = asked ? segmentOf(asked.redeemKey, state.confirm || id) : state.confirm || id;
    state.confirm = ''; state.asked = null;
    if (!asked) return segmentFocus(segment);
    const current = state.redeems.get(asked.redeemKey) ?? REDEEM_IDLE;
    if (current.busy) { state.redeems.set(asked.redeemKey, redeemStep(current, { type: 'cancel' })); return segmentFocus(segment); }
    state.redeems.set(asked.redeemKey, redeemStep(current, { type: 'start' }));
    void request(state, asked.target.environmentId, 'provider.consumeResetCredit', asked.target.input as Obj)
      .then(reply => { state.redeems.set(asked.redeemKey, redeemStep(state.redeems.get(asked.redeemKey) ?? REDEEM_IDLE, redeemEvent(reply))); });
    if (native?.available) await flush(client, native, state);
    return segmentFocus(segment);
  }
  if (op === 'open') {
    // ExternalUsage "Manage usage": shell.openExternal of a link the view showed (a failure is ignored, as `void` there).
    if (native?.available && state.links.includes(id)) await bridgeReply(native, { op: 'remoteEditorsOpen', url: id }).catch(() => undefined);
    return '';
  }
  if (op === 'cursor') {
    const env = state.envs.find(candidate => candidate.id === id && candidate.connected);
    if (!env || !native?.available || state.cursorPending.has(id)) return '';
    if (env.focused && (client.environmentId !== env.id || client.generation !== env.generation)) return '';
    state.cursorPending.add(id);
    try {
      // CursorEnableButton: updateSettings on that environment, then refresh usage and limits.
      const payload = { patch: { cursorKeychainUsageEnabled: true } };
      if (env.focused) await client.rpc(native, 'server.updateSettings', payload, true);
      else {
        const remote: Native = { available: native.available, watch: topic => native.watch(topic), later: request => native.later({ ...obj(request), fleet: env.fleet }) };
        const reply = await bridgeReply(remote, { op: 'request', method: 'server.updateSettings', payload, generation: env.generation });
        if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
        if (reply.generation !== env.generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
      }
      void rescanUsage(state);
      void refreshLimits(state, false, true);
      await flush(client, native, state);
    } finally { state.cursorPending.delete(id); }
    return '';
  }
  throw new ClientError(`Unknown usage action: ${op}`);
}
/** onEnabled's `refresh()`: the summaries rescan with fresh prices (refreshUsage), outside the button's busy state. */
export function rescanUsage(state: UsageState): Promise<void> {
  return refreshUsage(usagePort(state), state.envs.filter(env => isSelected(state, env.id)).map(env => env.id));
}

/** num() of a summary's contract version, for the menu's "Update required". */
export const contractVersionOf = (summary: Obj | null) => num(summary?.contractVersion);
