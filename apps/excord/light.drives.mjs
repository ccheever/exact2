// LLP 1133.037 §12.1's mention-light drives on Excord under the agent's substitute: drives 1, 2 and 5 on macOS (the
// attached list), 3 on the web (the chooser, then the remembered light), 4 and 6 where HID is absent (Android, iOS).
// Drive 7, a tool's write, is the runner's test (contract/cli/tests/it/devices.rs); drive 8 needs a person and a
// blink(1).
//
// usage: bun apps/excord/light.drives.mjs <macos|web|ios|android|linux> [agent flags]
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dir, '../..');
const host = process.argv[2] ?? 'macos', extra = process.argv.slice(3);
let failures = 0;
const check = (what, ok, detail = '') => { if (!ok) failures++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} ${what}${ok ? '' : `: ${detail}`}`); };

function drive(storage, ops, { reset = true } = {}) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'excord', '--storage', storage, ...(reset ? ['--reset-storage'] : []), ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length || process.env.EXACT_DRIVE_DEBUG) console.log(ops.map((o, i) => `${i} ${o.slice(0, 60)} → ${JSON.stringify(replies[i])?.slice(0, 400)}`).join('\n'), r.stderr.slice(-3000));
  return replies;
}
const text = v => JSON.stringify(v) ?? '';
const prefer = (key, value) => `prefer extension ${key}=${JSON.stringify(value)}`;
const light = (connected = true) => [{ id: 'b1', vendorId: 10168, productId: 493, productName: 'blink(1) mk3', connected, collections: [{ usagePage: 65280, usage: 1, featureReports: [{ reportId: 1 }] }] }];
const reports = state => (state?.writes ?? state?.value?.writes ?? []).filter(w => w.name === 'sendFeatureReport').map(w => Buffer.from(w.data, 'base64').toString('hex'));
const RED = '63ff0000001e00', OFF = '630000000' + '01e00';
const run = Date.now().toString(36);

if (host === 'ios' || host === 'android' || host === 'linux') {
  const r = drive(`light-${run}`, ['clock settle', prefer('hid.devices', light()), 'clock settle', 'tap account', 'clock settle', 'tree']);
  check('4, 6: no Mention Light row where HID is absent', !/light-switch|choose-light|light-missing/.test(text(r.at(-1))), text(r.at(-1)).match(/light[^"]*/g)?.join());
} else {
  const select = host === 'web' ? ['tap account', 'clock settle', 'tap choose-light', 'clock +50', 'state pending', 'tap @chooser b1', 'clock settle'] : [];
  // The fixture's world: the reader signs in; a mention arrives in a direct message, then is read.
  const r = drive(`light-${run}`, ['clock settle', prefer('hid.devices', light()), 'clock settle', ...select,
    'tap account', 'clock settle', 'tree', ...(host === 'web' ? [] : ['tap light-switch', 'clock settle']), 'state mentionTotal', 'state extensions.hid']);
  if (host === 'web') check('3: Choose Light… holds a chooser ticket', text(r[7]).includes('chooser'), text(r[7]));
  const menu = r.at(host === 'web' ? -3 : -5), total = r.at(-2), hid = r.at(-1);
  check('1: the account menu lists the Mention Light', text(menu).includes('light-switch'), text(menu).match(/light[^"]*/g)?.join());
  const mentions = Number(total?.value ?? total);
  const sent = reports(hid);
  check(`1: with ${mentions} mention(s) waiting, feature report 1 is red over 300 ms`, mentions > 0 ? sent.at(-1) === RED : sent.at(-1) === OFF, text(sent));
  if (host === 'web') {
    const again = drive(`light-3-${run}`, ['clock settle', prefer('hid.devices', light()), 'clock settle', ...select, 'reload', 'clock settle',
      prefer('hid.devices', light()), 'clock settle', 'state pending', 'tap account', 'clock settle', 'tree']);
    check('3: a reload finds the light remembered, no ticket', !text(again.at(-4)).includes('chooser') && text(again.at(-1)).includes('light-switch'), text(again.at(-4)) + (text(again.at(-1)).match(/light[^"]*/g)?.join() ?? ''));
  } else {
    // 5: unplugged, the row reads Not Connected; plugged in again, the light is set again.
    const u = drive(`light-${run}`, ['clock settle', prefer('hid.devices', light()), 'clock settle', 'tap account', 'clock settle', 'tap light-switch', 'clock settle',
      prefer('hid.devices', light(false)), 'clock settle', 'tap account', 'clock settle', 'tree', prefer('hid.devices', light()), 'clock settle', 'state extensions.hid']);
    check('5: unplugged, the row reads Not Connected', text(u[11]).includes('light-missing'), text(u[11]).match(/light[^"]*/g)?.join());
    check('5: plugged in again, the light is set again', reports(u.at(-1)).length >= 2 && reports(u.at(-1)).at(-1) === RED, text(reports(u.at(-1))));
  }
}
console.log(`${failures} failed`);
process.exit(failures ? 1 : 0);
