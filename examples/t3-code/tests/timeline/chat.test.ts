import { describe, expect, test } from 'bun:test';
import { timelineMessages, messageTime } from '../../src/timeline/timeline-presentation';
import { chatCommand } from '../../src/sidebar/chat-commands';
import type { T3Client } from '../../src/client/client';
import type { Message, Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { appPath, appText } from '../support/app-files';

const at = '2026-10-03T13:13:00.000Z';
function fakeClient(checkpoints: Obj[] = []) {
  return { threadId: 't1', projection: { checkpoints }, local: { deviceSettings: { timestampFormat: '24-hour' } } } as unknown as T3Client;
}
// An answer of run r1 the provider can fork (transcriptRows sets canFork from the projected item).
const row = (id: string, kind: string, extra: Partial<Message> = {}): Message => ({ id, kind, title: kind, body: '', createdAt: at, runId: 'r1', sourceThreadId: 't1', completed: true, canFork: true, ...extra });

describe('timeline rows', () => {
  test('the first row carries the 16pt lead and an answer owns its own actions', () => {
    const rows = timelineMessages(fakeClient(), [row('u', 'user'), row('w', 'work'), row('a', 'assistant')], Date.parse(at));
    expect(rows.map(message => message.first)).toEqual([true, false, false]);
    expect(rows[2]).toMatchObject({ actionsId: 'a', actionsFork: true, actionsTime: messageTime(at, Date.parse(at), '24-hour'), attached: false });
    expect(rows[0]).toMatchObject({ actionsId: '', actionsFork: false });
  });

  test('changed files attach to the answer of the same run and carry its actions below them', () => {
    const files = [{ path: 'fixture-result.md', additions: 3, deletions: 0 }];
    const rows = timelineMessages(fakeClient([{ id: 'cp', appRunOrdinal: 1 }]),
      [row('u', 'user'), row('a', 'assistant'), row('c', 'checkpoint', { checkpointId: 'cp', files })], Date.parse(at));
    expect(rows[1]).toMatchObject({ actionsId: '', attached: false });
    expect(rows[2]).toMatchObject({ attached: true, actionsId: 'a', actionsFork: true, checkpointOrdinal: 1, additions: 3, deletions: 0 });
    expect(rows[2]!.files).toEqual([{ id: 'file:fixture-result.md', path: 'fixture-result.md', additions: 3, deletions: 0, kind: 'file', name: 'fixture-result.md',
      depth: 0, expanded: false, icon: 'markdown', spacer: false, plus: '+3', minus: '-0' }]);
    expect(rows[2]).toMatchObject({ fileCount: 1, targetId: 'fixture-result.md' });
  });

  test('a checkpoint from another run stays standalone; one from another thread or without files is no row', () => {
    const files = [{ path: 'a.ts', additions: 1, deletions: 1 }];
    const rows = timelineMessages(fakeClient(), [row('a', 'assistant'), row('c', 'checkpoint', { runId: 'r2', files }),
      row('b', 'assistant'), row('d', 'checkpoint', { files: [] }), row('e', 'checkpoint', { sourceThreadId: 'source', files })], Date.parse(at));
    expect(rows.map(message => message.id)).toEqual(['a', 'c', 'b']);
    expect(rows[1]).toMatchObject({ attached: false, actionsId: '' });
    expect(rows[0]).toMatchObject({ actionsId: 'a' });
    expect(rows[2]).toMatchObject({ actionsId: 'b', last: true });
  });

  test('timestamps follow the selected 12/24-hour format and say yesterday across midnight', () => {
    const now = Date.parse('2026-10-04T12:00:00');
    expect(messageTime('2026-10-04T09:05:00', now, '24-hour')).toBe(new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit', hour12: false }).format(new Date('2026-10-04T09:05:00')));
    expect(messageTime('2026-10-03T21:05:00', now, '12-hour')).toStartWith('yesterday at ');
    expect(messageTime('not a date', now, 'locale')).toBe('');
  });
});

describe('sidebar row actions', () => {
  function client(capabilities: Obj = { threadSettlement: true }) {
    const dispatched: Obj[] = [];
    const value = {
      shell: { threads: [{ id: 't1', title: 'Settled one' }, { id: 't2', title: 'Live one' }] },
      config: { environment: { capabilities } }, writable: true, connection: 'connected',
      restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
        // Settle goes through the sidebar's settle (its undo notice), which dispatches by request.
        request: async (method: string, payload: Obj) => { dispatched.push({ ...payload, method }); return {}; },
        dispatch: async (_storage: Files, payload: Obj, description: string) => { dispatched.push({ ...payload, description }); return {}; } }),
    } as unknown as T3Client;
    return { value, dispatched };
  }
  const native = { later: async () => ({}) } as unknown as Native, storage = {} as Files;

  test('settle and un-settle dispatch the row thread, not the selected thread', async () => {
    const { value, dispatched } = client();
    await chatCommand(value, native, storage, 'settle', 't2');
    await chatCommand(value, native, storage, 'unsettle', 't1');
    expect(dispatched).toEqual([
      { type: 'thread.settle', commandId: 'id-0', threadId: 't2', method: 'orchestration.dispatchCommand' },
      { type: 'thread.unsettle', commandId: 'id-0', threadId: 't1', reason: 'user', description: 'Un-settle Settled one' },
    ]);
  });

  test('missing threads, unsupported servers and unknown actions are refused before dispatch', async () => {
    const { value, dispatched } = client();
    await expect(chatCommand(value, native, storage, 'settle', 'gone')).rejects.toThrow('no longer available');
    await expect(chatCommand(value, native, storage, 'archive', 't1')).rejects.toThrow('Unknown thread action');
    const unsupported = client({ threadSettlement: false });
    await expect(chatCommand(unsupported.value, native, storage, 'settle', 't1')).rejects.toThrow('does not support');
    expect(dispatched).toEqual([]);
    expect(unsupported.dispatched).toEqual([]);
  });
});

