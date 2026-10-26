// LLP 1133.026 §12's acceptance drives on the Calendar's Show my activity,
// under the agent over packages/health/fixture: admission and tickets by the
// runner, the answers by the Apple implementation's fixture store. Drives 1-4,
// 6 and 7 need a Health store, so they run on iOS; drive 8 (no switch) on
// macOS and the web. Drive 5 (tools) is `contract/cli/tests/it/health.rs`.
//
// usage: bun examples/calendar/tests/activity.mjs <ios|macos|web> [--sim <udid>]
//   (EXACT_APP_DIR set to examples/calendar and the app built for that host:
//   host/apple/build.mjs [--ios] calendar-ios|calendar-macos, or host/web/build.mjs calendar-web)
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../../packages/pedometer/test/fixture.mjs';

const root = resolve(import.meta.dir, '../../..'), app = resolve(root, 'examples/calendar');
const host = process.argv[2] ?? 'ios', extra = process.argv.slice(3);
const { check, failed } = checker();
const run = Date.now().toString(36);

/** One launch on 12 October 2026, 15:00 UTC: each op's JSON reply. */
function drive(storage, permissions, ops, { reset = false } = {}) {
  const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'calendar', '--size', '1180x800', '--storage', `${storage}-${run}`, ...(reset ? ['--reset-storage'] : []),
    '--epoch', '2026-10-12T15:00:00Z', '--time-zone', 'UTC', ...flags, ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = tree => JSON.stringify(tree) ?? '';
/** Each date's steps as the month shows them. */
const steps = tree => Object.fromEntries((tree?.nodes ?? []).filter(n => n.props?.testId?.startsWith('steps-')).map(n => [n.props.testId.slice(6), n.props.text]));
/** The first and the last tree a launch answered, and the first reply `has` holds. */
const first = r => r.find(x => x?.nodes), last = r => r.filter(x => x?.nodes).at(-1);
const status = r => r.find(x => Array.isArray(x?.types));
const workouts = tree => [...text(tree).matchAll(/"workout-(w\d+)"/g)].map(m => m[1]);
const ticket = pending => (Array.isArray(pending) ? pending : []).find(p => p.device?.capability === 'permission');
const asked = ['clock settle', 'tree', 'tap show-my-activity', 'clock +100', 'state pending'];

if (host === 'ios') {
  // 1: the switch asks for steps and workouts; allowed, the month and the day read the fixture.
  {
    const r = drive('activity-1', {}, [...asked, 'tap @permission allow', 'clock settle', 'tree'], { reset: true });
    check(host, '1: the switch shows, nothing read', text(first(r)).includes('show-my-activity') && Object.keys(steps(first(r))).length === 0, text(first(r)).match(/my-activity[^"]*/g)?.join(','));
    const t = ticket(r.find(x => Array.isArray(x) && ticket(x)));
    check(host, '1: one ticket naming steps and workouts', JSON.stringify(t?.device?.args ?? {}).includes('health.steps') && JSON.stringify(t?.device?.args ?? {}).includes('health.workouts'), JSON.stringify(t));
    const s = steps(last(r));
    check(host, "1: 12 October reads the fixture's 8,412", s['2026-10-12'] === '8,412', JSON.stringify(s));
    check(host, '1: the gap day shows nothing, the zero day 0', !('2026-10-04' in s) && s['2026-10-05'] === '0', JSON.stringify(s));
    check(host, "1: the day's workouts", JSON.stringify(workouts(last(r))) === '["w7","w8"]' && text(last(r)).includes('2.9 km'), JSON.stringify(workouts(last(r))));
    check(host, '1: the quiet line, never a denial', text(last(r)).includes('my-activity-shared') && !text(last(r)).includes('my-activity-none'), text(last(r)).match(/my-activity-[a-z]+/g)?.join(','));
  }
  // 2: steps only: steps show, no workouts, both unknown, the quiet line.
  {
    const r = drive('activity-2', {}, [...asked, 'type @permission steps', 'clock settle', 'tree', 'state health'], { reset: true });
    const types = status(r)?.types ?? [];
    check(host, '2: steps show, the workouts are empty', steps(last(r))['2026-10-12'] === '8,412' && workouts(last(r)).length === 0, `${JSON.stringify(steps(last(r))['2026-10-12'])} ${JSON.stringify(workouts(last(r)))}`);
    check(host, '2: both read unknown, complete false', types.length === 2 && types.every(t => t.read === 'unknown' && t.complete === false), JSON.stringify(status(r)));
    check(host, '2: the quiet line, not a denial', text(last(r)).includes('my-activity-shared') && !text(last(r)).includes('my-activity-none'), text(last(r)).match(/my-activity-[a-z]+/g)?.join(','));
  }
  // 3: a limited window from 1 October.
  {
    const r = drive('activity-3', {}, [...asked, 'type @permission steps@2026-10-01,workouts', 'clock settle', 'tree'], { reset: true });
    const s = steps(last(r));
    check(host, '3: days before 1 October show nothing', !('2026-09-30' in s) && s['2026-10-01'] !== undefined, JSON.stringify(Object.keys(s).slice(0, 4)));
    check(host, '3: the line names the date', text(last(r)).includes('Calendar can see your activity from 1 October.'), text(last(r)).match(/my-activity-[a-z]+/g)?.join(','));
  }
  // 4: denied: both unknown, shared false, nothing shown, the Settings path; 6: another store.
  {
    const r = drive('activity-4', {}, [...asked, 'tap @permission deny', 'clock settle', 'tree', 'state health', 'state extensions'], { reset: true });
    check(host, '4: both read unknown', (status(r)?.types ?? []).every(t => t.read === 'unknown') && (status(r)?.types ?? []).length === 2, JSON.stringify(status(r)));
    check(host, '4: state.extensions keeps shared false', JSON.stringify(r).includes('"shared":false'), JSON.stringify(r.at(-1) ?? {}).slice(0, 400));
    check(host, '4: nothing shows; the month says so, with the path', Object.keys(steps(last(r))).length === 0 && workouts(last(r)).length === 0 && text(last(r)).includes('Settings › Health › Data Access & Devices'), text(last(r)).match(/my-activity-[a-z]+/g)?.join(','));
  }
  {
    const r = drive('activity-6', {}, [...asked, 'tap @permission allow', 'clock settle', 'prefer extension health.store="other-health.json"', 'clock settle', 'tree', 'logs'], { reset: true });
    check(host, '6: another store answers without a relaunch', JSON.stringify(workouts(last(r))) === '["w10","w7","w8"]', `${JSON.stringify(workouts(last(r)))} ${(r.at(-1)?.lines ?? []).filter(l => /health/.test(l)).slice(-8).join(' | ')}`);
  }
  // 7: a relaunch keeps the switch; the first frame has no steps until the data crate answers.
  {
    drive('activity-7', {}, [...asked, 'tap @permission allow', 'clock settle'], { reset: true });
    const r = drive('activity-7', { 'health.steps': 'granted', 'health.workouts': 'granted' }, ['tree', 'clock settle', 'tree']);
    check(host, '7: the first frame shows no steps', Object.keys(steps(first(r))).length === 0, JSON.stringify(steps(first(r))));
    check(host, '7: after settling, the steps', steps(last(r))['2026-10-12'] === '8,412', JSON.stringify(steps(last(r))));
  }
} else {
  // 8: no switch; the rest unchanged.
  const r = drive('activity-8', {}, ['clock settle', 'tree'], { reset: true });
  check(host, '8: no switch', !text(last(r)).includes('show-my-activity') && text(last(r)).includes('inspector-date'), text(last(r)).match(/my-activity[^"]*/g)?.join(','));
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
