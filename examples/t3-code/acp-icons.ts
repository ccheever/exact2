// ACP Registry instance icons, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/contracts/src/acpRegistry.ts:13-44 (officialAcpRegistryIconUrlForAgentId,
// resolveOfficialAcpRegistryIconUrl) and apps/web/src/components/chat/ProviderInstanceIcon.tsx:48-58
// (resolveProviderInstanceAcpRegistryIconUrl). Changes: the URL is drawn by Contract's `image`
// (provider-icons.contract `AcpRegistryAgentIcon`), which shows the ACP glyph until the image
// loads and keeps it when the load fails; AcpRegistryIcon.tsx's validating fetch, size cap and
// CacheStorage are the host's image loading (X44, main #177), so only the allow-list is ported.
import { obj, str, type Obj } from './domain';

const ACP_REGISTRY_CDN_HOSTNAME = 'cdn.agentclientprotocol.com';
const ACP_REGISTRY_AGENT_ID_PATTERN = /^[a-z0-9][a-z0-9._-]*$/;

/** Derives the official Registry icon URL without trusting stored URL metadata. */
export function officialAcpRegistryIconUrlForAgentId(agentId: string | null | undefined): string | null {
  if (agentId === null || agentId === undefined || !ACP_REGISTRY_AGENT_ID_PATTERN.test(agentId)) return null;
  return `https://${ACP_REGISTRY_CDN_HOSTNAME}/registry/v1/latest/${agentId}.svg`;
}

/** Allows provider icons only from the credential-free official Registry CDN origin. */
export function resolveOfficialAcpRegistryIconUrl(icon: string | null | undefined): string | null {
  if (!icon) return null;
  try {
    const url = new URL(icon);
    if (url.protocol !== 'https:' || url.hostname !== ACP_REGISTRY_CDN_HOSTNAME || url.port !== '' || url.username !== '' || url.password !== '') return null;
    return url.href;
  } catch { return null; }
}

export function resolveProviderInstanceAcpRegistryIconUrl(input: { driverKind: string; agentId?: string; iconUrl?: string }): string | null {
  if (input.driverKind !== 'acpRegistry') return null;
  return resolveOfficialAcpRegistryIconUrl(input.iconUrl ?? null) ?? officialAcpRegistryIconUrlForAgentId(input.agentId?.trim() || null);
}

/**
 * The icon URL a provider instance draws (ProviderInstanceCard's titleIconNode): its saved
 * `registryIconUrl`, else the agent id's official URL; '' for every driver but acpRegistry.
 */
export function instanceIconUrl(instance: Obj | undefined, driver = str(instance?.driver)): string {
  const config = obj(instance?.config);
  return resolveProviderInstanceAcpRegistryIconUrl({ driverKind: driver, agentId: str(config.agentId) || undefined, iconUrl: str(config.registryIconUrl) || undefined }) ?? '';
}

