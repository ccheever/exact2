// A command's wait is a timer of the answer that awaits it (LLP 1027.000 D1): it resolves on the app's clock, and one
// the runtime refuses (its answer was let go or gave its value) unwinds the flow as a let-go native call does.
import { afterEach, describe, expect, jest, spyOn, test } from 'bun:test';
import { settle, sleep } from '../../src/client/command-waits';
import { letGo } from '../../src/client/let-go';
import type { Native } from '../../src/client/protocol';

afterEach(() => { jest.useRealTimers(); jest.restoreAllMocks(); });

describe('a command waits on a timer of its own', () => {
  test('sleep resolves once its time has passed, and settle is the same wait', async () => {
    jest.useFakeTimers();
    const done: string[] = [];
    void sleep(180).then(() => done.push('sleep'));
    void settle({} as Native, 450).then(() => done.push('settle'));
    jest.advanceTimersByTime(179);
    await Promise.resolve();
    expect(done).toEqual([]);
    jest.advanceTimersByTime(1);
    await Promise.resolve();
    expect(done).toEqual(['sleep']);
    jest.advanceTimersByTime(270);
    await Promise.resolve();
    expect(done).toEqual(['sleep', 'settle']);
  });

  test('a timer the runtime refuses is its answer\'s let-go', async () => {
    spyOn(globalThis, 'setTimeout').mockImplementation((() => {
      throw new Error('setTimeout() is unavailable here: a timer belongs to the answer that arms it, and none is running');
    }) as unknown as typeof setTimeout);
    const refused = await sleep(200).then(() => null, (error: unknown) => error);
    expect(letGo(refused)).toBe(true);
  });
});
