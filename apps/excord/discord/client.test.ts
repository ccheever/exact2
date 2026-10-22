import { describe, expect, test } from 'bun:test';
import { CAPABILITIES, IDENTITIES, identifyPayload, identity, restHeaders, type Host } from './client.ts';

const hosts: Host[] = ['ios', 'macos', 'windows', 'web', 'android'];
const session = {
  launchId: 'launch-from-caller',
  launchSignature: 'signature-from-caller',
  heartbeatSessionId: 'heartbeat-from-caller',
  deviceVendorId: 'vendor-from-caller',
  locale: 'ja-JP',
  timezone: 'Asia/Tokyo',
};

describe('client identity', () => {
  for (const host of hosts) {
    test(`${host}: properties, REST headers, and IDENTIFY agree`, () => {
      const id = identity(host, session);
      const properties = JSON.parse(new TextDecoder().decode(
        Uint8Array.from(atob(id.superProperties), char => char.charCodeAt(0)),
      ));
      expect(id.host).toBe(host);
      expect(properties).toEqual(id.properties);
      expect(properties.system_locale).toBe(session.locale);
      expect(properties.client_launch_id).toBe(session.launchId);
      expect(properties.client_heartbeat_session_id).toBe(session.heartbeatSessionId);
      if (host === 'ios' || host === 'android') {
        expect(properties.device_vendor_id).toBe(session.deviceVendorId);
      } else {
        expect(properties).not.toHaveProperty('device_vendor_id');
      }
      expect(id.apiVersion).toBe(9);
      expect(id.gatewayQuery).toBe('encoding=json&v=9&compress=zlib-stream');
      const headers = restHeaders(id, 'fixture-token');
      expect(headers.Authorization).toBe('fixture-token');
      expect(headers['X-Super-Properties']).toBe(id.superProperties);
      expect(headers['X-Discord-Locale']).toBe('ja-JP');
      expect(headers['X-Discord-Timezone']).toBe('Asia/Tokyo');
      expect(headers['X-Debug-Options']).toBe('bugReporterEnabled');
      expect(headers).not.toHaveProperty('Content-Type');
      expect(restHeaders(id, 'fixture-token', { body: true })['Content-Type']).toBe('application/json');
      if (host === 'web') {
        expect(id.userAgent).toBeNull();
        expect(headers).not.toHaveProperty('User-Agent');
      } else {
        expect(headers['User-Agent']).toBe(id.userAgent);
        expect(id.userAgent?.length).toBeGreaterThan(0);
      }
      expect(identifyPayload(id, 'fixture-token')).toEqual({
        token: 'fixture-token', capabilities: id.capabilities, properties: id.properties,
        presence: { status: 'unknown', since: 0, activities: [], afk: false },
        compress: false, client_state: { guild_versions: {} },
      });
      const namedMask = Object.values(CAPABILITIES).reduce((mask, bit) => mask | bit, 0);
      expect(id.capabilities & ~namedMask).toBe(0);
      expect(id.capabilities).toBeGreaterThan(0);
    });
  }

  test('all table values carry honest provenance and a month', () => {
    let count = 0;
    function visit(value: unknown): void {
      expect(value).not.toBeNull();
      expect(typeof value).toBe('object');
      const record = value as Record<string, unknown>;
      if ('value' in record) {
        expect(typeof record.source).toBe('string');
        expect((record.source as string).length).toBeGreaterThan(10);
        expect(record.asOf).toMatch(/^\d{4}-(0[1-9]|1[0-2])$/);
        // Researched values are today's; recollections keep the month they were current.
        expect(record.asOf).toBe((record.source as string).startsWith('LLP 0002') ? '2026-10' : '2025-01');
        count++;
      } else {
        for (const child of Object.values(record)) visit(child);
      }
    }
    visit(IDENTITIES);
    expect(count).toBeGreaterThan(100);
  });

  test('optional session ids are omitted and identities do not share mutable properties', () => {
    const first = identity('ios', { launchId: 'first' });
    const second = identity('ios', { launchId: 'second' });
    expect(first.properties).not.toHaveProperty('client_heartbeat_session_id');
    expect(first.properties).not.toHaveProperty('device_vendor_id');
    first.properties.system_locale = 'changed';
    expect(second.properties.system_locale).toBe('en-US');
    expect(second.properties.client_launch_id).toBe('second');
    expect(restHeaders(second, 'fixture-token', { locale: 'fr-FR', timezone: 'Europe/Paris' })).toMatchObject({
      'X-Discord-Locale': 'fr-FR', 'X-Discord-Timezone': 'Europe/Paris',
    });
  });

  test('web retains its Chrome identity and referrers without desktop-only fields', () => {
    const id = identity('web', session);
    expect(id.properties).toMatchObject({
      os: 'Mac OS X', browser: 'Chrome', device: '',
      referrer: '', referring_domain: '', referrer_current: '', referring_domain_current: '',
    });
    expect(id.properties).not.toHaveProperty('app_arch');
    expect(id.properties).not.toHaveProperty('native_build_number');
    expect(id.properties.browser_user_agent).toContain('Chrome/');
  });

  test('LLP 0002 versions replace the old identity pins', () => {
    expect(identity('ios', session).properties.client_version).toBe('350.0');
    for (const [host, version] of [['macos', '0.0.419'], ['windows', '1.0.9265']] as const) {
      const id = identity(host, session);
      expect(id.properties.client_version).toBe(version);
      expect(id.properties.browser_version).toBe('42.11.15');
      expect(id.properties.browser_user_agent).toContain(`discord/${version}`);
      expect(id.properties.browser_user_agent).toContain('Chrome/148.0.7778.280');
      expect(id.properties.browser_user_agent).toContain('Electron/42.11.15');
      expect(id.userAgent).toBe(id.properties.browser_user_agent);
    }
    const web = identity('web', session);
    expect(web.properties.client_build_number).toBe(647615);
    expect(web.properties.browser_version).toBe('148.0.0.0');
    expect(web.properties.browser_user_agent).toContain('Chrome/148.0.0.0 Safari/537.36');
  });

  test('web and desktop super-properties carry launch and current app state', () => {
    for (const host of ['web', 'macos', 'windows'] as const) {
      const first = identity(host, session);
      const unfocused = identity(host, { ...session, appState: 'unfocused' });
      const next = identity(host, { ...session, launchId: 'next', launchSignature: 'next-signature' });
      expect(first.properties).toMatchObject({
        device: '', device_model_name: '', client_app_state: 'focused',
        launch_signature: session.launchSignature,
        referrer: '', referring_domain: '', referrer_current: '', referring_domain_current: '',
      });
      expect(first.properties).not.toHaveProperty('install_source');
      expect(unfocused.properties.launch_signature).toBe(first.properties.launch_signature);
      expect(unfocused.properties.client_launch_id).toBe(first.properties.client_launch_id);
      expect(unfocused.properties.client_app_state).toBe('unfocused');
      expect(next.properties.launch_signature).toBe('next-signature');
      expect(next.properties.client_launch_id).toBe('next');
      expect(first.properties.launch_signature).toBe(session.launchSignature);
      expect(JSON.parse(atob(unfocused.superProperties))).toEqual(unfocused.properties);
    }
    expect(Object.keys(identity('web', session).properties).sort()).toEqual([
      'os', 'browser', 'device', 'device_model_name', 'system_locale', 'has_client_mods',
      'browser_user_agent', 'browser_version', 'os_version', 'referrer', 'referring_domain',
      'referrer_current', 'referring_domain_current', 'release_channel', 'client_build_number',
      'client_event_source', 'client_launch_id', 'launch_signature', 'client_heartbeat_session_id',
      'client_app_state',
    ].sort());
  });

  test('research pins cite their rows and uncaptured identities stay explicitly unverified', () => {
    for (const host of ['ios', 'macos', 'windows'] as const) {
      expect(IDENTITIES.hosts[host].properties.client_version.source).toContain('LLP 0002');
      expect(IDENTITIES.hosts[host].properties.client_version.source).toContain('Versions');
    }
    expect(IDENTITIES.hosts.web.properties.client_build_number.source).toContain('GLOBAL_ENV');
    for (const fact of [IDENTITIES.hosts.ios.userAgent, IDENTITIES.hosts.ios.properties.client_build_number,
      IDENTITIES.hosts.ios.properties.native_build_number]) {
      expect(fact.source).toContain("unverified; needs a capture of the iOS app's traffic");
    }
    for (const host of ['macos', 'windows'] as const) {
      for (const name of ['client_build_number', 'native_build_number'] as const) {
        expect(IDENTITIES.hosts[host].properties[name].source).toContain('needs a capture');
      }
    }
    for (const host of hosts) {
      expect(IDENTITIES.hosts[host].capabilities.source).toContain('from memory, unverified');
      expect(identity(host, session).capabilities).toBe(host === 'ios' || host === 'android' ? 16381 : 30717);
      expect(identity(host, session).capabilities & CAPABILITIES.AUTH_TOKEN_REFRESH).not.toBe(0);
    }
  });
});
