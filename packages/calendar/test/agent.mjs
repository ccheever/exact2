// @exact/calendar's conformance suite under the agent (LLP 1133.018 §10, §14
// stage 0): the fixture app built for a host and driven with the
// implementation's agent path over packages/calendar/fixture/calendar.json,
// in UTC. The reads are held equal to the suite's own reading of that file:
// the occurrences overlapping the range, ordered by `utcStart`, `utcEnd`,
// `id` and `recurrenceId`, `limit` at a time; a cursor asked again after a
// change starts after the same place in that order (D6).
//
// Three grants: both lines (full, then write-only and the raise), and the
// write-only line alone. The web has no store: there the status answers
// `available` false and every store name fails NotSupportedError. A
// tool-origin `saveEvent` failing `person-required` is the runner's
// (contract/cli/tests/it/contacts_calendar.rs).
//
// usage: bun packages/calendar/test/agent.mjs <web|macos|ios> [--sim <udid>] [--keep]
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { build, checker, drive, makeFixture, root, texts, ticket } from '../../contacts/test/fixture.mjs';

const argv = process.argv.slice(2), host = argv[0] ?? 'macos';
const sim = argv.includes('--sim') ? ['--sim', argv[argv.indexOf('--sim') + 1]] : [];
const { check, failed } = checker();
const store = file => JSON.parse(readFileSync(resolve(root, 'packages/calendar/fixture', file), 'utf8'));

/** The offset of `zone` from UTC at instant `t`, in ms. */
const offset = (t, zone) => {
  const p = Object.fromEntries(new Intl.DateTimeFormat('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })
    .formatToParts(new Date(t)).map(x => [x.type, x.value]));
  return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute, +p.second) - t;
};
/** An event's instants: a floating or all-day one in the session's zone (UTC here). */
const instants = e => {
  const [d, t] = e.start.split('T'), [y, m, day] = d.split('-').map(Number), [h, min, s] = t.split(':').map(Number);
  const local = Date.UTC(y, m - 1, day, h, min, s), zone = e.showWithoutTime ? null : e.timeZone;
  const start = zone ? local - offset(local - offset(local, zone), zone) : local;
  const [, days = 0, hours = 0, minutes = 0] = /^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?)?$/.exec(e.duration).map(v => +(v ?? 0));
  return [start, start + ((days * 24 + hours) * 60 + minutes) * 60_000];
};
const keyOf = e => `${e.id}${e.recurrenceId ? `@${e.recurrenceId.slice(0, 10)}` : ''}`;
const order = (a, b) => a.s - b.s || a.t - b.t || (a.e.id < b.e.id ? -1 : a.e.id > b.e.id ? 1 : 0) || ((a.e.recurrenceId ?? '') < (b.e.recurrenceId ?? '') ? -1 : (a.e.recurrenceId ?? '') > (b.e.recurrenceId ?? '') ? 1 : 0);
/** The suite's own answer: the occurrences overlapping `[start, end)`, in order. */
const expected = (events, start, end) => events.map(e => { const [s, t] = instants(e); return { e, s, t }; }).filter(x => x.s < end && x.t > start).sort(order);
const SEPTEMBER = Date.UTC(2026, 8, 1), DAY = 86_400_000;
const fixtureEvents = store('calendar.json').events, all = expected(fixtureEvents, SEPTEMBER, SEPTEMBER + 61 * DAY);
const walked = (list, size = 3) => `${list.map(x => keyOf(x.e)).join(',')} ${Math.max(1, Math.ceil(list.length / size))}`;

