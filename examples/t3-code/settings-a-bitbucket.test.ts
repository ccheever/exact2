// Lane settings-a: Source Control → Bitbucket credentials.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { bitbucketCommand, bitbucketPatch, bitbucketView, savedMethod } from './settings-a-bitbucket';

const none = { accessToken: '', email: '', apiToken: '' };
describe('bitbucket credentials', () => {
  test('the saved method and the view never carry a token', () => {
    expect(savedMethod(none)).toBe('');
    expect(savedMethod({ accessToken: '••••', email: 'a@b.c', apiToken: '••••' })).toBe('access-token');
    expect(savedMethod({ accessToken: '', email: 'a@b.c', apiToken: '••••' })).toBe('api-token');
    expect(bitbucketView({ bitbucket: { accessToken: '', email: 'me@x.io', apiToken: 'redacted' } }, 'env1', 3))
      .toEqual({ key: 'env1:3', saved: 'api-token', savedLabel: 'api token', email: 'me@x.io', apiTokenSaved: true, environmentId: 'env1' });
  });
  test('the patch follows the reference: one method clears the other; a saved token is resent to keep it', () => {
    expect(bitbucketPatch(none, 'access-token', '  tok  ', null, '')).toEqual({ accessToken: 'tok', email: '', apiToken: '' });
    expect(bitbucketPatch(none, 'access-token', '   ', null, '')).toBeNull();
    expect(bitbucketPatch(none, 'api-token', '', 'me@x.io', 'api')).toEqual({ accessToken: '', email: 'me@x.io', apiToken: 'api' });
    expect(bitbucketPatch(none, 'api-token', '', 'me@x.io', '')).toBeNull();
    const saved = { accessToken: '', email: 'me@x.io', apiToken: 'redacted' };
    // Nothing changed: not savable. A new email keeps the redacted token.
    expect(bitbucketPatch(saved, 'api-token', '', null, '')).toBeNull();
    expect(bitbucketPatch(saved, 'api-token', '', 'new@x.io', '')).toEqual({ accessToken: '', email: 'new@x.io', apiToken: 'redacted' });
  });
  test('save and remove write settings.bitbucket and rescan', async () => {
    const requests: [string, Obj][] = []; let rescans = 0;
    const client = { environmentId: 'env1', config: {}, restAccess: () => ({ request: async (method: string, payload: Obj) => { requests.push([method, payload]); return method === 'server.getSettings' ? { bitbucket: none } : { bitbucket: payload.patch }; } }) } as unknown as T3Client;
    const native = { available: true } as unknown as Native;
    await bitbucketCommand(client, native, 'env1', { action: 'save', method: 'access-token', accessToken: 'tok', email: '', emailEdited: 'false', apiToken: '' }, () => { rescans++; });
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { bitbucket: { accessToken: 'tok', email: '', apiToken: '' } } }]);
    await bitbucketCommand(client, native, 'env1', { action: 'remove' }, () => { rescans++; });
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { bitbucket: none } }]);
    expect(rescans).toBe(2);
    await expect(bitbucketCommand(client, native, 'env1', { action: 'save', method: 'api-token', email: '', emailEdited: 'true', apiToken: '' }, () => {})).rejects.toThrow('Enter your Atlassian account email and an API token.');
    await expect(bitbucketCommand(client, native, 'env2', { action: 'remove' }, () => {})).rejects.toThrow('That environment is no longer selected.');
  });
});
