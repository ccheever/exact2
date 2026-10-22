// Ported from T3 Code 74541bab17 packages/client-runtime/src/scheduledTaskWebhook.test.ts and
// apps/web/src/components/settings/scheduledTasksSettings.logic.test.ts (MIT, see LICENSE-T3), with
// the original test names. The editor sends its rows encoded (decodeWebhookFilterRows), and
// webhookSchedule is scheduleFromDraft plus submit()'s checks, so a refusal is a thrown message.
import { describe, expect, test } from 'bun:test';
import { decodeWebhookFilterRows, parseWebhookFilterDrafts, webhookDeliveryLabel, webhookDeliveryTone, webhookDraft, webhookDraftErrors, webhookFilterDrafts, webhookFilterDraftsChanged, webhookSchedule, type WebhookFilterDraft } from '../../src/settings/scheduled-webhook';
import { deliveriesPage } from '../../src/settings/scheduled-deliveries';
import type { T3Client } from '../../src/client/client';
import type { LiveEnvironment } from '../../src/client/live-streams';

/** settings-scheduled.contract `encodeFilters`, as the editor's save writes the rows. */
const encode = (rows: WebhookFilterDraft[]) => rows.map(row => `${encodeURIComponent(row.path)}:${row.op}:${encodeURIComponent(row.value)}`).join(',');
const webhookTask = (schedule: Record<string, unknown>) => ({ schedule: { type: 'webhook', signature: null, ...schedule }, webhook: { hasSecret: false } });
/** The editor's fields as the save's query sends them. */
function formInput(task: ReturnType<typeof webhookTask>, edit: Partial<Record<string, string>> = {}): Record<string, string> {
  const draft = webhookDraft(task);
  const rows = encode(draft.filters);
  return { ...Object.fromEntries(Object.entries(draft).map(([key, value]) => [key, String(value)])), filters: rows, savedFilters: rows, ...edit } as Record<string, string>;
}

describe('webhook filter drafts', () => {
  test('round-trips saved filters through the form rows', () => {
    const filters = [
      { path: 'headers.x-github-event', op: 'equals', value: 'pull_request' },
      { path: 'body.action', op: 'in', value: ['opened', 'reopened'] },
      { path: 'body.label.name', op: 'in', value: ['needs review', 'a-b'] },
      { path: 'query.source', op: 'exists' },
    ];
    const drafts = webhookFilterDrafts(filters);
    expect(drafts[1]).toMatchObject({ path: 'body.action', op: 'in', value: 'opened, reopened' });
    expect(parseWebhookFilterDrafts(drafts)).toEqual({ filters });
    // The editor's encoding keeps paths and values with the separators in them.
    const odd = [{ id: 'a', path: 'body.a:b', op: 'equals', value: 'x,y:z %' }, { id: 'b', path: 'query.q', op: 'startsWith', value: '' }];
    expect(decodeWebhookFilterRows(encode(odd)).map(({ path, op, value }) => ({ path, op, value }))).toEqual(odd.map(({ path, op, value }) => ({ path, op, value })));
    expect(decodeWebhookFilterRows('')).toEqual([]);
  });

  test('names the first row the server would refuse', () => {
    expect(parseWebhookFilterDrafts([
      { id: 'f1', path: 'body.action', op: 'equals', value: 'opened' },
      { id: 'f2', path: 'body.action', op: 'in', value: ' , ' },
      { id: 'f3', path: '', op: 'exists', value: '' },
    ])).toEqual({ error: 'Filter 2 needs a path under body, headers or query, and at least one value.' });
    expect(parseWebhookFilterDrafts([{ id: 'f1', path: ' ', op: 'exists', value: '' }])).toEqual({ error: 'Filter 1 needs a path under body, headers or query.' });
    expect(parseWebhookFilterDrafts([{ id: 'f1', path: 'bodyx.action', op: 'equals', value: 'a' }])).toEqual({ error: 'Filter 1 needs a path under body, headers or query, and a value.' });
    const seventeen = Array.from({ length: 17 }, (_, index) => ({ id: `f${index}`, path: 'body.action', op: 'exists', value: '' }));
    expect(parseWebhookFilterDrafts(seventeen)).toEqual({ error: 'Use at most 16 filters.' });
  });

  test('notices added, removed and edited rows but not an untouched list', () => {
    const saved = webhookFilterDrafts([{ path: 'body.action', op: 'equals', value: 'opened' }]);
    expect(webhookFilterDraftsChanged(saved, [...saved])).toBe(false);
    expect(webhookFilterDraftsChanged(saved, [])).toBe(true);
    expect(webhookFilterDraftsChanged(saved, [{ ...saved[0]!, value: 'closed' }])).toBe(true);
    expect(webhookFilterDraftsChanged([], [])).toBe(false);
    // `exists` ignores the value the row still holds.
    const exists = webhookFilterDrafts([{ path: 'query.source', op: 'exists' }]);
    expect(webhookFilterDraftsChanged(exists, [{ ...exists[0]!, value: 'left over' }])).toBe(false);
  });
});

