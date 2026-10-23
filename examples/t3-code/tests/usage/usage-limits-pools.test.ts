// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) packages/shared/src/usageLimits.test.ts:
// "pools" (:132-668, and 9c161462c9's three workspace and Codex slot cases), "pooled account columns" (:670, and 74541bab17's
// weekly-exhausted session cases), "Cursor limit presentation" (:761),
// "collectLimitNotices" (:808) and "external usage settings" (:1130), with their original names.
// Changes: branded ids are plain strings; `Map` inputs keep the reference's presentation shape.
import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import {
  accountHue, accountInitials, collectExternalUsageLinks, collectLimitAccounts, collectLimitNotices, collectLimitPools, displayLimitWindows,
  type LimitAccount, type LimitPresentation,
} from '../../src/usage/usage-limits-pools';

const it = test;
const now = Date.parse('2026-09-03T12:00:00.000Z');
const window = { id: 'five_hour', kind: 'session', label: 'Session', usedPercent: 40, windowDurationMins: 300, resetsAt: '2026-09-03T14:00:00.000Z' } as const;
function provider(overrides: Obj): Obj {
  return { instanceId: 'codex', driver: 'codex', enabled: true, installed: true, version: null, status: 'ready', auth: { status: 'authenticated' },
    checkedAt: '2026-09-03T11:00:00.000Z', models: [], slashCommands: [], skills: [], ...overrides };
}
const presentations = (entries: [string, { entry: { target: { label: string } }; serverConfig: Obj }][]) => new Map<string, LimitPresentation>(entries);

