// LLP 1133.024 §20 stage 5's drive on the Calendar example under the agent:
// the guests picked with their phones, "Email the Invitation…" writing the
// item's `.ics` and holding a `compose` ticket with it attached (or, where
// the host's mail carries no attachment, the share sheet's), and "Text the
// Guests…" holding one addressed to their phones; each ticket's tap is the
// answer. The journal names counts and types, never an address.
//
// usage: bun examples/calendar/tests/compose.mjs <web|macos|ios|android> [--sim <udid>]
//   (the app built for that host, as tests/drives.mjs says)
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../../packages/camera/test/fixture.mjs';

const root = resolve(import.meta.dir, '../../..'), app = resolve(root, 'examples/calendar');
const host = process.argv[2] ?? 'macos', extra = process.argv.slice(3);
const { check, failed } = checker();

/** One launch: each op's JSON reply, by its op. */
function drive(storage, ops) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'calendar', '--size', '1180x800', '--storage', storage, '--reset-storage',
    '--epoch', '2026-09-16T12:00:00Z', '--time-zone', 'UTC', '--permission', 'contacts=prompt', ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = tree => JSON.stringify(tree) ?? '';
const ticket = (pending, capability) => Array.isArray(pending) ? pending.find(p => p.device?.capability === capability) : undefined;
const open = ['clock settle', 'tap agenda-item-sample-2026-09-5', 'clock settle', 'tap invite', 'clock +100', 'type @contacts c2,c5', 'clock settle'];
const web = host === 'web';

const r = drive(`calendar-compose-${Date.now().toString(36)}`, [...open, 'state composer', 'state phones', 'tree',
  'tap email-invitation', 'clock settle', 'state invitation', 'state pending',
  ...(web ? [] : ['tap @compose sent', 'clock settle', 'state composed']),
  ...(web ? [] : ['tap text-guests', 'clock +100', 'state pending', 'tap @compose cancelled', 'clock settle', 'state composed']), 'logs']);
const at = (i) => r[open.length + i];
const [composer, phones, tree] = [at(0), at(1), at(2)];
check(host, 'the fact', composer?.email === true && composer?.emailAttachments === !web && composer?.message === !web, JSON.stringify(composer));
check(host, "the guests' phones are kept", JSON.stringify(phones?.tels) === '["+1 415 555 0102","+34 91 555 0105"]', JSON.stringify(phones));
check(host, 'the editor offers both composers where the host has them', text(tree).includes('email-invitation') && text(tree).includes('text-guests') === !web, '');
const invitation = at(5);
check(host, "the invitation is the item's .ics in app:/tmp", /^app:\/tmp\/invite-.+\.ics$/.test(invitation?.path ?? ''), JSON.stringify(invitation));
const held = at(6);
if (web) {
  // The web's mail carries no attachment: the file goes to the share sheet.
  check(host, "the web's mail takes no file, so the share sheet does", !ticket(held, 'compose') && JSON.stringify(ticket(held, 'share')?.device?.args?.files) === JSON.stringify([invitation?.path]), JSON.stringify(held));
} else {
  const mail = ticket(held, 'compose');
  const o = mail?.device?.args?.options ?? {};
  check(host, 'Email holds a compose ticket to the guests with the .ics', JSON.stringify(o.to) === '["ben@example.com","eli@example.com"]' && JSON.stringify(o.attachments) === JSON.stringify([invitation?.path]) && typeof o.subject === 'string' && o.subject.length > 0, JSON.stringify(mail));
  check(host, 'the ticket answers sent', at(9)?.action === 'sent', JSON.stringify(at(9)));
  const message = ticket(at(12), 'compose');
  check(host, 'Text holds a compose ticket to their phones', JSON.stringify(message?.device?.args?.options?.to) === '["+1 415 555 0102","+34 91 555 0105"]', JSON.stringify(message));
  check(host, 'the ticket answers cancelled', at(15)?.action === 'cancelled', JSON.stringify(at(15)));
  const lines = (r.at(-1)?.lines ?? []).join('\n');
  check(host, 'the journal counts, and names no address', /compose: email sent, 2 recipients, 1 attachments \(text\/calendar\)/.test(lines) && !lines.includes('example.com') && !lines.includes('555'), lines.split('\n').filter(l => l.includes('compose')).join(' | '));
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
