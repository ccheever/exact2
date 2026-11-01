// Timestamps in the Mac's locale. Reference apps/web/src/timestampFormat.ts (1e2ecbd975,
// resolveHour12 at 74541bab17; MIT, see LICENSE-T3): resolveTimestampLocale, resolveWeekStartsOn,
// resolveHour12 and the cached wall-clock formatters. The desktop host reports the system locale
// (Electron's app.getSystemLocale() through toSystemLocaleTag, 2359643cff; here Contract's `exactTime().formatLocale`, the Region's
// format locale, which app.contract passes with the snapshot request). The packaged Electron app's runtime default locale is en-US
// (it ships only that Chromium locale), while the macOS data runtime's default follows the Mac's
// region (en-KR on a Korean Mac; exact2 docs/reference.md: pass the locale explicitly), so a call
// the reference leaves at the runtime default passes RUNTIME_LOCALE here.
export const RUNTIME_LOCALE = 'en-US';

/**
 * toSystemLocaleTag (apps/desktop ElectronApp.ts at v0.1.6): the macOS Region override, which the locale carries as
 * a `rg` keyword (`en_US@rg=gbzzzz`, or `en-US-u-rg-gbzzzz` as Contract's formatLocale has it), replaces the region
 * subtag, because Intl ignores `rg`. An hour cycle (`hc`) is kept; other keywords are dropped, as the reference drops
 * ICU keywords Intl cannot read, rather than losing the whole locale.
 */
export function toSystemLocaleTag(identifier: string, hourCycle?: 'h12' | 'h23'): string {
  const [icuBase = '', icuKeywords = ''] = identifier.split('@');
  const [bcpBase = '', extension = ''] = icuBase.replace(/_/g, '-').split(/-u-/i);
  const keywords = new Map<string, string>();
  for (const pair of icuKeywords.split(';')) { const [key, value] = pair.split('='); if (key && value) keywords.set(key.toLowerCase(), value.toLowerCase()); }
  // A -u- extension is key (two characters) and value subtags, up to the next singleton.
  let key = '';
  for (const subtag of extension.split('-')) {
    if (subtag.length === 1) break;
    if (subtag.length === 2) key = subtag.toLowerCase();
    else if (key && !keywords.has(key)) keywords.set(key, subtag.toLowerCase());
  }
  const region = /^([a-z]{2}|\d{3})zzzz$/i.exec(keywords.get('rg') ?? '')?.[1]?.toUpperCase();
  const cycle = hourCycle ?? (/^h(11|12|23|24)$/.test(keywords.get('hc') ?? keywords.get('hours') ?? '') ? keywords.get('hc') ?? keywords.get('hours') : undefined);
  const [language = '', ...rest] = bcpBase.split('-');
  const script = rest.find(subtag => /^[a-z]{4}$/i.test(subtag));
  const ownRegion = rest.find(subtag => /^([a-z]{2}|\d{3})$/i.test(subtag));
  const variants = rest.filter(subtag => subtag !== script && subtag !== ownRegion);
  const tag = [language, script, region ?? ownRegion, ...variants].filter(Boolean).join('-') + (cycle ? `-u-hc-${cycle}` : '');
  try {
    return Intl.getCanonicalLocales(tag)[0] ?? tag;
  } catch {
    return tag; // resolveTimestampLocale refuses it if it is not a well-formed tag
  }
}

/**
 * Pick the locale to format wall-clock times in, given the locale the host reports. Hosts that
 * report nothing fall back to `undefined`, the runtime default.
 */
export function resolveTimestampLocale(systemLocale: string | null | undefined): string | undefined {
  const reported = systemLocale?.trim();
  if (!reported) return undefined;
  const tag = toSystemLocaleTag(reported);
  try {
    // Throws on a structurally invalid tag; a well-formed tag without data is left to ICU's fallback.
    Intl.DateTimeFormat.supportedLocalesOf([tag]);
    return tag;
  } catch {
    return undefined;
  }
}

type WeekdayIndex = 0 | 1 | 2 | 3 | 4 | 5 | 6;
const WEEKDAY_INDEXES: readonly WeekdayIndex[] = [0, 1, 2, 3, 4, 5, 6];
type LocaleWithWeekInfo = { readonly weekInfo?: { readonly firstDay: number }; getWeekInfo?: () => { readonly firstDay: number } };

/**
 * First weekday of a locale as a `Date#getDay` index (0 is Sunday), or `undefined` when the runtime
 * has no week data, so callers keep their own default. The macOS data runtime has `Intl.Locale` and
 * `getWeekInfo()` with Chrome's values since exact2 #204 (#118).
 */
