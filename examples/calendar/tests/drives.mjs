// LLP 1133.018 §11's acceptance drives 1-6 on the Calendar under the agent:
// the fixture stores of packages/contacts and packages/calendar, admission
// and tickets by the runner. Drives 1, 2, 4, 5 and 6 need a calendar store,
// so they run on macOS and iOS; drive 3 also runs on the web, whose agent
// offers the contact picker. Drive 7 is the web in production
// (packages/calendar/test/web.mjs).
//
// usage: bun examples/calendar/tests/drives.mjs <web|macos|ios> [--sim <udid>]
//   (EXACT_APP_DIR set to examples/calendar and the app built for that host:
//   host/web/build.mjs calendar-web, or host/apple/build.mjs [--ios] calendar-macos|calendar-ios)
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../../packages/camera/test/fixture.mjs';

const root = resolve(import.meta.dir, '../../..'), app = resolve(root, 'examples/calendar');
const host = process.argv[2] ?? 'macos', extra = process.argv.slice(3);
const { check, failed } = checker();

/** One launch: each op's JSON reply, by its op. */
function drive(storage, permissions, ops, { reset = false } = {}) {
  const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'calendar', '--size', '1180x800', '--storage', storage, ...(reset ? ['--reset-storage'] : []),
    '--epoch', '2026-09-16T12:00:00Z', '--time-zone', 'UTC', ...flags, ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = tree => JSON.stringify(tree) ?? '';
const titles = agenda => (agenda?.events ?? []).map(e => e.title);
const ticket = (pending, capability) => Array.isArray(pending) ? pending.find(p => p.device?.capability === capability) : undefined;
const mine = tree => [...text(tree).matchAll(/"my-event-([^"]+)"/g)].map(m => m[1]);
const dots = tree => [...text(tree).matchAll(/"device-dot-(\d{4}-\d\d-\d\d)"/g)].map(m => m[1]);
const FIXTURE_16 = ['Lunch with Ana', 'Standup', 'Release review', 'Dentist'];
const open = ['clock settle', 'tap agenda-item-sample-2026-09-5', 'clock settle'];

if (host !== 'web') {
  // 1 and 2: switch on, allow, the fixture's events and dots; the store replaced.
  {
    const r = drive('calendar-drives-1', { calendar: 'prompt' }, ['clock settle', 'tree', 'tap show-my-calendars', 'clock +100', 'state pending', 'tap @calendar.calendars allow', 'clock settle',
      'state mineAgenda', 'tree', 'state calStatus', 'prefer extension calendar.store="other-calendar.json"', 'clock settle', 'state mineAgenda', 'tree', 'logs'], { reset: true });
    check(host, '1: nothing is read before the switch', mine(r[1]).length === 0 && dots(r[1]).length === 0 && text(r[1]).includes('show-my-calendars'), text(r[1]).slice(0, 200));
    const asked = ticket(r[4], 'permission');
    check(host, '1: the switch asks for full access', asked?.device?.args?.capability === 'calendar' && asked?.device?.args?.asks?.access === 'full', JSON.stringify(r[4]));
    check(host, "1: the section lists 16 September's fixture events", JSON.stringify(titles(r[7])) === JSON.stringify(FIXTURE_16), JSON.stringify(titles(r[7])));
    check(host, '1: the month dots the fixture days', ['2026-09-16', '2026-09-23', '2026-10-03'].every(d => dots(r[8]).includes(d)) && mine(r[8]).length === 4, JSON.stringify(dots(r[8])));
    check(host, '1: granted, full', r[9]?.state === 'granted' && r[9]?.access === 'full', JSON.stringify(r[9]));
    check(host, '2: the other store answers', JSON.stringify(titles(r[12])) === '["Dentist (moved)"]' && JSON.stringify(dots(r[13]).sort()) === '["2026-09-16","2026-09-18"]', `${JSON.stringify(titles(r[12]))} ${JSON.stringify(dots(r[13]))}`);
    // Each of the two readers (the day, the month) asks again once: one page each.
    const lines = r[14]?.lines ?? [], at = lines.findIndex(l => /calendar: store /.test(l));
    const pages = lines.slice(at).filter(l => /calendar: page /.test(l));
    check(host, '2: the section and the dots asked again once', at >= 0 && pages.length === 2, pages.join(' | '));
  }
  // 6: relaunched granted, the switch kept on: the first frame is the app's
  // own, which waits for the clock as before the module, with no section or
  // dot kept from the last run; after settling, both. On iOS the module's
  // answer, asked at launch, can land before the first frame: there the
  // journal shows it asked in this launch.
  {
    const r = drive('calendar-drives-1', { calendar: 'granted' }, ['tree', 'logs', 'clock settle', 'state agenda', 'tree']);
    const asked = (r[1]?.lines ?? []).some(l => /calendar: page calendarEvents/.test(l));
    const fresh = (mine(r[0]).length === 0 && dots(r[0]).length === 0) || (host === 'ios' && asked);
    check(host, '6: the first frame shows nothing kept of the store', fresh && text(r[0]).includes('show-my-calendars'), text(r[0]).match(/my-calendars[^"]*/g)?.join(','));
    check(host, "6: after settling, the app's own items, the section and the dots", (r[3]?.items ?? []).some(i => i.title === 'Coast hike') && mine(r[4]).length === 4 && dots(r[4]).includes('2026-09-16'), `${JSON.stringify(mine(r[4]))} ${JSON.stringify(dots(r[4]))}`);
  }
  // 4 and 5: Add to Calendar, saved into the store and shown; denied, then granted again.
  {
    const r = drive('calendar-drives-1', { calendar: 'granted' }, [...open, 'tap add-to-calendar', 'clock +100', 'state pending', 'tap @editor save', 'clock settle', 'state exported', 'state mineAgenda', 'tree',
      'tap cancel-editor', 'clock settle', 'prefer permission.calendar denied', 'clock settle', 'tree', 'state agenda', 'prefer permission.calendar granted', 'clock settle', 'state mineAgenda']);
    const editor = ticket(r[5], 'editor');
    check(host, "4: the editor's ticket carries the item", editor?.device?.args?.options?.event?.title === 'Coast hike' && editor?.device?.args?.options?.event?.showWithoutTime === true && editor?.device?.args?.options?.event?.duration === 'P2D', JSON.stringify(editor));
    const said = host === 'ios' ? 'saved' : 'unknown';
    check(host, `4: save answers ${said}, and the section shows it unasked`, r[8]?.action === said && titles(r[9]).includes('Coast hike') && text(r[10]).includes('add-to-calendar-result'), `${JSON.stringify(r[8])} ${JSON.stringify(titles(r[9]))}`);
    check(host, "5: denied, the denied line, the app's own unchanged", text(r[15]).includes('my-calendars-denied') && text(r[15]).includes('open-calendar-settings') && mine(r[15]).length === 0 && (r[16]?.items ?? []).length === 2, text(r[15]).match(/my-calendars[^"]*/g)?.join(','));
    check(host, '5: granted again, the section returns', titles(r[19]).includes('Lunch with Ana'), JSON.stringify(titles(r[19])));
  }
}
// 3: Invite, two contacts picked and kept; relaunched, still there; cancel
// changes nothing. The web's agent keeps no app storage between launches
// (the app's own items are gone too), so there the cancel is in the same launch.
{
  const cancel = ['tap invite', 'clock +100', 'tap @contacts cancel', 'clock settle', 'state guests', 'state pending'];
  const r = drive('calendar-drives-3', { contacts: 'prompt' }, [...open, 'tap invite', 'clock +100', 'state pending', 'type @contacts c2,c5', 'clock settle', 'state guests', 'tree', ...(host === 'web' ? cancel : [])], { reset: true });
  const pick = ticket(r[5], 'contacts');
  check(host, '3: Invite holds a contacts ticket for name, email and phone, many', pick?.device?.args?.options?.multiple === true && JSON.stringify(pick?.device?.args?.options?.properties) === '["name","email","tel"]', JSON.stringify(r[5]));
  const names = g => (g?.guests ?? []).map(x => x.name).join(',');
  check(host, '3: two guests on the item', names(r[8]) === 'Ben Okafor,Eli Navarro' && text(r[9]).includes('Guests: Ben Okafor, Eli Navarro'), names(r[8]));
  let after = r.slice(10);
  if (host !== 'web') {
    const again = drive('calendar-drives-3', { contacts: 'prompt' }, [...open, 'state guests', ...cancel]);
    check(host, '3: relaunched, the guests are there', names(again[3]) === 'Ben Okafor,Eli Navarro', names(again[3]));
    after = again.slice(4);
  }
  check(host, '3: cancel leaves them', names(after[4]) === 'Ben Okafor,Eli Navarro' && Array.isArray(after[5]) && !ticket(after[5], 'contacts'), `${names(after[4])} ${JSON.stringify(after[5])}`);
}
if (host === 'web') {
  const r = drive('calendar-drives-web', {}, ['clock settle', 'tree', 'state calStatus']);
  check(host, '7 (agent): no store, so no switch', !text(r[1]).includes('show-my-calendars') && r[2]?.available === false, JSON.stringify(r[2]));
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
