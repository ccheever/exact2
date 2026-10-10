// Lane r13-store (F3): every wall-clock value this client persists is an instant (Unix ms, ISO), under the three
// runtime-clock shapes a data source can see, or nothing at all.
//   refused  — the data runtime: js/src/prelude.js replaces Date.now() with a function that throws (shell-details.ts,
//              composer-controls-queue.test.ts model it the same way);
//   launch   — a clock counting from launch (the round-12 report's reading of a normal run);
//   virtual  — the agent's virtual clock, which counts from 0.
// The window's time (app.contract: wallTime.epochAtZero + performanceNow()) is the only wall time in either case.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { noPrimary, resetPrimary } from './local-primary-fixture';
// The onboarding cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import type { T3Client } from './client';
import './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { EPOCH_FLOOR, epochNow, isEpoch, wallEpoch, wallIso } from './r8-pointer-clock';
import { pagesPrefs } from './pages-prefs';
import { importProjects, welcomeLocal, welcomeShowing, welcomeView } from './pages-welcome';
import { storedCompletion } from './r9-connect-onboarding';
import { adoptCommandTime, clock, wall, setRuntimeClock } from './sidebar-state';
import { sidebarCommand } from './sidebar-commands';
import { editorLocal } from './composer-editor';
import { stashEntries } from './composer-editor-stash';

// The data runtime has no clock (sidebar-state.ts `clock`); these tests stand in for a host clock that reads Date.now.
beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => setRuntimeClock(() => Number.NaN));

const REAL = Date.parse('2026-10-05T10:11:12.345Z');
const iso = (time: number) => new Date(time).toISOString();
const realNow = Date.now;
afterEach(() => { Date.now = realNow; });

const REFUSED = 'Date.now() is unavailable in data sources; pass time or a random seed as an argument';
const SHAPES: [string, () => void][] = [
  ['refused (the data runtime)', () => { Date.now = () => { throw new Error(REFUSED); }; }],
  ['counting from launch (97 s in)', () => { Date.now = () => 97_000; }],
  ['virtual (the agent, 6.1 s in)', () => { Date.now = () => 6_100; }],
];

function bare(): T3Client {
  return { local: {}, origin: '', environmentId: '', connection: 'disconnected', statusMessage: '', scopes: [], config: {},
    shell: { projects: [], threads: [], sequence: 0 } } as unknown as T3Client;
}
const native = (saved: Obj[]): Native => ({ available: true, watch: () => {}, later: async (request: Obj) => {
  if (request.op === 'environments') return { ok: true, value: { saved }, generation: 1 };
  return { ok: true, value: {}, generation: 1 };
} }) as unknown as Native;
const ENV = [{ origin: 'http://127.0.0.1:1', environmentId: 'e' }];

describe('the clock helpers separate an instant from a count', () => {
  test('only a number above 1e12 ms is an instant', () => {
    expect(EPOCH_FLOOR).toBe(1e12);
    for (const value of [0, 97_000, 118_501, 6_100, -5, NaN, Infinity, 1e12]) expect(isEpoch(value)).toBe(false);
    expect(isEpoch(REAL)).toBe(true);
  });
  for (const [name, install] of SHAPES) {
    test(`runtime clock ${name}: the window's instant stands, a count stands for none`, () => {
      install();
      expect(wallEpoch(REAL)).toBe(REAL);
      expect(wallIso(REAL)).toBe('2026-10-05T10:11:12.345Z');
      for (const count of [0, 97_000, 118_501]) {
        expect(wallEpoch(count)).toBeNull();
        expect(wallIso(count)).toBe('');
        expect(epochNow(count)).toBe(count); // the legacy number is never an input to a stored value
      }
    });
  }
  test('a runtime clock that is an instant (the bun tests) still stands in for a window time that is not', () => {
    const before = realNow();
    expect(wallEpoch(97_000)).toBeGreaterThanOrEqual(before);
    expect(storedCompletion(wallIso(97_000))).not.toBe('');
  });
});