export function resolveWeekStartsOn(locale: string | undefined): WeekdayIndex | undefined {
  try {
    const IntlLocale = (Intl as unknown as { Locale: new (tag: string) => LocaleWithWeekInfo }).Locale;
    const resolved = new IntlLocale(locale ?? Intl.DateTimeFormat().resolvedOptions().locale);
    // Week info counts Monday as 1 and Sunday as 7.
    const firstDay = resolved.getWeekInfo?.().firstDay ?? resolved.weekInfo?.firstDay;
    return firstDay === undefined ? undefined : WEEKDAY_INDEXES[firstDay % 7];
  } catch {
    return undefined;
  }
}

// The host's locale, adopted from each status read; formatters are rebuilt when it changes.
let hostTag: string | null | undefined;
let timestampLocale: string | undefined;
const formatters = new Map<string, Intl.DateTimeFormat>();

/** `exactTime().formatLocale` (presentation.ts snapshot); a repeat of the same tag is free. */
export function adoptHostLocale(systemLocale: unknown): void {
  const tag = typeof systemLocale === 'string' ? systemLocale : null;
  if (tag === hostTag) return;
  hostTag = tag;
  timestampLocale = resolveTimestampLocale(tag);
  formatters.clear();
}
/** The locale timestamps are shown in: the host's, else the reference's runtime default. */
export function currentTimestampLocale(): string { return timestampLocale ?? RUNTIME_LOCALE; }
/** Week start for calendars, from the same locale timestamps are shown in. */
export function weekStartsOn(): WeekdayIndex | undefined { return resolveWeekStartsOn(timestampLocale); }

function formatter(key: string, options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const cacheKey = `${currentTimestampLocale()}|${key}`;
  let cached = formatters.get(cacheKey);
  if (!cached) { cached = new Intl.DateTimeFormat(currentTimestampLocale(), options); formatters.set(cacheKey, cached); }
  return cached;
}
/** getTimestampFormatter: hour and minute (and seconds), with hour12 unless the format is "locale". */
export function timestampFormatter(format: string, includeSeconds = false): Intl.DateTimeFormat {
  const options: Intl.DateTimeFormatOptions = { hour: 'numeric', minute: '2-digit', ...(includeSeconds ? { second: '2-digit' } : {}) };
  if (format !== 'locale') options.hour12 = format === '12-hour';
  return formatter(`${format}:${includeSeconds ? 's' : 'm'}`, options);
}
/** resolveHour12: whether the Time format setting shows a 12-hour clock ("locale" asks the locale). */
export function resolveHour12(format: string): boolean {
  if (format === '12-hour' || format === '24-hour') return format === '12-hour';
  const resolved = timestampFormatter(format).resolvedOptions().hour12;
  return typeof resolved === 'boolean' ? resolved : /[AP]M/i.test(timestampFormatter(format).format(new Date(2026, 0, 1, 15)));
}
/** numericDateFormatter / numericDateWithYearFormatter. */
export function numericDateFormatter(withYear: boolean): Intl.DateTimeFormat {
  return formatter(withYear ? 'date:y' : 'date', { month: 'numeric', day: 'numeric', ...(withYear ? { year: 'numeric' } : {}) });
}
/**
 * formatUpcomingTimestamp (timestampFormat.ts:197-222): an instant that has not happened yet (a
 * usage-limit reset): today `12:34 PM`, tomorrow `tomorrow at 12:34 PM`, later `8/13 12:34 PM`;
 * an instant already past reads as formatDayAwareTimestamp does (`yesterday at …`).
 */
export function formatUpcomingTimestamp(isoDate: string, format: string, nowMs: number): string {
  const date = new Date(isoDate);
  if (!Number.isFinite(date.getTime())) return '';
  const time = timestampFormatter(format).format(date);
  const now = new Date(nowMs);
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfTargetDay = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  // Round so DST-shifted 23/25 hour days still count as whole days.
  const dayDiff = Math.round((startOfTargetDay - startOfToday) / 86_400_000);
  if (dayDiff < 0) return dayDiff === -1 ? `yesterday at ${time}` : `${numericDateFormatter(date.getFullYear() !== now.getFullYear()).format(date)} ${time}`;
  if (dayDiff === 0) return time;
  if (dayDiff === 1) return `tomorrow at ${time}`;
  return `${numericDateFormatter(date.getFullYear() !== now.getFullYear()).format(date)} ${time}`;
}
