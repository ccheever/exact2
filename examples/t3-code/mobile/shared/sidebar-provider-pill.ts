// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/sidebar-provider-pill.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// SidebarProviderUpdatePill (components/sidebar/SidebarProviderUpdatePill.tsx,
// ProviderUpdateLaunchNotification.logic.ts getProviderUpdateSidebarPillView;
// MIT, see LICENSE-T3): above the footer icons while a provider updates, then
// its outcome — failed and still-outdated notices until dismissed, a success
// for three seconds. Outcomes older than the window's first provider check
// never show; a dismissal holds until that provider's status changes (key).
import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';

export interface ProviderPill { key: string; tone: 'loading' | 'success' | 'warning' | 'error'; title: string; description: string; dismissible: boolean; dismissAfterMs: number }

const NAMES: Record<string, string> = { antigravity: 'Antigravity', codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor', grok: 'Grok', acpRegistry: 'ACP Registry', pi: 'Pi', opencode: 'OpenCode' };
export const SUCCESS_VISIBLE_MS = 3_000;
const name = (provider: Obj) => NAMES[str(provider.driver)] ?? str(provider.driver);
const version = (value: string) => value.startsWith('v') ? value : `v${value}`;
const status = (provider: Obj) => str(obj(provider.updateState).status);
const finishedAt = (provider: Obj): string | null => { const at = obj(provider.updateState).finishedAt; return typeof at === 'string' ? at : null; };
function list(providers: Obj[]): string {
  const names = providers.map(name);
  return names.length <= 2 ? names.join(' and ') : `${names.slice(0, -1).join(', ')}, and ${names[names.length - 1]}`;
}
/** dedupeProvidersByDriver: the default instance (id == driver) represents its driver, else the latest check. */
function byDriver(providers: Obj[]): Obj[] {
  const latest = new Map<string, Obj>();
  for (const provider of providers) {
    const driver = str(provider.driver), current = latest.get(driver);
    if (!current || str(provider.instanceId) === driver) { latest.set(driver, provider); continue; }
    if (str(current.instanceId) === driver) continue;
    if (str(provider.checkedAt).localeCompare(str(current.checkedAt)) >= 0) latest.set(driver, provider);
  }
  return [...latest.values()];
}
const tail = (providers: Obj[]) => providers.map(provider => `${str(provider.driver)}:${finishedAt(provider) ?? 'pending'}:${str(obj(provider.updateState).message)}`).sort().join('|');
const latestFinished = (providers: Obj[]) => providers.reduce<string | null>((latest, provider) => { const at = finishedAt(provider); return at !== null && (latest === null || at > latest) ? at : latest; }, null);

export function providerPillView(providers: Obj[], visibleAfter: string | undefined, dismissed: ReadonlySet<string>): ProviderPill | null {
  const deduped = byDriver(providers);
  const active = deduped.filter(provider => ['queued', 'running'].includes(status(provider)));
  if (active.length > 0) {
    return { key: `loading:${active.map(provider => `${str(provider.driver)}:${status(provider) || 'idle'}`).sort().join('|')}`, tone: 'loading',
      title: active.length === 1 ? `Updating ${name(active[0]!)}` : `Updating ${active.length} providers`,
      description: active.length === 1 ? `${list(active)} update in progress.` : `${list(active)} updates are in progress.`, dismissible: false, dismissAfterMs: 0 };
  }
  const recent = deduped.filter(provider => ['failed', 'unchanged', 'succeeded'].includes(status(provider))
    && (visibleAfter === undefined || (finishedAt(provider) !== null && finishedAt(provider)! >= visibleAfter)));
  const failed = recent.filter(provider => status(provider) === 'failed');
  const unchanged = recent.filter(provider => status(provider) === 'unchanged');
  const succeeded = recent.filter(provider => status(provider) === 'succeeded');
  const candidates: { view: ProviderPill; at: string }[] = [];
  if (failed.length) {
    const first = failed[0]!, attempted = str(obj(first.versionAdvisory).latestVersion);
    candidates.push({ at: latestFinished(failed) ?? '', view: { key: `failed:${tail(failed)}`, tone: 'error',
      title: failed.length === 1 ? (attempted ? `${name(first)} ${version(attempted)} update failed` : `${name(first)} update failed`) : `${failed.length} provider updates failed`,
      description: failed.length === 1 && str(obj(first.updateState).message) ? str(obj(first.updateState).message) : `${list(failed)} failed to update. Check provider settings for details.`,
      dismissible: true, dismissAfterMs: 0 } });
  }
  if (unchanged.length) {
    candidates.push({ at: latestFinished(unchanged) ?? '', view: { key: `unchanged:${tail(unchanged)}`, tone: 'warning',
      title: unchanged.length === 1 ? `${name(unchanged[0]!)} still needs an update` : `${unchanged.length} providers still need updates`,
      description: `${list(unchanged)} ${unchanged.length === 1 ? 'still appears' : 'still appear'} outdated. Review provider settings for details.`,
      dismissible: true, dismissAfterMs: 0 } });
  }
  if (succeeded.length) {
    const first = succeeded[0]!, current = str(first.version);
    candidates.push({ at: latestFinished(succeeded) ?? '', view: { key: `succeeded:${tail(succeeded)}`, tone: 'success',
      title: succeeded.length === 1 ? (current ? `${name(first)} updated: ${version(current)}` : `${name(first)} updated`) : `${succeeded.length} providers updated`,
      description: succeeded.length === 1 ? 'New sessions will use the updated provider.' : 'New sessions will use the updated providers.',
      dismissible: false, dismissAfterMs: SUCCESS_VISIBLE_MS } });
  }
  return candidates.sort((left, right) => right.at.localeCompare(left.at)).map(entry => entry.view).find(view => !dismissed.has(view.key)) ?? null;
}

/** The window's pill: the first provider check bounds what is recent; a success leaves after three seconds. */
const states = new WeakMap<T3Client, { visibleAfter?: string; dismissed: Set<string>; shown: Map<string, number>; scheduled: Set<string> }>();
function pillState(client: T3Client) {
  let state = states.get(client);
  if (!state) { state = { dismissed: new Set(), shown: new Map(), scheduled: new Set() }; states.set(client, state); }
  return state;
}
export function sidebarProviderPill(client: T3Client, now: number): ProviderPill | null {
  const providers = arr(client.config.providers), state = pillState(client);
  if (state.visibleAfter === undefined && providers.length) {
    state.visibleAfter = providers.reduce<string | undefined>((latest, provider) => latest === undefined || str(provider.checkedAt) > latest ? str(provider.checkedAt) : latest, undefined);
  }
  const view = providerPillView(providers, state.visibleAfter, state.dismissed);
  if (!view || !view.dismissAfterMs) return view;
  const shown = state.shown.get(view.key);
  if (shown === undefined) { state.shown.set(view.key, now); return view; }
  if (now - shown >= view.dismissAfterMs) { state.dismissed.add(view.key); return sidebarProviderPill(client, now); }
  return view;
}
/** After each refresh (client.ts): a timed pill asks the module to repaint once its time is up. */
export function scheduleProviderPill(client: T3Client, now: number, notify: (delay: number) => void): void {
  const view = sidebarProviderPill(client, now), state = pillState(client);
  if (!view || !view.dismissAfterMs || state.scheduled.has(view.key)) return;
  state.scheduled.add(view.key);
  notify(Math.max(0, view.dismissAfterMs - (now - (state.shown.get(view.key) ?? now))) + 50);
}
export function dismissProviderPill(client: T3Client, key: string): void { if (key) pillState(client).dismissed.add(key); }
