// The Compact chip for a stale Claude session, with or without a reported cache TTL,
// and the Woke dismissal that syncs through thread.visit (c5a929e1ac).
import { describe, expect, test } from 'bun:test';
import { arr, obj } from '../../src/client/domain';
import { threadStatusLine } from '../../src/timeline/thread-status-line';
import { snapshot } from '../../src/client/presentation';
import { shouldOffer, NEVER_ANSWER, keepsFullHistory } from '../../src/composer/composer-controls-resume';
import { connected } from '../support/composer-controls-fixture';

const NOW = Date.parse('2026-10-03T12:00:30.000Z');
const minutesAgo = (minutes: number) => new Date(NOW - minutes * 60_000).toISOString();

async function claudeThread(options: { used?: number; idle?: number; compact?: boolean; ttl?: number } = {}) {
  const context = await connected();
  const { native } = context;
  const claude = arr(native.config.providers).find(provider => provider.instanceId === 'claude')!;
  claude.slashCommands = options.compact === false ? [] : [{ name: 'compact' }];
  const projection = obj(native.details.t1!.projection);
  Object.assign(obj(projection.thread), { modelSelection: { instanceId: 'claude', model: 'model-a' }, activeProviderThreadId: 'pt' });
  projection.providerThreads = [{ id: 'pt', appThreadId: 't1', providerInstanceId: 'claude', updatedAt: minutesAgo(options.idle ?? 75),
    contextUsage: { usedTokens: options.used ?? 234_567, maxTokens: 1_000_000 } }];
  await context.command('select-thread', 't1');
  await context.client.refresh(context.native, context.disk);
  const loaded = context.client.thread!.projection;
  if (options.ttl) loaded.providerTurns = [{ tokenUsage: { usedTokens: options.used ?? 234_567, maxTokens: 1_000_000, updatedAt: minutesAgo(options.idle ?? 75), promptCacheTtlMs: options.ttl } }];
  loaded.runs = [{ id: 'r1', ordinal: 1, status: 'completed', startedAt: minutesAgo(80), completedAt: minutesAgo(75) }];
  loaded.visibleTurnItems = [{ item: { type: 'user_message', text: 'Build the parser', attachments: [] } }];
  return context;
}
const chip = (client: Parameters<typeof snapshot>[0]) => snapshot(client, NOW).composer;

describe('a stale session without a cache TTL (shouldOfferResumeCompaction)', () => {
  test('offered for a Claude session idle 70 minutes with 200k tokens', () => {
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 200_000, updatedAt: minutesAgo(70.5), now: NOW })).toBe(true);
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 199_999, updatedAt: minutesAgo(90), now: NOW })).toBe(false);
    expect(shouldOffer({ driver: 'codex', usedTokens: 500_000, updatedAt: minutesAgo(90), now: NOW })).toBe(false);
    // The clock counts whole minutes (12:00): 70 minutes and 20 seconds ago is still 69m40s before it.
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 200_000, updatedAt: new Date(NOW - 70 * 60_000 - 20_000).toISOString(), now: NOW })).toBe(false);
  });
  test('the Compact chip, not a notice: the cache counts as 70 minutes', async () => {
    const { client } = await claudeThread();
    expect(chip(client)).toMatchObject({ compactTokens: '235k', compactKeepFull: false, compactSend: true, sendLabel: 'Compact and send' });
    expect(chip(client).notices.some(notice => notice.title === 'Resume with less context')).toBe(false);
  });
  test('not offered when the provider cannot compact, under the thresholds, or after "Don\'t ask again"', async () => {
    expect(chip((await claudeThread({ compact: false })).client).compactTokens).toBe('');
    expect(chip((await claudeThread({ used: 80_000 })).client).compactTokens).toBe('');
    expect(chip((await claudeThread({ idle: 30 })).client).compactTokens).toBe('');
    const { client } = await claudeThread();
    client.thread!.projection.runtimeRequests = [{ id: 'q', kind: 'user_input', status: 'resolved',
      answers: { 'This session is 1h 15m old and uses 234,567 tokens. Compact it before continuing?': NEVER_ANSWER } }];
    expect(chip(client).compactTokens).toBe('');
    client.thread!.projection.runtimeRequests = [];
    expect(chip(client).compactTokens).toBe('');
  });
  test('not offered when idle compaction is Never, for the environment or the provider instance (resolveIdleCompaction)', async () => {
    const { client } = await claudeThread();
    const settings = obj(client.config.settings);
    Object.assign(settings, { compactIdleThreads: false, idleCompaction: 'off' });
    expect(chip(client).compactTokens).toBe('');
    Object.assign(settings, { providerInstances: { claude: { driver: 'claudeAgent', config: { idleCompaction: 'onReturn' } } } });
    expect(chip(client).compactTokens).toBe('235k');
    Object.assign(settings, { idleCompaction: undefined, compactIdleThreads: 'off', providerInstances: { claude: { driver: 'claudeAgent', config: { idleCompaction: 'default' } } } });
    expect(chip(client).compactTokens).toBe('');
    Object.assign(settings, { compactIdleThreads: true });
    expect(chip(client).compactTokens).toBe('235k');
  });
  test('a send compacts first, then queues the typed message', async () => {
    const { client, native, command } = await claudeThread();
    snapshot(client, NOW);
    await command('send', '', 'Continue the parser');
    const sent = native.committed.filter(entry => entry.type === 'message.dispatch');
    expect(sent.map(entry => [entry.text, obj(entry.dispatchMode).type])).toEqual([['/compact', 'start_immediately'], ['Continue the parser', 'queue_after_active']]);
  });
  test('"/compact" itself, or a thread without the chip, sends once', async () => {
    const { native, command } = await claudeThread();
    await command('send', '', '/compact');
    expect(native.committed.filter(entry => entry.type === 'message.dispatch').map(entry => entry.text)).toEqual(['/compact']);
    const quiet = await claudeThread({ idle: 10 });
    await quiet.command('send', '', 'Hello');
    expect(quiet.native.committed.filter(entry => entry.type === 'message.dispatch').map(entry => [entry.text, obj(entry.dispatchMode).type])).toEqual([['Hello', 'start_immediately']]);
  });
});