describe('pools', () => {
  const checkedAt = '2026-09-03T11:00:00.000Z';
  const weekly = { id: 'seven_day', kind: 'weekly', label: 'Weekly', windowDurationMins: 7 * 24 * 60, resetsAt: '2026-09-06T12:00:00.000Z' } as const;
  const claude = 'claudeAgent';
  const source = { id: 'hub', kind: 'cliproxy' as const, label: 'hub', checkedAt };
  const laptop = { entry: { target: { label: 'Laptop' } } };

  it('merges one account reported natively on two environments and by a hub into one entry', () => {
    const native = provider({ driver: claude, instanceId: 'claude', auth: { status: 'authenticated', email: 'Same@example.com' },
      usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 40 }] } });
    const input = presentations([
      ['env-a', { ...laptop, serverConfig: { providers: [native] } }],
      ['env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: {
        providers: [{ ...native, usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [{ ...window, usedPercent: 55 }] } }],
        usageLimitSources: [{ ...source, accounts: [{ id: 'claude-same@example.com.json', driver: claude, email: 'same@example.com', plan: 'Claude Subscription',
          usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 10 }] } }] }],
      } }],
    ]);
    const accounts = collectLimitAccounts(input);
    expect(accounts).toHaveLength(1);
    expect(accounts[0]).toMatchObject({
      key: 'env-a:claude', sourceLabel: null,
      // Desktop's read is fresher, so its credits and its redeem are the ones on show.
      redeem: { environmentId: 'env-b', input: { instanceId: 'claude' } },
      environments: [{ environmentId: 'env-a', label: 'Laptop' }, { environmentId: 'env-b', label: 'Desktop' }],
    });
    // The fresher native snapshot wins; the hub row is pre-filtered by email.
    expect((accounts[0]?.limits.windows as Obj[])[0]?.usedPercent).toBe(55);
  });

  it('merges OpenCode Go limits from machines with the same API key', () => {
    const go = provider({ driver: 'opencode', instanceId: 'opencode', auth: { status: 'authenticated' },
      usageLimits: { checkedAt, credentialFingerprint: 'shared-go-key', windows: [{ ...window, id: 'go_rolling', usedPercent: 3 }] } });
    const input = new Map<string, LimitPresentation>([
      ['env-a', { ...laptop, serverConfig: { providers: [go] } }],
      ['env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [{ ...go,
        usageLimits: { ...(go.usageLimits as Obj), checkedAt: '2026-09-03T11:30:00.000Z', windows: [{ ...window, id: 'go_rolling', usedPercent: 4 }] } }] } }],
    ]);
    const accounts = collectLimitAccounts(input);
    expect(accounts).toHaveLength(1);
    expect(accounts[0]?.environments).toEqual([{ environmentId: 'env-a', label: 'Laptop' }, { environmentId: 'env-b', label: 'Desktop' }]);
    expect(collectLimitPools(accounts, now)[0]?.windows[0]?.members).toHaveLength(1);
    expect((accounts[0]?.limits.windows as Obj[])[0]?.usedPercent).toBe(4);

    const differentKey = { ...go, usageLimits: { ...(go.usageLimits as Obj), credentialFingerprint: 'other-go-key' } };
    input.set('env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [differentKey] } });
    expect(collectLimitAccounts(input)).toHaveLength(2);

    input.set('env-a', { ...laptop, serverConfig: { providers: [{ ...go, auth: { status: 'authenticated', email: 'same@example.com' } }] } });
    input.set('env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [{ ...differentKey, auth: { status: 'authenticated', email: 'SAME@example.com' } }] } });
    expect(collectLimitAccounts(input)).toHaveLength(1);
  });

  it('takes windows from a fresher hub read but credits and redeem from the native instance', () => {
    const native = provider({ driver: claude, instanceId: 'claude', auth: { status: 'authenticated', email: 'same@example.com' },
      usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 40 }], resetCredits: { availableCount: 2 } } });
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [native], usageLimitSources: [{ ...source, accounts: [{
      id: 'claude-same@example.com.json', driver: claude, email: 'same@example.com',
      usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [{ ...window, usedPercent: 55 }] } }] }] } }]]);
    const [account] = collectLimitAccounts(input);
    expect((account?.limits.windows as Obj[])[0]?.usedPercent).toBe(55);
    expect((account?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(2);
    expect(account?.redeem).toEqual({ environmentId: 'env-a', input: { instanceId: 'claude' } });
    expect(account?.environments).toEqual([{ environmentId: 'env-a', label: 'Laptop' }]);
  });

  it('redeems through the hub when it holds a credit, even with a fresher native read', () => {
    const native = provider({ driver: claude, instanceId: 'claude', auth: { status: 'authenticated', email: 'same@example.com' },
      usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [{ ...window, usedPercent: 40 }], resetCredits: { availableCount: 3, nextCreditId: 'native-credit' } } });
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [native], usageLimitSources: [{ ...source, accounts: [{
      id: 'claude-same@example.com.json', driver: claude, email: 'same@example.com',
      usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 55 }], resetCredits: { availableCount: 2, nextCreditId: 'hub-credit' } } }] }] } }]]);
    const [account] = collectLimitAccounts(input);
    // Only the hub path clears the routing cooldown it holds for this account.
    expect(account?.redeem).toEqual({ environmentId: 'env-a', input: { sourceId: 'hub', accountId: 'claude-same@example.com.json', creditId: 'hub-credit' } });
    // The fresher native balance is still the one shown.
    expect((account?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(3);
  });

  it('redeems on the environment whose snapshot supplied the credits on show', () => {
    const stale = provider({ auth: { status: 'authenticated', email: 'same@example.com' }, usageLimits: { checkedAt, windows: [window], resetCredits: { availableCount: 0 } } });
    const fresh = { ...stale, usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [window], resetCredits: { availableCount: 2 } } };
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [stale] } }], ['env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [fresh] } }]]);
    const [account] = collectLimitAccounts(input);
    expect((account?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(2);
    expect(account?.redeem).toEqual({ environmentId: 'env-b', input: { instanceId: 'codex' } });
  });

  it('uses the freshest hub credit and its environment even when the account is also native', () => {
    const native = provider({ auth: { status: 'authenticated', email: 'same@example.com' }, usageLimits: { checkedAt, windows: [window], resetCredits: { availableCount: 1 } } });
    const hubAccount = { id: 'codex-same.json', driver: native.driver, email: 'same@example.com',
      usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [window], resetCredits: { availableCount: 2, nextCreditId: 'credit-2' } } };
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [native] } }],
      ['env-b', { ...laptop, serverConfig: { providers: [], usageLimitSources: [{ ...source, accounts: [hubAccount] }] } }]]);
    const [account] = collectLimitAccounts(input);
    expect((account?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(2);
    expect(account?.redeem).toEqual({ environmentId: 'env-b', input: { sourceId: 'hub', accountId: 'codex-same.json', creditId: 'credit-2' } });
    hubAccount.usageLimits.resetCredits.availableCount = 0;
    expect((collectLimitAccounts(input)[0]?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(0);
  });

  it('keeps distinct hub accounts redeemable through their own source', () => {
    const hubAccounts = ['first', 'second'].map(id => ({ id, driver: 'codex', email: `${id}@example.com`,
      usageLimits: { checkedAt, windows: [window], resetCredits: { availableCount: 2, nextCreditId: `${id}-credit` } } }));
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [], usageLimitSources: [{ ...source, accounts: hubAccounts }] } }]]);
    expect(collectLimitAccounts(input).map(account => account.redeem)).toEqual(hubAccounts.map(account => ({
      environmentId: 'env-a', input: { sourceId: 'hub', accountId: account.id, creditId: `${account.id}-credit` } })));
  });

  it('does not give old credits the timestamp of a newer window-only read', () => {
    const snapshots = [
      { checkedAt, windows: [window], resetCredits: { availableCount: 2 } },
      { checkedAt: '2026-09-03T12:00:00.000Z', windows: [window] },
      { checkedAt: '2026-09-03T11:30:00.000Z', windows: [window], resetCredits: { availableCount: 1 } },
    ];
    const input = new Map<string, LimitPresentation>(snapshots.map((usageLimits, i) => [`env-${i}`,
      { ...laptop, serverConfig: { providers: [provider({ auth: { status: 'authenticated', email: 'same@example.com' }, usageLimits })] } }]));
    const [account] = collectLimitAccounts(input);
    expect(account?.limits.checkedAt).toBe('2026-09-03T12:00:00.000Z');
    expect((account?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(1);
    expect(account?.redeem?.environmentId).toBe('env-2');
  });

  it("keeps one email in two workspaces apart while merging each workspace's duplicates", () => {
    const login = (instanceId: string, workspaceId: string, usedPercent: number) => provider({ instanceId,
      auth: { status: 'authenticated', email: 'same@example.com', workspaceId }, usageLimits: { checkedAt, windows: [{ ...weekly, usedPercent }] } });
    const hubAccount = (id: string, workspaceId?: string): Obj => ({ id, driver: 'codex', email: 'same@example.com', ...(workspaceId ? { workspaceId } : {}),
      usageLimits: { checkedAt, windows: [{ ...weekly, usedPercent: 50 }] } });
    const input = presentations([
      ['env-a', { ...laptop, serverConfig: { providers: [login('business', 'ws-business', 8), login('personal', 'ws-plus', 26)],
        // A hub that names the workspace joins it; one that does not cannot say which of the two it read, so it keeps its own row.
        usageLimitSources: [{ ...source, accounts: [hubAccount('plus.json', 'ws-plus'), hubAccount('x.json')] }] } }],
      ['env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [login('personal', 'ws-plus', 26)] } }],
    ]);
    const accounts = collectLimitAccounts(input);
    expect(accounts.map(account => account.key)).toEqual(['env-a:business', 'env-a:personal', 'hub:x.json']);
    expect((accounts[0]?.limits.windows as Obj[])[0]?.usedPercent).toBe(8);
    // One hub file name in two workspaces stays two accounts, even without an email to tell them apart.
    const withoutEmail = (id: string, workspaceId: string) => { const { email: _email, ...account } = hubAccount(id, workspaceId); return account; };
    const sameFile = presentations([
      ['env-a', { ...laptop, serverConfig: { usageLimitSources: [{ ...source, accounts: [withoutEmail('a.json', 'ws-1')] }] } }],
      ['env-b', { ...laptop, serverConfig: { usageLimitSources: [{ ...source, accounts: [withoutEmail('a.json', 'ws-2')] }] } }],
    ]);
    expect(collectLimitAccounts(sameFile).map(account => account.key)).toEqual(['hub:a.json:ws-1', 'hub:a.json:ws-2']);
    expect(accounts[1]?.environments.map(environment => environment.label)).toEqual(['Laptop', 'Desktop']);
  });

  it('joins a report without a workspace to the only workspace signed in with its email', () => {
    const native = provider({ driver: claude, auth: { status: 'authenticated', email: 'same@example.com', workspaceId: 'org-a' }, usageLimits: { checkedAt, windows: [window] } });
    const hubAccount = { id: 'claude.json', driver: claude, email: 'same@example.com', usageLimits: { checkedAt, windows: [window] } };
    const withOther = (other: Obj) => presentations([['env-a', { ...laptop, serverConfig: { providers: [native, other], usageLimitSources: [{ ...source, accounts: [hubAccount] }] } }]]);
    const unrelated = provider({ instanceId: 'codex-2' });
    expect(collectLimitAccounts(withOther(unrelated)).map(account => account.key)).toEqual(['env-a:codex']);
    // A second org whose limits could not be read still makes the hub ambiguous.
    const unreadable = provider({ ...native, instanceId: 'team', auth: { ...(native.auth as Obj), workspaceId: 'org-b' },
      usageLimits: { checkedAt, windows: [], unavailable: { reason: 'probeFailed' } } });
    expect(collectLimitAccounts(withOther(unreadable)).map(account => account.key)).toEqual(['env-a:codex', 'hub:claude.json']);
  });

  it('names an environment once however many of its instances share the account', () => {
    const shared = provider({ auth: { status: 'authenticated', email: 'same@example.com' }, usageLimits: { checkedAt, windows: [window] } });
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [shared, { ...shared, instanceId: 'work' }] } }]]);
    expect(collectLimitAccounts(input)[0]?.environments).toEqual([{ environmentId: 'env-a', label: 'Laptop' }]);
  });

  it('keys a hub account without an email by hub, so two environments on one hub share it', () => {
    const seat = { id: 'claude-team-seat.json', driver: claude, usageLimits: { checkedAt, windows: [window] } };
    const hub = { ...source, accounts: [seat] };
    const input = presentations([['env-a', { ...laptop, serverConfig: { usageLimitSources: [hub] } }], ['env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { usageLimitSources: [hub] } }]]);
    const accounts = collectLimitAccounts(input);
    expect(accounts.map(account => account.key)).toEqual(['hub:claude-team-seat.json']);
    expect(accounts[0]?.displayName).toBe('claude-team-seat');
  });

  it('pools Codex windows by kind, whichever slot reported them', () => {
    const codexAccount = (id: string, slots: readonly string[]): Obj => ({ id, driver: 'codex',
      usageLimits: { checkedAt, windows: slots.map(slot => ({ ...weekly, id: slot, usedPercent: 50 })) } });
    const pooled = (accounts: Obj[]) => collectLimitPools(collectLimitAccounts(presentations([['env-a', { ...laptop, serverConfig: { usageLimitSources: [{ ...source, accounts }] } }]])), now)[0]?.windows ?? [];
    // A weekly-only plan reports weekly as `primary`. The pool keeps one id whichever account sorts first.
    for (const order of [[codexAccount('business', ['primary']), codexAccount('plus', ['secondary'])], [codexAccount('plus', ['secondary']), codexAccount('business', ['primary'])]]) {
      const windows = pooled(order);
      expect(windows.map(window => window.id)).toEqual(['weekly']);
      expect(windows[0]?.members).toHaveLength(2);
    }
    // A second window of the same kind on one account stays apart, with its own id.
    expect(pooled([codexAccount('a', ['secondary']), codexAccount('b', ['primary', 'secondary'])]).map(window => window.id)).toEqual(['weekly', 'weekly:secondary']);
  });

  it('pools windows by id across accounts and orders resets by when they land', () => {
    const input = presentations([['env-a', { ...laptop, serverConfig: { providers: [], usageLimitSources: [{ ...source, accounts: [
      { id: 'a', driver: claude, usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 80, resetsAt: '2026-09-03T13:00:00.000Z' }, { ...weekly, usedPercent: 20 }] } },
      { id: 'b', driver: claude, usageLimits: { checkedAt, windows: [{ ...window, usedPercent: 40 }] } },
      { id: 'c', driver: 'codex', usageLimits: { checkedAt, windows: [{ ...weekly, usedPercent: 50 }] } },
      { id: 'unsupported', driver: claude, usageLimits: { checkedAt, windows: [], unavailable: { reason: 'unsupported' as const } } },
    ] }] } }]]);
    const pools = collectLimitPools(collectLimitAccounts(input), now);
    expect(pools.map(pool => [pool.driver, pool.accounts.length])).toEqual([['claudeAgent', 2], ['codex', 1]]);
    const [session, week] = pools[0]!.windows;
    // A member with no reset has no clock, so it does not vote on pace.
    const untimed = collectLimitPools(collectLimitAccounts(input).map(account => account.key === 'hub:b'
      ? { ...account, limits: { ...account.limits, windows: (account.limits.windows as Obj[]).map(w => ({ ...w, resetsAt: undefined })) } } : account), now);
    // Only a votes: 80% used, 80% elapsed.
    expect(untimed[0]?.windows[0]?.pace).toBe('on');
    // a is 80% through its window and b 60%: the pool is 70% elapsed, 60% used.
    expect(session).toMatchObject({ id: 'five_hour', remainingPercent: 40, usedPercent: 60, pace: 'under' });
    expect(session?.resets.map(reset => [reset.member.account.key, reset.restoresPercent])).toEqual([['hub:a', 40], ['hub:b', 20]]);
    expect(week).toMatchObject({ id: 'seven_day', remainingPercent: 80, members: [{}] });
    // Codex reports `primary` for both its five-hour and (on Go) monthly window.
    const mixed = collectLimitPools([...collectLimitAccounts(input), {
      key: 'go', driver: claude, displayName: 'Go', email: undefined, plan: undefined, accentColor: undefined, environments: [], sourceLabel: null, redeem: null,
      limits: { checkedAt, windows: [{ id: 'five_hour', kind: 'monthly', label: 'Monthly', usedPercent: 82, windowDurationMins: 30 * 24 * 60, resetsAt: '2026-09-14T12:00:00.000Z' }] },
    }], now);
    expect(mixed[0]?.windows.map(window => [window.kind, window.members.length])).toEqual([['session', 2], ['weekly', 1], ['monthly', 1]]);
    // Session resets determine the account order for every row.
    expect(session?.members.map(member => member.account.key)).toEqual(['hub:a', 'hub:b']);
    expect(pools[0]?.accounts.map(account => account.key)).toEqual(['hub:a', 'hub:b']);
  });
});

