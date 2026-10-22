// The client each host identifies as (llp/0001 §2): IDENTIFY's `properties` and `capabilities`, and
// the REST headers (`User-Agent`, `X-Super-Properties`). Every value is in `IDENTITIES` with where it
// came from; Charlie checks them against a capture of his own clients before anything here talks to
// discord.com. Per-launch ids come from the caller (a data module has no randomness of its own).
export type Host = 'ios' | 'macos' | 'windows' | 'web' | 'android';

export interface Sourced<T> {
  value: T;
  source: string;
  asOf: string;
}

function memory<T>(value: T, description: string, asOf: string): Sourced<T> {
  return { value, source: `from memory of ${description}, unverified`, asOf };
}

const protocol = <T>(value: T) => memory(value, 'Discord user gateway/header examples', '2025-01');
const desktop = <T>(value: T) => memory(value, 'Discord Electron stable super-properties examples; exact build pairing uncertain', '2025-01');
const web = <T>(value: T) => memory(value, 'Discord Chrome super-properties examples; exact build pairing uncertain', '2025-01');
const ios = <T>(value: T) => memory(value, 'Discord React Native iOS identify examples; version/build/device pairing uncertain', '2024-11');
const android = <T>(value: T) => memory(value, 'Discord React Native Android identify examples; version/device pairing uncertain', '2024-11');

// These are reviewable recollections, not verified captures or claims about today's clients.
// Every wire constant and session-field selection lives here so a capture can replace it together.
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
  hosts: {
    ios: {
      properties: {
        os: ios('iOS'),
        browser: ios('Discord iOS'),
        device: ios('iPhone16,1'),
        system_locale: ios('en-US'),
        has_client_mods: ios(false),
        client_version: ios('253.0'),
        release_channel: ios('stable'),
        os_version: ios('18.1'),
        os_sdk_version: ios('18.1'),
        device_id: ios('iPhone16,1'),
        client_build_number: ios(62365),
        native_build_number: ios('62365'),
        client_event_source: ios(null),
        client_app_state: ios('active'),
        design_id: ios(2),
      },
      sessionProperties: {
        client_launch_id: ios('launchId'),
        client_heartbeat_session_id: ios('heartbeatSessionId'),
        device_vendor_id: ios('deviceVendorId'),
      },
      capabilities: ios(16381),
      userAgent: ios('Discord/253.0 (iPhone; iOS 18.1; Scale/3.00)'),
    },
    macos: {
      properties: {
        os: desktop('Mac OS X'),
        browser: desktop('Discord Client'),
        release_channel: desktop('stable'),
        client_version: desktop('0.0.339'),
        os_version: desktop('24.2.0'),
        os_arch: desktop('arm64'),
        app_arch: desktop('arm64'),
        system_locale: desktop('en-US'),
        has_client_mods: desktop(false),
        browser_user_agent: desktop('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) discord/0.0.339 Chrome/128.0.6613.186 Electron/32.2.7 Safari/537.36'),
        browser_version: desktop('32.2.7'),
        client_build_number: desktop(363699),
        native_build_number: desktop(59123),
        client_event_source: desktop(null),
      },
      sessionProperties: {
        client_launch_id: desktop('launchId'),
        client_heartbeat_session_id: desktop('heartbeatSessionId'),
      },
      capabilities: desktop(30717),
      userAgent: desktop('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) discord/0.0.339 Chrome/128.0.6613.186 Electron/32.2.7 Safari/537.36'),
    },
    windows: {
      properties: {
        os: desktop('Windows'),
        browser: desktop('Discord Client'),
        release_channel: desktop('stable'),
        client_version: desktop('1.0.9181'),
        os_version: desktop('10.0.26100'),
        os_arch: desktop('x64'),
        app_arch: desktop('x64'),
        system_locale: desktop('en-US'),
        has_client_mods: desktop(false),
        browser_user_agent: desktop('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9181 Chrome/128.0.6613.186 Electron/32.2.7 Safari/537.36'),
        browser_version: desktop('32.2.7'),
        client_build_number: desktop(363699),
        native_build_number: desktop(59123),
        client_event_source: desktop(null),
      },
      sessionProperties: {
        client_launch_id: desktop('launchId'),
        client_heartbeat_session_id: desktop('heartbeatSessionId'),
      },
      capabilities: desktop(30717),
      userAgent: desktop('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9181 Chrome/128.0.6613.186 Electron/32.2.7 Safari/537.36'),
    },
    web: {
      properties: {
        os: web('Mac OS X'),
        browser: web('Chrome'),
        device: web(''),
        system_locale: web('en-US'),
        has_client_mods: web(false),
        browser_user_agent: web('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36'),
        browser_version: web('131.0.0.0'),
        os_version: web('10.15.7'),
        referrer: web(''),
        referring_domain: web(''),
        referrer_current: web(''),
        referring_domain_current: web(''),
        release_channel: web('stable'),
        client_build_number: web(363699),
        client_event_source: web(null),
      },
      sessionProperties: {
        client_launch_id: web('launchId'),
        client_heartbeat_session_id: web('heartbeatSessionId'),
      },
      capabilities: web(30717),
      userAgent: web(null),
    },
    android: {
      properties: {
        os: android('Android'),
        browser: android('Discord Android'),
        device: android('Pixel 8 Pro'),
        system_locale: android('en-US'),
        has_client_mods: android(false),
        client_version: android('253.19'),
        release_channel: android('googleRelease'),
        os_version: android('15'),
        os_sdk_version: android('35'),
        device_id: android('husky'),
        client_build_number: android(253019),
        native_build_number: android(253019),
        client_event_source: android(null),
        client_app_state: android('active'),
        design_id: android(2),
      },
      sessionProperties: {
        client_launch_id: android('launchId'),
        client_heartbeat_session_id: android('heartbeatSessionId'),
        device_vendor_id: android('deviceVendorId'),
      },
      capabilities: android(16381),
      userAgent: android('Discord-Android/253019;RNA'),
    },
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
  heartbeatSessionId?: string;
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
