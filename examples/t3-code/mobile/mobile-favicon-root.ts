// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
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

let serial = 0;
let demands: MobileFaviconDemand[] = [];
function scope(environmentId: string): string {
  return mobileFaviconCatalogRevision(environmentId, mobileCacheCatalogCurrent(mobileClient, fleet.saved, environmentId)
    ? mobileCacheCatalogIdentity(fleet.saved, environmentId) : '');
}
function imageInputs(rows: readonly MobileFaviconDemand[]) {
  return rows.map(target => ({ mountId: target.mountId, target, scopeRevision: scope(target.environmentId),
    url: mobileFaviconDisplay(mobileClient, target) ?? '' }));
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
  const revision = ++serial;
  noteNow(mobileClient, Number(args[9]));
  demands = mobileFaviconSurfaceDemands(args);
  mobileFaviconQueries.reconcile(demands, Number(args[9]));
  mobileFaviconImages.reconcile(imageInputs(demands));
  return { revision };
}
export async function mobileRootFavicons(request: number, epoch: number, native: Native | null | undefined) {
  const current = () => request === serial, captured = demands;
  if (!current()) return { revision: request, nextDeadline: 0 };
  noteNow(mobileClient, epoch);
  const result = await mobilePrepareFavicons({ client: mobileClient, native, demands: captured,
    now: () => composerNow(mobileClient), current });
  if (current()) mobileFaviconImages.reconcile(imageInputs(captured));
  return { revision: request, nextDeadline: result.nextDeadline };
}
export function mobileRootFaviconImages() { return mobileFaviconImages.snapshot(); }
export function mobileRootFaviconEvent(mountId: string, requestKey: string, url: string, event: string) {
  // An explicit clear/catalog replacement invalidates callbacks even before the
  // next asynchronous root preparation gets its turn.
  mobileFaviconImages.reconcile(imageInputs(demands));
  if (event === 'load' || event === 'error') mobileFaviconImages.event(mountId, requestKey, url, event);
  return { revision: mobileFaviconImages.snapshot().revision, message: '' };
}
