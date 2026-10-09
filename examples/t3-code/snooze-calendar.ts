// Custom snooze's date (shell-sidebar-palette-keys TH-8): CustomSnoozeDialog.tsx's outline date button
// and the Calendar popover it opens (components/ui/calendar.tsx over DayPicker; MIT reference, see
// LICENSE-T3). DayPicker with its defaults: the en-US caption ("October 2026"), two-letter weekdays,
// outside days shown, the weeks the month spans, past days disabled (`before` today's midnight), the
// week starting on the locale's first day (timestampFormat.ts weekStartsOn), one day in the Tab order
// (the focused, else the selected, else today, else the month's first enabled day), and its keys:
// ← → a day, ↑ ↓ a week, Home and End the week's ends, Page Up and Page Down a month (with Shift a
// year), a disabled day skipped in the same direction, the month following the focus.
import type { SidebarSession } from './sidebar-state';
import { RUNTIME_LOCALE, weekStartsOn } from './timestamp-format';

export type CalendarDay = { id: string; label: string; aria: string; outside: boolean; disabled: boolean; selected: boolean; today: boolean; stop: boolean };
export type CalendarWeek = { id: string; days: CalendarDay[] };
export type CalendarWeekday = { id: string; short: string; name: string };
export type SnoozeCalendar = { dateLabel: string; title: string; weekdays: CalendarWeekday[]; weeks: CalendarWeek[]; focus: string; focusSeq: number };

const SHORT = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];
const NAMES = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const pad = (value: number, width = 2) => String(value).padStart(width, '0');
/** A local calendar date as `YYYY-MM-DD` (localSnoozeDate). */
export const isoDay = (date: Date) => `${pad(date.getFullYear(), 4)}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
/** `YYYY-MM-DD` as local midnight; null when it is not a calendar date. */
export function parseDay(day: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!match) return null;
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return isoDay(date) === day ? date : null;
}
const monthOf = (day: string) => day.slice(0, 7);
const addDays = (date: Date, days: number) => new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
/** date-fns addMonths: the same day, clamped to the target month's last. */
function addMonths(date: Date, months: number): Date {
  const first = new Date(date.getFullYear(), date.getMonth() + months, 1);
  const last = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
  return new Date(first.getFullYear(), first.getMonth(), Math.min(date.getDate(), last));
}
const startOfWeek = (date: Date, firstDay: number) => addDays(date, -((date.getDay() - firstDay + 7) % 7));
const MONTH = new Intl.DateTimeFormat('en-US', { month: 'long' }), CAPTION = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' });
const ordinal = (n: number) => `${n}${n % 100 >= 11 && n % 100 <= 13 ? 'th' : ['th', 'st', 'nd', 'rd'][n % 10] ?? 'th'}`;
/** DayPicker's first weekday: the locale's, else its en-US default (Sunday). */
export const firstWeekday = (): number => weekStartsOn() ?? 0;

/** The trigger's text: `date.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })`. */
export function dateLabel(day: string): string {
  const date = parseDay(day);
  return date ? new Intl.DateTimeFormat(RUNTIME_LOCALE, { month: 'short', day: 'numeric', year: 'numeric' }).format(date) : day;
}

/** No calendar: the snapshot's value while Custom snooze is closed. */
export const NO_CALENDAR: SnoozeCalendar = { dateLabel: '', title: '', weekdays: [], weeks: [], focus: '', focusSeq: 0 };
/** The month `month` (`YYYY-MM`) as DayPicker draws it around the `selected` day, `today` being `YYYY-MM-DD`. */
export function snoozeCalendar(selected: string, month: string, today: string, firstDay: number, focus = '', focusSeq = 0): SnoozeCalendar {
  const first = parseDay(`${month}-01`) ?? parseDay(`${monthOf(today)}-01`) ?? new Date(2000, 0, 1);
  const last = new Date(first.getFullYear(), first.getMonth() + 1, 0);
  const shown = isoDay(first).slice(0, 7);
  const weeks: CalendarWeek[] = [];
  for (let start = startOfWeek(first, firstDay); start <= last; start = addDays(start, 7)) {
    const days = Array.from({ length: 7 }, (_, index) => {
      const date = addDays(start, index), id = isoDay(date);
      const isToday = id === today, isSelected = id === selected;
      const aria = `${isToday ? 'Today, ' : ''}${NAMES[date.getDay()]}, ${MONTH.format(date)} ${ordinal(date.getDate())}, ${date.getFullYear()}${isSelected ? ', selected' : ''}`;
      return { id, label: String(date.getDate()), aria, outside: monthOf(id) !== shown, disabled: id < today, selected: isSelected, today: isToday, stop: false };
    });
    weeks.push({ id: isoDay(start), days });
  }
  const inMonth = weeks.flatMap(week => week.days).filter(day => !day.outside && !day.disabled);
  const stop = [focus, selected, today].find(id => inMonth.some(day => day.id === id)) ?? inMonth[0]?.id ?? '';
  for (const day of weeks.flatMap(week => week.days)) day.stop = day.id === stop;
  return {
    dateLabel: dateLabel(selected),
    title: CAPTION.format(first),
    weekdays: Array.from({ length: 7 }, (_, index) => { const day = (firstDay + index) % 7; return { id: String(day), short: SHORT[day]!, name: NAMES[day]! }; }),
    weeks, focus, focusSeq,
  };
}

/** DayPicker's getNextFocus: where `key` moves the focus from `day`, skipping disabled days; '' for nowhere. */
export function calendarMove(day: string, key: string, today: string, firstDay: number): string {
  const step = (date: Date): Date | null => {
    switch (key) {
      case 'ArrowLeft': return addDays(date, -1);
      case 'ArrowRight': return addDays(date, 1);
      case 'ArrowUp': return addDays(date, -7);
      case 'ArrowDown': return addDays(date, 7);
      case 'Home': return startOfWeek(date, firstDay);
      case 'End': return addDays(startOfWeek(date, firstDay), 6);
      case 'PageUp': return addMonths(date, -1);
      case 'PageDown': return addMonths(date, 1);
      case 'Shift+PageUp': return addMonths(date, -12);
      case 'Shift+PageDown': return addMonths(date, 12);
      default: return null;
    }
  };
  let date = parseDay(day);
  for (let attempt = 0; date && attempt <= 365; attempt++) {
    date = step(date);
    if (date && isoDay(date) >= today) return isoDay(date);
  }
  return '';
}

/** The calendar's local ops (sidebar-commands.ts sidebarLocal), `today` being `YYYY-MM-DD`. */
export function calendarLocal(session: SidebarSession, op: string, id: string, value: string, today: string): void {
  // The popover mounts its DayPicker afresh: the selected day's month, nothing focused yet.
  if (op === 'calendar-open') { session.calendarMonth = monthOf(session.dialogDate || today); session.calendarFocus = ''; }
  else if (op === 'calendar-month') {
    const month = parseDay(`${session.calendarMonth || monthOf(today)}-01`);
    if (month) session.calendarMonth = monthOf(isoDay(addMonths(month, value === 'previous' ? -1 : 1)));
  } else if (op === 'calendar-key') {
    const target = calendarMove(id, value, today, firstWeekday());
    if (target) { session.calendarMonth = monthOf(target); session.calendarFocus = target; session.calendarFocusSeq++; }
  }
}
