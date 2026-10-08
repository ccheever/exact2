// Pairing links for "This machine" (20261005-this-machine-network-access item 5), after T3 Code
// (MIT, see LICENSE-T3; reference 1e2ecbd975): components/settings/pairingUrls.ts,
// apps/web/src/hostedPairing.ts buildHostedPairingUrl, ConnectionsSettings.logic.ts
// (isQrShareableEndpoint, selectQrEndpointOption) and ConnectionsSettings.tsx
// (selectPairingEndpoint, endpointDefaultPreferenceKey, endpointShareHint).
//  - An HTTP endpoint pairs directly: `<endpoint>/pair#token=<credential>`.
//  - An HTTPS endpoint pairs through the hosted web app (decision U8, 2026-10-08: as the reference,
//    checked line by line): `https://app.t3.codes/pair?host=<endpoint>[&label=]#token=`. The
//    hosted app connects to the endpoint itself; nothing goes through T3 Connect.
//  - A loopback endpoint never gets a QR code: a device scanning it would dial itself.
// The credential lives only in the URL's fragment, never in a query string the server logs.
import type { AdvertisedEndpoint } from './advertised-endpoint';

/** DEFAULT_HOSTED_APP_URL (packages/shared/src/connectAuth.ts). */
export const DEFAULT_HOSTED_APP_URL = 'https://app.t3.codes';

/** setPairingTokenOnUrl (packages/shared/src/remote.ts): the token as `#token=`. */
export function setPairingTokenOnUrl(url: URL, credential: string): URL {
  const next = new URL(url.toString());
  next.searchParams.delete('token');
  next.hash = new URLSearchParams([['token', credential]]).toString();
  return next;
}

export function buildHostedPairingUrl(input: { host: string; token: string; label?: string | null }, hostedAppUrl = DEFAULT_HOSTED_APP_URL): string {
  const url = new URL('/pair', hostedAppUrl);
  url.searchParams.set('host', input.host);
  const label = input.label?.trim();
  if (label) url.searchParams.set('label', label);
  return setPairingTokenOnUrl(url, input.token).toString();
}

export function resolveDesktopPairingUrl(endpointUrl: string, credential: string): string {
  const url = new URL(endpointUrl);
  url.pathname = '/pair';
  return setPairingTokenOnUrl(url, credential).toString();
}

export function resolveHostedPairingUrl(endpointUrl: string, credential: string, hostedAppUrl = DEFAULT_HOSTED_APP_URL): string | null {
  const url = new URL(endpointUrl);
  if (url.protocol !== 'https:') return null;
  return buildHostedPairingUrl({ host: endpointUrl, token: credential }, hostedAppUrl);
}

export function isHostedAppPairingUrl(value: string): boolean {
  try { const url = new URL(value); return url.pathname === '/pair' && url.searchParams.has('host'); } catch { return false; }
}

/** A QR code encoding a loopback URL makes the scanning device dial itself. */
export function isQrShareableEndpoint(endpoint: AdvertisedEndpoint): boolean {
  return endpoint.status !== 'unavailable' && endpoint.reachability !== 'loopback';
}

export type QrEndpointOption = { id: string; preferenceKey: string; qrShareable: boolean };
/**
 * The share panel's endpoint: the user's pick, else the saved default, else the first QR-shareable
 * option (never a loopback QR first), else the first option. A stale pick falls back.
 */
export function selectQrEndpointOption<T extends QrEndpointOption>(options: readonly T[], selectedId: string | null, defaultPreferenceKey: string | null): T | null {
  return (selectedId !== null ? options.find(option => option.id === selectedId) : undefined)
    ?? (defaultPreferenceKey !== null ? options.find(option => option.preferenceKey === defaultPreferenceKey) : undefined)
    ?? options.find(option => option.qrShareable) ?? options[0] ?? null;
}

export const isTailscaleHttpsEndpoint = (endpoint: AdvertisedEndpoint): boolean => endpoint.id.startsWith('tailscale-magicdns:');

/** The default endpoint's key, stable per endpoint type (t3-code.json `defaultAdvertisedEndpointKey`). */
export function endpointDefaultPreferenceKey(endpoint: AdvertisedEndpoint): string {
  if (endpoint.id.startsWith('desktop-loopback:')) return 'desktop-core:loopback:http';
  if (endpoint.id.startsWith('desktop-lan:')) return 'desktop-core:lan:http';
  if (endpoint.id.startsWith('tailscale-ip:')) return 'tailscale:ip:http';
  if (isTailscaleHttpsEndpoint(endpoint)) return 'tailscale:magicdns:https';
  let scheme = 'unknown';
  try { scheme = new URL(endpoint.httpBaseUrl).protocol.replace(/:$/u, ''); } catch { /* a malformed custom endpoint keeps a stable key */ }
  return `${endpoint.provider.id}:${endpoint.reachability}:${scheme}:${endpoint.label}`;
}

/** selectPairingEndpoint: the saved default when available, else the endpoint marked default, a non-loopback one, or a hosted-HTTPS one. */
export function selectPairingEndpoint(endpoints: readonly AdvertisedEndpoint[], defaultEndpointKey?: string | null): AdvertisedEndpoint | null {
  const available = endpoints.filter(endpoint => endpoint.status !== 'unavailable');
  if (defaultEndpointKey) {
    const selected = available.find(endpoint => endpointDefaultPreferenceKey(endpoint) === defaultEndpointKey);
    if (selected) return selected;
  }
  return available.find(endpoint => endpoint.isDefault) ?? available.find(endpoint => endpoint.reachability !== 'loopback')
    ?? available.find(endpoint => endpoint.compatibility.hostedHttpsApp === 'compatible') ?? null;
}

export function resolveAdvertisedEndpointPairingUrl(endpoint: AdvertisedEndpoint, credential: string): string {
  if (endpoint.compatibility.hostedHttpsApp === 'compatible') return resolveHostedPairingUrl(endpoint.httpBaseUrl, credential) ?? resolveDesktopPairingUrl(endpoint.httpBaseUrl, credential);
  return resolveDesktopPairingUrl(endpoint.httpBaseUrl, credential);
}

export function endpointShareHint(endpoint: AdvertisedEndpoint, url: string): string {
  if (isHostedAppPairingUrl(url)) return 'Opens the hosted app, no install needed';
  switch (endpoint.reachability) {
    case 'lan': return 'Devices on the same network';
    case 'private-network': return 'Devices on your private network';
    case 'public': return 'Reachable from anywhere';
    case 'loopback': return 'Clients on this machine';
  }
}
