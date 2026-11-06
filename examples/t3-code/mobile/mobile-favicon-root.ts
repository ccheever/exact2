// @ref examples/t3-code/mobile/docs/1130.009-mobile-settings.decision.md#offline-cache-storage
// Root-projected rows own demand. Native virtual-cell lifetimes are not exposed.
import { arr, obj, str } from './shared/domain';
import { noteNow, composerNow } from './shared/composer-controls';
import type { Native } from './shared/protocol';
import { mobileClient } from './client';
import { fleet } from './shared/settings-b-fleet';
import { mobileCacheCatalogIdentity, mobileCacheCatalogCurrent } from './mobile-client-cache-catalog';
import { mobileFaviconCatalogRevision } from './mobile-favicon-io';
import { mobilePrepareFavicons, mobileFaviconDisplay } from './mobile-favicon-runtime';
import { mobileFaviconImages } from './mobile-favicon-images';
import { mobileFaviconQueries, type MobileFaviconDemand } from './mobile-favicon-query';
import { mobileFaviconResourceKey } from './mobile-favicon-cache';
import { liveEnvironments } from './shared/live-streams';

let serial = 0;
let demands: MobileFaviconDemand[] = [];
let ownerKey = '', workKey = '', workRevision = 0, incarnation = 0;
let lifetimes = new Map<string, { incarnation: number; mounts: Set<string> }>();
const entryIds = new WeakMap<object, number>();
let entrySerial = 0;
function entryId(entry: object | undefined): number {
  if (!entry) return 0;
  let id = entryIds.get(entry);
  if (id === undefined) { id = ++entrySerial; entryIds.set(entry, id); }
  return id;
}
function scope(environmentId: string): string {
  return mobileFaviconCatalogRevision(environmentId, mobileCacheCatalogCurrent(mobileClient, fleet.saved, environmentId)
    ? mobileCacheCatalogIdentity(fleet.saved, environmentId) : '');
}
function imageInputs(rows: readonly MobileFaviconDemand[]) {
  return rows.map(target => ({ mountId: target.mountId, target, scopeRevision: scope(target.environmentId),
    url: mobileFaviconDisplay(mobileClient, target) ?? '' }));
}
function endpointsKey(): unknown[] {
  return [...new Set(fleet.saved.map(row => str(row.environmentId)).filter(Boolean))].sort().map(environmentId => {
    const focused = mobileClient.environmentId === environmentId;
    const entry = focused ? undefined : [...fleet.entries.values()].find(row => row.environmentId === environmentId);
    const saved = fleet.saved.filter(row => row.environmentId === environmentId);
    return [environmentId, scope(environmentId), saved.length === 1 && saved[0]!.enabled !== false, focused,
      focused ? mobileClient.origin : entry?.origin ?? '', focused ? mobileClient.generation : entry?.generation ?? -1,
      focused ? mobileClient.connection : entry?.phase ?? '', focused ? mobileClient.ready : entry?.synchronized === entry?.generation,
      entryId(entry)];
  });
}
// Retained query targets are private, so this is a deliberate work superset.
// Preparation still owns exact environment/cwd and first-clone eligibility.
function cloneWorkKey(): unknown[] {
  return liveEnvironments(mobileClient, null, fleet).map(environment => {
    const first = new Map<string, string>();
    for (const clone of environment.clones.value ?? []) {
      const cwd = str(clone.destinationPath);
      if (cwd && !first.has(cwd)) first.set(cwd, str(clone.phase));
    }
    return [environment.environmentId, [...first].sort(([a], [b]) => a.localeCompare(b))];
  }).sort((a, b) => str(a[0]).localeCompare(str(b[0])));
}
/** Surface IDs match the rendering components. A sidebar keeps its owner while
 * its detail route changes; removing a surface retires its image callbacks. */
