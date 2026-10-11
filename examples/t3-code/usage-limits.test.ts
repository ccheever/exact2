// Ported from T3 Code 1e2ecbd975 packages/shared/src/usageLimits.test.ts (MIT, see LICENSE-T3):
// "pace" (:58), "limitsNotice" (:83), "providersWithLimits" (:101), "/usage-limits"
// (:863-1072, six cases), "remainingPercent" (:1112) and "isUsageLimitsCommand" (:1121), with
// their original names. Changes: ids are plain strings (no ProviderInstanceId.make), and
// vite-plus/test is bun:test.
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import {
  collectProviderUsageLimits, elapsedShare, formatResetsIn, isUsageLimitsCommand, limitsNotice, paceOf, providersWithLimits,
  remainingPercent, withUsageLimitsCommands,
} from './usage-limits';

const now = Date.parse('2026-09-03T12:00:00.000Z');

const window = {
  id: 'five_hour',
  kind: 'session',
  label: 'Session',
  usedPercent: 40,
  windowDurationMins: 300,
  resetsAt: '2026-09-03T14:00:00.000Z',
} as const;

function provider(overrides: Obj): Obj {
  return {
    instanceId: 'codex',
    driver: 'codex',
    enabled: true,
    installed: true,
    version: null,
    status: 'ready',
    auth: { status: 'authenticated' },
    checkedAt: '2026-09-03T11:00:00.000Z',
    models: [],
    slashCommands: [],
    skills: [],
    ...overrides,
  };
}

describe('pace', () => {
  it('places the clock three fifths through a five-hour window with two hours left', () => {
    expect(elapsedShare(window, now)).toBeCloseTo(0.6);
    expect(paceOf(window, now)).toBe('under');
    expect(paceOf({ ...window, usedPercent: 62 }, now)).toBe('on');
    expect(paceOf({ ...window, usedPercent: 80 }, now)).toBe('ahead');
  });

  it('has no pace without a reset or a duration', () => {
    expect(paceOf({ ...window, resetsAt: undefined }, now)).toBeNull();
    expect(paceOf({ ...window, windowDurationMins: undefined }, now)).toBeNull();
    expect(formatResetsIn({ ...window, resetsAt: undefined }, now)).toBeNull();
  });

  it('phrases the reset as a countdown', () => {
    expect(formatResetsIn(window, now)).toBe('resets in 2h 0m');
    expect(formatResetsIn({ ...window, resetsAt: '2026-09-06T15:30:00.000Z' }, now)).toBe('resets in 3d 3h');
    expect(formatResetsIn({ ...window, resetsAt: '2026-09-03T11:00:00.000Z' }, now)).toBe('resets now');
  });
});

describe('limitsNotice', () => {
  it('explains empty bars and passes provider messages through', () => {
    const checkedAt = '2026-09-03T11:00:00.000Z';
    expect(limitsNotice({ checkedAt, windows: [window] })).toBeNull();
    expect(limitsNotice({ checkedAt, windows: [] })).toBe('No limits reported.');
    expect(limitsNotice({ checkedAt, windows: [], unavailable: { reason: 'unsupported' } })).toBe('This account has no subscription limits.');
    expect(limitsNotice({ checkedAt, windows: [], unavailable: { reason: 'probeFailed', message: 'Codex timed out.' } })).toBe('Codex timed out.');
  });
});

describe('providersWithLimits', () => {
  it('keeps only usable providers whose driver reports limits at all', () => {
    const limits = { checkedAt: '2026-09-03T11:00:00.000Z', windows: [window] };
    const codex = provider({ usageLimits: limits });
    expect(providersWithLimits([
      codex,
      provider({ instanceId: 'cursor', driver: 'cursor' }),
      provider({ instanceId: 'off', enabled: false, usageLimits: limits }),
      provider({ instanceId: 'gone', installed: false, usageLimits: limits }),
      provider({ instanceId: 'shadow', availability: 'unavailable', usageLimits: limits }),
    ])).toEqual([codex]);
  });
});

