// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
// Plain surface identity. The root favicon resource owns all reads and requests.
import { obj, str, type Obj } from './shared/domain';
import { mobileFaviconResourceKey } from './mobile-favicon-cache';

export interface MobileProjectFaviconTarget { key: string; environmentId: string; cwd: string; faviconPath: string }
export function mobileProjectFaviconTarget(environmentId: string, project?: Obj): MobileProjectFaviconTarget {
  const cwd = str(project?.workspaceRoot), faviconPath = str(project?.faviconPath);
  if (!environmentId || !cwd || str(obj(project?.projectIcon).kind)) return { key: '', environmentId: '', cwd: '', faviconPath: '' };
  const target = { environmentId, cwd, faviconPath };
  return { ...target, key: mobileFaviconResourceKey(target) };
}