export function mobileFaviconSurfaceDemands(args: readonly unknown[]): MobileFaviconDemand[] {
  const [route, routeId, homeVisible, sidebarVisible, home, sidebar, projects, archive, overview] = args;
  const result: MobileFaviconDemand[] = [];
  const add = (mountId: string, value: unknown) => {
    const target = obj(value);
    if (str(target.key) && str(target.environmentId) && str(target.cwd)) result.push({ mountId: mountId.startsWith('sidebar-list:') ? mountId : `${str(routeId)}:${mountId}`,
      environmentId: str(target.environmentId), cwd: str(target.cwd), faviconPath: str(target.faviconPath) || null });
  };
  if (homeVisible) for (const item of arr(home)) add(`home-list:${str(item.key)}`, item.faviconTarget);
  if (sidebarVisible) for (const item of arr(sidebar)) add(`sidebar-list:${str(item.key)}`, item.faviconTarget);
  if (route === 'newTask') for (const item of arr(projects)) add(`new-task:${str(item.id)}`, item.faviconTarget);
  if (route === 'archive') for (const item of arr(archive)) if (item.kind === 'project') add(`archive:${str(item.key)}`, item.faviconTarget);
  if (route === 'settingsProjectOverview') add('settings-project', obj(overview).faviconTarget);
  return result;
}
/** Synchronous resource publishes cached images and retires callbacks before
 * the separate awaited URL/image work. Its revision is an input to both views. */
export function mobileRootFaviconAdmission(args: readonly unknown[]) {
  noteNow(mobileClient, Number(args[9]));
  const next = mobileFaviconSurfaceDemands(args), nextLifetimes = new Map<string, { incarnation: number; mounts: Set<string> }>();
  for (const target of next) {
    const key = mobileFaviconResourceKey(target), row = nextLifetimes.get(key) ?? { incarnation: 0, mounts: new Set<string>() };
    row.mounts.add(target.mountId); nextLifetimes.set(key, row);
  }
  for (const [key, row] of nextLifetimes) {
    const previous = lifetimes.get(key);
    row.incarnation = previous && [...row.mounts].some(mount => previous.mounts.has(mount)) ? previous.incarnation : ++incarnation;
  }
  const continuing = next.filter(target => lifetimes.get(mobileFaviconResourceKey(target))?.mounts.has(target.mountId));
  // A replacement visit is a remount; a surviving sidebar keeps the shared query mounted.
  if (demands.some(target => !continuing.some(row => row.mountId === target.mountId
    && mobileFaviconResourceKey(row) === mobileFaviconResourceKey(target)))) mobileFaviconQueries.reconcile(continuing, composerNow(mobileClient));
  demands = next; lifetimes = nextLifetimes;
  const identity = JSON.stringify([[...lifetimes].map(([key, row]) => [key, row.incarnation]).sort(([a], [b]) => str(a).localeCompare(str(b))), endpointsKey()]);
  if (identity !== ownerKey) { ownerKey = identity; serial++; }
  mobileFaviconQueries.reconcile(demands, composerNow(mobileClient));
  const work = JSON.stringify([mobileFaviconQueries.version, cloneWorkKey()]);
  if (work !== workKey) { workKey = work; workRevision++; }
  const { revision } = mobileFaviconImages.reconcile(imageInputs(demands));
  return { revision, owner: serial, workRevision };
}
export async function mobileRootFavicons(request: number, native: Native | null | undefined) {
  const current = () => request === serial, captured = demands;
  if (!current()) return { revision: mobileFaviconImages.snapshot().revision, nextDeadline: 0 };
  const result = await mobilePrepareFavicons({ client: mobileClient, native, demands: captured,
    now: () => composerNow(mobileClient), current });
  if (current()) {
    mobileFaviconQueries.reconcile(demands, composerNow(mobileClient));
    mobileFaviconImages.reconcile(imageInputs(demands));
  }
  return { revision: mobileFaviconImages.snapshot().revision, nextDeadline: current() ? result.nextDeadline : 0 };
}
export function mobileRootFaviconImages() { return mobileFaviconImages.snapshot(); }
export function mobileRootFaviconEvent(mountId: string, requestKey: string, url: string, event: string) {
  // An explicit clear/catalog replacement invalidates callbacks even before the
  // next asynchronous root preparation gets its turn.
  mobileFaviconImages.reconcile(imageInputs(demands));
  if (event === 'load' || event === 'error') mobileFaviconImages.event(mountId, requestKey, url, event);
  return { revision: mobileFaviconImages.snapshot().revision, message: '' };
}
