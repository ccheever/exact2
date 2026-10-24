// @exact/calendar's conformance suite on Apple (LLP 1133.018 §11, §14 stage
// 2): the fixture app built for an iOS simulator or macOS and driven by the
// agent with the implementation bound (`--bind calendar`), so EventKit
// answers and not the agent's store. It never reads this Mac's calendars:
//
// - iOS: the simulator's own event store, its access set by `xcrun simctl
//   privacy`, the host's own means: full, revoked and undetermined;
//   write-only by the simulator's privacy database, which simctl cannot set.
//   The suite saves events in September 2026 there.
// - macOS: only what holds with no grant, since only a person can grant one
//   here and the store would be this Mac's: the status and that nothing was
//   asked. The fixture is built to read nothing; save, the raise and the
//   editor are not pressed (the editor would open the Calendar app).
//
// The system's prompts, the raise from write-only and the editor are a
// person's (checklist.md): the agent cannot answer the system's alert or tap
// inside the system's editor.
//
// usage: bun packages/calendar/test/apple.mjs [ios] [macos] [--sim <udid>]
import { readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from '../../contacts/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { check, failed } = checker();
const walked = (t) => /\s\d+$/.test(t ?? '');

/** The fixture built for `host`, its plist read, and `open` bound to it. */
async function fixtureFor(host, reads) {
  const fixture = makeFixture('calendar', { reads });
  Object.assign(process.env, { EXACT_APP_DIR: fixture.app, EXACT2: root, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
  if (sim) process.env.EXACT_SIM = sim;
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', '--sim', sim] : ['--bundle']), `${fixture.name}-apple`], root);
  const target = join(fixture.app, 'target'), files = readdirSync(target, { recursive: true });
  if (host === 'macos') {
    // The drive runs the binary inside the `.app`, so access is the fixture bundle's own.
    const bundle = files.find((f) => /macos/.test(f) && f.endsWith('.app/Contents/Info.plist'));
    if (!bundle) throw new Error('no macOS .app under target/ (build.mjs --bundle)');
    const contents = join(target, bundle, '..'), executable = JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', join(contents, 'Info.plist')], root)).CFBundleExecutable;
    process.env.EXACT_MAC_BIN = join(contents, 'MacOS', executable);
  }
  const at = files.find((f) => host === 'ios' ? /ios-sim/.test(f) && f.endsWith('.app/Info.plist') : /macos/.test(f) && f.endsWith('-Info.plist'));
  const plist = at ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', join(target, at)], root)) : {};
  const { open } = await import(resolve(root, 'scripts/agent.mjs'));
  return { ...fixture, plist, open: () => open({ host, app: fixture.name, bind: 'calendar' }) };
}

for (const host of hosts.length ? hosts : ['ios', 'macos']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  if (host === 'ios' && !sim) throw new Error('ios: name a simulator you booted (--sim <udid>); the suite changes its calendars and its privacy settings');
  const fixture = await fixtureFor(host, host === 'ios');
  try {
    const purpose = 'The conformance suite reads and writes the calendars.';
    check(where, 'Info.plist carries both usage keys with their purpose', fixture.plist.NSCalendarsFullAccessUsageDescription === purpose && fixture.plist.NSCalendarsWriteOnlyAccessUsageDescription === purpose,
      JSON.stringify([fixture.plist.NSCalendarsFullAccessUsageDescription, fixture.plist.NSCalendarsWriteOnlyAccessUsageDescription]));

    if (host === 'macos') {
      const s = await fixture.open();
      try {
        check(where, 'the extension is bound, not substituted', (await s.state()).extensions?.calendar?.bound === true, JSON.stringify((await s.state()).extensions?.calendar));
        const status = await until(s, 'status', (t) => /^true /.test(t ?? ''));
        check(where, 'calendarStatus answers from EventKit, with an editor', /^true (prompt none|denied none|granted (full|write-only)) true$/.test(status ?? ''), status);
        check(where, 'nothing was asked or read', !JSON.stringify(await s.logs()).includes('calendar: asked'), 'the journal says the module asked');
      } finally {
        await s.close();
      }
      continue;
    }

    const privacy = (verb) => run('xcrun', ['simctl', 'privacy', sim, verb, 'calendar', fixture.id], root);
    // Full access: the calendars, the range, a save the change observer shows, the remove, the raise.
    privacy('grant');
    let s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => t === 'true granted full true');
      check(where, 'granted: full access, with an editor', status === 'true granted full true', status);
      const calendars = await until(s, 'calendars', (t) => !!t && t !== 'off');
      check(where, 'calendars lists the store, one default', /^[^,]+/.test(calendars ?? '') && !/^error/.test(calendars) && (calendars.match(/\*/g) ?? []).length === 1, calendars);
      const before = await until(s, 'walk', walked);
      check(where, 'September and October read', walked(before) && !/^error/.test(before), before);
      await s.tap('save');
      const saved = await until(s, 'saved', (t) => !!t, 15000);
      const [id, calendar] = (saved ?? '').split(' ');
      check(where, 'a save answers an id, in the default calendar', !!id && id !== 'none' && calendars.split(',').some((c) => c.startsWith(calendar) && c.endsWith('*')), `${saved} ${(await texts(s))['saved-error']} of ${calendars}`);
      const grown = await until(s, 'walk', (t) => walked(t) && t.split(' ')[0].split(',').includes(id));
      check(where, 'the change observer announces it: the walk shows the event', (grown ?? '').split(' ')[0].split(',').includes(id), `${before} then ${grown}`);
      await s.tap('remove');
      const removed = await until(s, 'removed', (t) => !!t);
      check(where, 'removed by its id', removed === id && (await texts(s))['removed-error'] === '', `${removed} ${(await texts(s))['removed-error']}`);
      const after = await until(s, 'walk', (t) => walked(t) && !t.split(' ')[0].split(',').includes(id));
      check(where, 'and the walk is the store again', after === before, `${before} then ${after}`);
      await s.tap('raise');
      const raised = await until(s, 'raised', (t) => !!t);
      check(where, 'the raise under full access answers full, asking nothing', raised === 'full', `${raised} ${(await texts(s))['raised-error']}`);
      await s.tap('too-long');
      const long = await until(s, 'walk', (t) => /^error/.test(t ?? ''));
      check(where, 'more than four years fails calendar-range-too-long', /^error TypeError .*calendar-range-too-long/.test(long ?? ''), long);
      // D6's page measurements: each read's time off the main thread.
      const lines = (await s.logs())?.lines ?? [];
      console.log(`NOTE ${where}: ${lines.filter((l) => /page calendarEvents .* in \d+ ms/.test(l)).map((l) => l.replace(/^.*page /, '')).join('; ')}`);
    } finally {
      await s.close();
    }

    // Write-only (iOS 17): `simctl privacy` grants full access only, so the
    // grant is written to the simulator's own privacy database as Add Events
    // Only (TCC's auth_value 4; full access is 2). The raise from it asks the
    // person, which is the checklist's.
    privacy('reset');
    privacy('grant');
    const tcc = join(homedir(), 'Library/Developer/CoreSimulator/Devices', sim, 'data/Library/TCC/TCC.db');
    run('sqlite3', [tcc, `UPDATE access SET auth_value = 4, auth_reason = 4, auth_version = 2 WHERE service = 'kTCCServiceCalendar' AND client = '${fixture.id}'`], root);
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true granted/.test(t ?? ''));
      check(where, 'write-only: granted, write-only', status === 'true granted write-only true', status);
      const walk = await until(s, 'walk', (t) => /^error/.test(t ?? ''));
      check(where, 'write-only: reads fail NotAllowedError, write-only', /^error NotAllowedError .*write-only/.test(walk ?? ''), walk);
      await s.tap('save');
      const saved = await until(s, 'saved', (t) => !!t, 15000);
      check(where, 'write-only: a save answers no id or calendar', saved === 'none none', `${saved} ${(await texts(s))['saved-error']}`);
    } finally {
      await s.close();
    }

    // Revoked: reads and a save fail, asking nothing.
    privacy('revoke');
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true denied/.test(t ?? ''));
      check(where, 'revoked: denied', status === 'true denied none true', status);
      const walk = await until(s, 'walk', (t) => /^error/.test(t ?? ''));
      check(where, 'a read fails NotAllowedError', /^error NotAllowedError/.test(walk ?? ''), walk);
      await s.tap('save');
      const refused = await until(s, 'saved-error', (t) => !!t);
      check(where, 'a save fails NotAllowedError', /^NotAllowedError/.test(refused ?? ''), refused);
      check(where, 'nothing was asked', !JSON.stringify(await s.logs()).includes('calendar: asked'), 'the journal says the module asked');
    } finally {
      await s.close();
    }

    // Undetermined: the status says prompt; a read waits on the system's alert.
    privacy('reset');
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true /.test(t ?? ''));
      check(where, 'reset: prompt, no access', status === 'true prompt none true', status);
      await s.clock('+1000 real');
      check(where, 'a read waits on the person', !walked((await texts(s)).walk), (await texts(s)).walk);
    } finally {
      await s.close();
    }
    privacy('reset');
    run('xcrun', ['simctl', 'uninstall', sim, fixture.id], root);
  } finally {
    fixture.remove();
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
