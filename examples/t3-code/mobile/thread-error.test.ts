import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { mobileThreadErrorView, threadErrorBannerMessage, threadErrorExpandable } from './thread-error';

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
