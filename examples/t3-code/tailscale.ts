// Tailscale for "This machine" (20261005-this-machine-network-access item 2), after T3 Code (MIT,
// see LICENSE-T3; reference 1e2ecbd975): packages/tailscale/src/tailscale.ts (the pure parts) and
// apps/desktop/src/backend/tailscaleEndpointProvider.ts. The native module runs `tailscale status
// --json` (1.5 s limit, cached 60 s; T3LocalNetwork.swift) and probes the HTTPS endpoint; this file
// turns those facts into advertised endpoints. `tailscale serve` itself runs inside the embedded
// server (apps/server/src/server.ts), never in the app.
import { createAdvertisedEndpoint, type AdvertisedEndpoint, type AdvertisedEndpointProvider } from './advertised-endpoint';

export const DEFAULT_TAILSCALE_SERVE_PORT = 443;
export const TAILSCALE_STATUS_TIMEOUT_MS = 1_500;
export const TAILSCALE_PROBE_TIMEOUT_MS = 2_500;

/** The decoding failure; its message never quotes the CLI's output (it can carry auth keys). */
export class TailscaleStatusParseError extends Error {
  readonly _tag = 'TailscaleStatusParseError';
  constructor(readonly cause: unknown) { super('Failed to decode tailscale status JSON.'); }
}

export interface TailscaleStatus { magicDnsName: string | null; tailnetIpv4Addresses: string[] }

type StatusJson = { Self?: { DNSName?: unknown; TailscaleIPs?: unknown } };
function decodeStatus(raw: string): StatusJson {
  let parsed: unknown;
  try { parsed = JSON.parse(raw); } catch (cause) { throw new TailscaleStatusParseError(cause); }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) throw new TailscaleStatusParseError(new Error('not an object'));
  const self = (parsed as { Self?: unknown }).Self;
  if (self !== undefined && (typeof self !== 'object' || self === null || Array.isArray(self))) throw new TailscaleStatusParseError(new Error('Self is not an object'));
  return parsed as StatusJson;
}
function normalizeMagicDnsName(status: StatusJson): string | null {
  const dnsName = status.Self?.DNSName;
  if (typeof dnsName !== 'string') return null;
  const normalized = dnsName.trim().replace(/\.$/u, '');
  return normalized.length > 0 ? normalized : null;
}

/** The MagicDNS name of `tailscale status --json` (null when none); throws TailscaleStatusParseError. */
export function parseTailscaleMagicDnsName(rawStatusJson: string): string | null { return normalizeMagicDnsName(decodeStatus(rawStatusJson)); }

export function isTailscaleIpv4Address(address: string): boolean {
  const parts = address.split('.');
  if (parts.length !== 4) return false;
  const [first, second, third, fourth] = parts.map(part => Number.parseInt(part, 10));
  if (first === undefined || second === undefined || third === undefined || fourth === undefined
    || [first, second, third, fourth].some(part => !Number.isInteger(part) || part < 0 || part > 255)) return false;
  return first === 100 && second >= 64 && second <= 127;
}

export function parseTailscaleStatus(rawStatusJson: string): TailscaleStatus {
  const parsed = decodeStatus(rawStatusJson);
  const rawIps = parsed.Self?.TailscaleIPs;
  const tailnetIpv4Addresses: string[] = [];
  if (Array.isArray(rawIps)) for (const address of rawIps) if (typeof address === 'string' && isTailscaleIpv4Address(address)) tailnetIpv4Addresses.push(address);
  return { magicDnsName: normalizeMagicDnsName(parsed), tailnetIpv4Addresses };
}

export function buildTailscaleHttpsBaseUrl(input: { magicDnsName: string; servePort?: number }): string {
  const url = new URL(`https://${input.magicDnsName}`);
  const servePort = input.servePort ?? DEFAULT_TAILSCALE_SERVE_PORT;
  if (servePort !== DEFAULT_TAILSCALE_SERVE_PORT) url.port = String(servePort);
  url.pathname = '/';
  return url.toString();
}

