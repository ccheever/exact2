// The /usage-limits banner (composer-controls-usage.ts, usage-bars.contract) against the fake T3
// backend: ComposerUsageLimits.tsx's accounts, UsageLimits.tsx's bars, pace and countdowns,
// "Manage usage", and ResetCredits with its confirm and every scripted
// `provider.consumeResetCredit` outcome.
import { describe, expect, test } from 'bun:test';
import { arr, obj, type Obj } from './domain';
import { snapshot } from './presentation';
import { Fake, opened } from './composer-controls-fixture';

const now = Date.parse('2026-10-08T03:00:00.000Z');
const iso = (offset: number) => new Date(now + offset).toISOString();
type Reply = { ok: true; value: Obj } | { ok: false; error: Obj };

class LimitsFake extends Fake {
  redeems: Obj[] = [];
  opened: string[] = [];
  replies: Reply[] = [];
  async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    // A detached request (composer-replies.ts): its reply is filed in the inbox under `deliver`.
    if (request.op === 'request' && request.method === 'provider.consumeResetCredit') {
      this.redeems.push(obj(request.payload));
      const reply = this.replies.shift() ?? { ok: true, value: { outcome: 'reset' } };
      this.emit(String(request.deliver), reply.ok ? { _reply: reply.value } : { _replyError: { uncertain: false, ...reply.error } });
      return this.ok({ id: `rpc-${this.redeems.length}` });
    }
    if (request.op === 'remoteEditorsOpen') { this.opened.push(String(request.url)); return this.ok({ opened: true }); }
    return super.later(input);
  }
}
async function banner(limits: Obj, extra: { auth?: Obj; sources?: Obj[] } = {}) {
  const context = await opened();
  const native = new LimitsFake();
  Object.assign(native, { shell: context.native.shell, details: context.native.details, config: context.native.config, seq: context.native.seq, subs: context.native.subs, generation: context.native.generation });
  const command = (op: string, id = '', value = '', n = 0) => context.client.command(op, id, value, n, native, context.disk);
  const codex = arr(context.client.config.providers)[0]!;
  Object.assign(codex, { usageLimits: { checkedAt: iso(-60_000), ...limits }, auth: extra.auth ?? { status: 'authenticated', label: 'ChatGPT Pro' } });
  context.client.config.usageLimitSources = extra.sources ?? [];
  snapshot(context.client, now);
  await command('cclocal:usage-limits');
  const view = () => snapshot(context.client, now).composer;
  const usage = () => view().notices.find(notice => notice.title === 'Usage limits')!;
  const drain = async () => { await context.client.refresh(native, context.disk); await new Promise(resolve => setTimeout(resolve, 0)); };
  return { ...context, native, command, view, usage, drain };
}
const session = { id: 'primary', kind: 'session', label: '5h limit', usedPercent: 40, windowDurationMins: 300, resetsAt: iso(2 * 3_600_000) };
const weekly = { id: 'secondary', kind: 'weekly', label: 'Weekly limit', usedPercent: 80, windowDurationMins: 10_080, resetsAt: iso(3 * 86_400_000 + 3_600_000) };

