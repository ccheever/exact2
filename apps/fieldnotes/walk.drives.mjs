// LLP 1133.026 §12's walk drives on Fieldnotes under the agent: the
// pedometer's and altimeter's declared substitutes, their `motion-fitness`
// ticket and tracks. Drives 1-5 need a target with the sensors, so they run
// on iOS; drive 6 (no walk button) on macOS and the web.
//
// usage: bun apps/fieldnotes/walk.drives.mjs <ios|macos|web> [--sim <udid>]
//   (the app built for that host: host/apple/build.mjs [--ios] fieldnotes-apple,
//   or host/web/build.mjs fieldnotes)
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../packages/pedometer/test/fixture.mjs';

const root = resolve(import.meta.dir, '../..');
const host = process.argv[2] ?? 'ios', extra = process.argv.slice(3);
const { check, failed } = checker();
const EPOCH = Date.parse('2026-10-25T09:00:00Z');

/** One launch: each op's JSON reply, in order. */
function drive(storage, permissions, ops, { reset = false, epoch = EPOCH } = {}) {
  const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'fieldnotes', '--storage', storage, ...(reset ? ['--reset-storage'] : []),
    '--epoch', new Date(epoch).toISOString(), '--time-zone', 'UTC', ...flags, ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = tree => JSON.stringify(tree) ?? '';
const line = tree => text(tree).match(/Walking · [^"]*/)?.[0] ?? '';
const holds = pending => (Array.isArray(pending) ? pending : []).filter(p => p.device);
const track = (name, steps) => `prefer extension ${name}=${JSON.stringify(steps)}`;
const run = `drives-${Date.now().toString(36)}`;

if (host === 'ios') {
  // 1, 2: one ticket for both modules; 42 minutes of tracks; the summary, kept nowhere.
  {
    const r = drive(`fn-walk-1-${run}`, {}, ['clock settle', 'tree', 'tap start-walk', 'clock +100', 'state pending', 'tap @permission allow', 'clock settle',
      track('pedometer.pedometer', [{ at: 0, steps: 0 }, { at: 2_400_000, steps: 3120 }]),
      track('altimeter.altimeter', [{ at: 0, relativeAltitude: 0 }, { at: 2_400_000, relativeAltitude: 86.2 }]),
      'clock +2520000', 'clock settle', 'tree', 'tap end-walk', 'clock settle', 'tree', 'state'], { reset: true });
    check(host, '1: Start walk shows', text(r[1]).includes('start-walk'), text(r[1]).slice(0, 300));
    const asked = holds(r[4]);
    check(host, '1: one motion-fitness ticket for both requests', asked.length === 1 && asked[0].device.capability === 'permission' && JSON.stringify(asked[0].device.args).includes('motion-fitness'), JSON.stringify(r[4]));
    check(host, '1: the line reads 42 min, 3,120 steps and 86 m', line(r[11]) === 'Walking · 42 min · 3,120 steps · ↑ 86 m', line(r[11]));
    check(host, '2: End walk shows the summary', text(r[14]).includes('42 min · 3,120 steps · ↑ 86 m') && !line(r[14]), line(r[14]));
    // The summary's own slot holds its text until the app closes; nothing else of the app's holds the
    // count (`extensions` is the drive's own track).
    const { extensions: _, ...app } = r[15] ?? {};
    const all = JSON.stringify(app);
    check(host, '2: nothing but the summary holds the count', r[15] && !all.includes('3120') && (all.match(/3,120/g) ?? []).length <= 1, all.match(/.{80}3,?120.{40}/g)?.join(' | '));
    const again = drive(`fn-walk-1-${run}`, { 'motion-fitness': 'granted' }, ['clock settle', 'tree']);
    check(host, '2: a relaunch shows no summary', !text(again[1]).includes('3,120') && text(again[1]).includes('start-walk'), text(again[1]).match(/walk[^"]*/g)?.join(','));
  }
  // 3: a relaunch mid-walk ten minutes on, the count moved by a reboot: "since", the duration kept.
  {
    drive(`fn-walk-3-${run}`, { 'motion-fitness': 'granted' }, ['clock settle', 'tap start-walk', 'clock settle', 'clock +60000'], { reset: true });
    const later = EPOCH + 600_000, moved = later + 60_000;
    const r = drive(`fn-walk-3-${run}`, { 'motion-fitness': 'granted' }, [track('pedometer.pedometer', [{ at: 0, steps: 12, since: moved }]), 'clock +2000', 'clock settle', 'tree'], { epoch: later });
    check(host, '3: the steps say since when; the walk is ten minutes old', /^Walking · 10 min · 12 steps since 9:11 AM/.test(line(r[3])), line(r[3]));
  }
  // 4: denied: the denied line, nothing asked; notes as before.
  {
    const r = drive(`fn-walk-4-${run}`, { 'motion-fitness': 'denied' }, ['clock settle', 'tap start-walk', 'clock settle', 'state pending', 'tree'], { reset: true });
    check(host, '4: nothing asked', holds(r[3]).length === 0, JSON.stringify(r[3]));
    check(host, '4: the denied line, no walk', text(r[4]).includes('walk-denied') && !line(r[4]) && text(r[4]).includes('note-count'), text(r[4]).match(/walk[^"]*/g)?.join(','));
  }
  // 5: ten hidden minutes: the line shows the steps the track reached.
  {
    const r = drive(`fn-walk-5-${run}`, { 'motion-fitness': 'granted' }, ['clock settle', track('pedometer.pedometer', [{ at: 0, steps: 0 }, { at: 60_000, steps: 100 }, { at: 590_000, steps: 900 }]),
      'tap start-walk', 'clock settle', 'clock +61000', 'prefer visibility-state hidden', 'clock +600000', 'prefer visibility-state visible', 'clock +2000', 'clock settle', 'tree'], { reset: true });
    check(host, '5: after the hidden stretch the line holds the steps reached', line(r[10]).includes('900 steps'), line(r[10]));
  }
} else {
  // 6: no walk button; the notes as before.
  const r = drive(`fn-walk-6-${run}`, {}, ['clock settle', 'tree', 'state'], { reset: true });
  check(host, '6: no walk button', !text(r[1]).includes('start-walk') && text(r[1]).includes('note-count'), text(r[1]).match(/walk[^"]*/g)?.join(','));
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