const runs = [];
const fixture = makeFixture('calendar');
runs.push(fixture);
try {
  build(fixture, host, sim);
  const flags = state => ['--permission', `calendar=${state}`, ...sim];
  if (host === 'web') {
    const r = drive(fixture, host, flags('prompt'), ['clock settle', 'tree', 'tap save', 'clock settle', 'tap edit', 'clock +100', 'state pending', 'tap @editor save', 'clock settle', 'tree']);
    const t = texts(r[1]), after = texts(r[9]);
    check(host, 'no store: the status says so, and an editor', t.status === 'false unknown unknown true', t.status);
    check(host, 'no store: reads fail NotSupportedError', /^error NotSupportedError/.test(t.walk) && /^error NotSupportedError/.test(t.calendars), `${t.walk} ${t.calendars}`);
    check(host, 'no store: a save fails NotSupportedError', /^NotSupportedError/.test(after['saved-error']), after['saved-error']);
    check(host, "the editor's save answers unknown and adds to no store", ticket(r[6], 'editor') && after.edited === 'unknown', `${JSON.stringify(r[6])} ${after.edited}`);
  } else {
    // Full: the reads, the cursor, the range bound, a save and a remove, the editor.
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'tree', 'tap more', 'clock settle', 'tree', 'tap too-long', 'clock settle', 'tree',
        'tap save', 'clock settle', 'tree', 'tap remove', 'clock settle', 'tree', 'tap edit', 'clock +100', 'state pending', 'tap @editor cancel', 'clock settle', 'tree',
        'tap edit', 'clock +100', 'tap @editor save', 'clock settle', 'tree']);
      const at = i => texts(r[i]);
      check(host, 'granted, full', at(1).status === 'true granted full true', at(1).status);
      check(host, 'the calendars, the default marked, one read-only', at(1).calendars === 'home*,work,holidays(ro)', at(1).calendars);
      check(host, "September and October in the suite's order, three a page", at(1).walk === walked(all), `${at(1).walk}\n  vs ${walked(all)}`);
      check(host, 'key cursors: the second page follows the first', at(1).page === all.slice(0, 3).map(x => keyOf(x.e)).join(',') && at(4).page === all.slice(3, 6).map(x => keyOf(x.e)).join(','), `${at(1).page} | ${at(4).page}`);
      check(host, 'more than four years fails calendar-range-too-long', /^error TypeError .*calendar-range-too-long/.test(at(7).walk), at(7).walk);
      const [id, cal] = (at(10).saved ?? '').split(' ');
      check(host, 'a save answers an id, in the default calendar', /^\w+/.test(id) && id !== 'none' && cal === 'home', at(10).saved);
      check(host, 'removed by its id', at(13).removed === id && at(13)['removed-error'] === '', `${at(13).removed} ${at(13)['removed-error']}`);
      const editor = ticket(r[16], 'editor');
      check(host, "the editor's ticket carries the event", editor?.device?.args?.options?.event?.title === 'Suite event', JSON.stringify(r[16]));
      check(host, 'the editor cancelled answers canceled', at(19).edited === 'canceled', at(19).edited);
      check(host, `the editor saved answers ${host === 'ios' ? 'saved' : 'unknown'}`, at(24).edited === (host === 'ios' ? 'saved' : 'unknown'), at(24).edited);
    }
    // A save and a removal are announced: the walk asked again shows each.
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'tap save', 'clock settle', 'tree', 'tap remove', 'clock settle', 'tree']);
      const added = expected([...fixtureEvents, { id: 'x', start: '2026-09-16T08:00:00', timeZone: 'UTC', duration: 'PT30M' }], SEPTEMBER, SEPTEMBER + 61 * DAY);
      const id = (texts(r[3]).saved ?? '').split(' ')[0];
      const shown = (texts(r[3]).walk ?? '').split(' ')[0].split(',');
      check(host, 'saved, the walk shows it in its place', shown.length === added.length && shown[added.findIndex(x => x.e.id === 'x')] === id, texts(r[3]).walk);
      check(host, 'removed, the walk is the fixture again', texts(r[6]).walk === walked(all), texts(r[6]).walk);
    }
    // A cursor across a change: the other store, after the same place.
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'tap more', 'clock settle', 'tree', 'prefer extension calendar.store="other-calendar.json"', 'clock settle', 'clock +100', 'clock settle', 'tree']);
      const cursor = all[2], other = expected(store('other-calendar.json').events, SEPTEMBER, SEPTEMBER + 61 * DAY);
      const after = other.filter(x => order(x, cursor) > 0).slice(0, 3).map(x => keyOf(x.e)).join(',');
      check(host, 'a page asked again after a change starts after the same place', texts(r[8]).page === after, `${texts(r[8]).page} vs ${after}`);
    }
    // Write-only under both lines: reads refused, a save answers no id; the raise declined, then allowed.
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'prefer extension calendar.calendarStatus={"access":"write-only"}', 'clock settle', 'tree', 'tap save', 'clock settle', 'tree',
        'tap raise', 'clock +100', 'state pending', 'tap @access deny', 'clock settle', 'tree', 'tap raise', 'clock +100', 'tap @access allow', 'clock settle', 'tree']);
      const at = i => texts(r[i]);
      check(host, 'write-only: the status says so', at(3).status === 'true granted write-only true', at(3).status);
      check(host, 'write-only: reads fail NotAllowedError, write-only', /^error NotAllowedError .*write-only/.test(at(3).walk) && /^error NotAllowedError .*write-only/.test(at(3).calendars), `${at(3).walk} ${at(3).calendars}`);
      check(host, 'write-only: a save answers no id or calendar', at(6).saved === 'none none', `${at(6).saved} ${at(6)['saved-error']}`);
      check(host, 'the raise holds an access ticket', !!ticket(r[9], 'access'), JSON.stringify(r[9]));
      check(host, 'declined, it stays write-only', at(12).raised === 'write-only' && at(12).status === 'true granted write-only true', `${at(12).raised} ${at(12).status}`);
      check(host, 'allowed, full, and the reads answer', at(17).raised === 'full' && /^e/.test(at(17).walk) && at(17).walk.split(' ')[0].split(',').length === all.length + 1, `${at(17).raised} ${at(17).walk}`);
    }
    // A storm of changes costs at most two rounds of reads (D7).
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'prefer extension calendar.notify=25', 'clock settle', 'clock +3000', 'clock settle', 'tree', 'logs']);
      const lines = r[6]?.lines ?? [], from = lines.findIndex(l => /calendar: notify 25/.test(l));
      const rounds = lines.slice(from).filter(l => /calendar: announced/.test(l)).length;
      check(host, 'twenty-five changes, at most two rounds', from >= 0 && rounds >= 1 && rounds <= 2, `${rounds} rounds`);
      check(host, 'after the storm the reads still answer', texts(r[5]).walk === walked(all), texts(r[5]).walk);
    }
    // The write-only line alone: a save asks for write-only; reads are refused.
    {
      const only = makeFixture('calendar', { grants: 'device.calendar-write-only purpose.calendar' });
      runs.push(only);
      build(only, host, sim);
      const r = drive(only, host, flags('prompt'), ['clock settle', 'tree', 'tap save', 'clock +100', 'state pending', 'tap @permission allow', 'clock settle', 'tree']);
      const asked = ticket(r[4], 'permission');
      check(host, 'write-only line: a save asks for write-only', asked?.device?.args?.asks?.access === 'write-only', JSON.stringify(r[4]));
      check(host, 'write-only line: allowed, write-only; saved with no id', texts(r[7]).status === 'true granted write-only true' && texts(r[7]).saved === 'none none', `${texts(r[7]).status} ${texts(r[7]).saved} ${texts(r[7])['saved-error']}`);
      check(host, 'write-only line: reads refused', /^error NotAllowedError/.test(texts(r[7]).walk), texts(r[7]).walk);
    }
  }
} finally {
  if (!argv.includes('--keep')) for (const f of runs) f.remove();
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