// ── tailscaleEndpointProvider.ts ──────────────────────────────────────────
export type NetworkInterfaceInfo = { address: string; family: string | number; internal: boolean; netmask?: string; mac?: string; cidr?: string | null; scopeid?: number };
export type NetworkInterfaces = Readonly<Record<string, readonly NetworkInterfaceInfo[] | undefined>>;

const TAILSCALE_ENDPOINT_PROVIDER: AdvertisedEndpointProvider = { id: 'tailscale', label: 'Tailscale', kind: 'private-network', isAddon: true };

function resolveTailscaleIpAdvertisedEndpoints(input: { port: number; networkInterfaces: NetworkInterfaces }): AdvertisedEndpoint[] {
  const seen = new Set<string>();
  const endpoints: AdvertisedEndpoint[] = [];
  for (const interfaceAddresses of Object.values(input.networkInterfaces)) {
    if (!interfaceAddresses) continue;
    for (const address of interfaceAddresses) {
      if (address.internal || address.family !== 'IPv4' || !isTailscaleIpv4Address(address.address) || seen.has(address.address)) continue;
      seen.add(address.address);
      endpoints.push(createAdvertisedEndpoint({
        provider: TAILSCALE_ENDPOINT_PROVIDER, source: 'desktop-addon', id: `tailscale-ip:http://${address.address}:${input.port}`, label: 'Tailscale IP',
        httpBaseUrl: `http://${address.address}:${input.port}`, reachability: 'private-network', status: 'available',
        description: 'Reachable from devices on the same Tailnet.',
      }));
    }
  }
  return endpoints;
}

async function resolveTailscaleMagicDnsAdvertisedEndpoint(input: { dnsName: string | null; serveEnabled: boolean; servePort?: number;
  probe?: (baseUrl: string) => Promise<boolean> }): Promise<AdvertisedEndpoint | null> {
  if (!input.dnsName) return null;
  const httpBaseUrl = buildTailscaleHttpsBaseUrl({ magicDnsName: input.dnsName, ...(input.servePort === undefined ? {} : { servePort: input.servePort }) });
  const isReachable = input.serveEnabled ? await (input.probe ?? (async () => false))(httpBaseUrl).catch(() => false) : false;
  return createAdvertisedEndpoint({
    provider: TAILSCALE_ENDPOINT_PROVIDER, source: 'desktop-addon', id: `tailscale-magicdns:${httpBaseUrl}`, label: 'Tailscale HTTPS', httpBaseUrl,
    reachability: 'private-network', hostedHttpsCompatibility: isReachable ? 'compatible' : 'requires-configuration',
    status: isReachable ? 'available' : 'unavailable',
    description: isReachable ? 'HTTPS endpoint served by Tailscale Serve.' : 'MagicDNS hostname. Configure Tailscale Serve for HTTPS access.',
  });
}

/**
 * resolveTailscaleAdvertisedEndpoints: the Tailscale IPs from the interfaces, then the MagicDNS
 * HTTPS endpoint (available only when Serve is on and the probe answers). The MagicDNS name comes
 * from `statusJson` when given (null: none), else from `readMagicDnsName` (the native status read).
 */
export async function resolveTailscaleAdvertisedEndpoints(input: { port: number; serveEnabled?: boolean; servePort?: number; networkInterfaces: NetworkInterfaces;
  statusJson?: string | null; readMagicDnsName?: () => Promise<string | null>; probe?: (baseUrl: string) => Promise<boolean> }): Promise<AdvertisedEndpoint[]> {
  const ipEndpoints = resolveTailscaleIpAdvertisedEndpoints(input);
  const dnsName = input.statusJson === undefined
    ? await (input.readMagicDnsName ?? (async () => null))().catch(() => null)
    : input.statusJson ? (() => { try { return parseTailscaleMagicDnsName(input.statusJson); } catch { return null; } })() : null;
  const magicDnsEndpoint = await resolveTailscaleMagicDnsAdvertisedEndpoint({ dnsName, serveEnabled: input.serveEnabled === true,
    ...(input.servePort === undefined ? {} : { servePort: input.servePort }), ...(input.probe === undefined ? {} : { probe: input.probe }) });
  return magicDnsEndpoint ? [...ipEndpoints, magicDnsEndpoint] : ipEndpoints;
}
