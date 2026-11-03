import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { toasts } from './shared/toast';
import type { Native } from './shared/protocol';
import { CHATGPT_USAGE_URL } from './settings-usage-limits';
import { CHATGPT_WORKSPACE_ADMIN_REQUEST, mobileThreadErrorAction, mobileThreadErrorView, threadErrorBannerMessage, threadErrorExpandable } from './thread-error';
import { appText } from '../tests/support/app-files';

test('provider JSON errors show the nested message and retain expandable raw text', () => {
  const error = JSON.stringify({ error: { message: 'Provider stopped unexpectedly' } });
  expect(threadErrorBannerMessage(error)).toBe('Provider stopped unexpectedly');
  expect(threadErrorExpandable(error, 'Provider stopped unexpectedly')).toBe(true);
  expect(mobileThreadErrorView(new T3Client(), { error, clientError: '' })).toMatchObject({
    error, errorSummary: 'Provider stopped unexpectedly', errorExpandable: true, errorWarning: false,
  });
});

test('plain short errors stay collapsed without inventing a warning', () => {
  expect(threadErrorBannerMessage('Offline')).toBe('Offline');
  expect(threadErrorExpandable('Offline', 'Offline')).toBe(false);
  expect(mobileThreadErrorView(new T3Client(), { error: 'Offline', clientError: 'Offline' })).toMatchObject({
    error: 'Offline', errorSummary: 'Offline', errorExpandable: false, errorWarning: false,
  });
});

test('a latest usage-limit run stays owned by the composer instead of duplicating its banner', () => {
  const client = new T3Client();
  Object.assign(client, { threadId: 'thread' });
  client.shell.threads = [{ id: 'thread', status: 'failed', lastError: 'Limit reached', lastErrorClass: 'usage_limit', latestRun: 'run' }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 'thread', providerInstanceId: 'provider' }, runs: [{ id: 'run', ordinal: 1, status: 'failed' }], turnItems: [],
    providerSessions: [{ providerInstanceId: 'provider', lastError: 'Limit reached' }], visibleTurnItems: [],
  } };
  expect(mobileThreadErrorView(client, { error: 'Limit reached', clientError: '' }).error).toBe('');
});

test.each([
  ['workspace', 'subscription_sharing_workplace_individual_usage_limit_exceeded'],
  ['chatgpt', 'subscription_sharing_usage_limit_exceeded'],
] as const)('server usage limit %s gets the dedicated banner action', (limit, code) => {
  const client = new T3Client();
  Object.assign(client, { threadId: 'thread', providerId: 'provider' });
  client.shell.threads = [{ id: 'thread', status: 'idle', lastErrorClass: 'usage_limit', lastErrorCode: code }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 'thread', providerInstanceId: 'provider' }, runs: [], turnItems: [],
    providerSessions: [{ providerInstanceId: 'provider', lastError: 'Limit reached' }], visibleTurnItems: [],
  } };
  expect(mobileThreadErrorView(client, { error: 'Limit reached', clientError: '' })).toMatchObject({
    errorLimit: limit, errorWarning: true, errorSummary: 'Limit reached',
  });
});

test('usage-limit banner uses the Codex provider mark while ordinary errors keep the alert symbol', () => {
  const source = appText('mobile/thread.contract');
  expect(source).toContain('ProviderIcon(driver="codex", size=16, dark=false, iconUrl="")');
  expect(source).toContain('t3-symbol symbol-name="exclamationmark.circle"');
  expect(source).toContain('if data.errorLimit != ""');
});

test('workspace recovery copies the fixed admin request and reports clipboard failure', async () => {
  const client = new T3Client();
  const calls: Obj[] = [];
  let fail = false;
  client.restAccess = (() => ({ call: async (request: Obj) => {
    calls.push(request);
    if (fail) throw new Error('pasteboard busy');
    return {};
  } })) as T3Client['restAccess'];
  const native = { available: true } as Native;
  await mobileThreadErrorAction(client, native, 'admin-request');
  expect(calls).toEqual([{ op: 'copyText', text: CHATGPT_WORKSPACE_ADMIN_REQUEST }]);
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Request copied', description: 'Send it to your workspace admin.' });
  fail = true;
  await mobileThreadErrorAction(client, native, 'admin-request');
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not copy the request', description: 'pasteboard busy' });
});

test('ChatGPT recovery opens the canonical usage URL through the native bridge', async () => {
  const client = new T3Client();
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    calls.push(request);
    return { ok: true, generation: 0, value: { opened: true } };
  } };
  await mobileThreadErrorAction(client, native, 'chatgpt-usage');
  expect(calls).toEqual([{ op: 'mobileOpenURL', url: CHATGPT_USAGE_URL }]);
});
