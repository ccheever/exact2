import { RUNTIME_LOCALE, timestampFormatter } from '../shared/timestamp-format'; // the host's locale

export interface SnoozePreset { id: string; label: string; wakeLabel: string; until: string }

/** formatShortTimestamp in the host's locale (timestamp-format.ts). */
function clockLabel(date: Date, format: string): string {
  return timestampFormatter(format).format(date);
}
function atHour(date: Date, hour: number, days = 0): Date {
  const result = new Date(date);
  result.setDate(result.getDate() + days);
  result.setHours(hour, 0, 0, 0);
  return result;
}

/**
 * client-runtime resolveSnoozePresets: an hour, three hours, this evening at
 * 18:00 while more than an hour remains, tomorrow at 9:00, and next Monday at
 * 9:00 unless that is tomorrow. Labels use the selected timestamp format.
 */
export function snoozePresets(now: number, format: string): SnoozePreset[] {
  if (!Number.isFinite(now) || now <= 0) return [];
  const base = new Date(now), hour = 3_600_000;
  const preset = (id: string, label: string, date: Date, wakeLabel = clockLabel(date, format)) =>
    ({ id, label, wakeLabel, until: date.toISOString() });
  const presets = [preset('hour', 'In 1 hour', new Date(now + hour)), preset('three-hours', 'In 3 hours', new Date(now + 3 * hour))];
  const evening = atHour(base, 18);
  if (evening.getTime() - now > hour) presets.push(preset('evening', 'This evening', evening));
  const tomorrow = atHour(base, 9, 1);
  presets.push(preset('tomorrow', 'Tomorrow', tomorrow));
  const nextWeek = atHour(base, 9, (1 - base.getDay() + 7) % 7 || 7);
  if (nextWeek.getTime() !== tomorrow.getTime()) {
    presets.push(preset('next-week', 'Next week', nextWeek,
      `${nextWeek.toLocaleDateString(RUNTIME_LOCALE, { weekday: 'short' })} ${clockLabel(nextWeek, format)}`));
  }
  return presets;
}
