// ProviderStatusBanner's Refresh at T3 Code v0.1.7 (#24861, MIT, see LICENSE-T3): `server.refreshProviders` for the
// banner's instance with its models, one at a time, where the client may read the environment. The new status arrives
// on the providers stream; a failure is reported as useAtomCommand's are, not as a toast.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { can } from '../connections/command-permissions';
import { wakeShell } from '../client/command-waits';

const refreshing = new WeakSet<T3Client>();

export function providerBannerRefreshState(client: T3Client): { providerBannerCanRefresh: boolean; providerBannerRefreshing: boolean } {
  return { providerBannerCanRefresh: can(client, 'orchestration:read'), providerBannerRefreshing: refreshing.has(client) };
}

export async function refreshProviderBanner(client: T3Client, native: Native): Promise<void> {
  const instanceId = client.providerId;
  if (!instanceId || refreshing.has(client) || !can(client, 'orchestration:read')) return;
  refreshing.add(client);
  try {
    await wakeShell(native); // the button reads "Refreshing..." while the request runs
    await client.rpc(native, 'server.refreshProviders', { instanceId, refreshModels: true });
  } catch (error) {
    if (letGo(error)) throw error;
    console.warn('Refresh provider status failed', error);
  } finally { refreshing.delete(client); }
}
