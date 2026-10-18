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
  expect(show(0, 999, 'timer')).toBe('00:00');
  expect(show(0, 1000, 'timer')).toBe('00:01');
  expect(show(0, 3731000, 'timer')).toBe('1:02:11');
  expect(show(3731000, 0, 'timer')).toBe('1:02:11');
  expect(show(1000, 1, 'countdown')).toBe('00:01');
  expect(show(1000, 1000, 'countdown')).toBe('00:00');
  expect(show(1000, 2000, 'countdown')).toBe('00:00');
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

test('relative zero is named now at the instant and thirty seconds later', () => {
  expect(formatted(0, 0, 'relative', 'en-US', 'UTC')).toBe('now');
  expect(formatted(0, 30000, 'relative', 'en-US', 'UTC')).toBe('now');
  expect(formatted(0, 60000, 'relative', 'en-US', 'UTC')).toBe('1 min. ago');
});

test('old browsers show durations and their reservation uses only the displayed fields', async () => {
  const { durationReservation, relativeDelay, relativeReservation } = await import('./live-time.js');
  const native = Intl.DurationFormat;
  try {
    Intl.DurationFormat = undefined;
    expect(formatted(0, 2000, 'timer', 'en-US', 'UTC')).toBe('00:02');
    expect(formatted(0, 3731000, 'timer', 'en-US', 'UTC')).toBe('1:02:11');
    expect(formatted(0, 2000, 'countdown', 'en-US', 'UTC')).toBe('00:00');
    const values = durationReservation('00:02', 'en-US', 'UTC');
    expect(values).toHaveLength(11);
    expect(values.every(s => s.length === 5)).toBe(true);
  } finally { Intl.DurationFormat = native; }
  for (const locale of ['en-US', 'fr-FR', 'ar-EG']) {
    for (const sign of [-1, 1]) {
      const text = relativeReservation(locale, sign);
      expect(text).toBe(sign < 0 ? '59 min, 59 sec ago' : 'in 59 min, 59 sec');
    }
    const model = formatted(2000, 0, 'countdown', locale, 'UTC');
    const values = durationReservation(model, locale, 'UTC');
    expect(values.every(s => [...s].length === [...model].length)).toBe(true);
  }
  expect(relativeDelay(-259200000, 0)).toBe(43200000);
  expect(relativeDelay(0, 30000)).toBe(30000);
});

test('reservation spans measure the element font longhands, numeric features and transforms', async () => {
  const { chromium } = await import('playwright-core');
  const { chromium: browserPath } = await import('../../scripts/agent-launch.mjs');
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/live-time.js') return new Response(Bun.file(`${import.meta.dir}/live-time.js`), { headers: { 'content-type': 'text/javascript' } });
    if (path === '/rt.js') return new Response('export const clock={now:0,timers:[],agent:true},Clocked=[]; export function effect(fn){fn()} export function onEnd(){} export function drive(){}', { headers: { 'content-type': 'text/javascript' } });
    if (path === '/navigation.js') return new Response('export function reportTime(){return [0]} export function reportPlace(){return "en-US\\0UTC"}', { headers: { 'content-type': 'text/javascript' } });
    return new Response('<body style="font-size:16px"></body>', { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    const results = await page.evaluate(async () => {
      const { measureReservation } = await import('/live-time.js');
      const out = [];
      for (const mode of ['horizontal-tb', 'vertical-rl']) for (const text of ['88:88', 'now']) {
        const e = document.createElement('time');
        Object.assign(e.style, { display: 'inline-block', fontFamily: 'sans-serif', fontSize: '28px', fontVariantNumeric: 'tabular-nums', textTransform: 'uppercase', writingMode: mode });
        e.textContent = text; document.body.append(e);
        const original = Element.prototype.getBoundingClientRect;
        let measuredFont;
        Element.prototype.getBoundingClientRect = function () {
          if (this.style.visibility === 'hidden') { const s = getComputedStyle(this); measuredFont = [s.fontSize, s.fontVariantNumeric, s.textTransform, s.writingMode]; }
          return original.call(this);
        };
        const measured = measureReservation(e, [text]);
        Element.prototype.getBoundingClientRect = original;
        const rect = e.getBoundingClientRect();
        out.push({ measuredFont, measured, actual: mode === 'vertical-rl' ? rect.height : rect.width });
        e.remove();
      }
      return out;
    });
    const ticking = await page.evaluate(async () => {
      const { liveTime } = await import('/live-time.js'), { clock, Clocked } = await import('/rt.js');
      const timer = document.createElement('time'), countdown = document.createElement('time');
      for (const e of [timer, countdown]) { e.style.cssText = 'font-size:28px; font-variant-numeric:tabular-nums; display:inline-block'; document.body.append(e); }
      liveTime(timer, () => [0, 'timer', null, null]);
      liveTime(countdown, () => [7200000, 'countdown', null, null]);
      const read = () => [timer.style.minInlineSize, countdown.style.minInlineSize];
      const frames = [read()];
      for (const now of [2000, 3599000, 3600000, 36000000]) { clock.now = now; for (const moved of Clocked) moved(); frames.push(read()); }
      return frames.map(pair => pair.map(parseFloat));
    });
    expect(ticking[1][0]).toBe(ticking[0][0]);
    expect(ticking[2][0]).toBe(ticking[0][0]);
    expect(ticking[3][0]).toBeGreaterThan(ticking[2][0]);
    expect(ticking[4][0]).toBeGreaterThan(ticking[3][0]);
    expect(ticking.every(pair => pair[1] === ticking[0][1])).toBe(true);
    for (const r of results) {
      expect(r.measuredFont[0]).toBe('28px');
      expect(r.measuredFont[1]).toBe('tabular-nums');
      expect(r.measuredFont[2]).toBe('uppercase');
      expect(r.measured).toBeCloseTo(r.actual, 2);
    }
  } finally { await browser?.close(); server.stop(true); }
}, 30000);
