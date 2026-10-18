import { expect, mock, test } from 'bun:test';
mock.module('./rt.js', () => ({ clock: {}, Clocked: [], effect() {}, onEnd() {}, drive() {} }));
mock.module('./navigation.js', () => ({ reportTime() {}, reportPlace() {} }));
const { formatted } = await import('./live-time.js');

test('date and time presentations use the viewer locale and IANA zone', () => {
  for (const locale of ['en-US', 'fr-FR', 'ja-JP']) for (const timeZone of ['UTC', 'America/Los_Angeles']) {
    for (const format of ['date-short', 'date-medium', 'date-long', 'date-full', 'time-short', 'time-medium']) {
      const [kind, style] = format.split('-');
      for (const instant of [-120000, 0, 1792209600000]) {
        expect(formatted(instant, 0, format, locale, timeZone)).toBe(new Intl.DateTimeFormat(locale, { timeZone, [kind + 'Style']: style }).format(instant));
      }
    }
  }
});

test('durations tick at seconds, cross an hour and stop a countdown at zero', () => {
  const show = (date, now, style) => formatted(date, now, style, 'en-US', 'UTC');
  expect(show(0, 999, 'timer')).toBe('0:00');
  expect(show(0, 1000, 'timer')).toBe('0:01');
  expect(show(0, 3731000, 'timer')).toBe('1:02:11');
  expect(show(3731000, 0, 'timer')).toBe('1:02:11');
  expect(show(1000, 1, 'countdown')).toBe('0:01');
  expect(show(1000, 1000, 'countdown')).toBe('0:00');
  expect(show(1000, 2000, 'countdown')).toBe('0:00');
  expect(show(NaN, 0, 'timer')).toBeNull();
});

test('relative time advances one minute in either direction', () => {
  const show = (date, now) => formatted(date, now, 'relative', 'en-US', 'UTC');
  expect(show(-120000, 0)).toBe('2 min. ago');
  expect(show(-120000, 59999)).toBe('2 min. ago');
  expect(show(-120000, 60000)).toBe('3 min. ago');
  expect(show(120000, 0)).toBe('in 2 min.');
  expect(show(120000, 30000)).toBe('in 2 min.');
  expect(show(120000, 60000)).toBe('in 1 min.');
});