describe('Woke dismissal syncs (c5a929e1ac)', () => {
  test('a tracking server records the visit at the wake time; its watermark hides the banner', async () => {
    const { client, native, command, disk } = await connected();
    obj(obj(native.config.environment).capabilities).threadVisitedTracking = true;
    const wokeAt = minutesAgo(5);
    Object.assign(arr(native.shell.threads)[0]!, { snoozedUntil: wokeAt, snoozedAt: minutesAgo(60), lastVisitedAt: minutesAgo(30) });
    await command('select-thread', 't1');
    await client.refresh(native, disk);
    const woke = () => { snapshot(client, NOW); return threadStatusLine(client, NOW) ?? undefined; };
    expect(woke()).toBeDefined();
    await command(woke()!.detail!, woke()!.targetId!);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(native.committed.filter(entry => entry.type === 'thread.visit')).toEqual([expect.objectContaining({ type: 'thread.visit', threadId: 't1', visitedAt: wokeAt })]);
    expect(client.local.composerControls.wokeSeen).toEqual({});
    // Until the server's watermark moves the banner stays; then any device's acknowledgement hides it.
    expect(woke()).toBeDefined();
    client.shell.threads[0]!.lastVisitedAt = wokeAt;
    expect(woke()).toBeUndefined();
    // A mark-unread rewind on the server wins over a newer local value.
    client.local.composerControls.wokeSeen[`${client.environmentId}:t1`] = minutesAgo(1);
    client.shell.threads[0]!.lastVisitedAt = null;
    expect(woke()).toBeDefined();
  });
  test('an older server keeps the acknowledgement locally', async () => {
    const { client, native, command } = await connected();
    const wokeAt = minutesAgo(5);
    Object.assign(arr(native.shell.threads)[0]!, { snoozedUntil: wokeAt, snoozedAt: minutesAgo(60) });
    await command('select-thread', 't1');
    const woke = () => { snapshot(client, NOW); return threadStatusLine(client, NOW) ?? undefined; };
    expect(woke()).toBeDefined();
    await command(woke()!.detail!, woke()!.targetId!);
    expect(native.committed.some(entry => entry.type === 'thread.visit')).toBe(false);
    expect(client.local.composerControls.wokeSeen[`${client.environmentId}:t1`]).toBe(wokeAt);
    expect(woke()).toBeUndefined();
  });
});


describe('cache-aware Compact chip', () => {
  test('TTL controls expiry; Full toggles per thread, measured chip width wins', async () => {
    const { client, command } = await claudeThread({ idle: 6, ttl: 300_000 });
    expect(snapshot(client, NOW).composer).toMatchObject({ compactTokens: '235k', compactKeepFull: false, compactSend: true, sendLabel: 'Compact and send' });
    client.presentation.anchors = { 'compact:Compact:235k': [0, 157] };
    expect(snapshot(client, NOW).composer.compactWidth).toBe(157);
    await command('cclocal:compact-full');
    expect(snapshot(client, NOW).composer).toMatchObject({ compactKeepFull: true, compactSend: false, sendLabel: 'Submit message' });
    await command('select-thread', 't2');
    await command('select-thread', 't1');
    expect(keepsFullHistory(client)).toBe(true);
    expect(snapshot((await claudeThread({ idle: 4, ttl: 300_000 })).client, NOW).composer.compactTokens).toBe('');
  });
  test('Compact dispatches first, then queues the typed message on the selected model', async () => {
    const { client, native, command } = await claudeThread({ idle: 6, ttl: 300_000 });
    snapshot(client, NOW);
    await command('send', '', 'Continue the parser');
    const sent = native.committed.filter(entry => entry.type === 'message.dispatch');
    expect(sent.map(entry => [entry.text, obj(entry.dispatchMode).type])).toEqual([['/compact', 'start_immediately'], ['Continue the parser', 'queue_after_active']]);
    expect(sent.every(entry => obj(entry.modelSelection).instanceId === 'claude')).toBe(true);
  });
  test('Full sends only the draft and resets after success; explicit /compact sends once', async () => {
    const full = await claudeThread({ idle: 6, ttl: 300_000 });
    snapshot(full.client, NOW);
    await full.command('cclocal:compact-full');
    await full.command('send', '', 'Keep this context');
    expect(full.native.committed.filter(entry => entry.type === 'message.dispatch').map(entry => entry.text)).toEqual(['Keep this context']);
    expect(snapshot(full.client, NOW).composer.compactKeepFull).toBe(false);
    const explicit = await claudeThread({ idle: 6, ttl: 300_000 });
    snapshot(explicit.client, NOW);
    await explicit.command('send', '', '/compact');
    expect(explicit.native.committed.filter(entry => entry.type === 'message.dispatch').map(entry => entry.text)).toEqual(['/compact']);
  });
});
