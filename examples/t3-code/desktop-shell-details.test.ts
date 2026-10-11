// Task desktop-shell-details: the reference's timestampFormat.test.ts locale cases, the theme
// import size guard (ThemeImportDialog.test.ts), ProjectFaviconPickerDialog.test.tsx's native
// picker cases, and the full-screen title-row inset (1e2ecbd975).
import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from './local-primary-fixture';
afterEach(resetPrimary);
import './client';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { adoptHostLocale, currentTimestampLocale, resolveTimestampLocale, resolveWeekStartsOn, weekStartsOn } from './timestamp-format';
import { messageTime, shortTime, timestampTooltip } from './timeline-presentation';
import { snoozePresets } from './sidebar-presentation';
import { describeOversizedThemeFile, MAX_THEME_FILE_BYTES, readThemeFiles } from './settings-appearance-import';
import { canPickExternalProjectFavicon, faviconPickLabel, localFileManagerName, pickProjectFavicon } from './desktop-shell-favicon';
import { toasts } from './toast';
import { sidebarMinimumWidth, workspaceControlsLeft } from './r12-sidebar-width';

const iso = (y: number, monthIndex: number, d: number, h: number, mi: number) => new Date(y, monthIndex, d, h, mi).toISOString();
const plain = (text: string) => text.replace(/[\u202f\u00a0]/g, ' ');
afterEach(() => adoptHostLocale(null));

describe('resolveTimestampLocale', () => {
  test('defers to the runtime default when the host reports no locale', () => {
    expect(resolveTimestampLocale(null)).toBeUndefined();
    expect(resolveTimestampLocale(undefined)).toBeUndefined();
    expect(resolveTimestampLocale('   ')).toBeUndefined();
  });
  test('uses a BCP-47 tag reported by the host', () => {
    expect(resolveTimestampLocale('en-GB')).toBe('en-GB');
  });
  test('defers to the runtime default rather than throwing on an unusable tag', () => {
    expect(resolveTimestampLocale('not a locale')).toBeUndefined();
    expect(resolveTimestampLocale('en_GB')).toBeUndefined();
  });
});

describe('formatShortTimestamp', () => {
  for (const [locale, localTime] of [['en-GB', '15:44'], ['en-US', '3:44 PM']] as const) {
    test(`honors ${locale} and the explicit hour-cycle settings`, () => {
      adoptHostLocale(locale);
      const date = new Date(2026, 3, 7, 15, 44).toISOString();
      expect(plain(shortTime(date, 'locale'))).toBe(localTime);
      expect(plain(shortTime(date, '12-hour'))).toMatch(/^3:44 [ap]m$/i);
      expect(shortTime(date, '24-hour')).toBe('15:44');
    });
  }
  test('a Mac set to Korea shows Korean times; Germany 24-hour ones', () => {
    const date = new Date(2026, 3, 7, 14, 20).toISOString();
    adoptHostLocale('ko-KR');
    expect(plain(shortTime(date, 'locale'))).toBe('오후 2:20');
    adoptHostLocale('de-DE');
    expect(shortTime(date, 'locale')).toBe('14:20');
  });
  test('without a host locale the clone formats as the packaged app does (en-US)', () => {
    adoptHostLocale(null);
    expect(currentTimestampLocale()).toBe('en-US');
    expect(plain(shortTime(new Date(2026, 3, 7, 15, 44).toISOString(), 'locale'))).toBe('3:44 PM');
  });
});

describe('resolveWeekStartsOn', () => {
  // Bun and the macOS data runtime (Hermes, exact2 #204 for #118) both have Intl.Locale week data, with Chrome's values.
  for (const [locale, weekday] of [['en-US', 0], ['en-GB', 1], ['pl-PL', 1], ['ar-EG', 6], ['de-DE', 1], ['ko-KR', 0], ['fa-IR', 6]] as const) {
    test(`starts the ${locale} week on weekday ${weekday}`, () => {
      expect(resolveWeekStartsOn(locale)).toBe(weekday);
    });
  }
  test('leaves the default to the caller for a malformed locale', () => {
    expect(resolveWeekStartsOn('not a locale')).toBeUndefined();
  });
  test('follows the locale the desktop host reports', () => {
    adoptHostLocale('en-GB');
    expect(weekStartsOn()).toBe(1);
  });
});

describe('formatDayAwareTimestamp', () => {
  const now = new Date(2026, 7, 14, 12, 0).getTime();
  test('uses the host locale for both the numeric date and wall-clock time', () => {
    adoptHostLocale('en-GB');
    expect(messageTime(iso(2026, 7, 12, 15, 44), now, 'locale')).toBe('12/08 15:44');
  });
  test('labels yesterday and adds the year once it differs, in the host locale', () => {
    adoptHostLocale('en-US');
    expect(plain(messageTime(iso(2026, 7, 13, 23, 30), new Date(2026, 7, 14, 0, 30).getTime(), '12-hour'))).toBe('yesterday at 11:30 PM');
    expect(plain(messageTime(iso(2025, 11, 31, 18, 0), now, '12-hour'))).toBe('12/31/2025 6:00 PM');
  });
  test('keeps the English date label of the tooltip in another locale', () => {
    adoptHostLocale('de-DE');
    expect(timestampTooltip(iso(2026, 5, 4, 14, 4), '24-hour')).toBe('14:04, 4th June 2026');
  });
  test('snooze times follow the host locale; the weekday stays the runtime default', () => {
    adoptHostLocale('de-DE');
    const presets = snoozePresets(new Date(2026, 9, 6, 10, 0).getTime(), 'locale');
    expect(presets.find(preset => preset.id === 'tomorrow')?.wakeLabel).toBe('9:00');
    expect(presets.find(preset => preset.id === 'next-week')?.wakeLabel).toBe('Mon 9:00');
  });
});

