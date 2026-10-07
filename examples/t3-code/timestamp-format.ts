// Timestamps in the Mac's locale (task desktop-shell-details). Reference apps/web/src/timestampFormat.ts
// (1e2ecbd975; MIT, see LICENSE-T3): resolveTimestampLocale, resolveWeekStartsOn and the cached
// wall-clock formatters. The desktop host reports the system locale (Electron's
// app.getSystemLocale() with `_` → `-`; here the native module's `systemLocale`, T3Locale.swift,
// read with the status presentation). The packaged Electron app's runtime default locale is en-US
// (it ships only that Chromium locale), while the macOS data runtime's default follows the Mac's
// region (en-KR on a Korean Mac; exact2 docs/reference.md: pass the locale explicitly), so a call
// the reference leaves at the runtime default passes RUNTIME_LOCALE here.
export const RUNTIME_LOCALE = 'en-US';

/**
 * Pick the locale to format wall-clock times in, given the locale the host reports. Hosts that
 * report nothing fall back to `undefined`, the runtime default.
 */
export function resolveTimestampLocale(systemLocale: string | null | undefined): string | undefined {
  const tag = systemLocale?.trim();
  if (!tag) return undefined;
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

/** The status presentation's `systemLocale` (T3Locale.swift); a repeat of the same tag is free. */
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
/** numericDateFormatter / numericDateWithYearFormatter. */
export function numericDateFormatter(withYear: boolean): Intl.DateTimeFormat {
  return formatter(withYear ? 'date:y' : 'date', { month: 'numeric', day: 'numeric', ...(withYear ? { year: 'numeric' } : {}) });
}
