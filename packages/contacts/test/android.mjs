// @exact/contacts and @exact/calendar on Android (LLP 1133.018 stage 3), on `apps/contacts-calendar-fixture`, both
// packages' consumer there (the Calendar example has no Android crate).
//
// With no flag it builds the fixture's APK and checks what the build made: the permissions the grant lines derive,
// with `WRITE_CALENDAR` because the fixture calls `saveEvent` (D11; the derivation's other cases are
// `contract/cli/tests/it/contacts_calendar.rs`'s); the roster naming both extensions and their classes compiled in;
// the agent's fixtures carried byte for byte. `--emulator` then drives the fixture under the agent on a booted
// emulator or phone (adb's one device): both statuses, the store reads, a pick, a save and the editor. Both need an
// NDK in ANDROID_NDK_HOME and a JDK 17 or newer in JAVA_HOME; the drives also an arm64 device. checklist.md's
// Android part is what needs a person: the platform's dialogs, the pickers and the calendar app.
//
// usage: bun packages/contacts/test/android.mjs [--emulator]
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, root, texts, ticket } from './fixture.mjs';

const app = 'contacts-calendar-fixture';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const { check, failed } = checker();
const built = resolve(root, 'host/android/app/build', app);

const build = spawnSync('bun', ['scripts/agent-android.mjs', 'build', app, '--apk'], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
check('android', 'the fixture builds as an APK', build.status === 0, (build.stdout + build.stderr).slice(-2000));
const merged = existsSync(built) && readdirSync(built, { recursive: true })
  .find(f => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
const manifest = merged ? readFileSync(resolve(built, merged), 'utf8') : '';
const permission = name => new RegExp(`<uses-permission\\s+android:name="android\\.permission\\.${name}"`).test(manifest);
check('android', 'device.contacts derives READ_CONTACTS, and nothing writes contacts', permission('READ_CONTACTS') && !permission('WRITE_CONTACTS'), manifest.slice(0, 1200));
check('android', 'device.calendar derives READ_CALENDAR, and saveEvent WRITE_CALENDAR', permission('READ_CALENDAR') && permission('WRITE_CALENDAR'), manifest.slice(0, 1200));
const roster = resolve(built, 'extensions/exact/app/ExactExtensions.kt');
const listed = existsSync(roster) ? readFileSync(roster, 'utf8') : '';
check('android', 'the roster names both extensions', listed.includes('"contacts" to { c -> exact.contacts.ContactsModule(c) }')
  && listed.includes('"calendar" to { c -> exact.calendar.CalendarModule(c) }'), listed);
const apk = resolve(built, 'outputs/apk/release/app-release.apk');
const dex = spawnSync('unzip', ['-p', apk, 'classes*.dex'], { maxBuffer: 1 << 28 }).stdout ?? Buffer.alloc(0);
// R8 renames the classes, so each file is found by text only it holds: the picker's action, the provider's UID
// column, each agent path's asset directory.
const held = ['android.provider.action.PICK_CONTACTS', 'uid2445', 'app/assets/contacts-fixture/', 'app/assets/calendar-fixture/'].filter(s => !dex.includes(s));
check('android', "both packages' Kotlin is compiled in", !held.length, `missing ${held.join(', ')} in a dex of ${dex.length} bytes`);
for (const [dir, file, from] of [['contacts-fixture', 'contacts.json', 'packages/contacts/fixture'], ['calendar-fixture', 'calendar.json', 'packages/calendar/fixture'],
  ['calendar-fixture', 'other-calendar.json', 'packages/calendar/fixture']]) {
  const bytes = spawnSync('unzip', ['-p', apk, `assets/app/assets/${dir}/${file}`], { maxBuffer: 1 << 24 }).stdout;
  check('android', `${file} is carried as the package's fixture`, bytes?.length > 0 && sha(bytes) === sha(readFileSync(resolve(root, from, file))), `${bytes?.length ?? 0} bytes`);
}

if (process.argv.includes('--emulator')) {
  const book = JSON.parse(readFileSync(resolve(root, 'packages/contacts/fixture/contacts.json'), 'utf8'));
  /** One launch under the agent: each op's JSON reply. */
  const drive = (permissions, ops) => {
    const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
    const r = spawnSync('bun', ['scripts/agent.mjs', 'android', '--reader', '--json', '--app', app, '--epoch', '2026-09-16T12:00:00Z', ...flags, ...ops], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 26 });
    const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
    if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
    return replies;
  };
  // 1: granted, the reads answer the fixtures: the first page of contacts, the calendars, September's events.
  {
    const r = drive({ contacts: 'granted', calendar: 'granted' }, ['clock settle', 'tree']);
    const t = texts(r[1]);
    check('android', '1: both granted, full', t['contacts-status'] === 'granted full' && t['calendar-status'] === 'granted full', `${t['contacts-status']} | ${t['calendar-status']}`);
    check('android', "1: the contacts are the fixture's, in its order", t.contacts === book.contacts.slice(0, 100).map(c => c.name?.[0] ?? '-').join(', '), t.contacts);
    check('android', "1: the calendars are the fixture's", t.calendars === 'Home, Work, Holidays', t.calendars);
    check('android', '1: the events of September and October', (t.events ?? '').includes('Dentist'), t.events);
  }
  // 2: prompt: each read holds a permission ticket; a pick needs none and takes typed ids.
  {
    const r = drive({ contacts: 'prompt', calendar: 'prompt' }, ['clock settle', 'state pending', 'tap pick', 'clock +100', 'state pending', 'type @contacts c1,c4', 'clock settle', 'tree']);
    check('android', '2: the reads under prompt ask', ticket(r[1], 'permission'), JSON.stringify(r[1]));
    check('android', '2: the pick holds a contacts ticket', ticket(r[4], 'contacts')?.device?.args?.options?.multiple === true, JSON.stringify(r[4]));
    check('android', '2: the two picked', texts(r[7]).chosen === 'Ana Lima, Dana Kowalski', texts(r[7]).chosen);
  }
  // 3: a save lands in the default calendar and joins the events; the editor's save answers unknown, as Android's does.
  {
    const r = drive({ contacts: 'granted', calendar: 'granted' }, ['clock settle', 'tap save', 'clock settle', 'tree', 'tap edit', 'clock +100', 'tap @editor save', 'clock settle', 'tree']);
    check('android', '3: the save answers the event, fixed in its zone', texts(r[3]).saved === 'Fixture event UTC' && (texts(r[3]).events ?? '').includes('Fixture event'), JSON.stringify(texts(r[3])));
    check('android', '3: the editor answers unknown', texts(r[8]).edited === 'unknown', texts(r[8]).edited);
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
