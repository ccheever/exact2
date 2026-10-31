import { describe, expect, test } from 'bun:test';
import { CHATGPT_WORKSPACE_ADMIN_REQUEST, threadErrorBannerMessage, threadErrorView } from '../../src/timeline/timeline-errors';
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

describe('ChatGPT sharing limits (#23361)', () => {
  const workspace = 'subscription_sharing_workplace_individual_usage_limit_exceeded', plan = 'subscription_sharing_usage_limit_exceeded';
  test('a limit named by its code is a warning that offers its own recovery', () => {
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error', lastErrorCode: workspace, latestRunId: 'r1' }, 'provider_error'), base))
      .toMatchObject({ error: base.error, errorWarning: true, errorLimit: 'workspace' });
    expect(threadErrorView(client({ status: 'idle', lastErrorClass: 'usage_limit', lastErrorCode: plan }, 'usage_limit'), base))
      .toMatchObject({ errorWarning: true, errorLimit: 'chatgpt' });
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error', lastErrorCode: 'rate_limit' }, 'provider_error'), base)).toMatchObject({ errorLimit: '' });
  });
  test('the composer still owns a failed run its usage limit stopped, and a local error is no limit', () => {
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'usage_limit', lastErrorCode: workspace, latestRunId: 'r1' }, 'usage_limit'), base))
      .toMatchObject({ error: '', errorWarning: false, errorLimit: '' });
    expect(threadErrorView(client({ status: 'failed', lastErrorClass: 'provider_error', lastErrorCode: workspace }, 'provider_error'), { error: 'Could not send.', clientError: 'Could not send.' }))
      .toMatchObject({ errorWarning: false, errorLimit: '' });
  });
});

describe('Copy request for admin', () => {
  test('copies the fixed request and says so, or says it could not', async () => {
    const { chatLocal } = await import('../../src/timeline/timeline-presentation');
    const { toasts } = await import('../../src/shell/toast');
    const calls: Obj[] = [];
    let fail = false;
    const c = { restAccess: () => ({ call: async (call: Obj) => { calls.push(call); if (fail) throw new Error('pasteboard busy'); return {}; } }) } as unknown as T3Client;
    await chatLocal(c, { available: true } as never, 'admin-request', '', '');
    expect(calls).toEqual([{ op: 'copyText', text: CHATGPT_WORKSPACE_ADMIN_REQUEST }]);
    expect(CHATGPT_WORKSPACE_ADMIN_REQUEST).toStartWith("I've reached the limit for using my ChatGPT plan in T3 Code.");
    expect(toasts(c).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Request copied', 'Send it to your workspace admin.']]);
    fail = true;
    await chatLocal(c, { available: true } as never, 'admin-request', '', '');
    expect(toasts(c).at(-1)).toMatchObject({ kind: 'error', title: 'Could not copy the request' });
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
