// Port of the reference's apps/web/src/rpc/requestLatencyState.test.ts (T3 Code 1e2ecbd975), original
// test names. exact2-required changes: no fake timers; the state is a value and every call takes `now`
// (`vi.advanceTimersByTime(n)` becomes `getSlowRpcAckRequests(state, start + n)`); `trackRpcRequestSent`
// takes `now` before the optional tag.
import { beforeEach, describe, expect, test } from 'bun:test';
import {
  acknowledgeRpcRequest,
  createRequestLatencyState,
  getSlowRpcAckRequests,
  resetRequestLatencyStateForTests,
  trackRpcRequestSent,
  LONG_RUNNING_RPC_ACK_THRESHOLD_MS,
  SLOW_RPC_ACK_THRESHOLD_MS,
  MAX_TRACKED_RPC_ACK_REQUESTS,
} from './shell-slow';

// WS_METHODS in packages/contracts/src/rpc.ts.
const WS_METHODS = {
  previewAutomationConnect: 'previewAutomation.connect',
  serverGetUsageSummary: 'server.getUsageSummary',
  serverUpdateProvider: 'server.updateProvider',
  serverRefreshProviders: 'server.refreshProviders',
  serverUpdateServer: 'server.updateServer',
};
const PULL_REQUEST_METHODS = [
  'pullRequests.list', 'pullRequests.listStats', 'pullRequests.summary', 'pullRequests.routing', 'pullRequests.routingIdentity',
  'pullRequests.stack', 'pullRequests.linkedThreads', 'pullRequests.detail', 'pullRequests.preview', 'pullRequests.checks',
  'pullRequests.activity', 'pullRequests.threadComments', 'pullRequests.diffFileContents', 'pullRequests.filesViewed',
  'pullRequests.setFilesViewed', 'pullRequests.runAction', 'pullRequests.update', 'pullRequests.comment',
  'pullRequests.updateComment', 'pullRequests.submitReview', 'pullRequests.replyToThread', 'pullRequests.setThreadResolution',
  'pullRequests.setReaction', 'pullRequests.invalidate', 'pullRequests.subscribeRefreshes', 'pullRequests.reviewerCandidates',
  'pullRequests.requestReviewers', 'pullRequests.labelCandidates', 'pullRequests.setLabels',
];

describe('requestLatencyState', () => {
  const start = Date.UTC(2026, 9, 5, 12, 0, 0);
  const state = createRequestLatencyState();
  beforeEach(() => resetRequestLatencyStateForTests(state));

  test('marks unary requests as slow when the ack threshold is exceeded', () => {
    trackRpcRequestSent(state, '1', 'server.getConfig', start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS - 1)).toEqual([]);

    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS)).toMatchObject([
      { requestId: '1', tag: 'server.getConfig', thresholdMs: SLOW_RPC_ACK_THRESHOLD_MS },
    ]);
  });

  test('clears the slow request once the server acknowledges it', () => {
    trackRpcRequestSent(state, '1', 'git.status', start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS)).toHaveLength(1);

    acknowledgeRpcRequest(state, '1');
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS)).toEqual([]);
  });

  test('ignores long-lived subscribe requests', () => {
    trackRpcRequestSent(state, '1', 'subscribeServerConfig', start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS * 2)).toEqual([]);
  });

  test('ignores the long-lived preview automation connection', () => {
    trackRpcRequestSent(state, '1', WS_METHODS.previewAutomationConnect, start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS * 2)).toEqual([]);
  });

  test('ignores usage summary requests', () => {
    trackRpcRequestSent(state, '1', WS_METHODS.serverGetUsageSummary, start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS * 2)).toEqual([]);
  });

  test.each(PULL_REQUEST_METHODS)('ignores pull request workspace request %s', method => {
    trackRpcRequestSent(state, '1', method, start);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS * 2)).toEqual([]);
  });

  test('keeps ignoring untracked methods when a display tag is supplied', () => {
    trackRpcRequestSent(state, '1', WS_METHODS.previewAutomationConnect, start, `${WS_METHODS.previewAutomationConnect} · env-1`);
    expect(getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS * 2)).toEqual([]);
  });

  test('gives provider updates a longer threshold before warning', () => {
    trackRpcRequestSent(state, '1', WS_METHODS.serverUpdateProvider, start, 'server.updateProvider · env-1');
    expect(getSlowRpcAckRequests(state, start + LONG_RUNNING_RPC_ACK_THRESHOLD_MS - 1)).toEqual([]);

    expect(getSlowRpcAckRequests(state, start + LONG_RUNNING_RPC_ACK_THRESHOLD_MS)).toMatchObject([
      { requestId: '1', tag: 'server.updateProvider · env-1', thresholdMs: LONG_RUNNING_RPC_ACK_THRESHOLD_MS },
    ]);
  });

  test('gives provider refreshes and server updates the same longer threshold', () => {
    trackRpcRequestSent(state, 'refresh', WS_METHODS.serverRefreshProviders, start);
    trackRpcRequestSent(state, 'update', WS_METHODS.serverUpdateServer, start);
    expect(getSlowRpcAckRequests(state, start + LONG_RUNNING_RPC_ACK_THRESHOLD_MS - 1)).toEqual([]);
    expect(getSlowRpcAckRequests(state, start + LONG_RUNNING_RPC_ACK_THRESHOLD_MS).map(request => request.requestId)).toEqual(['refresh', 'update']);
  });

  test('evicts the oldest pending requests once the tracker reaches capacity', () => {
    for (let index = 0; index < MAX_TRACKED_RPC_ACK_REQUESTS + 1; index += 1) {
      trackRpcRequestSent(state, String(index), 'server.getConfig', start);
    }

    const slowRequests = getSlowRpcAckRequests(state, start + SLOW_RPC_ACK_THRESHOLD_MS);
    expect(slowRequests).toHaveLength(MAX_TRACKED_RPC_ACK_REQUESTS);
    expect(slowRequests[0]?.requestId).toBe('1');
    expect(slowRequests.at(-1)?.requestId).toBe(String(MAX_TRACKED_RPC_ACK_REQUESTS));
  });

  // The timers of the reference fire in due-time order; polling must report the same order.
  test('reports slow requests in the order their timers would fire', () => {
    trackRpcRequestSent(state, 'long', WS_METHODS.serverUpdateProvider, start);
    trackRpcRequestSent(state, 'short', 'git.status', start + 1000);
    expect(getSlowRpcAckRequests(state, start + LONG_RUNNING_RPC_ACK_THRESHOLD_MS).map(request => request.requestId)).toEqual(['short', 'long']);
  });
});