describe('pooled account columns', () => {
  const weekly = { ...window, id: 'seven_day', kind: 'weekly', label: 'Weekly', windowDurationMins: 7 * 24 * 60 } as const;
  const account = (key: string, windows: Obj[]): LimitAccount => ({ key, driver: 'claudeAgent', displayName: key, email: undefined, plan: undefined, accentColor: undefined,
    environments: [], sourceLabel: 'Hub', redeem: null, limits: { checkedAt: '2026-09-03T11:00:00.000Z', windows } });
  const keys = (pool: ReturnType<typeof collectLimitPools>[number]) => pool.windows.map(row => row.columns.map(member => (member.window ? member.account.key : null)));

  // regression: #20349 sessions stay visible while the weekly quota is exhausted
  it.each([
    ['claudeAgent', '13:00', 40, '14:00', 100],
    ['codex', '14:00', 100, '14:00', 100],
    ['claudeAgent', '15:00', 100, '15:00', 100],
    ['codex', '15:00', 40, '14:00', 60],
  ] as const)('%s session resetting at %s with %i percent spent becomes usable at %s with %i percent restored', (driver, sessionReset, usedPercent, usableAt, restoresPercent) => {
    const at = (time: string) => `2026-09-03T${time}:00.000Z`;
    const a = { ...account('a', [
      { ...window, id: driver === 'codex' ? 'primary' : window.id, usedPercent, resetsAt: at(sessionReset) },
      { ...weekly, id: driver === 'codex' ? 'secondary' : weekly.id, usedPercent: 100, resetsAt: at('14:00') },
    ]), driver };
    const [session] = collectLimitPools([a], now)[0]!.windows;
    expect(session?.remainingPercent).toBe(0);
    expect(session?.resets[0]).toMatchObject({ at: Date.parse(at(usableAt)), restoresPercent });
  });

  it('excludes weekly-blocked sessions from the pool without changing snapshots or model allowances', () => {
    const blocked = account('blocked', [window, { ...weekly, usedPercent: 100 }, { ...weekly, id: 'seven_day_fable', usedPercent: 20 }]);
    const accounts = [
      blocked,
      account('available', [{ ...window, usedPercent: 20 }, { ...weekly, usedPercent: 99 }, { ...weekly, id: 'seven_day_fable', usedPercent: 40 }]),
      { ...blocked, key: 'cursor', driver: 'cursor' },
    ];
    const snapshot = structuredClone(accounts);
    const [claude, cursor] = collectLimitPools(accounts, now);
    expect(claude?.windows.map(row => row.remainingPercent)).toEqual([40, 1, 70]);
    expect(cursor?.windows.map(row => row.remainingPercent)).toEqual([60, 0, 80]);
    expect(accounts).toEqual(snapshot);
  });

  it('keeps session columns across rows with opposite reset and usage orders', () => {
    const accounts = [
      account('a', [{ ...weekly, usedPercent: 80, resetsAt: '2026-09-05T12:00:00.000Z' }, { ...window, usedPercent: 10, resetsAt: '2026-09-03T15:00:00.000Z' }]),
      account('b', [{ ...weekly, usedPercent: 20, resetsAt: '2026-09-06T12:00:00.000Z' }, { ...window, usedPercent: 90, resetsAt: '2026-09-03T13:00:00.000Z' }]),
    ];
    const [pool] = collectLimitPools(accounts, now);
    expect(pool!.accounts.map(entry => entry.key)).toEqual(['b', 'a']);
    expect(keys(pool!)).toEqual([['b', 'a'], ['b', 'a']]);
    expect(pool!.windows[1]!.resets.map(reset => reset.member.account.key)).toEqual(['a', 'b']);
    expect(pool!.windows[1]!.remainingPercent).toBe(50);
    expect(keys(collectLimitPools(accounts.toReversed(), now)[0]!)).toEqual(keys(pool!));
  });

  it.each([undefined, 'invalid', '2026-09-03T11:30:00.000Z'])('keeps blocked sessions empty without predicting a refill from a missing or stale clock (%s)', resetsAt => {
    const [pool] = collectLimitPools([
      account('session-clock', [{ ...window, resetsAt }, { ...weekly, usedPercent: 100 }]),
      account('weekly-clock', [window, { ...weekly, usedPercent: 100, resetsAt }]),
    ], now);
    expect(pool!.windows[0]).toMatchObject({ remainingPercent: 0, pace: null, resets: [] });
    expect(pool!.windows[0]!.columns.map(column => column.window?.resetsAt)).toEqual([undefined, undefined]);
  });

  it('orders by usable session refill and clears suppression after a fresh weekly reading', () => {
    const blocked = account('blocked', [{ ...window, resetsAt: '2026-09-03T13:00:00.000Z' }, { ...weekly, usedPercent: 100, resetsAt: '2026-09-03T15:00:00.000Z' }]);
    const available = account('available', [window]);
    const [before] = collectLimitPools([blocked, available], now);
    expect(before!.accounts.map(entry => entry.key)).toEqual(['available', 'blocked']);
    expect(before!.windows[0]!.remainingPercent).toBe(30);
    const refreshed = { ...blocked, limits: { checkedAt: '2026-09-03T12:00:00.000Z', windows: [(blocked.limits.windows as Obj[])[0]!, { ...weekly, usedPercent: 10 }] } };
    const [after] = collectLimitPools([refreshed, available], now);
    expect(after!.accounts.map(entry => entry.key)).toEqual(['blocked', 'available']);
    expect(after!.windows[0]!.remainingPercent).toBe(60);
    expect(after!.windows[0]!.resets[0]).toMatchObject({ at: Date.parse('2026-09-03T13:00:00.000Z'), restoresPercent: 20 });
  });

  it('preserves gaps without counting missing windows toward pooled quota', () => {
    const [pool] = collectLimitPools([account('a', [window]), account('b', [{ ...window, resetsAt: '2026-09-03T15:00:00.000Z' }, { ...weekly, usedPercent: 80 }]), account('c', [weekly])], now);
    expect(keys(pool!)).toEqual([['a', 'b', null], [null, 'b', 'c']]);
    expect(pool!.windows[1]!.members.map(member => member.account.key)).toEqual(['b', 'c']);
    expect(pool!.windows[1]!.remainingPercent).toBe(40);
    expect(pool!.windows[1]!.resets.map(reset => reset.restoresPercent)).toEqual([40, 20]);
  });

  it('falls back to weekly resets when no account reports a session', () => {
    const [pool] = collectLimitPools([account('a', [{ ...weekly, resetsAt: '2026-09-06T12:00:00.000Z' }]), account('b', [{ ...weekly, resetsAt: '2026-09-05T12:00:00.000Z' }])], now);
    expect(keys(pool!)).toEqual([['b', 'a']]);
  });

  it('sorts unknown resets last and breaks ties consistently', () => {
    const accounts = [account('z', [{ ...window, resetsAt: undefined }]), account('b', [window]), account('a', [window]), account('y', [{ ...window, resetsAt: 'invalid' }])];
    expect(keys(collectLimitPools(accounts, now)[0]!)).toEqual([['a', 'b', 'y', 'z']]);
    expect(keys(collectLimitPools(accounts.toReversed(), now)[0]!)).toEqual([['a', 'b', 'y', 'z']]);
  });
});

