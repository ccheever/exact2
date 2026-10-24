// LLP 1133.019 §11's acceptance drives 1-6 and 8 under the agent, on the
// photo editor, on one host: the agent's library (packages/photos/fixture),
// admission and tickets by the runner, pictures through the host's raster
// loader. Drive 7 needs a tool door no host has (LLP 1111);
// contract/cli/tests/it/photos.rs covers it in the runner. Drives 5 and 6
// run on iOS alone (macOS has no limited picker); on the web every drive
// but 8 is skipped, since the library is absent there. Each drive is one
// launch from a fresh store.
//
// usage: bun packages/photos/test/drives.mjs <web|macos|ios> [--sim <udid>]
//   (the app built for that host first: host/web/build.mjs photo-editor, or
//   host/apple/build.mjs [--ios] photo-editor-apple)
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';

const here = import.meta.dir, root = resolve(here, '../../..');
const host = process.argv[2] ?? 'web', extra = process.argv.slice(3);
const sha = file => createHash('sha256').update(readFileSync(file)).digest('hex');
const AGENT = n => sha(resolve(here, `../fixture/agent-${n}.jpg`));
const shots = resolve(root, 'target/photos-drives', host);
mkdirSync(shots, { recursive: true });
const { check, failed } = checker();

/** One launch: each op's JSON reply, by its op. */
function drive(permissions, ops) {
  const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'photo-editor', '--storage', 'photos-drives', '--reset-storage', ...flags, ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' }, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const logsOf = replies => replies.find(r => Array.isArray(r?.lines))?.lines ?? [];
const added = logs => logs.map(l => /photos: added (\S+) from (\S+) sha256 ([0-9a-f]{64}) (\d+)x(\d+) (\S+)/.exec(l)).filter(Boolean)
  .map(m => ({ id: m[1], path: m[2], sha: m[3], width: +m[4], height: +m[5], type: m[6] }));
const copied = logs => logs.map(l => /photos: copied (\S+) to (\S+) sha256 ([0-9a-f]{64})/.exec(l)).filter(Boolean).map(m => ({ id: m[1], path: m[2], sha: m[3] }));
const ids = tree => JSON.stringify(tree);
const ticket = pending => Array.isArray(pending) ? pending.find(p => p.device) : undefined;
const strip = tree => [...(ids(tree) ?? '').matchAll(/"recent-(\d+)"/g)].map(m => +m[1]);
// The camera's photo, as LLP 1133.008 §10 drives 1 and 2 take it, then turned.
const shoot = ['clock settle', 'tap take-photo', 'clock settle', 'tap shutter', 'clock settle', 'tap rotate', 'clock +500'];

if (host !== 'web') {
  // 1 and 2: the add's ticket, the turned card saved; again with no ticket.
  {
    const r = drive({ camera: 'granted', 'photos-add': 'prompt', photos: 'prompt' }, [...shoot, 'state photoEdit', 'tap save', 'clock data', 'state pending', 'tap @photos.addToPhotoLibrary allow', 'clock settle',
      'state saveLine', 'state extensions', 'state saved', 'tap save', 'clock data', 'state pending', 'clock settle', 'state saveLine', 'logs']);
    const pending = r[10], logs = logsOf(r), adds = added(logs);
    check(host, "1: the editor reports the turn in the photo's terms", JSON.parse(r[7] ?? '{}').rotate === 90, r[7]);
    check(host, '1: Save waits on a photos-add ticket', ticket(pending)?.device?.args?.capability === 'photos-add', JSON.stringify(pending));
    check(host, '1: "Saved to Photos"', r[13] === 'Saved to Photos', JSON.stringify(r[13]));
    // The agent's card is 1280×960: turned, 960×1280.
    check(host, '1: the add is the card turned, JPEG, its metadata kept', adds[0]?.width === 960 && adds[0]?.height === 1280 && adds[0]?.type === 'image/jpeg' && /^app:\/tmp\/edits\/1/.test(adds[0]?.path) && r[15]?.metadata === 'keep', `${JSON.stringify(adds)} ${JSON.stringify(r[15])}`);
    check(host, '1: photos-add granted, the read still prompt', r[14]?.photos?.permissions?.['photos-add'] === 'granted' && r[14]?.photos?.permissions?.photos === 'prompt', JSON.stringify(r[14]?.photos?.permissions));
    check(host, '2: Save again asks nothing and adds again', !ticket(r[18]) && adds.length === 2 && r[20] === 'Saved to Photos', `${JSON.stringify(r[18])} ${adds.length}`);
  }
  // 3: adding denied: the denied line, no Save, no add.
  {
    const r = drive({ camera: 'granted', 'photos-add': 'denied' }, [...shoot, 'tree', 'state pending', 'logs']);
    const tree = ids(r[7]);
    check(host, '3: the denied line and Open Settings, no Save', tree.includes('save-denied') && tree.includes('open-photo-settings') && !tree.includes('"save"'), tree.slice(0, 300));
    check(host, '3: no ticket, no add', !ticket(r[8]) && added(logsOf(r)).length === 0, JSON.stringify(r[8]));
  }
  // 4: Show, allow: six pictures; the first opened as a copy whose bytes are the fixture's.
  {
    const r = drive({ photos: 'prompt' }, ['clock settle', 'state pending', 'tap show-recents', 'clock +100', 'state pending', 'tap @photos.photoAssets allow', 'clock settle',
      `screenshot ${shots}/4-recents.png`, 'tree', 'state extensions', 'tap recent-1', 'clock settle', 'state photo', 'logs']);
    const logs = logsOf(r);
    check(host, '4: nothing is asked before Show', Array.isArray(r[1]) && r[1].length === 0, JSON.stringify(r[1]));
    check(host, '4: Show reads, behind a photos ticket', ticket(r[4])?.device?.args?.capability === 'photos', JSON.stringify(r[4]));
    check(host, '4: six pictures in the strip', strip(r[8]).length === 6, JSON.stringify(strip(r[8])));
    check(host, '4: each picture admitted, none refused', (r[9]?.photos?.pictures?.admitted ?? 0) >= 6 && !r[9]?.photos?.pictures?.refused, JSON.stringify(r[9]?.photos?.pictures));
    check(host, "4: recent-1 opens agent-1's copy, byte for byte", r[12] === 'app:/tmp/photos/1.jpg' && copied(logs)[0]?.id === 'agent-1' && copied(logs)[0]?.sha === AGENT(1), `${r[12]} ${JSON.stringify(copied(logs))}`);
  }
  if (host === 'ios') {
    // 5: limited: three, the caption and Manage; a new selection of two; an edit saved is a third.
    {
      const r = drive({ photos: 'prompt', 'photos-add': 'prompt', camera: 'granted' }, ['clock settle', 'tap show-recents', 'clock +100', 'tap @photos.photoAssets limited', 'clock settle', 'tree',
        'tap manage', 'clock +100', 'state pending', 'type @managed agent-2,agent-5', 'clock settle', 'tree',
        ...shoot, 'tap save', 'clock data', 'state pending', 'clock settle', 'tree', `screenshot ${shots}/5-limited.png`, 'logs', 'state recent']);
      check(host, '5: limited shows three, the caption and Manage', strip(r[5]).length === 3 && ids(r[5]).includes('"limited"') && ids(r[5]).includes('"manage"'), JSON.stringify(strip(r[5])));
      check(host, '5: Manage waits on a selection ticket', ticket(r[8])?.device?.capability === 'selection', JSON.stringify(r[8]));
      check(host, '5: the new selection shows two', strip(r[11]).length === 2, JSON.stringify(strip(r[11])));
      check(host, '5: a save under limited access asks nothing and is a third', !ticket(r[21]) && strip(r[23]).length === 3, `${JSON.stringify(r[21])} ${JSON.stringify(strip(r[23]))}`);
      check(host, '5: the new photo is the newest', r[26]?.assets?.[0]?.id === 'agent-7', JSON.stringify(r[26]?.assets?.map(a => a.id)));
    }
    // 6: a photo leaving the selection is taken back, though the permission stays granted.
    {
      const r = drive({ photos: 'prompt' }, ['clock settle', 'tap show-recents', 'clock +100', 'tap @photos.photoAssets limited', 'clock settle', 'tap recent-2', 'clock settle', 'state openedPicture', 'state pictureGone',
        'prefer extension photos.selection=["agent-5"]', 'state pictureGone', 'clock settle', 'tree', 'state extensions', `screenshot ${shots}/6-revoked.png`,
        'prefer permission.photos denied', 'clock settle', 'tree', 'logs']);
      check(host, '6: the header shows agent-2', r[7] === 'photos:agent-2' && r[8] === false, `${r[7]} ${r[8]}`);
      check(host, "6: the selection's change empties the header's picture and fires error at once", r[10] === true, JSON.stringify(r[10]));
      check(host, '6: the strip loses agent-2; the read stays granted', strip(r[12]).length === 1 && r[13]?.photos?.permissions?.photos === 'granted', `${JSON.stringify(strip(r[12]))} ${JSON.stringify(r[13]?.photos?.permissions)}`);
      check(host, '6: denied, the strip goes and the denied line shows', strip(r[17]).length === 0 && ids(r[17]).includes('recents-denied'), JSON.stringify(strip(r[17])));
      const lines = logsOf(r), revoked = lines.findIndex(l => /photos: revoked photos:agent-2/.test(l));
      check(host, '6: the implementation said so, and no later reply drew it again', revoked >= 0 && !lines.slice(revoked).some(l => /photos: picture agent-2 /.test(l)), lines.filter(l => /photos:/.test(l)).join(' | '));
    }
    {
      const r = drive({ photos: 'denied' }, ['tree', 'state extensions']);
      check(host, '6: relaunched denied, the first frame has no strip and no picture', strip(r[0]).length === 0 && !ids(r[0]).includes('opened-picture') && ids(r[0]).includes('recents-denied'), JSON.stringify(strip(r[0])));
    }
  }
}
// 8: the web: the library is absent, so neither Save nor Recents shows.
if (host === 'web') {
  const r = drive({ camera: 'granted' }, ['clock settle', 'tree', 'state library']);
  const tree = ids(r[1]);
  check(host, '8: no Save and no Recents', !tree.includes('"save"') && !tree.includes('"recents"') && !tree.includes('show-recents'), tree.slice(0, 300));
  check(host, '8: the status says unavailable', r[2]?.available === false, JSON.stringify(r[2]));
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
