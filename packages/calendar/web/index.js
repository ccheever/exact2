// @exact/calendar on the web (LLP 1133.018 D15): no calendar store, and no
// API that adds an event, so the editor is an `.ics` file (RFC 5545) the
// browser downloads and the system offers to its calendar. Its answer is
// `unknown`: the page never learns whether the person added it (D4). Every
// store name is refused by the page runtime before it reaches here.
//
// Under the agent (§10) nothing is downloaded: the sheet's `save` answers
// `unknown`, as production does, and adds to no store.

const STATUS = { available: false, state: 'unknown', access: 'unknown', editor: true, apple: null, android: null };

/** RFC 5545 §3.3.11's TEXT escapes. */
const text = s => String(s).replace(/\\/g, '\\\\').replace(/;/g, '\\;').replace(/,/g, '\\,').replace(/\r?\n/g, '\\n');

/** A content line folded at 75 octets (§3.1), each continuation led by a space. */
function fold(line) {
  const bytes = new TextEncoder().encode(line);
  if (bytes.length <= 75) return line;
  const out = [];
  let start = 0, width = 75;
  while (start < bytes.length) {
    let end = Math.min(start + width, bytes.length);
    while (end < bytes.length && (bytes[end] & 0xc0) === 0x80) end--; // never inside a UTF-8 sequence
    out.push(new TextDecoder().decode(bytes.slice(start, end)));
    start = end;
    width = 74;
  }
  return out.join('\r\n ');
}

/** `2026-10-23T09:00:00` as its parts. */
const parts = local => {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2})(?::(\d{2}))?)?$/.exec(local ?? '');
  return m ? m.slice(1).map(n => Number(n ?? 0)) : null;
};

/** The instant a local time in IANA zone `zone` names: the offset found by asking the zone what its wall reads. */
function zoned(p, zone) {
  const wall = Date.UTC(p[0], p[1] - 1, p[2], p[3], p[4], p[5]);
  const format = new Intl.DateTimeFormat('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
  let t = wall;
  for (let i = 0; i < 3; i++) {
    const f = Object.fromEntries(format.formatToParts(new Date(t)).map(x => [x.type, x.value]));
    const seen = Date.UTC(+f.year, +f.month - 1, +f.day, +f.hour, +f.minute, +f.second);
    t += wall - seen;
  }
  return t;
}

const pad = (n, w = 2) => String(n).padStart(w, '0');
const utc = ms => { const d = new Date(ms); return `${d.getUTCFullYear()}${pad(d.getUTCMonth() + 1)}${pad(d.getUTCDate())}T${pad(d.getUTCHours())}${pad(d.getUTCMinutes())}${pad(d.getUTCSeconds())}Z`; };

/** The event as an iCalendar object: a zoned start in UTC (§3.3.5 form 2, so no VTIMEZONE is needed), a floating
 * one as local time (form 1), an all-day one as a DATE (§3.3.4). */
export function ics(event, now = Date.now()) {
  const p = parts(event.start);
  if (!p || typeof event.title !== 'string') return null;
  const start = event.showWithoutTime ? `DTSTART;VALUE=DATE:${p[0]}${pad(p[1])}${pad(p[2])}`
    : event.timeZone ? `DTSTART:${utc(zoned(p, event.timeZone))}`
    : `DTSTART:${p[0]}${pad(p[1])}${pad(p[2])}T${pad(p[3])}${pad(p[4])}${pad(p[5])}`;
  const lines = [
    'BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Exact//@exact/calendar//EN', 'BEGIN:VEVENT',
    `UID:${event.uid ?? `${now.toString(36)}-${Math.random().toString(36).slice(2)}@exact`}`,
    `DTSTAMP:${utc(now)}`, start, `DURATION:${event.duration || (event.showWithoutTime ? 'P1D' : 'PT1H')}`,
    `SUMMARY:${text(event.title)}`,
  ];
  if (event.description) lines.push(`DESCRIPTION:${text(event.description)}`);
  if (event.location) lines.push(`LOCATION:${text(event.location)}`);
  if (event.url) lines.push(`URL:${event.url}`);
  if (event.status) lines.push(`STATUS:${event.status.toUpperCase()}`);
  if (event.freeBusyStatus) lines.push(`TRANSP:${event.freeBusyStatus === 'free' ? 'TRANSPARENT' : 'OPAQUE'}`);
  lines.push('END:VEVENT', 'END:VCALENDAR');
  return lines.map(fold).join('\r\n') + '\r\n';
}

export function implementation({ agent, log }) {
  const say = line => log(`calendar: ${line}`);
  return {
    async calendarStatus() { return STATUS; },
    async showEventEditor({ event } = {}, call) {
      const file = event && ics(event);
      if (!file) return { error: 'TypeError', message: 'the event needs a title and a start' };
      if (agent) {
        const tap = call?.preferred?.tap;
        say(`editor ${tap ?? 'cancel'}: ${event.title} (${file.length} bytes of .ics, not downloaded under the agent)`);
        return { action: tap === 'save' ? 'unknown' : 'canceled' };
      }
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([file], { type: 'text/calendar' }));
      a.download = `${event.title.replace(/[\\/:*?"<>|]+/g, ' ').trim() || 'event'}.ics`;
      document.body.append(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 60_000);
      say(`editor: ${a.download} downloaded`);
      return { action: 'unknown' };
    },
  };
}
