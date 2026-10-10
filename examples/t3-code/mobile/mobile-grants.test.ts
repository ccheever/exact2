import { expect, test } from 'bun:test';
import { mobileNative } from './client';
import { mobileGrantRequest, mobileSessionGrants, MOBILE_STANDARD_CLIENT_SCOPES } from './mobile-grants';
import { withStandardScope } from './shared/remote-scopes';
import { obj, type Obj } from './shared/domain';

const expected = ['orchestration:read', 'orchestration:operate', 'settings:write', 'providers:manage',
  'environment:maintain', 'preview:operate', 'diagnostics:read', 'terminal:read', 'terminal:operate',
  'source-control:write', 'filesystem:read', 'filesystem:write', 'relay:read'];
test('actual mobile native boundary requests pinned granular scopes on every credential exchange', async () => {
  const requests: Obj[] = [];
  const native = mobileNative({ available: true, watch() {}, async later(input) { requests.push(obj(input)); return {}; } });
  for (const op of ['connect', 'pairEnvironment', 'fleetOutdatedPair']) {
    const request = { op, ...withStandardScope({ credential: 'synthetic-bootstrap' }), expectedEnvironmentId: 'one' };
    await native.later(request);
    expect(requests.at(-1)).toEqual({ ...request, scope: expected.join(' ') });
  }
  expect([...MOBILE_STANDARD_CLIENT_SCOPES]).toEqual(expected);
});
test('saved-token reconnects and unrelated requests retain exact identity and no broadened grant', () => {
  for (const request of [{ op: 'connect', credential: '' }, { op: 'retry' }, { op: 'http', path: '/api/auth/session' },
    { op: 'request', credential: 'synthetic', scope: 'orchestration:read' }]) expect(mobileGrantRequest(request)).toBe(request);
});
test('all nine pinned legacy parent fallbacks apply only to old session metadata', () => {
  const parents = {
    'filesystem:read': 'orchestration:read', 'diagnostics:read': 'orchestration:read',
    'settings:write': 'orchestration:operate', 'providers:manage': 'orchestration:operate',
    'environment:maintain': 'orchestration:operate', 'preview:operate': 'orchestration:operate',
    'source-control:write': 'orchestration:operate', 'filesystem:write': 'orchestration:operate', 'terminal:read': 'terminal:operate',
  };
  for (const [permission, parent] of Object.entries(parents)) {
    const legacy = { authenticated: true, scopes: [parent] };
    expect(mobileSessionGrants(legacy, permission)).toBe(true);
    expect(mobileSessionGrants({ ...legacy, auth: { serverUpdateScope: 'environment:maintain' } }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [] }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [parent] }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [permission] }, permission)).toBe(true);
    expect(mobileSessionGrants({ ...legacy, authenticated: false }, permission)).toBe(false);
  }
  expect(mobileSessionGrants({ authenticated: true, scopes: ['orchestration:operate'] }, 'access:write')).toBe(false);
});
test('requesting granular scopes never substitutes for actual constrained server permissions', () => {
  const grant = ['orchestration:read', 'relay:read'];
  const issued = MOBILE_STANDARD_CLIENT_SCOPES.filter(scope => grant.includes(scope));
  expect(mobileSessionGrants({ authenticated: true, permissions: issued }, 'filesystem:read')).toBe(false);
  expect(mobileSessionGrants({ authenticated: true, permissions: 'filesystem:read', scopes: ['filesystem:read'] }, 'filesystem:read')).toBe(false);
});
