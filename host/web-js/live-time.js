// HTML instants on the runtime clock; ticks run no compiled app expression.
import { clock, Clocked, effect, onEnd, drive } from './rt.js';
import { reportTime, reportPlace } from './navigation.js';

const valid = n => typeof n === 'number' && Number.isFinite(n) && n >= -62135596800000 && n <= 253402300799999;
export function relative(delta) {
  const round = n => Math.sign(n) * Math.round(Math.abs(n));
  const minutes = Math.ceil(delta / 60000);
  const [scale, unit] = Math.abs(minutes) < 60 ? [1, 'minute'] : Math.abs(minutes) < 1440 ? [60, 'hour'] : Math.abs(minutes) < 43200 ? [1440, 'day'] : Math.abs(minutes) < 525600 ? [43200, 'month'] : [525600, 'year'];
  return [round(minutes / scale) || 0, unit];
}
export function formatted(datetime, now, format, locale, timeZone) {
  if (!valid(datetime)) return null;
  if (format === 'relative') return new Intl.RelativeTimeFormat(locale, { style: 'short', numeric: 'always' }).format(...relative(datetime - now));
  if (format === 'timer' || format === 'countdown') {
    const s = format === 'countdown' ? Math.ceil(Math.max(0, datetime - now) / 1000) : Math.floor(Math.abs(datetime - now) / 1000);
    const pad = n => String(n).padStart(2, '0');
    return s >= 3600 ? `${Math.floor(s / 3600)}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}` : `${Math.floor(s / 60)}:${pad(s % 60)}`;
  }
  const [kind, style] = format.split('-');
  if (!['date', 'time'].includes(kind) || !['short', 'medium', 'long', 'full'].includes(style)) return null;
  return new Intl.DateTimeFormat(locale, { timeZone, [kind + 'Style']: style }).format(datetime);
}

export function liveTime(e, read, reserve = true) {
  let values, previous = null, reserved = '', fallback = [...e.childNodes];
  const timer = { due: Infinity, hatch: true, action: () => { update(); return true; } };
  clock.timers.push(timer);
  function update() {
    const [datetime, name, start, end] = values;
    const now = reportTime(clock.now)[0] + clock.now;
    const [locale, timeZone] = reportPlace().split('\0');
    timer.due = Infinity;
    if (e.localName === 'progress') {
      if (valid(start) && valid(end) && end > start) {
        e.max = 1; e.value = Math.max(0, Math.min(1, (now - start) / (end - start)));
        if (now < end) timer.due = clock.now + Math.min(now < start ? start - now : 1000, end - now);
      } else e.removeAttribute('value');
      return;
    }
    const format = name ?? 'date-medium';
    const text = formatted(datetime, now, format, locale, timeZone);
    if (text == null) {
      if (reserve) e.style.minInlineSize = '';
      reserved = '';
      if (previous != null) e.replaceChildren(...fallback);
      previous = null; return;
    }
    e.setAttribute('datetime', new Date(datetime).toISOString());
    const font = typeof getComputedStyle === 'function' ? getComputedStyle(e) : null;
    const keys = ['font', 'fontVariantNumeric', 'fontFeatureSettings', 'fontVariationSettings', 'letterSpacing', 'fontKerning', 'fontOpticalSizing'];
    const stamp = `${datetime}/${format}/${locale}/${timeZone}/${keys.map(k => font?.[k]).join('/')}`;
    if (stamp !== reserved) {
      const strings = [text];
      if (format === 'relative') {
        const relative = new Intl.RelativeTimeFormat(locale, { style: 'short', numeric: 'always' });
        for (const sign of [-1, 1]) strings.push(`${relative.format(sign * 59, 'minute')}, ${relative.format(sign * 59, 'second')}`);
        for (const seconds of [59, 3599, 86399, 2591999, 31535999, 315359999999]) for (const sign of [-1, 1]) strings.push(formatted(0, sign * seconds * 1000, format, locale, timeZone));
      } else if (format === 'timer' || format === 'countdown') {
        for (let i = 0; i <= 9; i++) strings.push(`${String(i).repeat(10)}:${i}${i}:${i}${i}`);
      }
      let width = Math.max(...strings.map(s => s.length * 8));
      if (font && document.body) {
        const measure = document.createElement('span');
        Object.assign(measure.style, { position: 'fixed', visibility: 'hidden', pointerEvents: 'none', whiteSpace: 'pre', width: 'max-content' });
        for (const k of keys) measure.style[k] = font[k];
        for (const text of strings) { const line = document.createElement('span'); line.style.display = 'block'; line.textContent = text; measure.append(line); }
        document.body.append(measure);
        width = measure.getBoundingClientRect().width;
        measure.remove();
      }
      if (reserve) e.style.minInlineSize = `${Math.ceil(width)}px`;
      reserved = stamp;
    }
    if (text !== previous) e.textContent = previous = text;
    if (format === 'relative') timer.due = clock.now + 60000 - ((now - datetime) % 60000 + 60000) % 60000;
    if ((format === 'timer' || format === 'countdown') && (format !== 'countdown' || datetime > now)) timer.due = clock.now + 1000 - ((now - datetime) % 1000 + 1000) % 1000;
  }
  effect(() => { values = read(); update(); if (!clock.agent) drive(); });
  const moved = () => { if (timer.due <= clock.now) update(); };
  Clocked.push(moved);
  const resized = typeof ResizeObserver === 'function' ? new ResizeObserver(() => update()) : null;
  resized?.observe(e);
  onEnd(() => { resized?.disconnect(); clock.timers.splice(clock.timers.indexOf(timer), 1); Clocked.splice(Clocked.indexOf(moved), 1); });
}
