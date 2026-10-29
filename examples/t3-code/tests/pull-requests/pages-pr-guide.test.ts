import { afterEach, describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import { connected } from '../support/client-fixture';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { ATTENTION_LABELS, decodeGuideResult, guideRunKey, guideState, moveGuide, settleGuideWatch, type GuideResult } from '../../src/pull-requests/pages-pr-guide-logic';
import { generateGuide, guideEntry, guideReader, guideNavigate, guideOpenTarget, guidesEnabled, presentGuide, readGuide } from '../../src/pull-requests/pages-pr-guide';

const reference = { projectId: 'p', host: 'github.com', repository: 'owner/repo', number: 3 };
const guide = { headSha: 'head', generatedAt: '2026-10-18T01:00:00Z', modelSelection: { instanceId: 'claude', model: 'sonnet' }, stops: [{ title: 'Contract', summary: 'Read the contract.', attention: { kind: 'contract', note: 'Confirm the invariant.' }, excerpts: [{ path: 'src/a.ts', side: 'old', start: 2, end: 3, caption: 'Removed code' }], files: [], notes: [{ path: 'src/a.ts', line: 2, title: 'Preserve this', body: 'The invariant.' }], questions: ['Why this contract?'] }] };
const result = (): GuideResult => decodeGuideResult({ guide, run: null, currentHeadSha: 'head' });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('review guide protocol and reader progress', () => {
  test('every attention kind uses the review heading and contributes its note to the Markdown documents', async () => {
    const { client } = await connected(), entry = guideEntry(client, reference);
    const labels = ['A tradeoff to weigh', 'A contract to check', 'A product call', 'A risk to judge', 'An accessibility call'];
    for (const [index, kind] of Object.keys(ATTENTION_LABELS).entries()) {
      const note = `Inspect **this invariant** in \`src/a.ts\`.`;
      entry.result = decodeGuideResult({ guide: { ...guide, stops: [{ ...guide.stops[0], attention: { kind, note } }] }, run: null, currentHeadSha: 'head' });
      const presented = presentGuide(client, reference, {});
      expect(presented.view.attention).toBe(labels[index]!);
      expect(presented.bodies.find(body => body.id === presented.view.attentionId)?.body).toBe(note);
    }
    entry.result = decodeGuideResult({ guide: { ...guide, stops: [{ ...guide.stops[0], attention: null }] }, run: null, currentHeadSha: 'head' });
    const presented = presentGuide(client, reference, {});
    expect(presented.view.attentionId).toBe('');
    expect(presented.bodies.some(body => body.id.endsWith(':attention'))).toBe(false);
  });
  test('decodes real variants and rejects unknown runs, reversed ranges and malformed dates', () => {
    expect(result().guide?.stops[0].excerpts[0].side).toBe('old');
    for (const value of [{ guide: null, run: { _tag: 'Done' }, currentHeadSha: 'head' }, { guide: { ...guide, generatedAt: 'yesterday' }, run: null, currentHeadSha: 'head' }, { guide: { ...guide, stops: [{ ...guide.stops[0], excerpts: [{ ...guide.stops[0].excerpts[0], end: 1 }] }] }, run: null, currentHeadSha: 'head' }]) expect(() => decodeGuideResult(value)).toThrow('invalid review guide');
  });
  test('retains old guide through running/failed rewrites and only shows a failure matching current head', () => {
    const data = result();
    data.run = { _tag: 'Running', headSha: 'new-head', startedAt: '2026-10-18T02:00:00Z' }; data.currentHeadSha = 'new-head';
    expect(guideState(data)).toMatchObject({ phase: 'ready', running: true, stale: true });
    data.run = { _tag: 'Failed', headSha: 'old-head', failedAt: '2026-10-18T03:00:00Z', detail: 'Failed' };
    expect(guideState(data).failure).toBe(''); data.run.headSha = 'new-head'; expect(guideState(data).failure).toBe('Failed');
  });
  test('forward movement marks only the departed stop; backwards/rail/finish clamp safely', () => {
    const one = moveGuide({ position: 0, seen: new Set() }, 2, 4);
    expect(one).toEqual({ position: 2, seen: new Set([0]) });
    expect(moveGuide(one, -100, 4)).toEqual({ position: 0, seen: new Set([0]) });
    expect(moveGuide(one, 999, 4, true)).toEqual({ position: 3, seen: new Set([0, 2]) });
    expect(moveGuide({ position: 0, seen: new Set() }, 0, 0, true).seen.size).toBe(0);
    expect(guideRunKey('a', reference)).not.toBe(guideRunKey('b', reference));
  });
  test('watches suppress old outcomes, notice new failure before old guide, and expire unseen runs', () => {
    const watch = { previousGuideAt: guide.generatedAt, previousFailedAt: null, since: 0, sawRunning: false };
    expect(settleGuideWatch(watch, result(), 59_999)).toBeNull();
    expect(settleGuideWatch(watch, result(), 60_000)).toBe('timeout');
    const data = result(); data.run = { _tag: 'Failed', headSha: 'head', failedAt: '2026-10-18T04:00:00Z', detail: 'Try another model' };
    expect(settleGuideWatch(watch, data, 10)).toBe('failed');
    data.run = { _tag: 'Running', headSha: 'head', startedAt: '2026-10-18T03:00:00Z' }; expect(settleGuideWatch(watch, data, 10)).toBeNull();
    data.run = null; expect(settleGuideWatch(watch, data, 11)).toBe('timeout');
  });
});

test('guide query and generate stay on the original background environment, deduplicate and retain progress', async () => {
  const { client, native } = await connected();
  client.config.settings = { reviewGuidesEnabled: false };
  const entry = { key: 'remote', environmentId: 'remote-env', origin: 'http://127.0.0.1:16433', phase: 'connected', generation: 3, synchronized: 3,
    scopes: ['orchestration:read', 'orchestration:operate'], config: { environment: { environmentId: 'remote-env', capabilities: { serverResolvedCommandContext: true } }, settings: { reviewGuidesEnabled: true }, providers: [] }, shell: { projects: [], threads: [], sequence: 1 } } as unknown as FleetEntry;
  fleet.entries.set(entry.key, entry);
  const calls: string[] = [], ctx = { client, native, reference: { ...reference, environmentId: entry.environmentId }, detail: { title: 'PR', capabilities: { review: { verdicts: ['approve'] } }, viewerPermissions: { verdicts: ['approve'] } }, now: 100, tab: 'summary', refresh: 0 };
  native.later = async input => { const request = obj(input); expect(request.fleet).toBe(entry.key); calls.push(String(request.method)); return { ok: true, generation: 3, value: request.method === 'pullRequests.generateReviewGuide' ? { guide, run: { _tag: 'Running', headSha: 'head', startedAt: '2026-10-18T03:00:00Z' }, currentHeadSha: 'head' } : { guide, run: null, currentHeadSha: 'head' } }; };
  expect(guidesEnabled(client, ctx.reference)).toBe(true);
  expect(await readGuide(ctx)).toBe('wake'); await readGuide(ctx);
  expect(presentGuide(client, ctx.reference, ctx.detail).view).toMatchObject({ enabled: true, hasGuide: true, canFinish: true });
  await generateGuide(ctx); await generateGuide(ctx);
  expect(calls).toEqual(['pullRequests.reviewGuide', 'pullRequests.generateReviewGuide']);
  expect(guideReader(client).watches.size).toBe(1);
  entry.scopes = ['orchestration:read']; guideEntry(client, ctx.reference).result.run = null;
  await expect(generateGuide(ctx)).rejects.toThrow('orchestration:operate');
  expect(calls).toHaveLength(2);
  // useReviewGuideWriter's copy names the settings page "Source control".
  (entry.config as { settings: Obj }).settings = { reviewGuidesEnabled: false };
  await expect(generateGuide(ctx)).rejects.toThrow('Enable review guides in Settings → Source control.');
});


test('focused references pin the actual owner across focus changes with identical project and PR keys', async () => {
  const { client } = await connected();
  client.environmentId = 'env-a';
  const first = guideEntry(client, reference); first.result = result();
  guideNavigate(client, reference, 0, true);
  guideReader(client).watches.set(first.key, { entry: first, previousGuideAt: null, previousFailedAt: null, since: 0, sawRunning: true, polledAt: 0 });
  client.environmentId = 'env-b';
  const second = guideEntry(client, reference); second.result = result();
  expect(first.environmentId).toBe('env-a'); expect(second.environmentId).toBe('env-b');
  expect(second).not.toBe(first); expect(second.key).not.toBe(first.key);
  expect(presentGuide(client, reference, {}).view.readCount).toBe(0);
  expect(guideReader(client).watches.get(first.key)?.entry.environmentId).toBe('env-a');
  expect(decodeURIComponent(guideOpenTarget(first))).toContain('"environmentId":"env-a"');
});

test('shell watch suppresses the shown guide, settles once, and preserves the original owner in native Open guide', async () => {
  const { coordinateGuides, decodeGuideOpenTarget } = await import('../../src/pull-requests/pages-pr-guide');
  const { toasts } = await import('../../src/shell/toast');
  const { notificationSounds, useNotificationCalls } = await import('../../src/shell/shell-notify');
  const posts: Record<string, unknown>[] = [];
  useNotificationCalls({ showNotification: async options => { posts.push(options); }, closeNotification: async () => {}, setAppBadge: async () => {}, clearAppBadge: async () => {},
    notificationStatus: async () => ({ available: true, state: 'granted', badge: true }), requestNotificationPermission: async () => ({ available: true, state: 'granted', badge: true }) });
  const { client, native } = await connected();
  client.config.settings = { reviewGuidesEnabled: true };
  client.local.clientSettings = { notificationMode: 'notifications-and-sound', inAppNotificationsEnabled: true };
  const entry = guideEntry(client, reference), reader = guideReader(client), operations: Record<string, unknown>[] = [];
  const ready = { guide, run: null, currentHeadSha: 'head' };
  native.later = async input => { const request = obj(input); operations.push(request); return native.good(request.method === 'pullRequests.reviewGuide' ? ready : {}); };
  const status = { active: true, authorization: 'granted', opened: '', openedThread: '' };
  const watch = () => reader.watches.set(entry.key, { entry, previousGuideAt: null, previousFailedAt: null, since: 0, sawRunning: true, polledAt: 0 });
  watch(); reader.shownKey = entry.key;
  await coordinateGuides(client, native, status, 5_000);
  expect(reader.watches.size).toBe(0); expect(toasts(client)).toEqual([]); expect(notificationSounds(client)).toEqual({ completion: 0, input: 0 });
  reader.shownKey = ''; watch();
  await coordinateGuides(client, native, status, 10_000);
  expect(toasts(client).at(-1)?.action).toMatchObject({ op: 'guide-open' });
  expect(JSON.parse(toasts(client).at(-1)!.action!.id!).environmentId).toBe(client.environmentId);
  const count = operations.length; await coordinateGuides(client, native, status, 15_000); expect(operations).toHaveLength(count);
  watch(); await coordinateGuides(client, native, { ...status, active: false }, 20_000);
  const [notification] = posts;
  expect([notification?.tag, notification?.silent]).toEqual([`guide:${entry.key}`, true]);
  const target = new URLSearchParams(String(notification?.navigate).split('?')[1]).get('t')!;
  expect(JSON.parse(decodeGuideOpenTarget(target)!).environmentId).toBe(entry.environmentId);
  expect(notificationSounds(client).completion).toBe(2);
  useNotificationCalls(null);
});

test('the tour loads later diff pages for its stop without replacing Code commit or whitespace scope', async () => {
  const { codeState } = await import('../../src/pull-requests/pages-pr-code');
  const { pullRequestReviewKey } = await import('../../src/pull-requests/pages-pr-writes-logic');
  const { client, native } = await connected(); client.config.settings = { reviewGuidesEnabled: true };
  const selectedGuide = { ...guide, stops: [{ ...guide.stops[0], excerpts: [{ path: 'later.ts', side: 'new', start: 1, end: 1, caption: '' }], notes: [] }] };
  const cursors: unknown[] = [];
  native.later = async input => {
    const request = obj(input);
    if (request.op === 'prDiff') {
      cursors.push(obj(request.payload).cursor ?? null);
      const path = cursors.length === 1 ? 'first.ts' : 'later.ts';
      return native.good({ patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1 +1 @@\n-old\n+new\n`, truncated: false, nextCursor: cursors.length === 1 ? 'later' : null, omittedFileStats: [] });
    }
    return native.good({ guide: selectedGuide, run: null, currentHeadSha: 'head' });
  };
  const code = codeState(client, `${client.environmentId}|${pullRequestReviewKey(reference)}`); code.commit = 'reader-commit'; code.ignoreWhitespace = true;
  const ctx = { client, native, reference, detail: { capabilities: { diff: true } }, now: 100, tab: 'guide', refresh: 0 };
  for (let read = 0; read < 8 && cursors.length < 2; read++) await readGuide(ctx);
  expect(cursors).toEqual([null, 'later']); expect(guideEntry(client, reference).code?.slices).toHaveLength(2);
  expect(code.commit).toBe('reader-commit'); expect(code.ignoreWhitespace).toBe(true);
  expect(guideEntry(client, reference).code?.commit).toBeNull(); expect(guideEntry(client, reference).code?.ignoreWhitespace).toBe(false);
});
