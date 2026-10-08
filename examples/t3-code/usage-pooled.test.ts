// The Usage page across environments (usage-environments.ts, usage-pooled-view.ts, usage-replies.ts)
// driven through `usagePage` with a focused client and a background environment of EnvironmentFleet.
// The last block ports T3 Code 1e2ecbd975 (MIT, see LICENSE-T3)
// apps/web/src/components/usage/UsagePage.refresh.test.tsx (5 cases, original names) as logic tests of
// the page with a `now` argument: a refresh press is the root's counter, Date.now is the answer's
// clock, a remount is the page closing and opening again.
import { afterEach, describe, expect, test } from 'bun:test';
import './client'; // the app's module order: pages-usage alone meets the r5/r6 measuring cycle first
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { composerReplyEvent } from './composer-replies';
import { usagePage } from './pages-usage';
import { pagesLocal } from './pages-commands';
import { fleet, environmentKey, EnvironmentFleet } from './settings-b-fleet';
import { initialShell } from './domain';
import { forgetUsageState } from './usage-environments';
import { popoverSides } from './usage-pooled-view';
import { usageFleetEvent, usageFleetReset } from './usage-replies';
import { forgetLimitsRefreshes } from './usage-refresh';

const it = test;
const NOW = Date.parse('2026-09-11T12:00:00Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const HOUR = 3_600_000;
const session = (usedPercent: number, extra: Obj = {}) => ({ id: 'five_hour', kind: 'session', label: 'Session', usedPercent, windowDurationMins: 300, resetsAt: iso(2 * HOUR), ...extra });
const weekly = (usedPercent: number) => ({ id: 'seven_day', kind: 'weekly', label: 'Weekly', usedPercent, windowDurationMins: 10_080, resetsAt: iso(3 * 24 * HOUR) });
const provider = (extra: Obj): Obj => ({ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, status: 'ready',
  auth: { status: 'authenticated', email: 'same@example.com', label: 'ChatGPT Pro' }, models: [], ...extra });
const summary = (extra: Obj = {}): Obj => ({ contractVersion: 6, readAt: iso(0), buckets: [], sources: [], ...extra });
const KEY_B = environmentKey('http://10.0.0.2:3773', 'env-b');

type Sent = { env: string; method: string; payload: Obj; deliver: string };
class Rig {
  sent: Sent[] = [];
  direct: Sent[] = [];
  opened: string[] = [];
  serial = 0;
  configA: Obj;
  client: T3Client;
  native: Native;
  constructor(configA: Obj, configB: Obj | null) {
    this.configA = configA;
    const rig = this;
    this.client = {
      environmentId: 'env-a', connection: 'connected', ready: true, generation: 1, revision: 0, presentation: {}, config: configA,
      local: { deviceSettings: { timestampFormat: '24-hour' } },
      async ids() { return [`id-${++rig.serial}`]; },
      async call(native: Native, request: unknown) { const reply = obj(await native.later(request)); return obj(reply.value); },
      async rpc(native: Native, method: string, payload: Obj) { const reply = obj(await native.later({ op: 'request', method, payload })); return obj(reply.value); },
    } as unknown as T3Client;
    this.native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = obj(input), env = str(request.fleet) ? 'env-b' : 'env-a';
      if (request.op === 'ids') return { ok: true, generation: 1, value: [`fleet-id-${++this.serial}`] };
      if (request.op === 'remoteEditorsOpen') { this.opened.push(str(request.url)); return { ok: true, generation: 1, value: {} }; }
      if (request.op === 'request' && str(request.deliver)) { this.sent.push({ env, method: str(request.method), payload: obj(request.payload), deliver: str(request.deliver) }); return { ok: true, generation: env === 'env-b' ? 7 : 1, value: { id: `rpc-${this.sent.length}` } }; }
      if (request.op === 'request') { this.direct.push({ env, method: str(request.method), payload: obj(request.payload), deliver: '' }); return { ok: true, generation: env === 'env-b' ? 7 : 1, value: {} }; }
      return { ok: false, generation: 1, error: { kind: 'transport', message: `unknown ${str(request.op)}`, uncertain: false } };
    } };
    fleet.entries.clear();
    if (configB) this.background(configB);
  }
  background(config: Obj, phase = 'connected') {
    fleet.entries.set(KEY_B, { key: KEY_B, origin: 'http://10.0.0.2:3773', environmentId: 'env-b', phase: phase as 'connected', message: '', traceId: '', generation: 7, synchronized: 7,
      lastEvent: 0, subscriptions: {}, config, shell: initialShell(), scopes: [], error: '', requested: true });
  }
  page(input: Partial<{ open: boolean; metric: string; windowDays: number; refresh: number; now: number; viewport: number; sidebar: number }> = {}) {
    const { open = true, metric = 'limits', windowDays = 30, refresh = 0, now = NOW, viewport = 1280, sidebar = 256 } = input;
    return usagePage(this.client, this.native, null, { open, metric, windowDays, breakdown: 'model', refresh, width: 600, now, viewport, sidebar });
  }
  /** The reply to a delivered request, heard by the drain of its environment's transport. */
  async reply(sent: Sent, value: Obj | { error: Obj }) {
    const item = 'error' in value && Object.keys(value).length === 1 ? { _replyError: { uncertain: false, ...obj(value.error) } } : { _reply: value };
    if (sent.env === 'env-a') composerReplyEvent(this.client, { key: sent.deliver, value: item });
    else usageFleetEvent({ key: KEY_B, generation: 7 }, { key: sent.deliver, value: item, generation: 7 });
    for (let index = 0; index < 12; index++) await Promise.resolve();
  }
  of(method: string, env?: string) { return this.sent.filter(entry => entry.method === method && (!env || entry.env === env)); }
  command(op: string, id = '', value = '') { return pagesLocal(this.client, this.native, { fs: {} } as never, op, id, value); }
}
afterEach(() => { fleet.entries.clear(); forgetLimitsRefreshes(); });

