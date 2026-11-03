// Stop on the background work line (useThreadLifecycle.tsx, isBackgroundWorkStopPending at T3 Code 5d3d78ab93, #25002).
import { describe, expect, test } from 'bun:test';
import { BACKGROUND_WORK_STOP_RETRY_AFTER_MS, backgroundStopPending, stopBackgroundWork } from '../../src/composer/background-stop';

describe('background work Stop', () => {
  test('stays pending until the stopped tasks leave, at most the retry window after acceptance', async () => {
    const client = { environmentId: 'env', threadId: 't1' }, tasks = [{ taskId: 'a' }, { taskId: 'b' }];
    let release = () => {}, retry = 0;
    const now = 1_000_000, clock = () => now;
    const stopping = stopBackgroundWork(client, tasks, clock, () => new Promise<void>(resolve => { release = resolve; }), ms => { retry = ms; });
    expect(backgroundStopPending(client, tasks, now)).toBe(true);
    // A second press while one is pending sends nothing.
    let second = false;
    await stopBackgroundWork(client, tasks, clock, async () => { second = true; }, () => {});
    expect(second).toBe(false);
    release();
    await stopping;
    expect(retry).toBe(BACKGROUND_WORK_STOP_RETRY_AFTER_MS);
    expect(backgroundStopPending(client, [{ taskId: 'b' }], now + 1)).toBe(true);
    expect(backgroundStopPending(client, [{ taskId: 'c' }], now + 1)).toBe(false);
    expect(backgroundStopPending(client, tasks, now + BACKGROUND_WORK_STOP_RETRY_AFTER_MS)).toBe(false);
    expect(backgroundStopPending({ environmentId: 'env', threadId: 't2' }, tasks, now)).toBe(false);
  });
  test('a failed stop brings Stop back', async () => {
    const client = { environmentId: 'env', threadId: 't1' }, tasks = [{ taskId: 'a' }];
    await expect(stopBackgroundWork(client, tasks, () => 5, async () => { throw new Error('refused'); }, () => {})).rejects.toThrow('refused');
    expect(backgroundStopPending(client, tasks, 5)).toBe(false);
  });
});
