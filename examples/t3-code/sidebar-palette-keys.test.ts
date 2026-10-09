// shell-sidebar-palette-keys: ⌘N on a focused row (SH-1), ⇧⌘S's undo notice (SH-2), the palette's
// project picks by ⌘1–⌘9 (SH-3), the no-projects header (SH-4), the palette path's start (SH-5) and
// Custom snooze's calendar (TH-8). The Contract wiring is read from the sources, as dialog-focus.test.ts
// reads it; the behavior is proven by the macOS drive in the task record.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { chatCommand } from './chat-commands';
import { sidebarCommand, sidebarLocal } from './sidebar-commands';
import { sidebarSession, setRuntimeClock, openSnoozeDialog } from './sidebar-state';
import { sidebarSnapshot } from './sidebar-view';
import { newThreadInItems } from './palette';
import { calendarMove, dateLabel, snoozeCalendar } from './snooze-calendar';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const NOW = new Date(2026, 9, 9, 21, 37).getTime(); // Friday, October 9, 2026, local time
beforeEach(() => setRuntimeClock(() => NOW));
afterEach(() => setRuntimeClock(() => Number.NaN));

const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true };
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', createdAt: new Date(NOW - 3_600_000).toISOString(),
  updatedAt: new Date(NOW - 3_600_000).toISOString(), archivedAt: null, settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, ...extra });
function fake(threads: Obj[], threadId = '') {
  const dispatched: Obj[] = [], opened: string[] = [];
  const client = {
    shell: { projects: [{ id: 'p1', title: 'Fixture', workspaceRoot: '/fixture' }], threads, sequence: 1 },
    config: { environment: { capabilities: CAPS }, providers: [], keybindings: [] },
    environmentId: 'env', threadId, projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: '12-hour' }, clientSettings: { confirmThreadUnpin: false }, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'Fixture', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      dispatch: async (_storage: Files, payload: Obj) => { dispatched.push(payload); return {}; },
      call: async () => ({}),
    }),
    async openSelected(_native: Native, id: string) { opened.push(id); },
  } as unknown as T3Client;
  return { client, dispatched, opened };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };

describe('SH-1: ⌘N and ⇧⌘N while a sidebar row holds the focus', () => {
  test('a modifier\'s own press on a focused row sends nothing, so it leaves commandPending for the chord\'s button', async () => {
    const app = await source('app.contract');
    const run = app.split('\n').find(line => line.trim().startsWith('if not commandPending and ((op != "row-key"'))!;
    for (const key of ['"Enter"', '" "', '"Meta"', '"Shift"', '"Control"', '"Alt"', '"CapsLock"']) expect(run).toContain(`value != ${key}`);
    expect(run).toContain('op != "legacy-row-key"');
    // The rows' key handlers all go through that action (the sidebar's `run`).
    expect(await source('sidebar-row.contract')).toContain('key=run("row-key", t.id)');
    expect(await source('legacy-sidebar.contract')).toContain('run("legacy-row-key", t.id, name)');
  });
});

describe('SH-2: ⇧⌘S settles with the sidebar\'s undo notice', () => {
  test('the chord\'s chat:settle records the same undo step as the row\'s Settle, without moving on', async () => {
    const chord = fake([thread('a'), thread('b')], 'a');
    await chatCommand(chord.client, native, {} as Files, 'settle', 'a');
    expect(sidebarSnapshot(chord.client, NOW, helpers).sidebar.undoText).toBe('Settled 1 thread,');
    expect(chord.opened).toEqual([]); // ChatView's thread.settle calls settleThread: no forward navigation
    await sidebarCommand(chord.client, native, {} as Files, 'undo', '', '');
    expect(chord.dispatched.map(payload => `${payload.type}:${payload.threadId}`)).toEqual(['thread.settle:a', 'thread.unsettle:a']);
    const row = fake([thread('a'), thread('b')]);
    await sidebarCommand(row.client, native, {} as Files, 'settle', 'a', '');
    expect(sidebarSnapshot(row.client, NOW, helpers).sidebar.undoText).toBe('Settled 1 thread,');
  });
  test('under the agent\'s clock the notice is timed on the window\'s instant, which the chord\'s command carries', async () => {
    setRuntimeClock(() => 50_000); // the data runtime's clock is the driver's, not the epoch
    const chord = fake([thread('a')], 'a');
    await chatCommand(chord.client, native, {} as Files, 'settle', 'a', '', NOW);
    // The window hides the notice once its own wall time passes undoUntil (app.contract).
    expect(sidebarSnapshot(chord.client, NOW, helpers).sidebar).toMatchObject({ undoText: 'Settled 1 thread,', undoUntil: NOW + 5000 });
    const app = await source('app.contract');
    expect(app).toContain('send changed = command(op, id, value, (n == 0 and startsWith(op, "chat:")) ? wallTime.epochAtZero + now() : n)');
    expect(await source('client-ops-lanes.ts')).toContain("chatCommand(this, native, storage, op.slice(5), id, value, n)");
  });
});

