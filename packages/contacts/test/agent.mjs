// @exact/contacts' conformance suite under the agent (LLP 1133.018 §10, §14
// stage 0): the fixture app built for a host and driven with the
// implementation's agent path over packages/contacts/fixture/contacts.json.
// The queries are held equal to the suite's own reading of that file: a
// case-insensitive substring of a name, an email or a number, in the
// fixture's order, `limit` at a time.
//
// The web has no store: there the status answers `available` false, a
// query fails NotSupportedError, and the picker answers as a browser with
// one does. A tool-origin pick failing `person-required` is the runner's
// (contract/cli/tests/it/contacts_calendar.rs).
//
// usage: bun packages/contacts/test/agent.mjs <web|macos|ios> [--sim <udid>] [--keep]
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { build, checker, drive, makeFixture, root, texts, ticket } from './fixture.mjs';

const argv = process.argv.slice(2), host = argv[0] ?? 'macos';
const sim = argv.includes('--sim') ? ['--sim', argv[argv.indexOf('--sim') + 1]] : [];
const { check, failed } = checker();
const book = JSON.parse(readFileSync(resolve(root, 'packages/contacts/fixture/contacts.json'), 'utf8'));

/** The suite's own answer: the ids matching `text`, among `among`. */
const expected = (text, among = book.contacts.map(c => c.id)) => book.contacts.filter(c => among.includes(c.id)).filter(c => {
  const t = text.toLowerCase();
  return !t || [...(c.name ?? []), ...(c.email ?? []), ...(c.tel ?? [])].some(v => v.toLowerCase().includes(t));
}).map(c => c.id);
const walked = (ids, size = 5) => `${ids.join(',')} ${Math.max(1, Math.ceil(ids.length / size))}`;

