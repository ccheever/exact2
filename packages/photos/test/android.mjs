// @exact/photos and the image codec on Android (LLP 1133.019 stage 3), on `apps/photos-fixture`, the library's
// consumer there (the photo editor has no Android port).
//
// With no flag it builds the fixture's APK and checks what the build made: the read permissions with their API
// ranges (READ_EXTERNAL_STORAGE ends at 32), ACCESS_MEDIA_LOCATION, no permission for adding; the roster naming the
// photos extension; the package's Kotlin and the reader's codec compiled in; the agent's six photos carried, byte for
// byte the package's fixture. `--emulator` then drives the fixture under the agent on a booted emulator or phone
// (adb's one device): the agent's library through the reader's picture door, a copy and an add, limited access and a
// revocation. Both need an NDK in ANDROID_NDK_HOME and a JDK 17 or newer in JAVA_HOME; the drives also an arm64
// device. checklist.md's Android part is what needs a person: the platform's dialogs, MediaStore for real, and the
// codec's orientation-6 checks on API 29, 33 and 34.
//
// usage: bun packages/photos/test/android.mjs [--emulator]
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';

const here = import.meta.dir, root = resolve(here, '../../..'), app = 'photos-fixture';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const AGENT = n => sha(readFileSync(resolve(here, `../fixture/agent-${n}.jpg`)));
const { check, failed } = checker();
const built = resolve(root, 'host/android/app/build', app);

const build = spawnSync('bun', ['scripts/agent-android.mjs', 'build', app, '--apk'], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
check('android', 'the fixture builds as an APK', build.status === 0, (build.stdout + build.stderr).slice(-2000));
const merged = existsSync(built) && readdirSync(built, { recursive: true })
  .find(f => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
const manifest = merged ? readFileSync(resolve(built, merged), 'utf8') : '';
const permission = name => new RegExp(`<uses-permission\\s+android:name="${name.replace(/\./g, '\\.')}"([^>]*)/>`).exec(manifest);
check('android', 'READ_EXTERNAL_STORAGE ends at API 32', /android:maxSdkVersion="32"/.test(permission('android.permission.READ_EXTERNAL_STORAGE')?.[1] ?? ''), merged ?? 'no merged manifest');
check('android', 'READ_MEDIA_IMAGES, READ_MEDIA_VISUAL_USER_SELECTED and ACCESS_MEDIA_LOCATION have no end',
  ['READ_MEDIA_IMAGES', 'READ_MEDIA_VISUAL_USER_SELECTED', 'ACCESS_MEDIA_LOCATION'].every(p => permission(`android.permission.${p}`) && !/maxSdkVersion/.test(permission(`android.permission.${p}`)[1])), manifest.slice(0, 1200));
check('android', 'adding asks no storage permission', !permission('android.permission.WRITE_EXTERNAL_STORAGE'), manifest.slice(0, 1200));
const roster = resolve(built, 'extensions/exact/app/ExactExtensions.kt');
check('android', 'the roster names the photos extension', existsSync(roster) && readFileSync(roster, 'utf8').includes('"photos" to { c -> exact.photos.PhotosModule(c) }'), roster);
const apk = resolve(built, 'outputs/apk/release/app-release.apk');
const listed = spawnSync('unzip', ['-l', apk], { encoding: 'utf8' }).stdout ?? '';
for (let n = 1; n <= 6; n++) {
  const bytes = spawnSync('unzip', ['-p', apk, `assets/app/assets/photos-fixture/agent-${n}.jpg`], { maxBuffer: 1 << 24 }).stdout;
  check('android', `agent-${n}.jpg is carried as the package's fixture`, bytes?.length > 0 && sha(bytes) === AGENT(n), listed.slice(0, 400));
}

if (process.argv.includes('--emulator')) {
  /** One launch under the agent: each op's JSON reply. */
  const drive = (permissions, ops) => {
    const flags = Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`]);
    const r = spawnSync('bun', ['scripts/agent.mjs', 'android', '--json', '--app', app, ...flags, ...ops], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 26 });
    const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
    if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
    return replies;
  };
  const tree = t => JSON.stringify(t ?? '');
  const grid = t => [...tree(t).matchAll(/"photo-(\d+)"/g)].length;
  const lines = r => r.find(x => Array.isArray(x?.lines))?.lines ?? [];
  // 1: the launch's read waits on a ticket; allowed, six pictures drawn by the door; the first copied byte for byte.
  {
    const r = drive({ photos: 'prompt' }, ['clock +100', 'state pending', 'tap @photos.photoAssets allow', 'clock settle', 'tree', 'state extensions',
      'tap photo-1', 'clock settle', 'state copied', 'logs']);
    const log = lines(r), copy = log.map(l => /photos: copied (\S+) to (\S+) sha256 ([0-9a-f]{64})/.exec(l)).find(Boolean);
    check('android', '1: the launch read waits on a photos ticket', r[1]?.some?.(p => p.device?.args?.capability === 'photos'), JSON.stringify(r[1]));
    check('android', '1: six pictures', grid(r[4]) === 6, tree(r[4]).slice(0, 400));
    check('android', '1: each picture admitted, asked of the extension, none refused', (r[5]?.photos?.pictures?.admitted ?? 0) >= 6 && !r[5]?.photos?.pictures?.refused
      && log.filter(l => /photos: picture agent-\d at \d+x\d+ cover/.test(l)).length >= 6, JSON.stringify(r[5]?.photos?.pictures));
    check('android', "1: photo-1 is agent-1's copy, byte for byte", copy?.[1] === 'agent-1' && copy?.[2] === 'app:/tmp/photos/1.jpg' && copy?.[3] === AGENT(1), JSON.stringify(copy));
  }
  // 2: a copy added back joins the library as the newest, readable as the app's own.
  {
    const r = drive({ photos: 'granted', 'photos-add': 'granted' }, ['clock settle', 'tap photo-3', 'clock settle', 'tap add', 'clock settle', 'state added', 'tree', 'logs']);
    const add = lines(r).map(l => /photos: added (\S+) from (\S+) sha256 ([0-9a-f]{64}) (\d+)x(\d+)/.exec(l)).find(Boolean);
    check('android', '2: the add is agent-7, the stored turned photo kept as it was', add?.[1] === 'agent-7' && add?.[3] === AGENT(3), JSON.stringify(add));
    check('android', '2: seven pictures, the add first', grid(r[6]) === 7, tree(r[6]).slice(0, 400));
  }
  // 3: limited access, a selection of two; then a photo leaves it and its picture is withdrawn.
  {
    const r = drive({ photos: 'prompt' }, ['clock +100', 'tap @photos.photoAssets limited', 'clock settle', 'tree', 'prefer extension photos.selection=["agent-5"]', 'clock settle', 'tree', 'logs']);
    check('android', '3: limited shows three and Manage', grid(r[3]) === 3 && tree(r[3]).includes('"manage"'), tree(r[3]).slice(0, 400));
    check('android', '3: a smaller selection shows one, the others revoked', grid(r[6]) === 1 && lines(r).some(l => /photos: revoked photos:agent-1/.test(l)), lines(r).filter(l => /photos:/.test(l)).join(' | '));
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
