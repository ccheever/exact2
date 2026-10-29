// LLP 1133.008 §10's acceptance drives 1-5 and 7 under the agent, on the photo
// editor, on one host: the agent's cameras and picture, admission by the
// runner, leases taken by the viewfinder's displays. Drive 6 needs a tool
// door no host has; contract/cli/tests/it/camera.rs covers it in the runner.
// Each drive is one launch from a fresh store.
//
// usage: bun packages/camera/test/drives.mjs <web|macos|ios> [--sim <udid>]
//   (the app built for that host first: host/web/build.mjs photo-editor, or
//   host/apple/build.mjs [--ios] photo-editor-apple)
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from './fixture.mjs';

const here = import.meta.dir, root = resolve(here, '../../..');
const host = process.argv[2] ?? 'web', extra = process.argv.slice(3);
const sha = file => createHash('sha256').update(readFileSync(file)).digest('hex');
const CARD = sha(resolve(here, '../fixture/card.jpg')), SAMPLE = sha(resolve(root, 'apps/photo-editor/assets/sample.jpg'));
const { check, failed } = checker();

/** One launch: each op's JSON reply, by its op. */
function drive(permission, ops) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'photo-editor', '--storage', 'camera-drives', '--reset-storage', '--permission', `camera=${permission}`, ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-2000), r.stderr.slice(-2000));
  return replies;
}
const logsOf = replies => replies.find(r => Array.isArray(r?.lines))?.lines ?? [];
const took = logs => logs.map(l => /camera: took (\S+) .*sha256 ([0-9a-f]{64})/.exec(l)).filter(Boolean).map(m => ({ path: m[1], sha: m[2] }));
const open = ['clock settle', 'tap take-photo', 'clock settle'];

// 1 and 2: the ticket, the card in the viewfinder, the photo, the edit.
{
  // The ticket's number is the launch's, the same in every launch: a first launch reads it.
  const ticket = drive('prompt', [...open, 'state pending'])[3]?.[0]?.ticket;
  const r = drive('prompt', [...open, 'state pending', `tap @${ticket} allow`, 'clock settle', 'state live', 'state extensions',
    'tap shutter', 'clock settle', 'state shot', 'state live.readyState', 'state photo', 'tap rotate', 'clock +500', 'state edit', 'logs', 'state extensions']);
  const pending = r[3], live = r[6], views = r[7]?.camera?.views, shot = r[10], logs = logsOf(r);
  const shown = r[7]?.['media-capture']?.implementation?.videoTracks, photos = r[17]?.camera?.implementation?.photos;
  check(host, '1: getUserMedia waits on a camera ticket', pending?.[0]?.device?.args?.capability === 'camera', JSON.stringify(pending));
  check(host, "1: the agent's back camera, live and unmuted", live?.label === 'Agent back camera' && live?.readyState === 'live' && live?.muted === false, JSON.stringify(live));
  if (views) check(host, '1: the viewfinder is displayed by the app', views.some(v => v.displayed && v.origin === 'app' && v.bound?.srcObject === 'app'), JSON.stringify(views));
  check(host, '1: state.extensions lists the track live, leased, its viewfinder an app consumer',
    shown?.length === 1 && shown[0].readyState === 'live' && shown[0].muted === false && shown[0].lease === true && shown[0].consumers?.join() === 'app', JSON.stringify(shown));
  check(host, "2: state.extensions lists the photo with the card's SHA-256",
    photos?.length === 1 && photos[0].path === 'app:/tmp/camera/1.jpg' && photos[0].sha256 === CARD, JSON.stringify(photos));
  check(host, '1: the viewfinder takes the lease with the app origin', logs.some(l => /takes the lease \(app origin\)/.test(l)), logs.filter(l => /lease/.test(l)).join(' | '));
  check(host, "2: the photo is the card, at app:/tmp/camera/1.jpg", shot?.path === 'app:/tmp/camera/1.jpg' && took(logs)[0]?.sha === CARD, `${JSON.stringify(shot)} ${JSON.stringify(took(logs))}`);
  check(host, '2: the editor shows it and the track ended', r[12] === 'app:/tmp/camera/1.jpg' && r[11] === 'ended', `${r[12]} ${r[11]}`);
  check(host, '2: the edit turns it', /rotation 90\.0°/.test(r[15] ?? ''), r[15]);
}
// 3: the picture a drive names.
{
  const r = drive('granted', ['clock settle', 'prefer extension camera.picture={"path":"assets/sample.jpg"}', 'tap take-photo', 'clock settle', 'tap shutter', 'clock settle', 'logs']);
  check(host, "3: the photo is the named picture's bytes", took(logsOf(r))[0]?.sha === SAMPLE, JSON.stringify(took(logsOf(r))));
}
// 4: flip; the viewfinder mirrors a front camera, the photo does not.
{
  const r = drive('granted', [...open, 'tap flip', 'clock settle', 'state live.settings.facingMode', 'tap shutter', 'clock settle', 'logs']);
  check(host, '4: flip moves the track to the front camera', r[5] === 'user', JSON.stringify(r[5]));
  check(host, "4: the front camera's photo is the card unmirrored", took(logsOf(r))[0]?.sha === CARD, JSON.stringify(took(logsOf(r))));
}
// 5: a sheet dismissed lets the camera go; Take Photo shows the same track again.
{
  const r = drive('granted', [...open, 'state track', 'back', 'clock settle', 'state live', 'state extensions', 'tap take-photo', 'clock settle', 'state track', 'state live', 'state pending', 'logs']);
  const idle = r[6], again = r[11], logs = logsOf(r), held = r[7]?.['media-capture']?.implementation?.videoTracks?.[0];
  check(host, '5: dismissed, the track is muted, idle', idle?.muted === true && idle?.muteReason === 'idle' && idle?.readyState === 'live', JSON.stringify(idle));
  check(host, '5: state.extensions shows no lease and no consumer', held?.lease === false && held?.muteReason === 'idle' && held?.consumers?.length === 0, JSON.stringify(held));
  check(host, '5: shown again, the same track unmutes with no ticket', r[10] === r[3] && again?.muted === false && Array.isArray(r[12]) && r[12].length === 0, `${r[3]} ${r[10]} ${JSON.stringify(again)} ${JSON.stringify(r[12])}`);
  check(host, '5: no second getUserMedia', logs.filter(l => /opened with/.test(l)).length === 1, logs.filter(l => /opened with/.test(l)).join(' | '));
}
// 7: denied: no ticket, the denied line, Open Settings.
{
  const r = drive('denied', ['clock settle', 'state pending', 'tree']);
  const tree = JSON.stringify(r[2]);
  check(host, '7: denied asks nothing', Array.isArray(r[1]) && r[1].length === 0, JSON.stringify(r[1]));
  check(host, '7: the denied line and Open Settings, no Take Photo', tree.includes('camera-denied') && tree.includes('open-settings') && !tree.includes('take-photo'), tree.slice(0, 400));
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