describe('webhook scheduled tasks', () => {
  test('sends filters only when the form changed them', () => {
    const filters = [
      { path: 'headers.x-github-event', op: 'equals', value: 'pull_request' },
      { path: 'body.action', op: 'in', value: ['opened', 'reopened'] },
    ];
    const task = webhookTask({ filters });
    const rows = webhookDraft(task).filters;
    expect(rows.map(filter => filter.op)).toEqual(['equals', 'in']);
    // A title-only save leaves the key out, so the server keeps whatever is stored now.
    expect(webhookSchedule(formInput(task, { title: 'Renamed' }), task)).not.toHaveProperty('filters');
    expect(webhookSchedule(formInput(task, { filters: encode(rows.slice(0, 1)) }), task)).toMatchObject({ filters: filters.slice(0, 1) });
    // Removing every row is a change, so it clears the stored filters.
    expect(webhookSchedule(formInput(task, { filters: '' }), task)).toMatchObject({ filters: [] });
    expect(() => webhookSchedule(formInput(task, { filters: encode([{ id: 'f1', path: 'body.action', op: 'in', value: ' , ' }]) }), task))
      .toThrow('Invalid filter: Filter 1 needs a path under body, headers or query, and at least one value.');
  });

  test('round-trips dedupe settings, clears them, and rejects invalid windows', () => {
    const dedupe = { replayKey: '{{headers.delivery-id}}', coalesceKey: '{{body.id}}', coalesceWindowSeconds: 10 };
    const task = webhookTask({ dedupe });
    expect(webhookSchedule(formInput(task), task)).toMatchObject({ dedupe });
    expect(webhookSchedule(formInput(task, { replayKey: ' \n ', coalesceKey: ' \t ', coalesceWindowSeconds: '' }), task)).toMatchObject({ dedupe: {} });
    expect(webhookSchedule(formInput(task, { replayKey: `  ${dedupe.replayKey}  `, coalesceKey: `\t${dedupe.coalesceKey}\n` }), task)).toMatchObject({ dedupe });
    expect(webhookSchedule(formInput(task, { coalesceWindowSeconds: '' }), task).dedupe).toEqual({ replayKey: dedupe.replayKey, coalesceKey: dedupe.coalesceKey });
    for (const coalesceWindowSeconds of ['9', '3601', '1.5', 'abc']) expect(() => webhookSchedule(formInput(task, { coalesceWindowSeconds }), task)).toThrow('Invalid webhook settings');
    expect(webhookSchedule(formInput(webhookTask({})), webhookTask({}))).toEqual({ type: 'webhook', signature: null, maxDeliveryAgeMinutes: null, dedupe: {} });
  });

  test('rejects dedupe templates without a request placeholder', () => {
    const task = webhookTask({});
    expect(() => webhookSchedule(formInput(task, { replayKey: 'literal' }), task)).toThrow('Replay key must contain a valid request placeholder.');
    expect(() => webhookSchedule(formInput(task, { coalesceKey: '{{body.bad' }), task)).toThrow('Coalesce key must contain a valid request placeholder.');
    expect(webhookSchedule(formInput(task, { replayKey: 'gh-{{ headers.x-github-delivery }}-{{request}}' }), task)).toMatchObject({ dedupe: { replayKey: 'gh-{{ headers.x-github-delivery }}-{{request}}' } });
  });
});