describe('onboardingCompletedAt (FirstRunGate persistCompletion, WelcomeRouteView.onDone)', () => {
  for (const [name, install] of SHAPES) {
    test(`runtime clock ${name}: a client with an environment stores the window's wall time`, async () => {
      install();
      const client = bare();
      await welcomeView(client, native(ENV), { step: 'connect', now: REAL });
      expect(pagesPrefs(client).onboardingCompletedAt).toBe('2026-10-05T10:11:12.345Z');
    });
    test(`runtime clock ${name}: Finish stores the wall time the command carried`, async () => {
      install();
      const client = bare();
      const view = await welcomeView(client, native([]), { step: 'connect', now: REAL });
      expect(view.show).toBe(true);
      await welcomeLocal(client, native([]), 'finish', '', String(REAL + 90_000));
      expect(pagesPrefs(client).onboardingCompletedAt).toBe(iso(REAL + 90_000));
      expect(welcomeShowing(client)).toBe(false);
    });
    test(`runtime clock ${name}: Import with nothing picked finishes at the wall time it carried`, async () => {
      install();
      const client = bare();
      await welcomeView(client, native([]), { step: 'connect', now: REAL });
      await welcomeLocal(client, native([]), 'import', '', String(REAL + 5_000));
      expect(pagesPrefs(client).onboardingCompletedAt).toBe(iso(REAL + 5_000));
    });
    test(`runtime clock ${name}: the old finish value (ms since launch) never replaces the last instant — the 1970-01-01T00:01:37Z fault`, async () => {
      install();
      const client = bare();
      await welcomeView(client, native([]), { step: 'connect', now: REAL });
      await welcomeLocal(client, native([]), 'finish', '', '97000');
      expect(pagesPrefs(client).onboardingCompletedAt).toBe(iso(REAL));
      const second = bare();
      await welcomeView(second, native([]), { step: 'connect', now: REAL });
      await welcomeLocal(second, native([]), 'import', '', '118501');
      expect(pagesPrefs(second).onboardingCompletedAt).toBe(iso(REAL));
    });
    test(`runtime clock ${name}: before the host has told the date nothing is stored, and the first ask that knows it stores it`, async () => {
      install();
      // epochAtZero is 0 until then: the window's time is the count since launch.
      const early = bare();
      await welcomeView(early, native(ENV), { step: 'connect', now: 97_000 });
      expect(pagesPrefs(early).onboardingCompletedAt).toBe('');
      await welcomeView(early, native(ENV), { step: 'connect', now: REAL });
      expect(pagesPrefs(early).onboardingCompletedAt).toBe(iso(REAL));
      const wizard = bare();
      expect((await welcomeView(wizard, native([]), { step: 'connect', now: 97_000 })).show).toBe(true);
      await welcomeLocal(wizard, native([]), 'finish', '', '97000');
      expect(pagesPrefs(wizard).onboardingCompletedAt).toBe('');
      expect(welcomeShowing(wizard)).toBe(false);
      await welcomeView(wizard, native(ENV), { step: 'connect', now: REAL + 1_000 });
      expect(pagesPrefs(wizard).onboardingCompletedAt).toBe(iso(REAL + 1_000));
    });
    test(`runtime clock ${name}: importing picked projects finishes at the carried wall time`, async () => {
      install();
      const client = bare();
      const calls: string[] = [];
      Object.assign(client, { origin: 'http://127.0.0.1:1', environmentId: 'e', connection: 'connected', shell: { projects: [] as Obj[], threads: [], sequence: 0 },
        rpc: async (_native: Native, method: string) => { calls.push(method); return method === 'agentSessions.scan' ? { candidates: [] } : {}; } });
      await welcomeView(client, native([]), { step: 'connect', now: REAL });
      await welcomeLocal(client, native([]), 'import', '', String(REAL + 7_000));
      expect(pagesPrefs(client).onboardingCompletedAt).toBe(iso(REAL + 7_000));
      expect(await importProjects(client, native([]))).toBe('');
    });
  }
  test('every stored stamp reads back through the reader: a stamp the writers make is never "unset"', async () => {
    SHAPES[0]![1]();
    const client = bare();
    await welcomeView(client, native(ENV), { step: 'connect', now: REAL });
    expect(storedCompletion(pagesPrefs(client).onboardingCompletedAt)).toBe(pagesPrefs(client).onboardingCompletedAt);
    expect(storedCompletion('1970-01-01T00:01:37Z')).toBe('');
    expect(storedCompletion('1970-01-01T00:01:58.501Z')).toBe('');
  });
});