describe('Cursor limit presentation', () => {
  const cursorAccount: LimitAccount = { key: 'cursor', driver: 'cursor', displayName: 'Cursor', email: undefined, plan: undefined, accentColor: undefined, environments: [],
    sourceLabel: 'Cursor', redeem: null, limits: { checkedAt: '2026-09-03T11:00:00.000Z', windows: [
      { id: 'apiPercentUsed', kind: 'monthly', label: 'Other Models', usedPercent: 49 },
      { id: 'autoPercentUsed', kind: 'monthly', label: 'Cursor Models', usedPercent: 9 },
      { id: 'totalPercentUsed', kind: 'monthly', label: 'Overall', usedPercent: 15 },
    ] } };

  it('hides the combined percentage and orders the two pools', () => {
    const [pool] = collectLimitPools([cursorAccount], now);
    expect(displayLimitWindows(pool!).map(entry => entry.id)).toEqual(['autoPercentUsed', 'apiPercentUsed']);
  });

  it('keeps the combined percentage as a card if either allowance is missing', () => {
    const [pool] = collectLimitPools([{ ...cursorAccount, limits: { ...cursorAccount.limits, windows: (cursorAccount.limits.windows as Obj[]).filter(entry => entry.id !== 'apiPercentUsed') } }], now);
    expect(displayLimitWindows(pool!).map(entry => entry.id)).toEqual(['totalPercentUsed', 'autoPercentUsed']);
  });
});