describe('theme import size guard', () => {
  test('accepts anything a theme file could plausibly be', () => {
    for (const bytes of [0, 4_096, MAX_THEME_FILE_BYTES]) expect(describeOversizedThemeFile(bytes)).toBeNull();
  });
  test('rejects a file too large to be a theme and names its size', () => {
    const message = describeOversizedThemeFile(100 * 1024 * 1024);
    expect(message).toContain('100.0 MB');
    expect(message).toContain('256 KB');
  });
  test('reports sizes just past the limit in KB', () => {
    expect(describeOversizedThemeFile(MAX_THEME_FILE_BYTES + 1)).toContain('256 KB');
  });
  test('one oversized file gives the reference message; one readable file fills the editor', () => {
    expect(() => readThemeFiles([{ name: 'big.json', size: 300 * 1024, text: '' }], [])).toThrow(
      'That file is 300 KB. Theme files are only a few KB, so this one was not read (limit 256 KB).');
    expect(readThemeFiles([{ name: 'a.json', size: 12, text: '{"x":1}' }], [])).toEqual({ kind: 'single', json: '{"x":1}', fileName: 'a.json' });
  });
  test('a batch imports the good files and lists the rest as "<file>: too large" / "<file>: <reason>"', () => {
    const good = JSON.stringify({ name: 'Night Owl', type: 'dark', colors: { 'editor.background': '#011627', 'editor.foreground': '#d6deeb' }, tokenColors: [] });
    const result = readThemeFiles([{ name: 'owl.json', size: good.length, text: good }, { name: 'big.json', size: 300 * 1024, text: '' },
      { name: 'locked.json', size: 0, text: '' }], []);
    expect(result.kind).toBe('batch');
    if (result.kind !== 'batch') return;
    expect(result.themes.map(theme => theme.label)).toEqual(['Night Owl']);
    expect(result.failures).toEqual(['big.json: too large', 'locked.json: Theme files must contain a JSON object.']);
    expect(result.failures.join(' — ')).toBe('big.json: too large — locked.json: Theme files must contain a JSON object.');
  });
});

describe('ProjectFaviconPickerDialog', () => {
  const client = (reply: () => Promise<unknown>) => ({ restAccess: () => ({ call: reply }) }) as unknown as T3Client;
  const native = { available: true } as Native;
  test('selects an image from the native file picker', async () => {
    const calls: unknown[] = [];
    const c = { restAccess: () => ({ call: async (request: unknown) => { calls.push(request); return { path: '/Users/me/Pictures/icon.png' }; } }) } as unknown as T3Client;
    expect(await pickProjectFavicon(c, native, 'cwd=%2FUsers%2Fme%2Fproject')).toBe('path=%2FUsers%2Fme%2FPictures%2Ficon.png');
    expect(calls).toEqual([{ op: 'pickProjectFavicon', path: '/Users/me/project' }]);
  });
  test('hides the native picker for WSL project paths', () => {
    expect(canPickExternalProjectFavicon('/home/me/project', 'Win32')).toBe(false);
    expect(canPickExternalProjectFavicon('C:\\Users\\me\\project', 'Win32')).toBe(true);
    expect(localFileManagerName('MacIntel')).toBe('Finder');
  });
  test('keeps the dialog open when the native picker fails', async () => {
    const c = client(async () => { throw new Error('picker failed'); });
    await expect(pickProjectFavicon(c, native, 'cwd=%2Fp')).rejects.toThrow('toasted:');
    expect(toasts(c).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not open image picker', 'picker failed']]);
  });
  test('a cancelled panel keeps the dialog open without a toast', async () => {
    const c = client(async () => ({ path: '' }));
    await expect(pickProjectFavicon(c, native, 'cwd=%2Fp')).rejects.toThrow('toasted:');
    expect(toasts(c)).toEqual([]);
  });
  test('offered only for the primary environment (this Mac\'s embedded server)', () => {
    primaryAt('http://127.0.0.1:16090', 'env');
    expect(faviconPickLabel('http://127.0.0.1:16090', [{ workspaceRoot: '/Users/me/project' }])).toBe('Open in Finder');
    expect(faviconPickLabel('https://box.example.com', [{ workspaceRoot: '/srv/project' }])).toBe('');
    expect(faviconPickLabel('http://127.0.0.1:16090', [])).toBe('');
  });
});

describe('full-screen title row', () => {
  test('the traffic lights inset goes in full screen and comes back after', () => {
    expect(workspaceControlsLeft(16)).toBe(90);
    expect(workspaceControlsLeft(16, true)).toBe(12);
    expect(workspaceControlsLeft(20, true)).toBe(15);
    expect(workspaceControlsLeft(16, false)).toBe(90);
  });
  test('the sidebar minimum follows the inset (brand probe) in both states', () => {
    expect([12, 16, 20].map(size => sidebarMinimumWidth(size))).toEqual([208, 208, 223]);
    expect([12, 16, 20].map(size => sidebarMinimumWidth(size, true))).toEqual([208, 208, 208]);
  });
});
