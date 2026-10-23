// The client each host identifies as (llp/0001 §2): IDENTIFY's `properties` and `capabilities`, and
// the REST headers (`User-Agent`, `X-Super-Properties`). Every value is in `IDENTITIES` with where it
// came from; Charlie can check the desktop ones against what his own Chrome sends discord.com. Per-launch ids come from the caller (a data module has no randomness of its own).
export type Host = 'ios' | 'macos' | 'windows' | 'web' | 'android';

export interface Sourced<T> {
  value: T;
  source: string;
  asOf: string;
}

// A recollection is dated by when it was current, not when it was written down.
function memory<T>(value: T, description: string): Sourced<T> {
  return { value, source: `from memory, unverified; ${description}`, asOf: '2025-01' };
}

function research<T>(value: T, row: string): Sourced<T> {
  return { value, source: `LLP 0002 (llp/0002-client-identity.research.md), ${row}`, asOf: '2026-10' };
}

const protocol = <T>(value: T) => memory(value, 'Discord user gateway/header examples');
const web = <T>(value: T) => memory(value, 'Discord Chrome super-properties examples; exact build pairing uncertain');
const mobileWeb = <T>(value: T) => memory(value, "Discord's web client in a phone's browser; its browser names as Discord derives them from the User-Agent, uncertain");
const bundle = <T>(value: T) => research(value, 'X-Super-Properties: web.2ae8383b003d87ae.js field construction');
// Chrome's User-Agent as the browser reports it, its version and platform reduced to fixed values.
function reduced<T>(value: T): Sourced<T> {
  return { value, source: 'Chromium User-Agent Reduction (https://www.chromium.org/updates/ua-reduction/), Chrome 148 per LLP 0002', asOf: '2026-10' };
}

interface Chrome {
  os: string;
  osVersion: string;
  userAgent: string | null;
  // In a browser the browser's own; the native hosts' is what they send.
  browserUserAgent?: string;
  browser?: string;
  browserVersion?: string;
  // Chrome on iOS does not reduce its User-Agent: its values are recalled, not derived.
  unreduced?: true;
}

/** The web client's identity in Chrome on one OS: the super-properties of web.2ae8383b003d87ae.js. */
function chrome(c: Chrome) {
  const ua = c.browserUserAgent ?? c.userAgent ?? '';
  const phone = c.browser !== undefined;
  const named = phone ? mobileWeb : web;
  const agent = c.unreduced ? mobileWeb : reduced;
  return {
    properties: {
      os: named(c.os),
      browser: named(c.browser ?? 'Chrome'),
      device: web(''),
      device_model_name: bundle(''),
      system_locale: web('en-US'),
      has_client_mods: web(false),
      browser_user_agent: agent(ua),
      browser_version: agent(c.browserVersion ?? '148.0.0.0'),
      os_version: agent(c.osVersion),
      referrer: web(''),
      referring_domain: web(''),
      referrer_current: web(''),
      referring_domain_current: web(''),
      release_channel: web('stable'),
      client_build_number: research(647615, 'Versions / Web client: GLOBAL_ENV in discord.com/app'),
      client_event_source: bundle(null),
      client_app_state: bundle('focused'),
    },
    sessionProperties: {
      client_launch_id: bundle('launchId'),
      launch_signature: bundle('launchSignature'),
      client_heartbeat_session_id: bundle('heartbeatSessionId'),
      client_app_state: bundle('appState'),
    },
    capabilities: web(30717),
    userAgent: agent(c.userAgent),
  };
}