describe('/usage-limits', () => {
  const limits = { checkedAt: '2026-09-03T11:00:00.000Z', windows: [window] };
  const selected = provider({ usageLimits: limits, auth: { status: 'authenticated', email: 'same@example.com' } });
  const sources = [
    {
      id: 'hub',
      kind: 'cliproxy' as const,
      label: 'Accounts',
      checkedAt: limits.checkedAt,
      accounts: [
        { id: 'duplicate', driver: selected.driver, email: 'SAME@example.com', usageLimits: limits },
        { id: 'oss', driver: selected.driver, plan: 'Codex OSS', usageLimits: limits },
        { id: 'other-provider', driver: 'claude', usageLimits: limits },
      ],
    },
  ];

  it('uses hub credit balances and redemption targets in the composer, including native duplicates', () => {
    const hubs = sources.map(source => ({
      ...source,
      accounts: source.accounts.map(account => ({
        ...account,
        usageLimits: { ...account.usageLimits, resetCredits: { availableCount: 2, nextCreditId: `${account.id}-credit` } },
      })),
    }));
    const report = collectProviderUsageLimits(String(selected.instanceId), [selected], hubs, now);
    expect((report?.accounts[0]?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(2);
    expect(report?.accounts[0]?.resetCreditInput).toEqual({ sourceId: 'hub', accountId: 'duplicate', creditId: 'duplicate-credit' });
    expect(report?.accounts.find(account => account.id === 'hub:oss')?.resetCreditInput).toEqual({ sourceId: 'hub', accountId: 'oss', creditId: 'oss-credit' });
  });

  it('redeems a native duplicate through the hub even when the native snapshot is fresher', () => {
    const fresher = provider({
      usageLimits: { checkedAt: '2026-09-03T11:30:00.000Z', windows: [window], resetCredits: { availableCount: 3, nextCreditId: 'native-credit' } },
      auth: { status: 'authenticated', email: 'same@example.com' },
    });
    const stale = [{
      id: 'hub', kind: 'cliproxy' as const, label: 'Accounts', checkedAt: limits.checkedAt,
      accounts: [{ id: 'duplicate', driver: fresher.driver, email: 'SAME@example.com', usageLimits: { ...limits, resetCredits: { availableCount: 2, nextCreditId: 'hub-credit' } } }],
    }];
    const report = collectProviderUsageLimits(String(fresher.instanceId), [fresher], stale, now);
    // Only redeeming through the hub clears the routing cooldown it holds for
    // this account, so the hub wins the path even with a staler balance.
    expect(report?.accounts[0]?.resetCreditInput).toEqual({ sourceId: 'hub', accountId: 'duplicate', creditId: 'hub-credit' });
    // The fresher native balance is still the one shown.
    expect((report?.accounts[0]?.limits.resetCredits as Obj | undefined)?.availableCount).toBe(3);
  });

  it('keeps accounts and custom instances separate, filtering by driver', () => {
    const report = collectProviderUsageLimits(String(selected.instanceId), [
      selected,
      provider({ instanceId: 'codex-work', displayName: 'Work', usageLimits: { ...limits, resetCredits: { availableCount: 2 } } }),
      provider({ driver: 'claude', instanceId: 'claude', usageLimits: limits }),
    ], sources, now);
    expect(report?.createdAt).toBe('2026-09-03T12:00:00.000Z');
    expect(report?.accounts.map(account => account.id)).toEqual(['codex', 'codex-work', 'hub:oss']);
    expect(report?.accounts[0]).toMatchObject({ instanceId: selected.instanceId, email: (selected.auth as Obj).email });
    expect(report?.accounts[1]).toMatchObject({ displayName: 'Work', limits: { resetCredits: { availableCount: 2 } } });
    expect(report?.accounts[2]).toMatchObject({ label: 'Accounts · oss', sourceLabel: 'CLI Proxy', plan: 'Codex OSS' });
    expect(report?.notices).toEqual([]);
  });

  it('supports a source-only provider and keeps duplicates when the native probe failed', () => {
    expect(collectProviderUsageLimits(String(selected.instanceId), [provider({})], sources, now)?.accounts.map(account => account.id)).toEqual(['hub:duplicate', 'hub:oss']);
    const failed = provider({ usageLimits: { ...limits, unavailable: { reason: 'probeFailed' } } });
    expect(collectProviderUsageLimits(String(selected.instanceId), [failed], sources, now)?.accounts.map(account => account.id)).toEqual(['codex', 'hub:duplicate', 'hub:oss']);
    expect(collectProviderUsageLimits(String(selected.instanceId), [provider({})], [], now)).toBeNull();
    expect(collectProviderUsageLimits(String(selected.instanceId), [provider({ enabled: false, usageLimits: limits })], [], now)).toBeNull();
  });

  it('surfaces source errors only for sources that carry the selected driver', () => {
    const failing = { ...sources[0]!, error: 'token expired' };
    expect(collectProviderUsageLimits(String(selected.instanceId), [selected], [failing], now)?.notices).toEqual(['Accounts: token expired']);
    const claudeOnly = { ...failing, accounts: failing.accounts.slice(2) };
    expect(collectProviderUsageLimits(String(selected.instanceId), [selected], [claudeOnly], now)?.notices).toEqual([]);
    // A read failure clears the accounts, so the error must not depend on a match.
    const unreadable = { ...failing, accounts: [] };
    expect(collectProviderUsageLimits(String(selected.instanceId), [selected], [unreadable], now)?.notices).toEqual(['Accounts: token expired']);
    // A source-only provider still gets the report, carrying only the error.
    const sourceOnly = collectProviderUsageLimits(String(selected.instanceId), [provider({})], [unreadable], now);
    expect(sourceOnly?.accounts).toEqual([]);
    expect(sourceOnly?.notices).toEqual(['Accounts: token expired']);
  });

  it('advertises global and workspace commands only for providers present in Limits', () => {
    const withWorkspace = provider({ workspaceSnapshots: [{ cwd: '/tmp/project', checkedAt: limits.checkedAt, slashCommands: [], skills: [] }] });
    const [supported] = withUsageLimitsCommands([withWorkspace], sources);
    expect((supported?.slashCommands as Obj[]).map(command => command.name)).toEqual(['usage-limits']);
    expect(((supported?.workspaceSnapshots as Obj[])[0]?.slashCommands as Obj[]).map(command => command.name)).toEqual(['usage-limits']);
    expect(withUsageLimitsCommands([withWorkspace], [])[0]?.slashCommands).toEqual([]);
    // A provider's own command of the same name is left alone without coverage.
    const ownCommand = provider({ slashCommands: [{ name: 'usage-limits', description: "Provider's own" }] });
    expect(withUsageLimitsCommands([ownCommand], [])[0]?.slashCommands).toEqual([{ name: 'usage-limits', description: "Provider's own" }]);
    const unreadable = { ...sources[0]!, accounts: [], error: 'token expired' };
    expect((withUsageLimitsCommands([withWorkspace], [unreadable])[0]?.slashCommands as Obj[]).map(command => command.name)).toEqual(['usage-limits']);
    expect((withUsageLimitsCommands([selected], [])[0]?.slashCommands as Obj[]).map(command => command.name)).toEqual(['usage-limits']);
  });
});

describe('remainingPercent', () => {
  it('inverts and clamps the reported usage', () => {
    expect(remainingPercent(window)).toBe(60);
    expect(remainingPercent({ ...window, usedPercent: 0 })).toBe(100);
    expect(remainingPercent({ ...window, usedPercent: 100 })).toBe(0);
    expect(remainingPercent({ ...window, usedPercent: 33.4 })).toBe(67);
  });
});

describe('isUsageLimitsCommand', () => {
  it('recognizes only the standalone local action', () => {
    expect(isUsageLimitsCommand('  /USAGE-LIMITS\n')).toBe(true);
    expect(isUsageLimitsCommand('/usage-limits explain')).toBe(false);
    expect(isUsageLimitsCommand('Explain /usage-limits')).toBe(false);
    expect(isUsageLimitsCommand('/usage')).toBe(false);
  });
});
