// The snooze menu's options: client-runtime state/threadSettled.ts (resolveSnoozePreset, resolveSnoozePresets,
// pickSnoozePreset) and contracts settings.ts (SnoozePresetSetting, DEFAULT_SNOOZE_PRESETS) at T3 Code 6d450a4284
// (MIT, LICENSE-T3). The options are the environment's `snoozePresets` setting; times use the selected timestamp format.
import { obj } from '../client/domain';
import { RUNTIME_LOCALE, timestampFormatter } from '../shared/timestamp-format'; // the host's locale

export interface SnoozePreset { id: string; label: string; wakeLabel: string; until: string }

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];
const DAYS = ['today', 'tomorrow', ...WEEKDAYS];
const MAX_SNOOZE_PRESET_MINUTES = 30 * 24 * 60;
const MAX_SNOOZE_PRESETS = 8;
/** `after` wakes a fixed time from now. `at` wakes at a local time; a weekday is its next occurrence after today. */
export type SnoozePresetSetting = { kind: 'after'; minutes: number } | { kind: 'at'; day: string; time: string };
export const DEFAULT_SNOOZE_PRESETS: readonly SnoozePresetSetting[] = [
  { kind: 'after', minutes: 60 },
  { kind: 'after', minutes: 180 },
  { kind: 'at', day: 'today', time: '18:00' },
  { kind: 'at', day: 'tomorrow', time: '09:00' },
  { kind: 'at', day: 'monday', time: '09:00' },
];

function decodeSetting(value: unknown): SnoozePresetSetting | null {
  const entry = obj(value);
  if (entry.kind === 'after' && Number.isInteger(entry.minutes) && Number(entry.minutes) >= 1 && Number(entry.minutes) <= MAX_SNOOZE_PRESET_MINUTES) {
    return { kind: 'after', minutes: Number(entry.minutes) };
  }
  if (entry.kind === 'at' && typeof entry.day === 'string' && DAYS.includes(entry.day) && typeof entry.time === 'string' && /^([01]\d|2[0-3]):[0-5]\d$/.test(entry.time)) {
    return { kind: 'at', day: entry.day, time: entry.time };
  }
  return null;
}
/** An environment's options (server settings `snoozePresets`); a server that sends none, or a list it would refuse, has the defaults. */
export function snoozePresetSettings(config: unknown): readonly SnoozePresetSetting[] {
  const list = obj(obj(config).settings).snoozePresets;
  if (!Array.isArray(list) || list.length > MAX_SNOOZE_PRESETS) return DEFAULT_SNOOZE_PRESETS;
  const settings = list.map(decodeSetting);
  return settings.every(setting => setting !== null) ? settings as SnoozePresetSetting[] : DEFAULT_SNOOZE_PRESETS;
}

/** formatShortTimestamp in the host's locale (timestamp-format.ts). */
function clockLabel(date: Date, format: string): string {
  return timestampFormatter(format).format(date);
}
const pluralize = (count: number, unit: string) => `${count} ${unit}${count === 1 ? '' : 's'}`;
/** "3 hours", "90 minutes", "2 days": the largest unit that divides evenly. */
function formatSnoozeDuration(minutes: number): string {
  if (minutes % (24 * 60) === 0) return pluralize(minutes / (24 * 60), 'day');
  if (minutes % 60 === 0) return pluralize(minutes / 60, 'hour');
  return pluralize(minutes, 'minute');
}
const capitalize = (word: string) => word[0]!.toUpperCase() + word.slice(1);
function daysAheadOf(setting: { day: string }, now: Date): number {
  if (setting.day === 'today') return 0;
  if (setting.day === 'tomorrow') return 1;
  return (WEEKDAYS.indexOf(setting.day) - now.getDay() + 7) % 7 || 7;
}
/** Labels follow the actual day gap, so a Monday option on a Sunday reads "Tomorrow". */
function presetLabel(setting: SnoozePresetSetting, daysAhead: number): string {
  if (setting.kind === 'after') return `In ${formatSnoozeDuration(setting.minutes)}`;
  if (daysAhead === 0) {
    const hour = Number(setting.time.slice(0, 2));
    return hour < 12 ? 'This morning' : hour < 17 ? 'This afternoon' : 'This evening';
  }
  if (daysAhead === 1) return 'Tomorrow';
  if (setting.day === 'monday') return 'Next week';
  return daysAhead === 7 ? `Next ${capitalize(setting.day)}` : capitalize(setting.day);
}
export function snoozePresetId(setting: SnoozePresetSetting): string {
  return setting.kind === 'after' ? `after:${setting.minutes}` : `at:${setting.day}:${setting.time}`;
}

/** One option's wake from `now`; null for a today option whose time has passed (it is left out until tomorrow). */
export function resolveSnoozePreset(setting: SnoozePresetSetting, now: Date, format: string): SnoozePreset | null {
  let wake: Date, daysAhead = 0;
  if (setting.kind === 'after') wake = new Date(now.getTime() + setting.minutes * 60_000);
  else {
    daysAhead = daysAheadOf(setting, now);
    // A calendar-day advance, not 24 hours: a fixed offset lands on the wrong local day across DST.
    wake = new Date(now);
    wake.setDate(wake.getDate() + daysAhead);
    wake.setHours(Number(setting.time.slice(0, 2)), Number(setting.time.slice(3)), 0, 0);
    if (wake.getTime() <= now.getTime()) return null;
  }
  const label = presetLabel(setting, daysAhead);
  // Durations and "Next week" don't name the wake day, so a wake on another day shows it.
  const labelNamesDay = setting.kind === 'at' && label !== 'Next week', time = clockLabel(wake, format);
  return { id: snoozePresetId(setting), label, until: wake.toISOString(),
    wakeLabel: labelNamesDay || wake.toDateString() === now.toDateString() ? time : `${wake.toLocaleDateString(RUNTIME_LOCALE, { weekday: 'short' })} ${time}` };
}

/** The snooze menu: soonest first, one row per wake minute (the first in settings order keeps a tie). */
export function snoozePresets(now: number, format: string, settings: readonly SnoozePresetSetting[] = DEFAULT_SNOOZE_PRESETS): SnoozePreset[] {
  if (!Number.isFinite(now) || now <= 0) return [];
  const minute = (preset: SnoozePreset) => Math.floor(Date.parse(preset.until) / 60_000), seen = new Set<number>();
  return settings.flatMap(setting => resolveSnoozePreset(setting, new Date(now), format) ?? [])
    .sort((left, right) => minute(left) - minute(right))
    .filter(preset => !seen.has(minute(preset)) && !!seen.add(minute(preset)));
}

/**
 * A menu choice as picked at `now`. A duration re-resolves from its setting, so it starts at selection; a set time
 * keeps its wake while that is still ahead. Undefined when the time has passed.
 */
export function pickSnoozePreset(id: string, now: number, format: string, settings: readonly SnoozePresetSetting[] = DEFAULT_SNOOZE_PRESETS): SnoozePreset | undefined {
  const setting = settings.find(candidate => snoozePresetId(candidate) === id);
  if (!setting || !Number.isFinite(now) || now <= 0) return undefined;
  return resolveSnoozePreset(setting, new Date(now), format) ?? undefined;
}