const laptop = () => ({ environment: { environmentId: 'env-a', label: 'Laptop' },
  providers: [provider({ usageLimits: { checkedAt: iso(-60_000), windows: [session(40), weekly(20)], resetCredits: { availableCount: 2, nextExpiresAt: iso(27 * 24 * HOUR) },
    externalUsage: { label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage' } } })],
  usageLimitSources: [
    { id: 'hub', kind: 'cliproxy', label: 'Team hub', checkedAt: iso(-60_000), accounts: [{ id: 'seat.json', driver: 'claudeAgent', email: 'seat@team.dev', plan: 'Claude Max',
      usageLimits: { checkedAt: iso(-60_000), windows: [session(70, { resetsAt: iso(HOUR) })] } }] },
    { id: 'down', kind: 'cliproxy', label: 'Old hub', checkedAt: iso(-60_000), accounts: [], error: 'token expired' },
  ] });
const desktop = () => ({ environment: { environmentId: 'env-b', label: 'Desktop' },
  providers: [provider({ usageLimits: { checkedAt: iso(-120_000), windows: [session(55), weekly(30)] } })] });

describe('pooled limits across environments', () => {
  it('pools one account signed in on two environments, names a hub-only account by its hub, and labels notices by environment', async () => {
    const rig = new Rig(laptop(), desktop());
    const page = await rig.page();
    const pooled = page.pooled;
    expect(page.environments.map(row => [row.key, row.label, row.checked, row.disabled, row.status])).toEqual([['env-a', 'Laptop', true, false, ''], ['env-b', 'Desktop', true, false, '']]);
    expect(pooled.pools.map(pool => [pool.driver, pool.label, pool.windows.map(window => window.label)])).toEqual([['codex', 'Codex', ['Session', 'Weekly']], ['claudeAgent', 'Claude', ['Session']]]);
    const codex = pooled.pools[0]!.windows[0]!.segments;
    expect(codex).toHaveLength(1); // one account for the shared email
    expect(codex[0]).toMatchObject({ remaining: 60, whereLabel: 'Signed in', where: 'Laptop, Desktop', plan: 'ChatGPT Pro', title: 'Codex', avatar: 'instance', credits: 2,
      redeemable: true, creditsText: '2 banked · expires in 27d 0h', left: '60%', resets: '↻ 2h 0m', label: 'Codex: 60% left, resets in 2h 0m, 2 reset credits banked', showName: false });
    expect(codex[0]!.email).toBe('same@example.com');
    expect(codex[0]!.emailMask).not.toContain('same');
    const hub = pooled.pools[1]!.windows[0]!.segments[0]!;
    expect(hub).toMatchObject({ whereLabel: 'Via', where: 'Laptop · Team hub', avatar: 'chip', chip: 'ST', name: '', redeemable: false, plan: 'Claude Max' });
    expect(pooled.notices.map(notice => notice.text)).toEqual(['Laptop · Old hub: token expired']);
    expect(pooled.links).toEqual([{ key: 'https://chatgpt.com/#settings/Usage', label: 'ChatGPT usage', chatgpt: true, message: 'View usage in ChatGPT with your connected account.', url: 'https://chatgpt.com/#settings/Usage' }]);
    // One automatic provider check per connected environment, each on its own transport.
    expect(rig.of('server.refreshProviders').map(entry => entry.env).sort()).toEqual(['env-a', 'env-b']);
    await rig.command('usage-pool-open', 'https://chatgpt.com/#settings/Usage');
    await rig.command('usage-pool-open', 'https://example.com/elsewhere');
    expect(rig.opened).toEqual(['https://chatgpt.com/#settings/Usage']);
  });

  it('wide segments carry their labels, narrow ones a legend; the first card hangs its popover below', async () => {
    const two = laptop();
    (two.providers as Obj[]).push(provider({ instanceId: 'codex-work', displayName: 'Work', auth: { status: 'authenticated', email: 'work@example.com' },
      usageLimits: { checkedAt: iso(-60_000), windows: [session(10, { resetsAt: iso(3 * HOUR) })] } }));
    const rig = new Rig(two, null);
    const wide = (await rig.page({ viewport: 1280, sidebar: 256 })).pooled;
    expect(wide.wide).toBe(true);
    const row = wide.pools[0]!.windows[0]!;
    expect(row.segments.map(segment => [segment.name, segment.column, segment.side, segment.align, segment.showName])).toEqual([['Codex', 1, 'bottom', 'start', true], ['Work', 2, 'bottom', 'end', true]]);
    expect(row.refill).toBe('↻ +20%'); // the soonest reset that hands anything back
    expect(row.remaining).toBe(75);
    // The weekly bar's top lies about 223 pt down the scroll area, short of the popover's ~250: it flips below too.
    expect(wide.pools[0]!.windows[1]!.segments[0]!.side).toBe('bottom');
    expect((await rig.page({ viewport: 840, sidebar: 256 })).pooled.wide).toBe(false);
  });

  it('the environment filter recomputes the pools from the selected environments', async () => {
    const rig = new Rig(laptop(), desktop());
    await rig.page();
    await rig.command('usage-pool-env', 'env-b');
    let page = await rig.page();
    expect(page.environmentLabel).toBe('Laptop');
    expect(page.pooled.pools[0]!.windows[0]!.segments[0]).toMatchObject({ where: 'Laptop', remaining: 60 });
    expect(page.pooled.notices.map(notice => notice.text)).toEqual(['Old hub: token expired']); // one environment: no label
    await rig.command('usage-pool-env', 'env-a');
    page = await rig.page();
    expect([page.environmentLabel, page.empty]).toEqual(['0 environments', 'Select an environment to see limits.']);
    await rig.command('usage-pool-env', '');
    page = await rig.page();
    expect([page.environmentLabel, page.allSelected, page.pooled.pools[0]!.windows[0]!.segments[0]!.where]).toEqual(['All environments', true, 'Laptop, Desktop']);
  });

  it('a background environment that is not connected is listed with its phase and contributes nothing', async () => {
    const rig = new Rig(laptop(), null);
    rig.background(desktop(), 'reconnecting');
    const page = await rig.page();
    expect(page.environments.map(row => [row.label, row.disabled, row.status])).toEqual([['Laptop', false, ''], ['Desktop', true, 'Reconnecting…']]);
    expect(page.pooled.pools[0]!.windows[0]!.segments[0]!.where).toBe('Laptop');
    expect(rig.of('server.refreshProviders').map(entry => entry.env)).toEqual(['env-a']);
  });

  it('Use reset asks first; Cancel sends nothing; Use credit redeems on the environment that showed the credits, and the outcome sits under the bar', async () => {
    const rig = new Rig(laptop(), desktop());
    const segment = (await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!;
    expect(await rig.command('usage-pool-reset-ask', segment.id)).toBe('focus:reset-credit-cancel');
    expect((await rig.page()).pooled.confirm).toBe(segment.id);
    expect(await rig.command('usage-pool-reset-cancel', segment.id)).toBe(`focus:usage-seg-${segment.id}`);
    expect((await rig.page()).pooled.confirm).toBe('');
    expect(rig.of('provider.consumeResetCredit')).toHaveLength(0);
    await rig.command('usage-pool-reset-ask', segment.id);
    expect(await rig.command('usage-pool-reset-confirm', segment.id)).toBe(`focus:usage-seg-${segment.id}`);
    const [redeem] = rig.of('provider.consumeResetCredit');
    expect(redeem).toMatchObject({ env: 'env-a', payload: { instanceId: 'codex' } });
    expect((await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!.busy).toBe(true);
    await rig.reply(redeem!, { outcome: 'reset' });
    const after = (await rig.page()).pooled.pools[0]!.windows[0]!;
    expect(after.segments[0]).toMatchObject({ busy: false, status: 'Reset applied. Your windows have cleared.' });
    expect(after.statuses).toEqual([{ key: segment.id, row: 2, name: 'Codex', chip: '', chipBg: expect.any(String), chipFg: expect.any(String), text: 'Reset applied. Your windows have cleared.' }]);
    // A typed failure says its message; another failure the generic line.
    await rig.command('usage-pool-reset-ask', segment.id); await rig.command('usage-pool-reset-confirm', segment.id);
    await rig.reply(rig.of('provider.consumeResetCredit')[1]!, { error: { kind: 'UsageLimitSourceError', message: 'The hub refused the credit.' } });
    expect((await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!.status).toBe('The hub refused the credit.');
  });

  it('a hub credit redeems through its hub on the environment that reports it', async () => {
    const configB = desktop();
    configB.providers = [];
    (configB as Obj).usageLimitSources = [{ id: 'hub-b', kind: 'cliproxy', label: 'Hub B', checkedAt: iso(0), accounts: [{ id: 'codex-same.json', driver: 'codex', email: 'same@example.com',
      usageLimits: { checkedAt: iso(0), windows: [session(50)], resetCredits: { availableCount: 1, nextCreditId: 'credit-9' } } }] }];
    const rig = new Rig(laptop(), configB);
    const segment = (await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!;
    expect(segment).toMatchObject({ where: 'Laptop', credits: 1 });
    await rig.command('usage-pool-reset-ask', segment.id); await rig.command('usage-pool-reset-confirm', segment.id);
    expect(rig.of('provider.consumeResetCredit')).toEqual([expect.objectContaining({ env: 'env-b', payload: { sourceId: 'hub-b', accountId: 'codex-same.json', creditId: 'credit-9' } })]);
  });
});

describe('popover sides (Base UI collision flip on an unscrolled page)', () => {
  test('a bar too near the top opens its popover below; one with room above opens above', () => {
    const segment = () => ({ side: '', email: 'a@b.c', plan: 'Pro', whereLabel: 'Signed in', resetsAt: 'x', restores: '+1% of pool', redeemable: true });
    const window = () => ({ count: 2, refill: '↻ +5%', description: '', statuses: [], segments: [segment(), segment()] });
    const pools = [{ cursorBefore: false, windows: [window(), window()] }, { cursorBefore: false, windows: [window(), window()] }];
    popoverSides(pools, { wide: true, md: true, cursorRows: 0 });
    expect(pools.flatMap(pool => pool.windows.map(entry => entry.segments[0]!.side))).toEqual(['bottom', 'bottom', 'top', 'top']);
  });
});

describe('review fixes (2026-10-08)', () => {
  it('a confirm redeems the account it was asked for, even when a redraw reorders the segments', async () => {
    const two = laptop();
    (two.providers as Obj[]).push(provider({ instanceId: 'codex-work', displayName: 'Work', auth: { status: 'authenticated', email: 'work@example.com' },
      usageLimits: { checkedAt: iso(-60_000), windows: [session(10, { resetsAt: iso(3 * HOUR) })], resetCredits: { availableCount: 1 } } }));
    const rig = new Rig(two, null);
    const first = (await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!;
    expect(first.name).toBe('Codex');
    await rig.command('usage-pool-reset-ask', first.id);
    // Work's session now resets first, so it takes the first column.
    ((two.providers as Obj[])[1]!.usageLimits as Obj).windows = [session(10, { resetsAt: iso(HOUR) })];
    const reordered = (await rig.page()).pooled.pools[0]!.windows[0]!.segments;
    expect(reordered.map(segment => segment.name)).toEqual(['Work', 'Codex']);
    expect(await rig.command('usage-pool-reset-confirm', first.id)).toBe(`focus:usage-seg-${reordered[1]!.id}`);
    expect(rig.of('provider.consumeResetCredit').map(entry => entry.payload)).toEqual([{ instanceId: 'codex' }]);
    await rig.reply(rig.of('provider.consumeResetCredit')[0]!, { outcome: 'reset' });
    const after = (await rig.page()).pooled.pools[0]!.windows[0]!.segments;
    expect(after.map(segment => [segment.name, segment.status])).toEqual([['Work', ''], ['Codex', 'Reset applied. Your windows have cleared.']]);
  });

  it('a reconnected environment reads its summary again after a failure', async () => {
    const rig = new Rig(laptop(), desktop());
    await rig.page({ metric: 'cost' });
    await rig.reply(rig.of('server.getUsageSummary', 'env-a')[0]!, summary());
    await rig.reply(rig.of('server.getUsageSummary', 'env-b')[0]!, { error: { kind: 'RPC', message: 'scan failed' } });
    let page = await rig.page({ metric: 'cost' });
    expect(page.environments.map(row => row.status)).toEqual(['Ready', 'Unavailable']);
    const entry = fleet.entries.get(KEY_B)!;
    entry.generation = 8; entry.synchronized = 8;
    page = await rig.page({ metric: 'cost' });
    expect(rig.of('server.getUsageSummary', 'env-b')).toHaveLength(2);
    expect(page.environments[1]!.status).toBe('Scanning…');
  });

  it('a background inbox that overflowed settles its waiters, so a refresh never stays busy', async () => {
    const rig = new Rig(laptop(), desktop());
    await rig.page();
    for (const check of rig.of('server.refreshProviders', 'env-a')) await rig.reply(check, {});
    let page = await rig.page({ refresh: 1 });
    expect(page.refreshing).toBe(true);
    await rig.reply(rig.of('server.refreshProviders', 'env-a')[1] ?? rig.of('server.refreshProviders', 'env-a')[0]!, {});
    usageFleetReset({ key: KEY_B, generation: 7 });
    for (let index = 0; index < 12; index++) await Promise.resolve();
    page = await rig.page({ refresh: 1 });
    expect(page.refreshing).toBe(false);
  });

  it('never sends a focused environment request after the focus moved', async () => {
    const rig = new Rig(laptop(), null);
    const segment = (await rig.page()).pooled.pools[0]!.windows[0]!.segments[0]!;
    await rig.command('usage-pool-reset-ask', segment.id);
    (rig.client as unknown as { environmentId: string }).environmentId = 'env-z';
    await rig.command('usage-pool-reset-confirm', segment.id);
    expect(rig.of('provider.consumeResetCredit')).toHaveLength(0);
  });
});

describe('usage across environments', () => {
  it('reads each selected environment once and merges what answered; a slow environment leaves the totals partial', async () => {
    const rig = new Rig(laptop(), desktop());
    let page = await rig.page({ metric: 'cost' });
    expect(rig.of('server.getUsageSummary').map(entry => entry.env).sort()).toEqual(['env-a', 'env-b']);
    expect([page.total, page.environmentIcon]).toEqual(['', 'pending']); // the skeleton
    const bucket = (provider: string, cost: number, home: string) => ({ day: '2026-09-11', provider, model: 'm', sourcePath: home, costUsd: cost, records: 1, unpricedRecords: 0,
      totals: { uncachedInputTokens: 10, cachedInputTokens: 0, cacheCreationTokens: 0, outputTokens: 0 } });
    const source = (home: string) => ({ status: 'ok', distinctSessions: 1, fingerprint: { hostId: 'h', provider: 'codex', resolvedHomePath: home, volumeId: 'v' } });
    await rig.reply(rig.of('server.getUsageSummary', 'env-a')[0]!, summary({ sources: [source('/a')], buckets: [bucket('codex', 1.5, '/a')] }));
    page = await rig.page({ metric: 'cost' });
    expect([page.total, page.partial, page.environments.map(row => row.status)]).toEqual(['$1.50', 'Totals are partial while selected environments scan.', ['Ready', 'Scanning…']]);
    await rig.reply(rig.of('server.getUsageSummary', 'env-b')[0]!, summary({ sources: [source('/b')], buckets: [bucket('codex', 2, '/b')] }));
    page = await rig.page({ metric: 'cost' });
    expect([page.total, page.partial, page.environmentIcon]).toEqual(['$3.50', '', '']);
    expect(rig.of('server.getUsageSummary')).toHaveLength(2);
  });

  it('refresh sends the rates once per selected connected environment, then rescans each, even after a failed rates call; a stopped environment makes no refetch', async () => {
    const rig = new Rig(laptop(), desktop());
    await rig.page({ metric: 'cost' });
    for (const read of rig.of('server.getUsageSummary')) await rig.reply(read, summary());
    let page = await rig.page({ metric: 'cost', refresh: 1 });
    expect(page.refreshing).toBe(true);
    expect(rig.of('server.refreshUsageRates').map(entry => entry.env).sort()).toEqual(['env-a', 'env-b']);
    await rig.reply(rig.of('server.refreshUsageRates', 'env-a')[0]!, { error: { kind: 'RPC', message: 'Pricing offline' } });
    page = await rig.page({ metric: 'cost', refresh: 1 });
    expect(rig.of('server.getUsageSummary', 'env-a')).toHaveLength(2); // invalidated even on failure, then refetched
    expect(page.environments[0]!.status).toBe('Refreshing…');
    rig.background(desktop(), 'reconnecting'); // env-b stops mid-refresh
    page = await rig.page({ metric: 'cost', refresh: 1 });
    await rig.reply(rig.of('server.getUsageSummary', 'env-a')[1]!, summary());
    page = await rig.page({ metric: 'cost', refresh: 1 });
    expect(rig.of('server.getUsageSummary', 'env-b')).toHaveLength(1);
    expect(page.refreshing).toBe(false);
    // A second press while one runs does nothing (refreshingRef).
    await rig.page({ metric: 'cost', refresh: 2 });
    expect(rig.of('server.refreshUsageRates', 'env-a')).toHaveLength(2);
  });

  it('offers Cursor Keychain access where Cursor is ready and the summary asks for it, and enables it on that environment', async () => {
    const configB = desktop();
    (configB.providers as Obj[]).push({ instanceId: 'cursor', driver: 'cursor', enabled: true, installed: true, status: 'ready', auth: { status: 'unknown' } });
    const rig = new Rig(laptop(), configB);
    await rig.page({ metric: 'cost' });
    await rig.reply(rig.of('server.getUsageSummary', 'env-a')[0]!, summary());
    await rig.reply(rig.of('server.getUsageSummary', 'env-b')[0]!, summary({ sources: [{ status: 'ok', distinctSessions: 0, action: 'enableCursorKeychain', message: 'Cursor account usage is off on this environment.',
      fingerprint: { hostId: 'host', provider: 'cursor', resolvedHomePath: '/x', volumeId: 'v' } }] }));
    const cost = await rig.page({ metric: 'cost' });
    expect(cost.providers.map(row => [row.key, row.label, row.enable])).toEqual([['enable:env-b', 'Cursor · Desktop', 'env-b']]);
    const limits = (await rig.page()).pooled;
    expect(limits.cursor).toEqual([{ key: 'env-b', label: 'Desktop', button: 'Enable', busy: false }]);
    expect(limits.pools.map(pool => [pool.driver, pool.cursorBefore])).toEqual([['codex', false], ['claudeAgent', false]]);
    expect(limits.cursorAfter).toBe(true); // after Codex and Claude
    await rig.command('usage-pool-cursor', 'env-b');
    expect(rig.direct).toEqual([{ env: 'env-b', method: 'server.updateSettings', payload: { patch: { cursorKeychainUsageEnabled: true } }, deliver: '' }]);
    expect(rig.of('server.refreshUsageRates').map(entry => entry.env).sort()).toEqual(['env-a', 'env-b']);
  });
});

describe('fleet hub sources', () => {
  it('a background environment subscribes to its server config with usageLimitSources', async () => {
    const source = new EnvironmentFleet(), calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = obj(input); calls.push(request);
      const ok = (value: unknown) => ({ ok: true, generation: 7, value });
      if (request.op === 'environments') return ok({ saved: [{ origin: 'http://10.0.0.2:3773', environmentId: 'env-b', enabled: true }] });
      if (request.op === 'status') return ok({ state: 'connected' });
      if (request.op === 'http') return ok(request.path === '/api/auth/session' ? { authenticated: true, scopes: [] } : { snapshotSequence: 1, projects: [], threads: [] });
      if (request.op === 'request') return ok(desktop());
      if (request.op === 'subscribe') return ok({ id: `sub-${calls.length}` });
      if (request.op === 'events') return ok({ events: [], latest: 0 });
      return { ok: false, generation: 7, error: { kind: 'transport', message: 'n/a', uncertain: false } };
    } };
    await source.sync(native, { origin: 'http://127.0.0.1:3773', environmentId: 'env-a', connection: 'connected' });
    expect(calls.filter(call => call.method === 'subscribeServerConfig' && call.fleet === KEY_B).map(call => call.payload)).toEqual([{ usageLimitSources: true }]);
  });
});

// ── UsagePage.refresh.test.tsx ─────────────────────────────────────────────

describe('UsagePage refresh (UsagePage.refresh.test.tsx)', () => {
  let environment = 0;
  const one = () => {
    environment += 1;
    const rig = new Rig({ environment: { environmentId: 'env-a', label: 'Test' }, providers: [provider({ auth: { status: 'authenticated' }, usageLimits: { checkedAt: iso(0), windows: [session(40)] } })] }, null);
    // A fresh environment id each test, as the reference's `test-${n}`: the five-minute window is per environment.
    (rig.client as unknown as { environmentId: string }).environmentId = `test-${environment}`;
    (rig.configA.environment as Obj).environmentId = `test-${environment}`;
    forgetUsageState(rig.client);
    return rig;
  };
  const resets = (page: Awaited<ReturnType<Rig['page']>>) => page.pooled.pools[0]!.windows[0]!.segments[0]!.resetsAt;

  it.each([0, 1])('refreshes the visible limits countdown with refresh button %i without switching tabs, even when quota is unchanged', async () => {
    const rig = one();
    expect(resets(await rig.page())).toContain('in 2h 0m');
    for (const check of rig.of('server.refreshProviders')) await rig.reply(check, {});
    const later = NOW + 30 * 60_000;
    await rig.page({ now: later, refresh: 1 });
    expect(rig.of('server.refreshProviders')).toHaveLength(2);
    expect(rig.of('server.refreshProviders')[1]).toMatchObject({ env: 'env-a', payload: {} });
    await rig.reply(rig.of('server.refreshProviders')[1]!, {});
    const page = await rig.page({ now: later, refresh: 1 });
    expect(resets(page)).toContain('in 1h 30m');
    expect(resets(page)).not.toContain('in 2h 0m');
  });

  it('uses the current time when returning to limits from tokens', async () => {
    const rig = one();
    await rig.page();
    for (const check of rig.of('server.refreshProviders')) await rig.reply(check, {});
    await rig.page({ metric: 'tokens' });
    const page = await rig.page({ now: NOW + HOUR });
    expect(resets(page)).toContain('in 1h 0m');
    expect(rig.of('server.refreshProviders')).toHaveLength(2);
  });

  it('refreshes once on opening Limits and suppresses rapid returns and remounts', async () => {
    const rig = one();
    await rig.page({ metric: 'tokens' });
    expect(rig.of('server.refreshProviders')).toHaveLength(0);
    await rig.page();
    expect(rig.of('server.refreshProviders')).toHaveLength(1);
    await rig.reply(rig.of('server.refreshProviders')[0]!, {});
    await rig.page({ metric: 'tokens' });
    await rig.page();
    await rig.page({ open: false }); // unmount
    await rig.page(); // remount on Limits
    expect(rig.of('server.refreshProviders')).toHaveLength(1);
    await rig.page({ metric: 'tokens' });
    await rig.page({ now: NOW + 5 * 60_000 });
    expect(rig.of('server.refreshProviders')).toHaveLength(2);
  });

  it('waits for connection and refreshes new environments during a slow refresh', async () => {
    const rig = one();
    (rig.client as unknown as { connection: string }).connection = 'reconnecting';
    await rig.page();
    expect(rig.of('server.refreshProviders')).toHaveLength(0);
    (rig.client as unknown as { connection: string }).connection = 'connected';
    await rig.page();
    expect(rig.of('server.refreshProviders')).toHaveLength(1); // left pending: a slow refresh
    rig.background({ environment: { environmentId: 'env-b', label: 'Next' }, providers: [] });
    await rig.page();
    expect(rig.of('server.refreshProviders')).toHaveLength(2);
    expect(rig.of('server.refreshProviders')[1]).toMatchObject({ env: 'env-b', payload: {} });
  });

  it('keeps manual refresh busy until the already-running automatic check settles', async () => {
    const rig = one();
    await rig.page();
    expect(rig.of('server.refreshProviders')).toHaveLength(1);
    let page = await rig.page({ refresh: 1 });
    expect(page.refreshing).toBe(true);
    expect(rig.of('server.refreshProviders')).toHaveLength(1);
    await rig.reply(rig.of('server.refreshProviders')[0]!, {});
    page = await rig.page({ refresh: 1 });
    expect(page.refreshing).toBe(false);
  });
});


// popover-escape-parity: light dismiss, read from the Contract sources as dialog-focus.test.ts reads
// its handlers (the page's popover state is Contract state). The behavior is proven by the macOS drives
// in tasks/20261008-popover-escape-parity.md. Reference: UsageLimitsPooled.tsx PoolSegment is a Base UI
// Popover (non-modal); its useDismiss closes it on a press outside the popup and the popover's own
// triggers (the segment and its LegendRow), on the click for a mouse ("intentional"), and a press on
// another segment is outside it.
describe('light dismiss of a pinned segment popover (popover-escape-parity)', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
  /** The lines of `component name` in `file`, up to the next top-level declaration. */
  const component = async (file: string, name: string) => {
    const lines = (await source(file)).split('\n');
    const start = lines.findIndex(line => line === `component ${name}`);
    if (start < 0) throw new Error(`${file}: no component ${name}`);
    const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
    return lines.slice(start, end < 0 ? undefined : end).join('\n');
  };
  /** A Contract boolean expression as JavaScript (`and`, `or`, `not`). */
  const js = (expr: string) => expr.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');

  test('usageHit holds a point inside a box and none in a missing box (frame() of an absent id is 0 by 0)', async () => {
    const fn = /^fn usageHit\(g: Geometry, x: number, y: number\): bool = (.+)$/m.exec(await source('pages-usage.contract'))?.[1];
    expect(fn).toBeDefined();
    const hit = new Function('g', 'x', 'y', `return ${js(fn!)};`) as (g: { x: number; y: number; width: number; height: number }, x: number, y: number) => boolean;
    const card = { x: 100, y: 40, width: 288, height: 200 };
    expect([hit(card, 100, 40), hit(card, 387.5, 239.5), hit(card, 200, 100)]).toEqual([true, true, true]);
    // The right and bottom edges are the next box's, as a hit test's are.
    expect([hit(card, 388, 100), hit(card, 200, 240), hit(card, 99.5, 100), hit(card, 200, 39.5)]).toEqual([false, false, false, false]);
    expect(hit({ x: 0, y: 0, width: 0, height: 0 }, 0, 0)).toBe(false);
  });

  test('a primary press that starts outside the pinned popover and both its triggers closes it as it ends', async () => {
    const page = await component('pages-usage.contract', 'UsagePage');
    // The page's root takes every press on the page but a segment's (below); the scroll's content fills
    // its port, since on macOS a press on a scroll view's empty area reaches no node.
    expect(page).toMatch(/\n {4}column [^\n]*pointerdown=pressDown pointerup=pressUp testId="usage-page"/);
    expect(page).toContain('scroll flex=1 min-height=0 width="100%" testId="usage-scroll"\n');
    expect(page).toMatch(/\n {8}column width="100%" min-height="100%" testId="usage-ground"\n {10}column width="100%" max-width="64rem"/);
    // Down: outside the card and both handles of the popover on show, by the primary button only (a
    // right-click is no click, so Base UI's intentional dismissal ignores it).
    expect(page).toContain('action pressDown(e: PointerEvent)\n'
      + '    let seg = frame(`usage-seg-${held}`)\n'
      + '    let pop = frame(`usage-seg-pop-${held}`)\n'
      + '    let leg = frame(`usage-legend-${held}`)\n'
      + '    downOutside = held != "" and e.buttons == 1 and not (usageHit(seg, e.clientX, e.clientY) or usageHit(pop, e.clientX, e.clientY) or usageHit(leg, e.clientX, e.clientY))');
    // Up (DOM's order: down, up, then the press): it closes before the press runs, so a press on another
    // segment then pins that one and a press on the pinned segment (inside) toggles it closed.
    expect(page).toContain('action pressUp\n    if downOutside\n      pinned = ""\n    downOutside = false');
    // A press on the pinned popover's own segment closes it even with the pointer still there (the hover
    // states too); the segment's next pointer move opens it by hover again, as Base UI's restMs hover does.
    expect(page).toContain('action pin(id: string)\n    if held == id\n      pinned = ""\n      overSeg = ""\n      overPop = ""\n      overMail = ""\n    else\n      pinned = id\n    pinnedAt = outside');
    // The three boxes are the ones usage-pooled.contract draws, by these ids.
    const pooled = await source('usage-pooled.contract');
    expect(pooled).toContain('button id=`usage-seg-${seg.id}` press=pin(seg.id) hover=enterSeg(seg.id) pointermove=enterSeg(seg.id, true)');
    expect(pooled).toMatch(/column width="18rem"[^\n]*role="dialog" aria-label=seg\.title id=`usage-seg-pop-\$\{seg\.id\}`/);
    expect(pooled).toMatch(/button [^\n]*press=pin\(seg\.id\)[^\n]*id=`usage-legend-\$\{seg\.id\}`/);
  });

  test('a press anywhere else in the window (the sidebar, the theme editor, a toast) closes it too', async () => {
    const page = await component('pages-usage.contract', 'UsagePage');
    // A pin holds while the window's count of outside presses is the one it was made at.
    expect(page).toContain('derive held = pinnedAt == outside ? pinned : ""');
    expect(page).toContain('derive shown = overPop != "" ? overPop : overMail != "" ? overMail : overSeg != "" ? overSeg : held');
    expect(page).toContain('shown=shown, pinned=held,');
    const window = await component('app-window.contract', 'T3Window');
    expect(window).toContain('main testId="t3-code" pointerdown=outsidePressDown pointerup=outsidePressUp');
    expect(window).toContain('action outsidePressDown(e: PointerEvent)\n    outsideDown = e.buttons == 1');
    expect(window).toContain('action outsidePressUp\n    outsidePresses = outsideDown ? outsidePresses + 1 : outsidePresses\n    outsideDown = false');
    expect(window).toContain('PagesCover(pageCover=pageCover, outside=outsidePresses,');
    const cover = await component('app-main.contract', 'PagesCover');
    expect(cover.split('\n').filter(line => line.includes('UsagePage(')).map(line => line.endsWith('outside=outside)'))).toEqual([true, true]);
    // The nodes that take the pointer themselves hand their presses on (the theme editor's colour controls).
    expect(window).toContain('outsideDown=outsidePressDown, outsideUp=outsidePressUp)');
    const picker = await source('theme-color-picker.contract');
    for (const name of ['triggerDown', 'planeDown', 'hueDown']) expect(picker).toMatch(new RegExp(`action ${name}\\(e: PointerEvent\\)\\n(    .*\\n)*?    outsideDown\\(e\\)`));
    for (const name of ['triggerUp', 'planeUp', 'hueUp']) expect(picker).toMatch(new RegExp(`action ${name}(\\(e: PointerEvent\\))?\\n    outsideUp\\(\\)`));
    expect(picker).toContain('pointerdown=triggerDown pointerup=triggerUp');
    // No other node in the app takes the pointer, so every other press reaches the window's count (a
    // segment holds its own presses: a press on it is never outside its popover, and one on another
    // segment pins that one in place of the first).
    const { readdirSync } = await import('node:fs');
    const takers: string[] = [];
    for (const file of readdirSync(new URL('./', import.meta.url)).filter(name => name.endsWith('.contract')).sort()) {
      (await source(file)).split('\n').forEach(line => { if (/^\s*[a-z][\w-]*\b[^\n]*\spointer(down|up|move)=/.test(line)) takers.push(`${file} ${/testId=(?:"([^"]+)"|`([^`]+)`)/.exec(line)?.slice(1).find(Boolean) ?? ''}`); });
    }
    expect(takers).toEqual([
      'app-window.contract t3-code',
      'pages-usage.contract usage-page',
      'theme-color-picker.contract theme-editor-swatch-${row.id}',
      'theme-color-picker.contract theme-color-${row.id}-plane',
      'theme-color-picker.contract theme-color-${row.id}-hue',
      'usage-pooled.contract usage-seg-${seg.id}',
    ]);
  });
});