describe('collectLimitNotices', () => {
  const checkedAt = '2026-09-03T11:00:00.000Z';
  const claude = 'claudeAgent';
  const laptop = { entry: { target: { label: 'Laptop' } } };
  const hub = { id: 'hub', kind: 'cliproxy' as const, label: 'hub', checkedAt, accounts: [] };

  it('names failures and silence, skips unsupported accounts, and labels environments only when several', () => {
    const failed = provider({ instanceId: 'claude', driver: claude, displayName: 'Claude Max', usageLimits: { checkedAt, windows: [], unavailable: { reason: 'probeFailed' } } });
    const apiKey = provider({ instanceId: 'api', driver: claude, usageLimits: { checkedAt, windows: [], unavailable: { reason: 'unsupported' } } });
    const silent = provider({ usageLimits: { checkedAt, windows: [] } });
    const one = new Map<string, LimitPresentation>([['env-a', { ...laptop, serverConfig: { providers: [failed, apiKey, silent],
      usageLimitSources: [hub, { ...hub, id: 'down', label: 'down', error: 'ECONNREFUSED' }] } }]]);
    expect(collectLimitNotices(one)).toEqual(['Claude Max: Could not read limits.', 'codex: No limits reported.', 'hub: No accounts reported.', 'down: ECONNREFUSED']);

    one.set('env-b', { entry: { target: { label: 'Desktop' } }, serverConfig: { providers: [], usageLimitSources: [] } });
    expect(collectLimitNotices(one)[0]).toBe('Laptop · Claude Max: Could not read limits.');
  });
});

