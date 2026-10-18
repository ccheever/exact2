// HTML instants on the runtime clock; ticks run no compiled app expression.
import { clock, Clocked, effect, onEnd, drive } from './rt.js';
import { reportTime, reportPlace } from './navigation.js';

const valid = n => typeof n === 'number' && Number.isFinite(n) && n >= -62135596800000 && n <= 253402300799999;
const seconds = (date, now, format) => format === 'countdown' ? Math.ceil(Math.max(0, date - now) / 1000) : Math.floor(Math.abs(date - now) / 1000);
const shape = s => s < 3600 ? 0 : String(Math.floor(s / 3600)).length;
export function relative(delta) {
  const round = n => Math.sign(n) * Math.round(Math.abs(n));
  const minutes = Math.ceil(delta / 60000);
  const [scale, unit] = Math.abs(minutes) < 60 ? [1, 'minute'] : Math.abs(minutes) < 1440 ? [60, 'hour'] : Math.abs(minutes) < 43200 ? [1440, 'day'] : Math.abs(minutes) < 525600 ? [43200, 'month'] : [525600, 'year'];
  return [round(minutes / scale) || 0, unit];
}
export function relativeDelay(datetime, now) {
  const minutes = Math.ceil((datetime - now) / 60000), current = relative(datetime - now).join('/');
  let lo = 1, hi = 525600;
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2);
    if (relative((minutes - mid) * 60000).join('/') === current) lo = mid + 1;
    else hi = mid;
  }
  return 60000 - ((now - datetime) % 60000 + 60000) % 60000 + (lo - 1) * 60000;
}
const formatters = new Map();
function intl(kind, locale, options) {
  const key = JSON.stringify([kind, locale, options]);
  if (!formatters.has(key)) formatters.set(key, new Intl[kind](locale, options));
  return formatters.get(key);
}
export function formatted(datetime, now, format, locale, timeZone) {
  if (!valid(datetime)) return null;
  if (format === 'relative') {
    const [n, unit] = relative(datetime - now);
    return intl('RelativeTimeFormat', locale, { style: 'short', numeric: n === 0 ? 'auto' : 'always' }).format(n, n === 0 ? 'second' : unit);
  }
  if (format === 'timer' || format === 'countdown') {
    const s = seconds(datetime, now, format), hours = Math.floor(s / 3600), minutes = Math.floor(s / 60) % 60;
    if (typeof Intl.DurationFormat === 'function') return intl('DurationFormat', locale, { style: 'digital', hoursDisplay: 'auto', minutesDisplay: 'always', secondsDisplay: 'always' }).format({ hours, minutes, seconds: s % 60 });
    const pad = n => String(n).padStart(2, '0');
    return `${hours ? hours + ':' : ''}${pad(minutes)}:${pad(s % 60)}`;
  }
  const [kind, style] = format.split('-');
  if (!['date', 'time'].includes(kind) || !['short', 'medium', 'long', 'full'].includes(style)) return null;
  return intl('DateTimeFormat', locale, { timeZone, [kind + 'Style']: style }).format(datetime);
}
export function relativeReservation(locale, sign) {
  if (typeof Intl.DurationFormat === 'function') {
    const phrase = intl('DurationFormat', locale, { style: 'short' }).format({ minutes: 59, seconds: 59 });
    const unit = intl('NumberFormat', locale, { style: 'unit', unit: 'minute', unitDisplay: 'long' }).format(59);
    const wrapper = intl('RelativeTimeFormat', locale, { style: 'long', numeric: 'always' }).format(sign * 59, 'minute');
    if (wrapper.includes(unit)) return wrapper.replace(unit, phrase);
  }
  return sign < 0 ? '59 min, 59 sec ago' : 'in 59 min, 59 sec';
}
export function durationReservation(model, locale, timeZone, fields) {
  if (fields != null) model = formatted(0, 3600000 * 10 ** fields, 'timer', locale, timeZone);
  const digits = Array.from({ length: 10 }, (_, i) => [...formatted(0, i * 1000, 'timer', locale, timeZone)].reverse().find(c => /\p{Number}/u.test(c)));
  return [...digits.map(digit => [...model].map(c => digits.includes(c) ? digit : c).join('')), model];
}
const fontKeys = ['fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'fontStretch', 'lineHeight', 'fontSizeAdjust', 'fontVariantCaps', 'fontVariantLigatures', 'fontVariantPosition', 'fontVariantNumeric', 'fontVariantEastAsian', 'fontVariantAlternates', 'fontVariantEmoji', 'fontFeatureSettings', 'fontVariationSettings', 'letterSpacing', 'wordSpacing', 'fontKerning', 'fontOpticalSizing', 'textTransform', 'writingMode', 'textOrientation'];
export function measureReservation(e, strings, font = getComputedStyle(e)) {
  const measure = document.createElement('span');
  Object.assign(measure.style, { position: 'fixed', visibility: 'hidden', pointerEvents: 'none', whiteSpace: 'pre', inlineSize: 'max-content', blockSize: 'max-content' });
  for (const key of fontKeys) if (font[key]) measure.style[key] = font[key];
  for (const text of strings) { const line = document.createElement('span'); line.style.display = 'block'; line.textContent = text; measure.append(line); }
  document.body.append(measure);
  const rect = measure.getBoundingClientRect(), width = /^(vertical|sideways)/.test(font.writingMode) ? rect.height : rect.width;
  measure.remove();
  return width;
}

export function liveTime(e, read, reserve = true) {
  let values, previous = null, reserved = '', identity = '', model = '', fields = 0;
  const fallback = [...e.childNodes], timer = { due: Infinity, hatch: true, action: () => { update(); return true; } };
  clock.timers.push(timer);
  function update() {
    const [datetime, name, start, end] = values;
    const epoch = reportTime(clock.now)[0], now = epoch + clock.now;
    const [locale, timeZone] = reportPlace().split('\0');
    timer.due = Infinity;
    if (e.localName === 'progress') {
      if (valid(start) && valid(end) && end > start) {
        e.max = 1; e.value = Math.max(0, Math.min(1, (now - start) / (end - start)));
        if (now < end) timer.due = clock.now + Math.min(now < start ? start - now : 1000 - ((now - start) % 1000 + 1000) % 1000, end - now);
      } else e.removeAttribute('value');
      return;
    }
    const format = name ?? 'date-medium', text = formatted(datetime, now, format, locale, timeZone);
    if (text == null) {
      if (reserve) e.style.minInlineSize = '';
      reserved = ''; identity = '';
      if (previous != null) e.replaceChildren(...fallback);
      previous = null; return;
    }
    e.setAttribute('datetime', new Date(datetime).toISOString());
    if (reserve) {
      const key = `${datetime}/${format}/${locale}/${timeZone}`;
      const currentFields = shape(seconds(datetime, now, format));
      if (key !== identity || (format === 'timer' && currentFields > fields)) {
        identity = key; fields = currentFields; model = text; reserved = '';
      }
      const font = getComputedStyle(e), stamp = `${identity}/${fields}/${fontKeys.map(k => font[k]).join('/')}`;
      if (stamp !== reserved) {
        let strings = [text];
        if (format === 'relative') {
          strings.push(relativeReservation(locale, -1), relativeReservation(locale, 1));
          for (const minutes of [59, 23 * 60, 29 * 1440, 11 * 43200, 9998 * 525600]) for (const sign of [-1, 1]) strings.push(formatted(0, sign * minutes * 60000, format, locale, timeZone));
        } else if (format === 'timer' || format === 'countdown') strings = durationReservation(model, locale, timeZone, format === 'timer' ? fields : undefined);
        e.style.minInlineSize = `${Math.ceil(measureReservation(e, strings, font))}px`;
        reserved = stamp;
      }
    }
    if (text !== previous) e.textContent = previous = text;
    if (format === 'relative') timer.due = clock.now + relativeDelay(datetime, now);
    if ((format === 'timer' || format === 'countdown') && (format !== 'countdown' || datetime > now)) timer.due = clock.now + 1000 - ((now - datetime) % 1000 + 1000) % 1000;
  }
  effect(() => { values = read(); update(); if (!clock.agent) drive(); });
  const moved = () => { if (timer.due <= clock.now) update(); };
  Clocked.push(moved);
  const resized = reserve && typeof ResizeObserver === 'function' ? new ResizeObserver(() => update()) : null;
  resized?.observe(e);
  onEnd(() => { resized?.disconnect(); clock.timers.splice(clock.timers.indexOf(timer), 1); Clocked.splice(Clocked.indexOf(moved), 1); });
}
