import { describe, expect, test } from 'bun:test';
import { threadErrorBannerMessage, threadErrorView } from '../../src/timeline/timeline-errors';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';

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

// ThreadErrorBanner.test.tsx at 74541bab17 (#21483).
describe('thread error banner message', () => {
  test('extracts a readable nested message without trimming its whitespace', () => {
    const message = '  This model is not supported with a ChatGPT account.\nChoose another model.  ';
    expect(threadErrorBannerMessage(` \n${JSON.stringify({ error: { message, code: 'unsupported_model' } })}\t `)).toBe(message);
  });
  test.each([
    ['plain text', '  Could not start the turn.\nTry again.  '],
    ['invalid JSON', '{"error":{"message":"Cut off"'],
    ['an unfamiliar shape', '{"message":"Top-level message","status":400}'],
  ])('leaves %s unchanged', (_description, error) => {
    expect(threadErrorBannerMessage(error)).toBe(error);
  });
  test('keeps the diagnostics when the nested message is blank, and unwraps one level only', () => {
    const blank = JSON.stringify({ error: { message: ' \n\t ', code: 'unsupported_model' } });
    expect(threadErrorBannerMessage(blank)).toBe(blank);
    const inner = JSON.stringify({ error: { message: 'Inner message' } });
    expect(threadErrorBannerMessage(JSON.stringify({ error: { message: inner } }))).toBe(inner);
  });
  test('a short JSON error collapses to its message and expands to the exact text', () => {
    const error = '  {"error": {"message":"Turn failed", "code":"unsupported_value"}}\t ';
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error' }, 'provider_error'), { error, clientError: '' }))
      .toMatchObject({ error, errorSummary: 'Turn failed', errorExpandable: true });
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error' }, 'provider_error'), { error: 'Short error', clientError: '' }))
      .toMatchObject({ errorSummary: 'Short error', errorExpandable: false });
  });
});