const fixture = makeFixture('contacts');
try {
  build(fixture, host, sim);
  const flags = state => ['--permission', `contacts=${state}`, ...sim];
  if (host === 'web') {
    const r = drive(fixture, host, flags('prompt'), ['clock settle', 'tree', 'tap pick', 'clock +100', 'state pending', 'type @contacts c1', 'clock settle', 'tree']);
    const t = texts(r[1]), after = texts(r[7]);
    check(host, 'no store: the status says so', t.status === 'false unknown unknown', t.status);
    check(host, 'no store: a query fails NotSupportedError', t.walk === 'error NotSupportedError', t.walk);
    check(host, "the picker's properties are a browser's with one", t.properties === 'address,email,icon,name,tel', t.properties);
    check(host, 'a pick answers the fixture contact, icon copied', ticket(r[4], 'contacts') && /^Ana Lima app:\/tmp\/picked\/contacts\/\S+$/.test(after.chosen), `${JSON.stringify(r[4])} ${after.chosen}`);
  } else {
    // Granted: every query walks to the suite's answer, in its order.
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'tree', 'type search ex', 'clock settle', 'tree', 'type search 555 01', 'clock settle', 'tree',
        'type search ZZZ', 'clock settle', 'tree', 'tap more', 'clock settle', 'tree', 'tap more', 'clock settle', 'tree']);
      const at = i => texts(r[i]);
      check(host, 'granted, full', at(1).status === 'true granted full', at(1).status);
      check(host, 'everyone, five at a time', at(1).walk === walked(expected('')), `${at(1).walk} vs ${walked(expected(''))}`);
      check(host, 'a name or email substring, any case', at(4).walk === walked(expected('ex')), `${at(4).walk} vs ${walked(expected('ex'))}`);
      check(host, 'a number substring', at(7).walk === walked(expected('555 01')), `${at(7).walk} vs ${walked(expected('555 01'))}`);
      check(host, 'no match: one empty page', at(10).walk === ' 1', JSON.stringify(at(10).walk));
      const all = expected('');
      check(host, 'key cursors: the pages follow on, the last with no cursor', at(1).page === all.slice(0, 4).join(',') && at(13).page === all.slice(4, 8).join(',') && at(16).page === all.slice(8, 12).join(',') && at(16).next === '',
        [at(1).page, at(13).page, at(16).page, at(16).next].join(' | '));
    }
    // Asked: each read under prompt holds a permission ticket (the two
    // readers' are @4 and @5); allowing one grants, and the other read answers too.
    {
      const r = drive(fixture, host, flags('prompt'), ['clock settle', 'state pending', 'tap @4 allow', 'clock settle', 'tree', 'state pending']);
      const asked = ticket(r[1], 'permission');
      check(host, 'a read under prompt asks', asked?.device?.args?.capability === 'contacts', JSON.stringify(r[1]));
      check(host, 'allowed, both reads answer', texts(r[4]).walk === walked(expected('')) && texts(r[4]).page === expected('').slice(0, 4).join(',') && texts(r[4]).status === 'true granted full' && !ticket(r[5], 'permission'), `${texts(r[4]).walk} ${texts(r[4]).page} ${JSON.stringify(r[5])}`);
    }
    // Denied: the read fails; the pick needs no access.
    {
      const r = drive(fixture, host, flags('denied'), ['clock settle', 'tree', 'tap pick', 'clock +100', 'state pending', 'type @contacts c1,c4', 'clock settle', 'tree', 'logs',
        'tap pick-one', 'clock +100', 'type @contacts c1,c2']);
      const t = texts(r[1]), after = texts(r[7]);
      check(host, 'denied: a read fails NotAllowedError', t.walk === 'error NotAllowedError' && t.status === 'true denied none', `${t.walk} ${t.status}`);
      check(host, 'denied: the pick still asks the person', ticket(r[4], 'contacts')?.device?.args?.options?.multiple === true, JSON.stringify(r[4]));
      const icons = (after.chosen ?? '').match(/app:\/tmp\/picked\/contacts\/\S+/g) ?? [];
      check(host, 'the pick answers the two, icons copied', /^Ana Lima app:\/tmp\/picked\/contacts\/\S+; Dana Kowalski app:\/tmp\/picked\/contacts\/\S+$/.test(after.chosen) && icons.length === 2, after.chosen);
      check(host, 'two ids for a single pick are refused by the driver', r.length < 12 || /not|refus|one id/i.test(JSON.stringify(r[11])), JSON.stringify(r[11]));
    }
    // Limited (D5): reads answer the shared four; on iOS the access picker adds one.
    {
      const ops = ['clock settle', 'prefer extension contacts.contactsStatus={"access":"limited"}', 'clock settle', 'tree', 'tap share', 'clock +100', 'state pending'];
      const ios = host === 'ios' ? ['type @share c7', 'clock settle', 'tree'] : ['clock settle', 'tree'];
      const r = drive(fixture, host, flags('granted'), [...ops, ...ios]);
      const t = texts(r[3]), shared = texts(r[r.length - 1]);
      check(host, 'limited: the status says so', t.status === 'true granted limited', t.status);
      check(host, "limited: reads answer the fixture's limited four", t.walk === walked(expected('', book.limited)), `${t.walk} vs ${walked(expected('', book.limited))}`);
      if (host === 'ios') {
        check(host, 'limited: the access picker holds a share ticket', !!ticket(r[6], 'share'), JSON.stringify(r[6]));
        check(host, 'limited: a contact shared joins the reads', shared.walk === walked(expected('', [...book.limited, 'c7'])), shared.walk);
      } else {
        check(host, 'the access picker is iOS only', shared['share-error'] === 'NotSupportedError' && !ticket(r[6], 'share'), `${shared['share-error']} ${JSON.stringify(r[6])}`);
      }
    }
    // A storm of changes costs at most two rounds of reads (D7).
    {
      const r = drive(fixture, host, flags('granted'), ['clock settle', 'prefer extension contacts.notify=25', 'clock settle', 'clock +3000', 'clock settle', 'tree', 'logs']);
      const lines = r[6]?.lines ?? [], from = lines.findIndex(l => /contacts: notify 25/.test(l));
      const rounds = lines.slice(from).filter(l => /contacts: announced/.test(l)).length;
      check(host, 'twenty-five changes, at most two rounds', from >= 0 && rounds >= 1 && rounds <= 2, `${rounds} rounds`);
      check(host, 'after the storm the reads still answer', texts(r[5]).walk === walked(expected('')), texts(r[5]).walk);
    }
  }
} finally {
  if (!argv.includes('--keep')) fixture.remove();
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