describe('SH-3: the New thread in… picks by ⌘1–⌘9', () => {
  const desktop = { type: 'identifier', name: 'isDesktop' };
  function client(count: number, bindings: Obj[]): T3Client {
    const c = new T3Client();
    Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, environmentId: 'env' });
    c.config = { environment: { capabilities: {} }, settings: {}, keybindings: bindings };
    c.shell = { sequence: 1, threads: [], projects: Array.from({ length: count }, (_, index) => ({ id: `p${index + 1}`, title: `Project ${index + 1}`, workspaceRoot: `/p${index + 1}`, updatedAt: new Date(NOW - index * 1000).toISOString() })) };
    c.projectId = 'p1';
    return c;
  }
  const jump = (n: number, key = String(n)) => ({ command: `thread.jump.${n}`, shortcut: { key, modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }, whenAst: desktop });
  test('the first nine rows take thread.jump.N\'s label and chord; the tenth none', () => {
    const rows = newThreadInItems(client(10, Array.from({ length: 9 }, (_, index) => jump(index + 1)))).map(item => item.row);
    expect(rows.map(row => row.shortcut)).toEqual(['⌘1', '⌘2', '⌘3', '⌘4', '⌘5', '⌘6', '⌘7', '⌘8', '⌘9', '']);
    expect(rows.map(row => row.jump)).toEqual(['Meta+1', 'Meta+2', 'Meta+3', 'Meta+4', 'Meta+5', 'Meta+6', 'Meta+7', 'Meta+8', 'Meta+9', '']);
  });
  test('a rebound jump follows the keybindings; a model picker rule on the same chord does not take it', () => {
    const picker = { command: 'modelPicker.jump.1', shortcut: { key: '1', modKey: true }, whenAst: { type: 'and', left: { type: 'identifier', name: 'modelPickerOpen' }, right: desktop } };
    const rows = newThreadInItems(client(2, [jump(1), picker, jump(2, 'j')])).map(item => item.row);
    expect(rows.map(row => [row.shortcut, row.jump])).toEqual([['⌘1', 'Meta+1'], ['⌘J', 'Meta+j']]);
  });
  test('the palette field runs the row whose chord was typed, before the root\'s keys', async () => {
    const palette = await source('palette.contract');
    expect(palette).toContain('input id="palette-input" testId="palette-input" value=query input=edit key=inputKey submit=run(tOp, tArg, tArg2)');
    expect(palette).toContain('match first(filter(view.rows, (r) => r.jump != "" and r.jump == `${e.metaKey ? "Meta+" : ""}${e.ctrlKey ? "Control+" : ""}${e.altKey ? "Alt+" : ""}${e.shiftKey ? "Shift+" : ""}${k}`))');
    expect(palette).toMatch(/case some\(r\)\n\s+preventDefault\(\)\n\s+run\(r\.op, r\.arg, r\.arg2\)\n\s+case none\n\s+onKey\(k\)/);
  });
});

describe('SH-4 and SH-5: the header without projects, the palette row\'s start', () => {
  test('Filter and Add project show only with a project group; New thread stays, disabled without projects', async () => {
    const lines = (await source('sidebar.contract')).split('\n');
    const gate = lines.findIndex(line => line.trim() === 'when data.sidebar.projectGroupCount > 0');
    const indent = (line: string) => line.length - line.trimStart().length;
    const inside = lines.slice(gate + 1, lines.findIndex((line, index) => index > gate && line.trim() !== '' && !line.trim().startsWith('//') && indent(line) <= indent(lines[gate]!)));
    expect(inside.some(line => line.includes('testId="filter-project"'))).toBe(true);
    expect(inside.some(line => line.includes('testId="add-project"'))).toBe(true);
    expect(inside.some(line => line.includes('testId="new-thread"'))).toBe(false);
    expect(lines.find(line => line.includes('testId="new-thread"'))).toContain('disabled=(busy or not data.connected or not data.sidebar.hasProjects)');
  });
  test('a palette option row starts its text at the left (X57: a button centres an overflowing line)', async () => {
    const option = (await source('palette.contract')).split('\n').find(line => line.includes('testId=`palette-row-${row.key}` width="100%" min-height="2rem"'))!;
    expect(option).toContain('text-align="left"');
  });
});

