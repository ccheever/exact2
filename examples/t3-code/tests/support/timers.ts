// A flow that waits on its answer's own timers (command-waits.ts `sleep`), run under fake timers: each due timer fires
// once the microtasks before it have run, until the flow settles, so a test's polls and debounces take no wall time.
import { jest } from 'bun:test';

const TURNS = 64;

/** Runs `flow` under fake timers until it settles; its result or its rejection. */
export async function withTimers<T>(flow: () => Promise<T>): Promise<T> {
  jest.useFakeTimers();
  try {
    let settled = false;
    const run = flow().finally(() => { settled = true; });
    run.catch(() => undefined);
    while (!settled) {
      for (let turn = 0; turn < TURNS && !settled; turn++) await Promise.resolve();
      if (settled) break;
      if (jest.getTimerCount() === 0) break;
      jest.advanceTimersToNextTimer();
    }
    return await run;
  } finally {
    jest.useRealTimers();
  }
}
