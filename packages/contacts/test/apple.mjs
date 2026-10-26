// @exact/contacts' conformance suite on Apple (LLP 1133.018 §11, §14 stage
// 2): the fixture app built for an iOS simulator or macOS and driven by the
// agent with the implementation bound (`--bind contacts`), so the Contacts
// framework answers and not the agent's address book. It never reads this
// Mac's own contacts:
//
// - iOS: the simulator's own address book (a new simulator's six sample
//   contacts), its access set by `xcrun simctl privacy`, the host's own
//   means: full, revoked and undetermined; limited (iOS 18) by the
//   simulator's privacy database, which simctl cannot set.
// - macOS: only what holds with no grant, since only a person can grant one
//   here and the store would be this Mac's: the status and the properties. The fixture is built to read nothing, and no pick is
//   pressed: the picker would show this Mac's contacts.
//
// The pickers and the system's prompt are a person's (checklist.md): the
// agent cannot tap inside the system's picker or answer its alert.
//
// usage: bun packages/contacts/test/apple.mjs [ios] [macos] [--sim <udid>]
import { readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { check, failed } = checker();
// The six contacts a new iOS simulator's address book holds.
const SAMPLE = ['Kate Bell', 'Daniel Higgins Jr.', 'John Appleseed', 'Anna Haro', 'Hank M. Zakroff', 'David Taylor'];

/** The fixture built for `host`, its plist read, and `open` bound to it. */
async function fixtureFor(host, reads) {
  const fixture = makeFixture('contacts', { reads });
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
  return { ...fixture, plist, open: () => open({ host, app: fixture.name, bind: 'contacts' }) };
}

for (const host of hosts.length ? hosts : ['ios', 'macos']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  if (host === 'ios' && !sim) throw new Error('ios: name a simulator you booted (--sim <udid>); the suite changes its privacy settings');
  const fixture = await fixtureFor(host, host === 'ios');
  try {
    check(where, 'Info.plist carries the usage key with its purpose', fixture.plist.NSContactsUsageDescription === 'The conformance suite reads the contacts.', JSON.stringify(fixture.plist.NSContactsUsageDescription));

    if (host === 'macos') {
      const s = await fixture.open();
      try {
        check(where, 'the extension is bound, not substituted', (await s.state()).extensions?.contacts?.bound === true, JSON.stringify((await s.state()).extensions?.contacts));
        const status = await until(s, 'status', (t) => /^true /.test(t ?? ''));
        check(where, 'contactsStatus answers from the framework', /^true (prompt none|denied none|granted (full|limited))$/.test(status ?? ''), status);
        check(where, "the picker's properties", (await texts(s)).properties === 'address,email,icon,name,tel', (await texts(s)).properties);
        check(where, 'nothing was asked or read', !JSON.stringify(await s.logs()).includes('contacts: asked:'), 'the journal says the module asked');
      } finally {
        await s.close();
      }
      continue;
    }

    const privacy = (verb, service) => run('xcrun', ['simctl', 'privacy', sim, verb, service, fixture.id], root);
    // Full access: the simulator's six, a query by substring, and pages by key.
    privacy('grant', 'contacts');
    let s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => t === 'true granted full');
      check(where, 'granted: full access', status === 'true granted full', status);
      const names = await until(s, 'names', (t) => !!t && !t.startsWith('error'));
      const got = (names ?? '').split('|');
      check(where, 'queryContacts reads the address book, every sample contact', SAMPLE.every((n) => got.includes(n)), names);
      const walk = await until(s, 'walk', (t) => /\s\d+$/.test(t ?? ''));
      const [ids, pages] = (walk ?? '').split(' ');
      check(where, 'five at a time: every contact, one id each, the pages it takes', ids.split(',').length === got.length && Number(pages) === Math.max(1, Math.ceil(got.length / 5)) && new Set(ids.split(',')).size === got.length, walk);
      const first = (await texts(s)).page;
      await s.tap('more');
      const second = await until(s, 'page', (t) => t !== first);
      check(where, 'a page after the cursor follows on, the ids distinct', !!second && second.split(',').every((id) => !first.split(',').includes(id)), `${first} then ${second}`);
      await s.type('search', 'an');
      const found = await until(s, 'names', (t) => t !== names);
      const expect = got.filter((n) => n.toLowerCase().includes('an'));
      check(where, 'a name substring, any case', expect.every((n) => (found ?? '').split('|').includes(n)), `${found} vs ${expect.join('|')}`);
      await s.type('search', 'ZZZ');
      const none = await until(s, 'walk', (t) => t === ' 1');
      check(where, 'no match: one empty page', none === ' 1', JSON.stringify(none));
      // D6's page measurements: each read's time off the main thread.
      const lines = (await s.logs())?.lines ?? [];
      console.log(`NOTE ${where}: ${lines.filter((l) => /page queryContacts .* in \d+ ms/.test(l)).map((l) => l.replace(/^.*page /, '')).join('; ')}`);
    } finally {
      await s.close();
    }

    // Limited (iOS 18): the store holds only what the person shared, none yet.
    // `simctl privacy`'s contacts-limited is the older "basic contact info",
    // which grants as full does, so the grant is written to the simulator's
    // own privacy database as limited (TCC's auth_value 3).
    privacy('reset', 'contacts');
    privacy('grant', 'contacts');
    const tcc = join(homedir(), 'Library/Developer/CoreSimulator/Devices', sim, 'data/Library/TCC/TCC.db');
    run('sqlite3', [tcc, `UPDATE access SET auth_value = 3 WHERE service = 'kTCCServiceAddressBook' AND client = '${fixture.id}'`], root);
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true granted/.test(t ?? ''));
      check(where, 'limited: granted, limited', status === 'true granted limited', status);
      const names = await until(s, 'names', (t) => t !== undefined && t !== null && !/^error/.test(t), 5000);
      check(where, 'limited: a read answers only what was shared', !SAMPLE.some((n) => (names ?? '').includes(n)), names);
    } finally {
      await s.close();
    }

    // Revoked: the read fails, asking nothing.
    privacy('revoke', 'contacts');
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true denied/.test(t ?? ''));
      check(where, 'revoked: denied', status === 'true denied none', status);
      const walk = await until(s, 'walk', (t) => /^error/.test(t ?? ''));
      check(where, 'a read fails NotAllowedError', walk === 'error NotAllowedError', walk);
      check(where, 'nothing was asked', !JSON.stringify(await s.logs()).includes('contacts: asked:'), 'the journal says the module asked');
    } finally {
      await s.close();
    }

    // Undetermined: the status says prompt; a read waits on the system's alert.
    privacy('reset', 'contacts');
    s = await fixture.open();
    try {
      const status = await until(s, 'status', (t) => /^true /.test(t ?? ''));
      check(where, 'reset: prompt, no access', status === 'true prompt none', status);
      await s.clock('+1000 real');
      check(where, 'a read waits on the person', !/\s\d+$/.test((await texts(s)).walk ?? ''), (await texts(s)).walk);
    } finally {
      await s.close();
    }
    privacy('reset', 'contacts');
    run('xcrun', ['simctl', 'uninstall', sim, fixture.id], root);
  } finally {
    fixture.remove();
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
