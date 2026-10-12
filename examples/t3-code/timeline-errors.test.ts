import { describe, expect, test } from 'bun:test';
import { threadErrorView } from './timeline-errors';
import type { T3Client } from './client';
import type { Obj } from './domain';

function client(shell: Obj, failureClass: string) {
  return { threadId: 't1', shell: { threads: [{ id: 't1', ...shell }] }, projection: {
    thread: { providerInstanceId: 'p' }, providerSessions: [], runs: [{ id: 'r1', ordinal: 1, status: 'failed' }],
    turnItems: [{ type: 'error', status: 'failed', runId: 'r1', updatedAt: 'x', failure: { class: failureClass, message: "You've hit your usage limit." } }] } } as unknown as T3Client;
}
const base = { error: "You've hit your usage limit.", clientError: '' };
describe('thread error banner variants', () => {
  test('a failed latest run stopped by its usage limit leaves the banner to the composer', () => {
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'usage_limit', latestRunId: 'r1' }, 'usage_limit'), base)).toMatchObject({ error: '', errorWarning: false });
  });
  test('another usage-limit server error is a warning; other classes and local errors stay errors', () => {
    expect(threadErrorView(client({ status: 'idle', lastErrorClass: 'usage_limit' }, 'usage_limit'), base)).toMatchObject({ error: base.error, errorWarning: true });
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error', latestRunId: 'r1' }, 'provider_error'), base)).toMatchObject({ error: base.error, errorWarning: false });
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'usage_limit', latestRunId: 'r1' }, 'usage_limit'), { error: 'Could not send.', clientError: 'Could not send.' }))
      .toMatchObject({ error: 'Could not send.', errorWarning: false });
  });
});