describe('TH-8: Custom snooze\'s date button and calendar', () => {
  test('the trigger reads as the reference\'s toLocaleDateString (en-US, short month)', () => {
    expect(dateLabel('2026-10-09')).toBe('Oct 9, 2026');
    expect(dateLabel('2026-02-30')).toBe('2026-02-30');
  });
  test('October 2026 from Sunday: five weeks, outside days, past days disabled, today and the selection', () => {
    const cal = snoozeCalendar('2026-10-09', '2026-10', '2026-10-09', 0);
    expect(cal.title).toBe('October 2026');
    expect(cal.weekdays.map(day => day.short)).toEqual(['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']);
    expect(cal.weeks.map(week => week.id)).toEqual(['2026-09-27', '2026-10-04', '2026-10-11', '2026-10-18', '2026-10-25']);
    const days = cal.weeks.flatMap(week => week.days);
    expect(days[0]).toMatchObject({ id: '2026-09-27', label: '27', outside: true, disabled: true });
    expect(days.at(-1)).toMatchObject({ id: '2026-10-31', outside: false, disabled: false });
    const today = days.find(day => day.id === '2026-10-09')!;
    expect(today).toMatchObject({ today: true, selected: true, disabled: false, stop: true, aria: 'Today, Friday, October 9th, 2026, selected' });
    expect(days.find(day => day.id === '2026-10-08')).toMatchObject({ disabled: true, stop: false, aria: 'Thursday, October 8th, 2026' });
    expect(days.filter(day => day.stop).length).toBe(1);
  });
  test('the locale\'s week start: from Monday, November 2026 spans six weeks', () => {
    const cal = snoozeCalendar('2026-10-09', '2026-11', '2026-10-09', 1);
    expect(cal.weekdays.map(day => day.short)).toEqual(['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']);
    expect(cal.weeks.map(week => week.id)).toEqual(['2026-10-26', '2026-11-02', '2026-11-09', '2026-11-16', '2026-11-23', '2026-11-30']);
    // The selection is in October: the month's first enabled day is the Tab stop.
    expect(cal.weeks.flatMap(week => week.days).find(day => day.stop)!.id).toBe('2026-11-01');
  });
  test('keys move a day, a week, to the week\'s ends and by months, skipping disabled days', () => {
    const today = '2026-10-09';
    expect(calendarMove('2026-10-10', 'ArrowLeft', today, 0)).toBe('2026-10-09');
    expect(calendarMove('2026-10-09', 'ArrowLeft', today, 0)).toBe(''); // every earlier day is disabled
    expect(calendarMove('2026-10-31', 'ArrowRight', today, 0)).toBe('2026-11-01');
    expect(calendarMove('2026-10-14', 'ArrowUp', today, 0)).toBe('');
    expect(calendarMove('2026-10-28', 'ArrowDown', today, 0)).toBe('2026-11-04');
    expect(calendarMove('2026-10-14', 'Home', today, 1)).toBe('2026-10-12');
    expect(calendarMove('2026-10-14', 'End', today, 0)).toBe('2026-10-17');
    expect(calendarMove('2026-10-10', 'Home', today, 0)).toBe(''); // Sunday the 4th is past
    expect(calendarMove('2026-10-31', 'PageDown', today, 0)).toBe('2026-11-30');
    expect(calendarMove('2026-11-30', 'PageUp', today, 0)).toBe('2026-10-30');
    expect(calendarMove('2026-10-09', 'Shift+PageDown', today, 0)).toBe('2027-10-09');
    expect(calendarMove('2026-10-09', 'Enter', today, 0)).toBe('');
  });
  test('the popover opens on the selected month, pages months, and a key moves the focus with the month', async () => {
    const { client } = fake([thread('a')]);
    const session = sidebarSession(client);
    openSnoozeDialog(session, ['a'], 'Thread a', NOW);
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar).toMatchObject({ dateLabel: 'Oct 9, 2026', title: 'October 2026', focusSeq: 0 });
    await sidebarLocal(client, native, 'calendar-month', '', 'next');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar.title).toBe('November 2026');
    await sidebarLocal(client, native, 'calendar-open', '', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar.title).toBe('October 2026');
    await sidebarLocal(client, native, 'calendar-key', '2026-10-31', 'ArrowRight');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar).toMatchObject({ title: 'November 2026', focus: '2026-11-01', focusSeq: 1 });
    await sidebarLocal(client, native, 'dialog-field', 'date', '2026-11-01');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar.dateLabel).toBe('Nov 1, 2026');
    // Closed, the snapshot carries no month.
    await sidebarLocal(client, native, 'dialog-close', '', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.calendar.weeks).toEqual([]);
  });
  test('the dialog draws the date button, the root focuses a moved day, and the Unit select spans its column', async () => {
    const overlays = await source('sidebar-overlays.contract');
    expect(overlays).toContain('SnoozeDateField(cal=data.sidebar.calendar, local=local)');
    expect(overlays).not.toContain('type="date"');
    expect(overlays).toContain('testId="snooze-unit" width="100%"');
    const field = await source('snooze-calendar.contract');
    expect(field).toContain('button id="snooze-date" popovertarget="snooze-calendar" press=local("calendar-open", "", "")');
    expect(field).toContain('column id="snooze-calendar" popover="auto" role="dialog" aria-label="Choose snooze date"');
    expect(field).toContain('column aria-modal=true retainFocus=true tabindex=-1 key=gridKey'); // Escape is the popover's while it shows
    expect(field).toContain('press=pick popovertarget="snooze-calendar" popovertargetaction="hide"');
    const app = await source('app.contract');
    expect(app).toContain('task snoozeCalendarFocus when data.sidebar.calendar.focusSeq > 0 key=data.sidebar.calendar.focusSeq');
    expect(app).toContain('focus(`snooze-day-${data.sidebar.calendar.focus}`)');
  });
});
