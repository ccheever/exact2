// Lane composer-controls, round 3: "Resume with less context" and sending past
// it (a976f8c74c), and the Woke dismissal that syncs through thread.visit (c5a929e1ac).
import { describe, expect, test } from 'bun:test';
import { arr, obj } from './domain';
import { snapshot } from './presentation';
import { shouldOffer, NEVER_ANSWER } from './r3-composer-controls-resume';
import { connected } from './composer-controls-fixture';

const NOW = Date.parse('2026-10-03T12:00:30.000Z');
const minutesAgo = (minutes: number) => new Date(NOW - minutes * 60_000).toISOString();

async function claudeThread(options: { used?: number; idle?: number; compact?: boolean } = {}) {
  const context = await connected();
  const { native } = context;
  const claude = arr(native.config.providers).find(provider => provider.instanceId === 'claude')!;
  claude.slashCommands = options.compact === false ? [] : [{ name: 'compact' }];
  const projection = obj(native.details.t1!.projection);
  Object.assign(obj(projection.thread), { modelSelection: { instanceId: 'claude', model: 'model-a' }, activeProviderThreadId: 'pt' });
  projection.providerThreads = [{ id: 'pt', appThreadId: 't1', providerInstanceId: 'claude', updatedAt: minutesAgo(options.idle ?? 75),
    contextUsage: { usedTokens: options.used ?? 123_456, maxTokens: 200_000 } }];
  await context.command('select-thread', 't1');
  await context.client.refresh(context.native, context.disk);
  const loaded = context.client.thread!.projection;
  loaded.runs = [{ id: 'r1', ordinal: 1, status: 'completed', startedAt: minutesAgo(80), completedAt: minutesAgo(75) }];
  loaded.visibleTurnItems = [{ item: { type: 'user_message', text: 'Build the parser', attachments: [] } }];
  return context;
}
const resume = (client: Parameters<typeof snapshot>[0]) => snapshot(client, NOW).composer.notices.find(notice => notice.title === 'Resume with less context');

describe('resume with less context (a976f8c74c)', () => {
  test('offered for a Claude session idle 70 minutes with 100k tokens', () => {
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 100_000, updatedAt: minutesAgo(70.5), now: NOW })).toBe(true);
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 99_999, updatedAt: minutesAgo(90), now: NOW })).toBe(false);
    expect(shouldOffer({ driver: 'codex', usedTokens: 500_000, updatedAt: minutesAgo(90), now: NOW })).toBe(false);
    // The clock counts whole minutes (12:00): 70 minutes and 20 seconds ago is still 69m40s before it.
    expect(shouldOffer({ driver: 'claudeAgent', usedTokens: 200_000, updatedAt: new Date(NOW - 70 * 60_000 - 20_000).toISOString(), now: NOW })).toBe(false);
  });
  test('the banner: tokens from earlier, Compact, Keep full history', async () => {
    const { client, command } = await claudeThread();
    const notice = resume(client)!;
    expect(notice).toMatchObject({ variant: 'info', icon: 'minimize', description: '123k tokens from earlier', action: 'cc:compact', actionLabel: 'Compact',
      actionReason: '', dismissLabel: 'Keep full history' });
    await command(notice.dismiss, notice.dismissId);
    expect(resume(client)).toBeUndefined();
    // A newer context snapshot is a new key.
    arr(client.thread!.projection.providerThreads)[0]!.updatedAt = minutesAgo(72);
    expect(resume(client)).toBeDefined();
  });
  test('Compact is disabled with its reason when the provider cannot compact', async () => {
    const { client } = await claudeThread({ compact: false });
    expect(resume(client)).toMatchObject({ actionReason: 'Compaction is unavailable for this provider' });
  });
  test('not offered under the thresholds, or after "Don\'t ask again"', async () => {
    expect(resume((await claudeThread({ used: 80_000 })).client)).toBeUndefined();
    expect(resume((await claudeThread({ idle: 30 })).client)).toBeUndefined();
    const { client } = await claudeThread();
    client.thread!.projection.runtimeRequests = [{ id: 'q', kind: 'user_input', status: 'resolved',
      answers: { 'This session is 1h 15m old and uses 123,456 tokens. Compact it before continuing?': NEVER_ANSWER } }];
    expect(resume(client)).toBeUndefined();
    client.thread!.projection.runtimeRequests = [];
    expect(resume(client)).toBeUndefined();
  });
  test('sending past the banner compacts first, then queues the message behind it', async () => {
    const { client, native, command } = await claudeThread();
    expect(resume(client)).toBeDefined();
    await command('send', '', 'Continue the parser');
    const sent = native.committed.filter(entry => entry.type === 'message.dispatch');
    expect(sent.map(entry => [entry.text, obj(entry.dispatchMode).type])).toEqual([['/compact', 'start_immediately'], ['Continue the parser', 'queue_after_active']]);
    expect(sent[0]).toMatchObject({ modelSelection: { instanceId: 'claude', model: 'model-a' }, attachments: [] });
  });
  test('"/compact" itself, or a thread without the banner, sends once', async () => {
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
    const woke = () => snapshot(client, NOW).composer.notices.find(notice => notice.title === 'Thread woke from snooze');
    expect(woke()).toBeDefined();
    await command(woke()!.dismiss, woke()!.dismissId);
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
    const woke = () => snapshot(client, NOW).composer.notices.find(notice => notice.title === 'Thread woke from snooze');
    expect(woke()).toBeDefined();
    await command(woke()!.dismiss, woke()!.dismissId);
    expect(native.committed.some(entry => entry.type === 'thread.visit')).toBe(false);
    expect(client.local.composerControls.wokeSeen[`${client.environmentId}:t1`]).toBe(wokeAt);
    expect(woke()).toBeUndefined();
  });
});
