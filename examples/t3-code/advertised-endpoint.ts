// Advertised endpoints (20261005-this-machine-network-access item 2), after T3 Code (MIT, see
// LICENSE-T3; reference 1e2ecbd975): packages/shared/src/advertisedEndpoint.ts and the
// AdvertisedEndpoint contract (packages/contracts/src/environment.ts). An endpoint is one address
// this Mac's embedded server can be reached at: its loopback, its LAN address, a Tailscale IP or
// MagicDNS name, or a user-configured URL. Pairing links and QR codes are built from them.

export type AdvertisedEndpointReachability = 'loopback' | 'lan' | 'private-network' | 'public';
export type AdvertisedEndpointHostedHttpsCompatibility = 'compatible' | 'mixed-content-blocked' | 'requires-configuration' | 'unknown';
export type AdvertisedEndpointSource = 'desktop-core' | 'desktop-addon' | 'user' | 'server';
export type AdvertisedEndpointStatus = 'available' | 'unavailable' | 'unknown';
export type AdvertisedEndpointProvider = { id: string; label: string; kind: string; isAddon: boolean };

export interface AdvertisedEndpoint {
  id: string;
  label: string;
  provider: AdvertisedEndpointProvider;
  httpBaseUrl: string;
  wsBaseUrl: string;
  reachability: AdvertisedEndpointReachability;
  compatibility: { hostedHttpsApp: AdvertisedEndpointHostedHttpsCompatibility; desktopApp: 'compatible' | 'unknown' };
  source: AdvertisedEndpointSource;
  status: AdvertisedEndpointStatus;
  isDefault?: boolean;
  description?: string;
}

export interface CreateAdvertisedEndpointInput {
  id: string;
  label: string;
  provider: AdvertisedEndpointProvider;
  httpBaseUrl: string;
  reachability: AdvertisedEndpointReachability;
  hostedHttpsCompatibility?: AdvertisedEndpointHostedHttpsCompatibility;
  desktopCompatibility?: 'compatible' | 'unknown';
  source: AdvertisedEndpointSource;
  status?: AdvertisedEndpointStatus;
  isDefault?: boolean;
  description?: string;
}

export function normalizeHttpBaseUrl(rawValue: string): string {
  const url = new URL(rawValue);
  if (url.protocol === 'ws:') url.protocol = 'http:';
  else if (url.protocol === 'wss:') url.protocol = 'https:';
  if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error(`Endpoint must use HTTP or HTTPS. Received ${url.protocol}`);
  url.pathname = '/';
  url.search = '';
  url.hash = '';
  return url.toString();
}

export function deriveWsBaseUrl(httpBaseUrl: string): string {
  const url = new URL(normalizeHttpBaseUrl(httpBaseUrl));
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

export function classifyHostedHttpsCompatibility(httpBaseUrl: string, fallback: AdvertisedEndpointHostedHttpsCompatibility = 'unknown'): AdvertisedEndpointHostedHttpsCompatibility {
  const url = new URL(normalizeHttpBaseUrl(httpBaseUrl));
  if (url.protocol === 'http:') return 'mixed-content-blocked';
  return fallback === 'mixed-content-blocked' ? 'unknown' : fallback;
}

export function createAdvertisedEndpoint(input: CreateAdvertisedEndpointInput): AdvertisedEndpoint {
  const httpBaseUrl = normalizeHttpBaseUrl(input.httpBaseUrl);
  return {
    id: input.id,
    label: input.label,
    provider: input.provider,
    httpBaseUrl,
    wsBaseUrl: deriveWsBaseUrl(httpBaseUrl),
    reachability: input.reachability,
    compatibility: {
      hostedHttpsApp: input.hostedHttpsCompatibility ?? classifyHostedHttpsCompatibility(httpBaseUrl),
      desktopApp: input.desktopCompatibility ?? 'compatible',
    },
    source: input.source,
    status: input.status ?? 'available',
    ...(input.isDefault === undefined ? {} : { isDefault: input.isDefault }),
    ...(input.description === undefined ? {} : { description: input.description }),
  };
}