// LLP 0002 supplies the researched pins; unverified values still need the host's own capture.
export const IDENTITIES = {
  capabilityBits: {
    LAZY_USER_NOTES: protocol(1 << 0),
    NO_AFFINE_USER_IDS: protocol(1 << 1),
    VERSIONED_READ_STATES: protocol(1 << 2),
    VERSIONED_USER_GUILD_SETTINGS: protocol(1 << 3),
    DEDUPE_USER_OBJECTS: protocol(1 << 4),
    PRIORITIZED_READY_PAYLOAD: protocol(1 << 5),
    MULTIPLE_GUILD_EXPERIMENT_POPULATIONS: protocol(1 << 6),
    NON_CHANNEL_READ_STATES: protocol(1 << 7),
    AUTH_TOKEN_REFRESH: protocol(1 << 8),
    USER_SETTINGS_PROTO: protocol(1 << 9),
    CLIENT_STATE_V2: protocol(1 << 10),
    PASSIVE_GUILD_UPDATE: protocol(1 << 11),
    AUTO_CALL_CONNECT: protocol(1 << 12),
    DEBOUNCE_MESSAGE_REACTIONS: protocol(1 << 13),
    PASSIVE_GUILD_UPDATE_V2: protocol(1 << 14),
  },
  defaults: {
    locale: protocol('en-US'),
    timezone: protocol('America/Los_Angeles'),
    gatewayQuery: protocol('encoding=json&v=9&compress=zlib-stream'),
    apiVersion: protocol(9 as const),
    debugOptions: protocol('bugReporterEnabled'),
    contentType: protocol('application/json'),
  },
  identify: {
    presence: protocol({ status: 'unknown', since: 0, activities: [] as unknown[], afk: false }),
    compress: protocol(false),
    client_state: protocol({ guild_versions: {} }),
  },
  // Every host identifies as Discord's web client in Chrome on the host's own OS (Charlie, 2026-10-23):
  // its JSON over zlib-stream is the gateway as built, and its build number is the one read from
  // discord.com. A native host sends the browser's User-Agent itself; in a browser the browser does.
  hosts: {
    web: chrome({ os: 'Mac OS X', osVersion: '10.15.7', userAgent: null,
      browserUserAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' }),
    macos: chrome({ os: 'Mac OS X', osVersion: '10.15.7',
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' }),
    windows: chrome({ os: 'Windows', osVersion: '10',
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' }),
    // Chrome on iOS is WebKit under Chrome's name (CriOS); Apple freezes the OS version it reports.
    ios: chrome({ os: 'iOS', osVersion: '18.6', browser: 'Chrome iOS', browserVersion: '148.0.7778.73', unreduced: true,
      userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/148.0.7778.73 Mobile/15E148 Safari/604.1' }),
    android: chrome({ os: 'Android', osVersion: '10', browser: 'Android Chrome',
      userAgent: 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36' }),
  },
};

export const CAPABILITIES = Object.fromEntries(
  Object.entries(IDENTITIES.capabilityBits).map(([name, fact]) => [name, fact.value]),
) as Record<keyof typeof IDENTITIES.capabilityBits, number>;

export interface ClientIdentity {
  host: Host;
  properties: Record<string, unknown>;
  capabilities: number;
  userAgent: string | null;
  superProperties: string;
  gatewayQuery: string;
  apiVersion: 9;
  locale: string;
  timezone: string;
}

export interface ClientSession {
  launchId: string;
  launchSignature?: string;
  heartbeatSessionId?: string;
  appState?: string;
  deviceVendorId?: string;
  locale?: string;
  timezone?: string;
}

function base64Json(value: unknown): string {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export function identity(host: Host, session: ClientSession): ClientIdentity {
  const pin = IDENTITIES.hosts[host];
  const properties: Record<string, unknown> = Object.fromEntries(
    Object.entries(pin.properties).map(([key, fact]) => [key, fact.value]),
  );
  for (const [property, fact] of Object.entries(pin.sessionProperties)) {
    const value = session[fact.value as keyof ClientSession];
    if (value !== undefined) properties[property] = value;
  }
  const locale = session.locale ?? IDENTITIES.defaults.locale.value;
  properties.system_locale = locale;
  return {
    host, properties, capabilities: pin.capabilities.value, userAgent: pin.userAgent.value,
    superProperties: base64Json(properties),
    gatewayQuery: IDENTITIES.defaults.gatewayQuery.value,
    apiVersion: IDENTITIES.defaults.apiVersion.value,
    locale,
    timezone: session.timezone ?? IDENTITIES.defaults.timezone.value,
  };
}

export function restHeaders(
  id: ClientIdentity,
  token: string,
  extra: { body?: boolean; locale?: string; timezone?: string } = {},
): Record<string, string> {
  const headers: Record<string, string> = {
    Authorization: token,
    'X-Super-Properties': id.superProperties,
    'X-Discord-Locale': extra.locale ?? id.locale,
    'X-Discord-Timezone': extra.timezone ?? id.timezone,
    'X-Debug-Options': IDENTITIES.defaults.debugOptions.value,
  };
  if (id.userAgent !== null) headers['User-Agent'] = id.userAgent;
  if (extra.body) headers['Content-Type'] = IDENTITIES.defaults.contentType.value;
  return headers;
}

export function identifyPayload(id: ClientIdentity, token: string): object {
  return {
    token,
    capabilities: id.capabilities,
    properties: id.properties,
    presence: { ...IDENTITIES.identify.presence.value, activities: [...IDENTITIES.identify.presence.value.activities] },
    compress: IDENTITIES.identify.compress.value,
    client_state: { guild_versions: { ...IDENTITIES.identify.client_state.value.guild_versions } },
  };
}
