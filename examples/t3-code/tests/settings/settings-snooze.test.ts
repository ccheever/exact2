// Settings › Threads › Snooze options: the presets, their order and every edit (SnoozePresetsSettings at 6d450a4284).
import { describe, expect, test } from 'bun:test';
import { DEFAULT_SNOOZE_PRESETS, MAX_SNOOZE_PRESETS, decodeSnoozePreset, nextSnoozeCandidate, resolveSnoozePreset, resolveSnoozePresets, sameSnoozePresets, snoozeEditor,
  snoozePresetsEdit, snoozePresetsOf, sortSnoozeRows, splitMinutes, type SnoozePresetSetting } from '../../src/settings/settings-snooze';

// Thursday 2026-10-29 16:00 local.
const now = new Date(2026, 9, 29, 16, 0, 0);
const hhmm = (date: Date) => `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
const friday: SnoozePresetSetting = { kind: 'at', day: 'friday', time: '09:00' };

describe('snooze presets', () => {
  test('decode: the schema\'s kinds and bounds, and the defaults for a missing or invalid list', () => {
    expect(decodeSnoozePreset({ kind: 'after', minutes: 90 })).toEqual({ kind: 'after', minutes: 90 });
    expect([decodeSnoozePreset({ kind: 'after', minutes: 0 }), decodeSnoozePreset({ kind: 'after', minutes: 30 * 24 * 60 + 1 }), decodeSnoozePreset({ kind: 'at', day: 'someday', time: '09:00' }),
      decodeSnoozePreset({ kind: 'at', day: 'friday', time: '24:00' })]).toEqual([null, null, null, null]);
    expect(snoozePresetsOf({})).toEqual([...DEFAULT_SNOOZE_PRESETS]);
    expect(snoozePresetsOf({ snoozePresets: [{ kind: 'after', minutes: 0 }] })).toEqual([...DEFAULT_SNOOZE_PRESETS]);
    expect(snoozePresetsOf({ snoozePresets: [] })).toEqual([]);
  });

  test('labels follow the day gap, and a passed today option has no wake', () => {
    const label = (setting: SnoozePresetSetting) => resolveSnoozePreset(setting, now, hhmm);
    expect(label({ kind: 'after', minutes: 180 })).toMatchObject({ label: 'In 3 hours', whenLabel: '19:00' });
    expect(label({ kind: 'after', minutes: 90 })!.label).toBe('In 90 minutes');
    expect(label({ kind: 'after', minutes: 2 * 24 * 60 })).toMatchObject({ label: 'In 2 days', whenLabel: 'Sat 16:00' });
    expect(label({ kind: 'at', day: 'today', time: '18:00' })).toMatchObject({ label: 'This evening', whenLabel: '18:00' });
    expect(label({ kind: 'at', day: 'today', time: '09:00' })).toBeNull();
    expect(label(friday)).toMatchObject({ label: 'Tomorrow', whenLabel: '09:00' }); // Friday is tomorrow on a Thursday
    expect(label({ kind: 'at', day: 'monday', time: '09:00' })!.label).toBe('Next week');
    expect(label({ kind: 'at', day: 'thursday', time: '09:00' })!.label).toBe('Next Thursday');
    expect(label({ kind: 'at', day: 'saturday', time: '10:30' })!.label).toBe('Saturday');
  });

  test('sorting: soonest wake first, no wake last, ties in saved order', () => {
    const presets: SnoozePresetSetting[] = [{ kind: 'at', day: 'monday', time: '09:00' }, { kind: 'at', day: 'today', time: '09:00' }, { kind: 'after', minutes: 180 }, { kind: 'at', day: 'today', time: '18:00' }, { kind: 'after', minutes: 60 }];
    expect(sortSnoozeRows(presets, now)).toEqual([4, 3, 2, 0, 1]);
    // The menu drops the passed option and keeps one row per wake minute.
    expect(resolveSnoozePresets(now, [...presets, { kind: 'after', minutes: 120 }], hhmm).map(entry => entry.label)).toEqual(['In 1 hour', 'This evening', 'In 3 hours', 'Next week']);
  });

  test('validation and edits write the whole list, keyed by the saved index', () => {
    const list = [...DEFAULT_SNOOZE_PRESETS];
    expect(snoozePresetsEdit(list, 'when:4', 'friday')).toEqual([...list.slice(0, 4), friday]);
    expect(snoozePresetsEdit(list, 'when:0', 'friday')).toEqual([friday, ...list.slice(1)]);
    expect(snoozePresetsEdit(list, 'when:2', 'after')).toEqual([list[0], list[1], { kind: 'after', minutes: 60 }, list[3], list[4]]);
    expect(snoozePresetsEdit(list, 'amount:1', '5')).toEqual([list[0], { kind: 'after', minutes: 300 }, ...list.slice(2)]);
    expect(snoozePresetsEdit(list, 'amount:1', '0')).toEqual([list[0], { kind: 'after', minutes: 60 }, ...list.slice(2)]);
    expect(snoozePresetsEdit(list, 'amount:1', '')).toEqual(list);
    expect(snoozePresetsEdit([{ kind: 'after', minutes: 2 * 24 * 60 }], 'amount:0', '99')).toEqual([{ kind: 'after', minutes: 30 * 24 * 60 }]);
    expect(snoozePresetsEdit(list, 'unit:0', 'days')).toEqual([{ kind: 'after', minutes: 24 * 60 }, ...list.slice(1)]);
    expect(snoozePresetsEdit(list, 'time:3', '07:45')).toEqual([...list.slice(0, 3), { kind: 'at', day: 'tomorrow', time: '07:45' }, list[4]]);
    expect(() => snoozePresetsEdit(list, 'time:3', '7:45pm')).toThrow('HH:MM');
    expect(() => snoozePresetsEdit(list, 'time:0', '07:45')).toThrow('duration');
    expect(() => snoozePresetsEdit(list, 'when:0', 'someday')).toThrow();
    expect(() => snoozePresetsEdit(list, 'remove:9', '')).toThrow('no longer available');
    expect(snoozePresetsEdit(list, 'remove:2', '')).toEqual([list[0], list[1], list[3], list[4]]);
    // Add offers the defaults, then 30 minutes, Saturday 09:00 and a day, skipping any already listed; up to eight.
    expect(snoozePresetsEdit(list, 'add', '')).toEqual([...list, { kind: 'after', minutes: 30 }]);
    expect(nextSnoozeCandidate([])).toEqual(DEFAULT_SNOOZE_PRESETS[0]);
    const full = Array.from({ length: MAX_SNOOZE_PRESETS }, (_, index): SnoozePresetSetting => ({ kind: 'after', minutes: index + 1 }));
    expect(() => snoozePresetsEdit(full, 'add', '')).toThrow('Up to 8 options.');
    expect([splitMinutes(90), splitMinutes(120), splitMinutes(2880)]).toEqual([{ amount: 90, unit: 'minutes' }, { amount: 2, unit: 'hours' }, { amount: 2, unit: 'days' }]);
    expect(sameSnoozePresets([...list].reverse(), list)).toBe(true);
  });

  test('the editor: menu order, previews, the menu line and the limit', () => {
    const editor = snoozeEditor([...DEFAULT_SNOOZE_PRESETS, friday, { kind: 'at', day: 'today', time: '09:00' }, { kind: 'after', minutes: 60 }], now, '24-hour');
    expect(editor.rows.map(row => [row.whenLabel, row.after ? `${row.amount} ${row.unitLabel}` : row.time, row.previewMenu ? row.previewLabel : row.previewText]))
      .toEqual([['In', '1 hour', 'In 1 hour'], ['In', '1 hour', 'Duplicate of an option above'], ['Today', '18:00', 'This evening'], ['In', '3 hours', 'In 3 hours'],
        ['Tomorrow', '09:00', 'Tomorrow'], ['Friday', '09:00', 'Same time as an option above'], ['Monday', '09:00', 'Next week'], ['Today', '09:00', 'Passed for today']]);
    expect(editor.rows.map(row => row.index)).toEqual([0, 7, 2, 1, 3, 5, 4, 6]);
    expect(editor.menuLine).toBe('Menu now: In 1 hour · This evening · In 3 hours · Tomorrow · Next week');
    expect([editor.full, editor.fullNote]).toEqual([true, 'Up to 8 options']);
    expect(snoozeEditor([], now, '24-hour').menuLine).toBe('Menus offer only Custom…');
    // Two options with one label name their times.
    expect(snoozeEditor([{ kind: 'at', day: 'today', time: '17:00' }, { kind: 'at', day: 'today', time: '19:00' }], now, '24-hour').menuLine).toBe('Menu now: This evening 17:00 · This evening 19:00');
  });
});