describe('/usage-limits banner', () => {
  test('one account: the summary, window rows with figures, pace and countdowns, Manage usage and banked credits', async () => {
    const { usage, native, command } = await banner({ windows: [session, weekly], resetCredits: { availableCount: 2, nextExpiresAt: iso(27 * 86_400_000 + 23 * 3_600_000) },
      externalUsage: { label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage' } });
    const notice = usage();
    expect(notice).toMatchObject({ description: 'Codex · ChatGPT Pro', redact: '', lines: [], dismissLabel: 'Dismiss usage limits', variant: 'info', icon: 'gauge' });
    const [account] = notice.usage;
    expect(account).toMatchObject({ key: 'codex', showLabel: false, notice: '', manage: true, showCredits: true, canRedeem: true, busy: false, status: '',
      credits: '2 reset credits banked · next expires in 27d 23h', light: '#27272a', dark: '#f5f5f5' });
    expect(account!.windows).toEqual([
      { id: 'primary', label: '5h limit', remaining: 60, timeLeft: 40, pace: 'under', paceLabel: 'Under pace: headroom left for the rest of the window', resetsIn: 'resets in 2h 0m',
        summary: '5h limit: 60% left, 40% of the window left, resets in 2h 0m', tipFigures: '60% left · 40% of the window left', tipResets: expect.stringMatching(/^Resets .+ · resets in 2h 0m$/) },
      { id: 'secondary', label: 'Weekly limit', remaining: 20, timeLeft: 43, pace: 'ahead', paceLabel: 'Ahead of pace: spending faster than the window elapses', resetsIn: 'resets in 3d 1h',
        summary: 'Weekly limit: 20% left, 43% of the window left, resets in 3d 1h', tipFigures: '20% left · 43% of the window left', tipResets: expect.stringMatching(/^Resets .+ · resets in 3d 1h$/) },
    ]);
    await command('cclocal:usage-manage', 'codex');
    expect(native.opened).toEqual(['https://chatgpt.com/#settings/Usage']);
  });

  test('several accounts: each is labelled; an address is redacted; a hub error is a line', async () => {
    const hub = { id: 'hub', kind: 'cliproxy', label: 'Accounts', checkedAt: iso(-30_000), accounts: [
      { id: 'codex-me@example.com.json', driver: 'codex', plan: 'ChatGPT Plus', usageLimits: { checkedAt: iso(-30_000), windows: [session], resetCredits: { availableCount: 1, nextCreditId: 'credit-9' } } },
    ] };
    const broken = { id: 'other', kind: 'cliproxy', label: 'Backup', checkedAt: iso(-30_000), accounts: [], error: 'token expired' };
    const { usage } = await banner({ windows: [], unavailable: { reason: 'probeFailed', message: 'Codex timed out.' } }, { sources: [hub, broken] });
    const notice = usage();
    expect(notice).toMatchObject({ description: '2 accounts', redact: '', lines: ['Backup: token expired'] });
    expect(notice.usage.map(account => [account.key, account.showLabel, account.label, account.redact !== '', account.plan, account.notice, account.windows.length, account.credits])).toEqual([
      ['codex', true, 'Codex', false, ' · ChatGPT Pro', 'Codex timed out.', 0, ''],
      ['hub:codex-me@example.com.json', true, '', true, ' · ChatGPT Plus', '', 1, '1 reset credit banked'],
    ]);
    const hidden = notice.usage[1]!;
    expect(hidden.redact).toBe('Accounts · codex-me@example.com.json');
    expect(hidden.redactMask).toHaveLength(hidden.redact.length);
    expect(hidden.redactMask).not.toContain('example');
    expect(hidden.redactMask.split('').filter(char => '@.-_'.includes(char))).toEqual(hidden.redact.split('').filter(char => '@.-_'.includes(char)));
  });

  test('a single account whose label holds an address is redacted in the title row', async () => {
    const context = await opened();
    const codex = arr(context.client.config.providers)[0]!;
    Object.assign(codex, { displayName: 'me@example.com', usageLimits: { checkedAt: iso(-60_000), windows: [session] }, auth: { status: 'authenticated', label: 'Pro' } });
    snapshot(context.client, now);
    await context.command('cclocal:usage-limits');
    const notice = snapshot(context.client, now).composer.notices.find(entry => entry.title === 'Usage limits')!;
    expect(notice).toMatchObject({ description: '', redact: 'Codex · me@example.com', redactAfter: ' · Pro' });
    // The value travels for the reveal; the title row draws only the same-shape mask until a click.
    expect(notice.redactMask).toHaveLength(notice.redact.length);
    expect(notice.redactMask).not.toContain('example');
  });
});

describe('reset credits', () => {
  const credited = { windows: [session], resetCredits: { availableCount: 1, nextExpiresAt: iso(86_400_000) } };
  test('Use reset asks; Cancel sends nothing and returns the focus to Use reset', async () => {
    const { view, usage, native, command } = await banner(credited);
    expect((await command('cclocal:usage-reset-ask', 'codex')).message).toBe('focus:reset-credit-cancel');
    expect(view().resetConfirm).toBe('codex');
    expect((await command('cclocal:usage-reset-cancel', 'codex')).message).toBe('focus:usage-reset-0');
    expect(view().resetConfirm).toBe('');
    expect(native.redeems).toEqual([]);
    expect(usage().usage[0]).toMatchObject({ busy: false, status: '' });
  });

  test('Use credit closes the confirm, reads "Using…" while it runs, sends the input once and says the outcome', async () => {
    const { view, usage, native, command, drain } = await banner(credited);
    await command('cclocal:usage-reset-ask', 'codex');
    native.replies.push({ ok: true, value: { outcome: 'reset' } });
    const started = await command('cclocal:usage-reset-confirm', 'codex');
    expect(started.message).toBe('');
    expect(view().resetConfirm).toBe('');
    expect(usage().usage[0]).toMatchObject({ busy: true, status: '' });
    // Asking again while busy does nothing (the button is disabled).
    await command('cclocal:usage-reset-ask', 'codex');
    expect(view().resetConfirm).toBe('');
    await drain();
    expect(native.redeems).toEqual([{ instanceId: 'codex' }]);
    expect(usage().usage[0]).toMatchObject({ busy: false, status: 'Reset applied. Your windows have cleared.' });
  });

  for (const [name, reply, text] of [
    ['nothingToReset', { ok: true, value: { outcome: 'nothingToReset' } }, 'Nothing to reset right now.'],
    ['noCredit', { ok: true, value: { outcome: 'noCredit' } }, 'No reset credit left.'],
    ['alreadyRedeemed', { ok: true, value: { outcome: 'alreadyRedeemed' } }, 'That credit was already redeemed.'],
    ['a warning', { ok: true, value: { outcome: 'reset', warning: 'Redeemed, but the hub cooldown could not be cleared.' } }, 'Redeemed, but the hub cooldown could not be cleared.'],
    ['a typed error', { ok: false, error: { kind: 'ProviderSetupError', message: 'This provider does not bank reset credits.' } }, 'This provider does not bank reset credits.'],
    ['a transport failure', { ok: false, error: { kind: 'transport', message: 'The socket closed.' } }, 'Could not use the reset credit.'],
  ] as const) {
    test(`${name} has its words`, async () => {
      const { usage, native, command, drain } = await banner(credited);
      await command('cclocal:usage-reset-ask', 'codex');
      native.replies.push(reply as Reply);
      await command('cclocal:usage-reset-confirm', 'codex');
      await drain();
      expect(usage().usage[0]).toMatchObject({ busy: false, status: text });
    });
  }

  test('a hub account redeems its pinned credit; the row hides at zero credits until an outcome shows', async () => {
    const hub = { id: 'hub', kind: 'cliproxy', label: 'Accounts', checkedAt: iso(-30_000), accounts: [
      { id: 'oss', driver: 'codex', usageLimits: { checkedAt: iso(-30_000), windows: [session], resetCredits: { availableCount: 1, nextCreditId: 'credit-9' } } },
      { id: 'empty', driver: 'codex', usageLimits: { checkedAt: iso(-30_000), windows: [session], resetCredits: { availableCount: 0 } } },
    ] };
    const { usage, native, command, drain } = await banner({ windows: [session] }, { sources: [hub] });
    expect(usage().usage.map(account => [account.key, account.showCredits, account.canRedeem])).toEqual([['codex', false, false], ['hub:oss', true, true], ['hub:empty', false, false]]);
    await command('cclocal:usage-reset-ask', 'hub:oss');
    native.replies.push({ ok: true, value: { outcome: 'noCredit' } });
    await command('cclocal:usage-reset-confirm', 'hub:oss');
    await drain();
    expect(native.redeems).toEqual([{ sourceId: 'hub', accountId: 'oss', creditId: 'credit-9' }]);
    // The server republishes the account with nothing banked: the outcome keeps the row.
    hub.accounts[0]!.usageLimits.resetCredits = { availableCount: 0, nextCreditId: 'credit-9' };
    expect(usage().usage[1]).toMatchObject({ showCredits: true, canRedeem: false, credits: 'No reset credits banked', status: 'No reset credit left.' });
  });

  test('reopening the banner starts without the last outcome', async () => {
    const { usage, native, command, drain } = await banner(credited);
    await command('cclocal:usage-reset-ask', 'codex');
    native.replies.push({ ok: true, value: { outcome: 'nothingToReset' } });
    await command('cclocal:usage-reset-confirm', 'codex');
    await drain();
    expect(usage().usage[0]!.status).toBe('Nothing to reset right now.');
    await command('cclocal:usage-limits-dismiss');
    await command('cclocal:usage-limits');
    expect(usage().usage[0]!.status).toBe('');
  });
});
