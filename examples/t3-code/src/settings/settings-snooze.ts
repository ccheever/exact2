// Adapted from T3 Code 6d450a4284 (MIT); see LICENSE-T3.
// Settings › Threads › Snooze, "Snooze options": the environment's `snoozePresets`, the choices every snooze menu
// lists. The presets and their limits are contracts settings.ts (SnoozePresetSetting, DEFAULT_SNOOZE_PRESETS,
// MAX_SNOOZE_PRESETS, MAX_SNOOZE_PRESET_MINUTES); the menu's resolution is client-runtime threadSettled.ts
// (snoozePresetId, resolveSnoozePreset, resolveSnoozePresets); the editor is SnoozePresetsSettings.tsx and its
// .logic.ts (sortSnoozeRows).
//
// The editor lists the saved presets soonest first, as the menu does. Each control writes the whole list with one
// preset changed, keyed by its index in the saved list, so a row keeps its identity while the order changes.
import { obj, type Json, type Obj } from '../client/domain';
import { ClientError } from '../client/protocol';
import { RUNTIME_LOCALE, timestampFormatter } from '../shared/timestamp-format';

export const SNOOZE_DAYS = ['today', 'tomorrow', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'] as const;
export type SnoozeDay = (typeof SNOOZE_DAYS)[number];
/** `after` wakes a fixed time from now. `at` wakes at a local time; a weekday is its next occurrence after today. */
export type SnoozePresetSetting = { kind: 'after'; minutes: number } | { kind: 'at'; day: SnoozeDay; time: string };
export const MAX_SNOOZE_PRESET_MINUTES = 30 * 24 * 60;
export const MAX_SNOOZE_PRESETS = 8;
export const DEFAULT_SNOOZE_PRESETS: readonly SnoozePresetSetting[] = [
  { kind: 'after', minutes: 60 },
  { kind: 'after', minutes: 180 },
  { kind: 'at', day: 'today', time: '18:00' },
  { kind: 'at', day: 'tomorrow', time: '09:00' },
  { kind: 'at', day: 'monday', time: '09:00' },
];
const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;
const isDay = (value: unknown): value is SnoozeDay => typeof value === 'string' && (SNOOZE_DAYS as readonly string[]).includes(value);

/** One stored preset, or null when it is not a SnoozePresetSetting. */
export function decodeSnoozePreset(value: unknown): SnoozePresetSetting | null {
  const entry = obj(value);
  if (entry.kind === 'after') {
    const minutes = entry.minutes;
    return typeof minutes === 'number' && Number.isInteger(minutes) && minutes >= 1 && minutes <= MAX_SNOOZE_PRESET_MINUTES ? { kind: 'after', minutes } : null;
  }
  if (entry.kind === 'at' && isDay(entry.day) && typeof entry.time === 'string' && TIME.test(entry.time)) return { kind: 'at', day: entry.day, time: entry.time };
  return null;
}
/** The environment's presets; a server that predates them (or an invalid list) reads as the defaults, as the schema decodes. */
export function snoozePresetsOf(settings: Obj): SnoozePresetSetting[] {
  if (!Array.isArray(settings.snoozePresets)) return [...DEFAULT_SNOOZE_PRESETS];
  const decoded = settings.snoozePresets.map(decodeSnoozePreset);
  return decoded.every(entry => entry !== null) && decoded.length <= MAX_SNOOZE_PRESETS ? decoded as SnoozePresetSetting[] : [...DEFAULT_SNOOZE_PRESETS];
}

// ── The menu (threadSettled.ts) ─────────────────────────────────────────────
export interface SnoozePresetChoice { id: string; label: string; whenLabel: string; snoozedUntil: string }
const WEEKDAY_INDEX: Record<string, number> = { sunday: 0, monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6 };
/** formatShortTimestamp under the Time format setting. */
export const snoozeTimeFormat = (format: string) => (date: Date) => timestampFormatter(format).format(date);
// Calendar-day advance instead of adding a day's milliseconds, which lands on the wrong local day across DST.
function addDays(base: Date, days: number): Date {
  const next = new Date(base.getTime());
  next.setDate(next.getDate() + days);
  return next;
}
const pluralize = (count: number, unit: string) => `${count} ${unit}${count === 1 ? '' : 's'}`;
/** "3 hours", "90 minutes", "2 days": the largest unit that divides evenly. */
function formatDuration(minutes: number): string {
  if (minutes % (24 * 60) === 0) return pluralize(minutes / (24 * 60), 'day');
  if (minutes % 60 === 0) return pluralize(minutes / 60, 'hour');
  return pluralize(minutes, 'minute');
}
const capitalize = (word: string) => word[0]!.toUpperCase() + word.slice(1);
function daysAhead(setting: Extract<SnoozePresetSetting, { kind: 'at' }>, now: Date): number {
  if (setting.day === 'today') return 0;
  if (setting.day === 'tomorrow') return 1;
  return (WEEKDAY_INDEX[setting.day]! - now.getDay() + 7) % 7 || 7;
}
/** Labels follow the actual day gap, so a Monday option on a Sunday reads "Tomorrow". */
function presetLabel(setting: SnoozePresetSetting, ahead: number): string {
  if (setting.kind === 'after') return `In ${formatDuration(setting.minutes)}`;
  if (ahead === 0) {
    const hour = Number(setting.time.slice(0, 2));
    return hour < 12 ? 'This morning' : hour < 17 ? 'This afternoon' : 'This evening';
  }
  if (ahead === 1) return 'Tomorrow';
  if (setting.day === 'monday') return 'Next week';
  return ahead === 7 ? `Next ${capitalize(setting.day)}` : capitalize(setting.day);
}
export function snoozePresetId(setting: SnoozePresetSetting): string {
  return setting.kind === 'after' ? `after:${setting.minutes}` : `at:${setting.day}:${setting.time}`;
}
/** A preset's wake from `now`, or null for a today option whose time has passed. */
export function resolveSnoozePreset(setting: SnoozePresetSetting, now: Date, formatTime: (date: Date) => string): SnoozePresetChoice | null {
  let wake: Date, ahead = 0;
  if (setting.kind === 'after') wake = new Date(now.getTime() + setting.minutes * 60_000);
  else {
    ahead = daysAhead(setting, now);
    wake = addDays(now, ahead);
    wake.setHours(Number(setting.time.slice(0, 2)), Number(setting.time.slice(3)), 0, 0);
    if (wake.getTime() <= now.getTime()) return null;
  }
  const label = presetLabel(setting, ahead);
  // Durations and "Next week" don't name the wake day, so a wake on another day shows it. The weekday is the runtime
  // default's (`toLocaleDateString(undefined, …)`), the time the Time format's.
  const labelNamesDay = setting.kind === 'at' && label !== 'Next week';
  const time = formatTime(wake);
  return { id: snoozePresetId(setting), label, snoozedUntil: wake.toISOString(),
    whenLabel: labelNamesDay || wake.toDateString() === now.toDateString() ? time : `${wake.toLocaleDateString(RUNTIME_LOCALE, { weekday: 'short' })} ${time}` };
}
/** The snooze menu: soonest first, one row per wake minute (a duration and a set time in one minute keep the first). */
export function resolveSnoozePresets(now: Date, presets: readonly SnoozePresetSetting[], formatTime: (date: Date) => string): SnoozePresetChoice[] {
  const wakeMinute = (preset: SnoozePresetChoice) => Math.floor(Date.parse(preset.snoozedUntil) / 60_000);
  const seen = new Set<number>();
  return presets.flatMap(setting => resolveSnoozePreset(setting, now, formatTime) ?? [])
    .sort((a, b) => wakeMinute(a) - wakeMinute(b))
    .filter(preset => { if (seen.has(wakeMinute(preset))) return false; seen.add(wakeMinute(preset)); return true; });
}

// ── The editor (SnoozePresetsSettings) ──────────────────────────────────────
/** sortSnoozeRows: the saved indexes in the menu's order, soonest wake first and options with no wake now last. Ties keep their order. */
export function sortSnoozeRows(presets: readonly SnoozePresetSetting[], now: Date): number[] {
  const wake = (setting: SnoozePresetSetting) => resolveSnoozePreset(setting, now, () => '')?.snoozedUntil ?? null;
  return presets.map((preset, index) => ({ index, wake: wake(preset) }))
    .sort((a, b) => a.wake === b.wake ? a.index - b.index : a.wake === null ? 1 : b.wake === null ? -1 : a.wake < b.wake ? -1 : 1)
    .map(entry => entry.index);
}
const UNIT_MINUTES = { minutes: 1, hours: 60, days: 24 * 60 } as const;
type Unit = keyof typeof UNIT_MINUTES;
const UNITS = Object.keys(UNIT_MINUTES) as Unit[];
const SINGULAR: Record<Unit, string> = { minutes: 'minute', hours: 'hour', days: 'day' };
export const WHEN_LABELS: Record<'after' | SnoozeDay, string> = { after: 'In', today: 'Today', tomorrow: 'Tomorrow', monday: 'Monday', tuesday: 'Tuesday',
  wednesday: 'Wednesday', thursday: 'Thursday', friday: 'Friday', saturday: 'Saturday', sunday: 'Sunday' };
// Offered in order when adding, skipping any already in the list.
const ADD_CANDIDATES: readonly SnoozePresetSetting[] = [...DEFAULT_SNOOZE_PRESETS, { kind: 'after', minutes: 30 }, { kind: 'at', day: 'saturday', time: '09:00' }, { kind: 'after', minutes: 24 * 60 }];

export function splitMinutes(minutes: number): { amount: number; unit: Unit } {
  if (minutes % UNIT_MINUTES.days === 0) return { amount: minutes / UNIT_MINUTES.days, unit: 'days' };
  if (minutes % UNIT_MINUTES.hours === 0) return { amount: minutes / UNIT_MINUTES.hours, unit: 'hours' };
  return { amount: minutes, unit: 'minutes' };
}
export function nextSnoozeCandidate(presets: readonly SnoozePresetSetting[]): SnoozePresetSetting {
  const used = new Set(presets.map(snoozePresetId));
  return ADD_CANDIDATES.find(candidate => !used.has(snoozePresetId(candidate))) ?? { kind: 'after', minutes: 60 };
}
/** Row order is only display order, so it does not make a list differ from the defaults. */
export function sameSnoozePresets(left: readonly SnoozePresetSetting[], right: readonly SnoozePresetSetting[]): boolean {
  const ids = (presets: readonly SnoozePresetSetting[]) => presets.map(snoozePresetId).sort().join('\n');
  return ids(left) === ids(right);
}

/**
 * The list one editor control writes. `part` names the control and the saved index it edits: `when:<i>` (In or a day),
 * `amount:<i>` (in the row's unit), `unit:<i>`, `time:<i>` (HH:MM), `remove:<i>`, or `add`.
 */
export function snoozePresetsEdit(current: readonly SnoozePresetSetting[], part: string, raw: string): Json {
  const [control = '', at = ''] = part.split(':');
  const list = [...current];
  if (control === 'add') {
    if (list.length >= MAX_SNOOZE_PRESETS) throw new ClientError(`Up to ${MAX_SNOOZE_PRESETS} options.`);
    return [...list, nextSnoozeCandidate(list)] as Json;
  }
  const index = Number(at), preset = list[index];
  if (!Number.isInteger(index) || !preset) throw new ClientError('That snooze option is no longer available.');
  if (control === 'remove') return list.filter((_, candidate) => candidate !== index) as Json;
  let next: SnoozePresetSetting;
  if (control === 'when') {
    // Switching between a duration and a set time starts the other kind at an hour, or at 09:00.
    if (raw === 'after') next = preset.kind === 'after' ? preset : { kind: 'after', minutes: 60 };
    else if (isDay(raw)) next = { kind: 'at', day: raw, time: preset.kind === 'at' ? preset.time : '09:00' };
    else throw new ClientError('Choose when the option wakes.');
  } else if (control === 'amount' || control === 'unit') {
    if (preset.kind !== 'after') throw new ClientError('That snooze option is a set time.');
    const { amount, unit } = splitMinutes(preset.minutes);
    if (control === 'unit') {
      if (!(UNITS as string[]).includes(raw)) throw new ClientError('Choose minutes, hours or days.');
      next = { kind: 'after', minutes: Math.min(amount * UNIT_MINUTES[raw as Unit], MAX_SNOOZE_PRESET_MINUTES) };
    } else {
      // NumberField: a blank or invalid entry keeps the amount; the rest round into 1 … the unit's maximum.
      const value = raw.trim() === '' ? NaN : Number(raw);
      if (!Number.isFinite(value)) return list as Json;
      const max = Math.floor(MAX_SNOOZE_PRESET_MINUTES / UNIT_MINUTES[unit]);
      next = { kind: 'after', minutes: Math.min(max, Math.max(1, Math.round(value))) * UNIT_MINUTES[unit] };
    }
  } else if (control === 'time') {
    if (preset.kind !== 'at') throw new ClientError('That snooze option is a duration.');
    const time = raw.trim();
    if (!TIME.test(time)) throw new ClientError('Enter a time as HH:MM, such as 09:00.');
    next = { ...preset, time };
  } else throw new ClientError('That snooze control is no longer available.');
  return list.map((entry, candidate) => candidate === index ? next : entry) as Json;
}

export type SnoozeEditorRow = {
  key: string; index: number; position: number; when: string; whenLabel: string; after: boolean; amount: string; amountDown: string; amountUp: string; unit: string; unitLabel: string;
  time: string; previewMenu: boolean; previewLabel: string; previewWhen: string; previewText: string;
  whens: { id: string; value: string; label: string; detail: string; icon: string; selected: boolean; disabled: boolean }[];
  units: { id: string; value: string; label: string; detail: string; icon: string; selected: boolean; disabled: boolean }[];
};
export type SnoozeEditor = { rows: SnoozeEditorRow[]; menuLine: string; full: boolean; fullNote: string };
const choice = (value: string, label: string, selected: boolean) => ({ id: value, value, label, detail: '', icon: '', selected, disabled: false });

/** SnoozePresetList: each row's fields and its preview (what the menu shows, or why it shows nothing), and the menu line. */
export function snoozeEditor(presets: readonly SnoozePresetSetting[], now: Date, timestampFormat: string): SnoozeEditor {
  const formatTime = snoozeTimeFormat(timestampFormat);
  const seenIds = new Set<string>(), seenWakes = new Set<string>();
  const rows = sortSnoozeRows(presets, now).map((index, position): SnoozeEditorRow => {
    const preset = presets[index]!, resolved = resolveSnoozePreset(preset, now, formatTime);
    let preview: Pick<SnoozeEditorRow, 'previewMenu' | 'previewLabel' | 'previewWhen' | 'previewText'>;
    if (resolved === null) preview = { previewMenu: false, previewLabel: '', previewWhen: '', previewText: 'Passed for today' };
    else if (seenIds.has(resolved.id)) preview = { previewMenu: false, previewLabel: '', previewWhen: '', previewText: 'Duplicate of an option above' };
    else if (seenWakes.has(resolved.snoozedUntil)) preview = { previewMenu: false, previewLabel: '', previewWhen: '', previewText: 'Same time as an option above' };
    else {
      seenIds.add(resolved.id);
      seenWakes.add(resolved.snoozedUntil);
      preview = { previewMenu: true, previewLabel: resolved.label, previewWhen: resolved.whenLabel, previewText: '' };
    }
    const when = preset.kind === 'after' ? 'after' : preset.day;
    const split = preset.kind === 'after' ? splitMinutes(preset.minutes) : { amount: 0, unit: 'minutes' as Unit };
    const labels = split.amount === 1 ? SINGULAR : { minutes: 'minutes', hours: 'hours', days: 'days' };
    // NumberField's steppers: the amount one down and one up within 1 … the unit's maximum ('' where a stepper is disabled).
    const max = Math.floor(MAX_SNOOZE_PRESET_MINUTES / UNIT_MINUTES[split.unit]);
    return { key: String(index), index, position: position + 1, when, whenLabel: WHEN_LABELS[when], after: preset.kind === 'after', amount: String(split.amount),
      amountDown: split.amount > 1 ? String(split.amount - 1) : '', amountUp: split.amount < max ? String(split.amount + 1) : '', unit: split.unit,
      unitLabel: labels[split.unit], time: preset.kind === 'at' ? preset.time : '', ...preview,
      whens: (['after', ...SNOOZE_DAYS] as const).map(value => choice(value, WHEN_LABELS[value], value === when)),
      units: UNITS.map(unit => choice(unit, labels[unit], unit === split.unit)) };
  });
  const menu = resolveSnoozePresets(now, presets, formatTime);
  const repeated = new Set(menu.map(preset => preset.label).filter((label, index, labels) => labels.indexOf(label) !== index));
  const order = menu.map(preset => repeated.has(preset.label) ? `${preset.label} ${preset.whenLabel}` : preset.label);
  return { rows, menuLine: order.length === 0 ? 'Menus offer only Custom…' : `Menu now: ${order.join(' · ')}`, full: presets.length >= MAX_SNOOZE_PRESETS, fullNote: `Up to ${MAX_SNOOZE_PRESETS} options` };
}

/** True when a stored value differs from the defaults by more than order (the row's reset). */
export const snoozePresetsChanged = (value: unknown) => !sameSnoozePresets(snoozePresetsOf({ snoozePresets: value as Json }), DEFAULT_SNOOZE_PRESETS);