describe('external usage settings', () => {
  it('deduplicates destinations across accounts and environments without inventing quota pools', () => {
    const managed = provider({ usageLimits: { checkedAt: '2026-09-03T11:00:00.000Z', windows: [], unavailable: { reason: 'unsupported', message: 'Track usage in ChatGPT.' },
      externalUsage: { label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage' } } });
    const input = presentations([['a', { entry: { target: { label: 'A' } }, serverConfig: { providers: [managed, { ...managed, instanceId: 'personal' }] } }],
      ['b', { entry: { target: { label: 'B' } }, serverConfig: { providers: [managed] } }]]);
    expect(collectExternalUsageLinks(input)).toEqual([{ label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage', message: 'Track usage in ChatGPT.',
      accounts: [`${String(managed.instanceId)} on A`, 'personal on A', `${String(managed.instanceId)} on B`] }]);
    expect(collectLimitAccounts(input)).toEqual([]);
    expect(collectLimitNotices(input)).toEqual([]);
  });
  it('omits disabled, uninstalled and signed-out providers', () => {
    const managed = provider({ usageLimits: { checkedAt: '2026-09-03T11:00:00.000Z', windows: [], externalUsage: { label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage' } } });
    const input = presentations([['a', { entry: { target: { label: 'A' } }, serverConfig: { providers: [{ ...managed, enabled: false }, { ...managed, installed: false },
      { ...managed, auth: { status: 'unauthenticated' } }, provider({})] } }]]);
    expect(collectExternalUsageLinks(input)).toEqual([]);
  });
});

describe('account chips (UsageLimitsPooled.tsx, no reference test)', () => {
  test('two letters from the local part and the domain, a stable hue per address', () => {
    expect(accountInitials('someone@example.com')).toBe('SE');
    expect(accountInitials('')).toBe('?');
    expect(accountHue('someone@example.com')).toBe(accountHue('someone@example.com'));
    expect(accountHue('a@b.c')).toBeGreaterThanOrEqual(0);
    expect(accountHue('a@b.c')).toBeLessThan(360);
  });
});
