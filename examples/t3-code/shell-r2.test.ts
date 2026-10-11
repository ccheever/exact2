import { describe, expect, test } from 'bun:test';
import { pushToast, toasts } from './toast';
import { toastViews } from './shell';
import { shellSuccess } from './shell-commands';
import type { T3Client } from './client';
import type { Obj } from './domain';

function fakeClient(extra: Obj = {}): T3Client {
  return { threadId: '', projectId: '', ready: true, environmentId: 'env', connection: 'connected', shell: { threads: [], projects: [] },
    config: { environment: { capabilities: {} } }, local: { clientSettings: {}, deviceSettings: { timestampFormat: '24-hour' } }, ...extra } as unknown as T3Client;
}

describe('appearance.cycle toast', () => {
  test('an untyped 1.5 s notice that replaces the previous one', () => {
    const client = fakeClient();
    shellSuccess(client, 'device-setting', 'light', '', 'appearanceMode');
    shellSuccess(client, 'device-setting', 'dark', '', 'appearanceMode');
    expect(toasts(client).map(toast => [toast.title, toast.timeoutMs])).toEqual([['Appearance: Dark', 1500]]);
    expect(toastViews(toasts(client))[0]).toMatchObject({ icon: '', iconColor: '', copyText: '', stacked: false });
    // A failed write or another device setting says nothing.
    shellSuccess(client, 'device-setting', 'system', 'Unsupported device setting.', 'appearanceMode');
    shellSuccess(client, 'device-setting', '24-hour', '', 'timestampFormat');
    expect(toasts(client)).toHaveLength(1);
  });

  test('a leading glyph still overrides the kind icon', () => {
    const client = fakeClient();
    pushToast(client, { kind: 'success', title: 'Thread completed', leading: 'circle-check:success-foreground' });
    expect(toastViews(toasts(client))[0]).toMatchObject({ icon: 'circle-check', iconColor: 'light-dark(#007a55, #00d492)' });
  });
});

describe('slow request notice', () => {
  test('tracks unary RPCs past their threshold into one warning updated in place', async () => {
    const { trackRpc, slowRequests, tracking, describeSlow, startedLabel } = await import('./shell-slow');
    const client = fakeClient();
    const t0 = Date.UTC(2026, 9, 4, 22, 14, 5);
    slowRequests(client, t0);
    expect(trackRpc(client, { op: 'request', method: 'orchestration.subscribeShell' })).toBeInstanceOf(Function);
    expect(trackRpc(client, { op: 'http', path: '/api/orchestration/shell' })).toBeInstanceOf(Function);
    expect(tracking(client)).toBe(false);
    const ackA = trackRpc(client, { op: 'request', method: 'orchestration.dispatchCommand' }, t0);
    const ackB = trackRpc(client, { op: 'request', method: 'server.updateProvider' }, t0);
    expect(tracking(client)).toBe(true);
    slowRequests(client, t0 + 14_999);
    expect(toasts(client)).toHaveLength(0);
    slowRequests(client, t0 + 15_000);
    const [toast] = toasts(client);
    expect(toast).toMatchObject({ kind: 'warning', title: 'Some requests are slow', description: '1 request waiting longer than 15s.', timeoutMs: 0 });
    expect(toast!.details!.map(detail => detail.title)).toEqual(['orchestration.dispatchCommand · env']);
    // A second request past its 15 s joins the same toast, in place. The update (120 s) is not counted.
    const ackC = trackRpc(client, { op: 'request', method: 'git.pull' }, t0 + 10_000);
    slowRequests(client, t0 + 25_000);
    expect(toasts(client)).toHaveLength(1);
    expect(toasts(client)[0]).toMatchObject({ id: toast!.id, description: '2 requests waiting longer than 15s.' });
    const view = toastViews(toasts(client))[0]!;
    expect(view).toMatchObject({ expandLabel: 'Show requests', collapseLabel: 'Hide requests' });
    expect(view.details.map(detail => detail.last)).toEqual([false, true]);
    ackA();
    slowRequests(client, t0 + 25_500);
    expect(toasts(client)[0]).toMatchObject({ id: toast!.id, description: '1 request waiting longer than 15s.' });
    ackC();
    slowRequests(client, t0 + 26_000);
    expect(toasts(client)).toHaveLength(0);
    // The update still waits: T3Transport fails every request at 30 s, which ends it (shell-slow.test.ts).
    expect(tracking(client)).toBe(true);
    ackB();
    slowRequests(client, t0 + 27_000);
    expect(tracking(client)).toBe(false);
    expect(describeSlow([{ thresholdMs: 15_000 }, { thresholdMs: 120_000 }])).toBe('2 requests waiting longer than 15s.');
    expect(startedLabel(new Date(2026, 0, 1, 15, 4, 9).getTime())).toBe('Started 3:04:09 PM');
    expect(startedLabel(new Date(2026, 0, 1, 0, 30, 0).getTime())).toBe('Started 12:30:00 AM');
  });

  test('a notice closed by its × stays closed until the slow set empties', async () => {
    const { trackRpc, slowRequests } = await import('./shell-slow');
    const { dismissToast } = await import('./toast');
    const client = fakeClient();
    slowRequests(client, 1000);
    const ack = trackRpc(client, { op: 'request', method: 'git.status' }, 1000);
    slowRequests(client, 16_000);
    dismissToast(client, toasts(client)[0]!.id);
    slowRequests(client, 17_000);
    expect(toasts(client)).toHaveLength(0);
    ack();
    slowRequests(client, 17_500);
    trackRpc(client, { op: 'request', method: 'git.status' });
    slowRequests(client, 18_000);
    slowRequests(client, 33_000);
    expect(toasts(client).map(entry => entry.title)).toEqual(['Some requests are slow']);
  });
});