describe('snooze', () => {
  test('presets follow the shared rules: soonest first, a passed today left out, one row per wake minute', () => {
    const { snoozePresets, pickSnoozePreset, snoozePresetSettings } = require('../../src/sidebar/sidebar-presentation') as typeof import('../../src/sidebar/sidebar-presentation');
    const sundayMorning = new Date(2026, 9, 4, 0, 9).getTime();
    // Monday 9:00 is tomorrow: the same wake minute as Tomorrow, which comes first in the settings.
    expect(snoozePresets(sundayMorning, '12-hour').map(preset => preset.id)).toEqual(['after:60', 'after:180', 'at:today:18:00', 'at:tomorrow:09:00']);
    const wednesdayEvening = new Date(2026, 9, 7, 17, 30).getTime();
    const late = snoozePresets(wednesdayEvening, '24-hour');
    expect(late.map(preset => [preset.id, preset.label])).toEqual([['at:today:18:00', 'This evening'], ['after:60', 'In 1 hour'], ['after:180', 'In 3 hours'],
      ['at:tomorrow:09:00', 'Tomorrow'], ['at:monday:09:00', 'Next week']]);
    expect(new Date(late[3]!.until).getHours()).toBe(9);
    expect(new Date(late[4]!.until).getDay()).toBe(1);
    expect(late[1]!.until).toBe(new Date(wednesdayEvening + 3_600_000).toISOString());
    expect(snoozePresets(new Date(2026, 9, 7, 18, 0).getTime(), '24-hour').map(preset => preset.id)).not.toContain('at:today:18:00');
    // The environment's own options, with labels by the day gap and the largest even unit.
    const settings = snoozePresetSettings({ settings: { snoozePresets: [{ kind: 'at', day: 'friday', time: '09:00' }, { kind: 'after', minutes: 90 },
      { kind: 'after', minutes: 2880 }, { kind: 'at', day: 'wednesday', time: '08:00' }, { kind: 'at', day: 'today', time: '09:30' }] } });
    // 09:30 twice: the first in the settings stays.
    expect(snoozePresets(new Date(2026, 9, 7, 8, 0).getTime(), '24-hour', settings).map(preset => [preset.label, preset.wakeLabel])).toEqual([
      ['In 90 minutes', '09:30'], ['In 2 days', 'Fri 08:00'], ['Friday', '09:00'], ['Next Wednesday', '08:00']]);
    // A list the server would refuse falls back to the defaults.
    expect(snoozePresetSettings({ settings: { snoozePresets: [{ kind: 'after', minutes: 0 }] } })).toHaveLength(5);
    expect(snoozePresetSettings({ settings: { snoozePresets: [] } })).toEqual([]);
    // A duration starts when picked; a set time keeps the wake shown, refused once that has passed.
    expect(pickSnoozePreset(late, 'after:60', wednesdayEvening + 60_000, '24-hour')?.until).toBe(new Date(wednesdayEvening + 3_660_000).toISOString());
    expect(pickSnoozePreset(late, 'at:today:18:00', new Date(2026, 9, 7, 18, 1).getTime(), '24-hour')).toBeUndefined();
    const beforeMidnight = new Date(2026, 9, 7, 23, 59).getTime(), afterMidnight = new Date(2026, 9, 8, 0, 1).getTime();
    expect(pickSnoozePreset(snoozePresets(beforeMidnight, '24-hour'), 'at:tomorrow:09:00', afterMidnight, '24-hour')?.until).toBe(new Date(2026, 9, 8, 9, 0).toISOString());
    expect(pickSnoozePreset([], 'at:tomorrow:09:00', afterMidnight, '24-hour')).toBeUndefined();
    expect(snoozePresets(0, 'locale')).toEqual([]);
  });

  test('snooze and wake dispatch for the row thread and validate the wake time', async () => {
    const dispatched: Obj[] = [];
    const value = { shell: { threads: [{ id: 't1', title: 'Live' }] }, config: { environment: { capabilities: { threadSnooze: true } } },
      restAccess: () => ({ ids: async () => ['c1'], dispatch: async (_s: Files, payload: Obj) => { dispatched.push(payload); return {}; } }) } as unknown as T3Client;
    await chatCommand(value, {} as Native, {} as Files, 'snooze', 't1', '2026-10-04T09:00:00.000Z');
    await chatCommand(value, {} as Native, {} as Files, 'unsnooze', 't1');
    expect(dispatched).toEqual([{ type: 'thread.snooze', commandId: 'c1', threadId: 't1', snoozedUntil: '2026-10-04T09:00:00.000Z' },
      { type: 'thread.unsnooze', commandId: 'c1', threadId: 't1', reason: 'user' }]);
    await expect(chatCommand(value, {} as Native, {} as Files, 'snooze', 't1', 'later')).rejects.toThrow('Choose when');
    const off = { ...value, config: { environment: { capabilities: { threadSnooze: false } } } } as unknown as T3Client;
    await expect(chatCommand(off, {} as Native, {} as Files, 'snooze', 't1', '2026-10-04T09:00:00.000Z')).rejects.toThrow('does not support snoozing');
  });
});

test('every TypeScript source app.ts answers is registered with the mixed Rust/TypeScript owner', async () => {
  const app = await Bun.file(appPath('app.ts')).text();
  const rust = await Bun.file(appPath('data/src/markdown.rs')).text();
  const listed = new Set([...rust.split('&["renderMarkdown"]')[0]!.split('exact_data::Mixed::new(')[1]!.matchAll(/"([A-Za-z]+)"/g)].map(match => match[1]));
  const contract = await appText('app.contract');
  const answered = [...app.matchAll(/source === '([A-Za-z]+)'/g), ...contract.matchAll(/\b(serverBrowser[A-Z][A-Za-z]+)\(/g)].map(match => match[1]!);
  expect(answered.filter(source => !listed.has(source))).toEqual([]);
});

// cb4fe60a4b (#24864): the thread details toggle keeps its tooltip while the card is open, as the panel toggles do.
describe('the thread details toggle', () => {
  test('its tooltip shows on hover whether or not the card is open', () => {
    const toggle = appText('chat.contract').split('\n').find(line => line.includes('TipBubble(label=(shell.keyThreadPanel == ""'))!;
    expect(toggle).toContain('shown=(hovered == "details"))');
    expect(toggle).not.toContain('detailsOpen');
  });
});
