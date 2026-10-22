import { describe, expect, test } from 'bun:test';
import { CAPABILITIES, IDENTITIES, identifyPayload, identity, restHeaders, type Host } from './client.ts';

const hosts: Host[] = ['ios', 'macos', 'windows', 'web', 'android'];
const session = {
  launchId: 'launch-from-caller',
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
});