describe('webhook delivery labels and validation messages', () => {
  test('preserves Ran for legacy and dispatched deliveries and distinguishes pending windows', () => {
    expect(webhookDeliveryLabel({ outcome: 'accepted' })).toBe('Ran');
    expect(webhookDeliveryLabel({ outcome: 'accepted', pendingWindow: false })).toBe('Ran');
    expect(webhookDeliveryLabel({ outcome: 'accepted', pendingWindow: true })).toBe('Opened window');
    expect(['duplicate', 'coalesced', 'filtered', 'some_new_outcome'].map(outcome => webhookDeliveryLabel({ outcome }))).toEqual(['Duplicate', 'Coalesced', 'Filtered', 'some new outcome']);
    expect([{ outcome: 'accepted' }, { outcome: 'accepted', pendingWindow: true }, { outcome: 'duplicate' }, { outcome: 'rejected_signature' }, { outcome: 'filtered' }].map(webhookDeliveryTone))
      .toEqual(['success', 'warning', 'warning', 'error', 'outline']);
  });

  test('keeps the existing age-limit title and body', () => {
    expect(webhookDraftErrors({ maxDeliveryAgeMinutes: '0', replayKey: 'literal' })).toEqual([{ title: 'Invalid age limit', description: 'Enter whole minutes from 1 to 1440, or leave it blank.' }]);
  });

  const cases: [Record<string, string>, string][] = [
    [{ replayKey: 'literal' }, 'Replay key must contain a valid request placeholder.'],
    [{ coalesceKey: '{{body.bad' }, 'Coalesce key must contain a valid request placeholder.'],
    [{ coalesceWindowSeconds: '9' }, 'Coalesce window must be whole seconds from 10 to 3600.'],
  ];
  for (const [invalid, description] of cases) {
    test(`names only the invalid webhook field for ${JSON.stringify(invalid)}`, () => {
      expect(webhookDraftErrors(invalid)).toEqual([{ title: 'Invalid webhook settings', description }]);
    });
  }

  test('reports separate short errors when several dedupe fields fail', () => {
    expect(webhookDraftErrors({ replayKey: 'bad', coalesceKey: 'bad', coalesceWindowSeconds: '0' }).map(error => error.description)).toEqual([
      'Replay key must contain a valid request placeholder.',
      'Coalesce key must contain a valid request placeholder.',
      'Coalesce window must be whole seconds from 10 to 3600.',
    ]);
    // One `Title: description` line each, which settingsFailure toasts one by one.
    const task = webhookTask({});
    expect(() => webhookSchedule(formInput(task, { replayKey: 'bad', coalesceWindowSeconds: '0' }), task))
      .toThrow('Invalid webhook settings: Replay key must contain a valid request placeholder.\nInvalid webhook settings: Coalesce window must be whole seconds from 10 to 3600.');
  });
});

test('deliveries ask for any outcome and mark filtered, duplicate, coalesced and pending-window requests', async () => {
  const calls: unknown[] = [], client = {} as T3Client;
  const deliveries = [
    { id: 'f', outcome: 'filtered', method: 'POST', receivedAt: '2026-10-20T00:00:00Z', missingFields: [], error: 'body.action equals opened did not match' },
    { id: 'd', outcome: 'duplicate', method: 'POST', receivedAt: '2026-10-20T00:00:00Z', missingFields: [], error: null },
    { id: 'c', outcome: 'coalesced', method: 'POST', receivedAt: '2026-10-20T00:00:00Z', missingFields: [], error: null },
    { id: 'w', outcome: 'accepted', pendingWindow: true, method: 'POST', receivedAt: '2026-10-20T00:00:00Z', missingFields: [], error: null },
  ];
  const environment = { environmentId: 'e', key: 'k', request: async (method: string, payload: { deliveryId?: string }) => {
    calls.push([method, payload]);
    return method.endsWith('listWebhookDeliveries') ? { deliveries } : { delivery: { ...deliveries.find(item => item.id === payload.deliveryId), headers: {}, body: '{}', renderedPrompt: null } };
  } } as unknown as LiveEnvironment;
  const task = { id: 'task', title: 'Hook' };
  const list = await deliveriesPage(client, environment, task, '', 0, Date.parse('2026-10-20T00:05:00Z'));
  expect(list.items.map(item => [item.label, item.tone, item.note])).toEqual([
    ['Filtered', 'outline', 'body.action equals opened did not match'], ['Duplicate', 'warning', ''], ['Coalesced', 'warning', ''], ['Opened window', 'warning', ''],
  ]);
  const filtered = await deliveriesPage(client, environment, task, 'f', 0, 0);
  expect(filtered).toMatchObject({ outcome: 'Filtered', filtered: true, error: 'body.action equals opened did not match' });
  const pending = await deliveriesPage(client, environment, task, 'w', 0, 0);
  expect(pending.blocks[0]).toEqual({ title: 'Scheduled run', value: 'Run scheduled when the window closes; no run started yet.' });
  expect(calls).toEqual([
    ['scheduledTasks.listWebhookDeliveries', { id: 'task', acceptAnyOutcome: true }],
    ['scheduledTasks.getWebhookDelivery', { id: 'task', deliveryId: 'f', acceptAnyOutcome: true }],
    ['scheduledTasks.getWebhookDelivery', { id: 'task', deliveryId: 'w', acceptAnyOutcome: true }],
  ]);
});