describe('the sidebar writers (sidebar-state wall time → snoozedUntil on the server)', () => {
  const AT = REAL;
  function sidebarClient(dispatched: Obj[]): T3Client {
    const threads = [{ id: 'a', projectId: 'p1', title: 'A', status: 'idle', createdAt: '2026-10-04T10:00:00.000Z', updatedAt: '2026-10-04T10:00:00.000Z', lineage: {} }];
    return {
      shell: { projects: [{ id: 'p1', title: 'P' }], threads }, config: { environment: { capabilities: { threadSnooze: true } }, providers: [], keybindings: [] },
      threadId: '', projectId: 'p1', query: '', writable: true, ready: true, connection: 'connected', environmentId: 'env', presentation: {},
      local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {} },
      projectGroups: () => [{ key: 'g', name: 'P', members: [{ id: 'p1' }] }],
      restAccess: () => ({ ids: async () => ['c'], request: async (_method: string, payload: Obj) => { dispatched.push(payload); return {}; }, call: async () => ({}) }),
    } as unknown as T3Client;
  }
  const handle = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
  for (const [name, install] of SHAPES) {
    test(`runtime clock ${name}: a snooze preset is the command's wall time plus the preset, exactly`, async () => {
      install();
      const dispatched: Obj[] = [];
      const client = sidebarClient(dispatched);
      await sidebarCommand(client, handle, {} as Files, 'snooze:hour', 'a', '', AT);
      expect(dispatched.map(payload => payload.type)).toEqual(['thread.snooze']);
      expect(dispatched[0]!.snoozedUntil).toBe(iso(AT + 3_600_000));
      expect(wall(client)).toBeGreaterThan(EPOCH_FLOOR);
    });
  }
  test('runtime clock refused: a command that carried no instant snoozes nothing (wall is 0 and the presets are empty), never 1970', async () => {
    SHAPES[0]![1]();
    const dispatched: Obj[] = [];
    const client = sidebarClient(dispatched);
    await sidebarCommand(client, handle, {} as Files, 'snooze:hour', 'a', '', 97_000);
    expect(dispatched).toEqual([]);
    expect(wall(client)).toBe(0);
  });
  test('adopting a command time measures later reads from it; a count is not adopted', () => {
    Date.now = () => { throw new Error(REFUSED); };
    const client = {} as T3Client;
    expect(clock()).toBe(0);
    adoptCommandTime(client, 97_000);
    expect(wall(client)).toBe(0);
    adoptCommandTime(client, AT);
    expect(wall(client)).toBe(AT);
    adoptCommandTime(client, 118_501);
    expect(wall(client)).toBe(AT);
  });
});

describe('the prompt stash (composer-editor-stash createdAt)', () => {
  for (const [name, install] of SHAPES) {
    test(`runtime clock ${name}: a stashed prompt is stamped with the window's wall time`, async () => {
      install();
      const local = { drafts: {} as Record<string, string> };
      const client = { local, draft: 'keep this', draftKey: 'e:new:p', environmentId: 'e', config: {}, shell: { projects: [], threads: [], sequence: 0 } } as unknown as T3Client;
      const editor = { available: true, watch: () => {}, later: async (request: Obj) => ({ ok: true, generation: 1,
        value: request.op === 'editorState' ? { text: 'keep this' } : { applied: true } }) } as unknown as Native;
      await editorLocal(client, editor, 'stash', '', '', REAL);
      expect(stashEntries(local).map(entry => [entry.prompt, entry.createdAt])).toEqual([['keep this', '2026-10-05T10:11:12.345Z']]);
    });
  }
});
