// @exact/photos and the image codec on Android (LLP 1133.019 stage 3), on `apps/photos-fixture`, the library's
// consumer there (the photo editor has no Android port).
//
// With no flag it builds the fixture's APK and checks what the build made: the read permissions with their API
// ranges (READ_EXTERNAL_STORAGE ends at 32), ACCESS_MEDIA_LOCATION, no permission for adding; the roster naming the
// photos extension; the package's Kotlin and the reader's codec compiled in; the agent's six photos carried, byte for
// byte the package's fixture. `--emulator` then drives the APK on an arm64 emulator (ANDROID_SERIAL, else adb's
// one device) whose adb can run as root, to read the edits back: under the agent, the library through the reader's
// picture door, a copy and an add, limited access and a revocation; the image codec's conformance (LLP 1133.019 §11,
// checklist step 9) through the fixture's `editImage`; and MediaStore itself, the implementation bound and the read
// granted by `pm grant`. Both need an NDK in ANDROID_NDK_HOME and a JDK 17 or newer in JAVA_HOME. checklist.md's
// Android part is what needs a person: the platform's dialogs and what is chosen in them.
//
// usage: bun packages/photos/test/android.mjs [--emulator]
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import { androidDevice, androidSdk } from '../../../scripts/agent-android.mjs';

/** What a JPEG's EXIF says of the checks (orientation, GPS, date, camera, rights, maker note), or null with none. */
function exif(path) {
  if (!path) return null;
  const b = readFileSync(path);
  for (let i = 2; i + 4 < b.length && b[i] === 0xff;) {
    const marker = b[i + 1], length = b.readUInt16BE(i + 2);
    if (marker === 0xe1 && b.toString('latin1', i + 4, i + 10) === 'Exif\0\0') {
      const t = i + 10, le = b.toString('latin1', t, t + 2) === 'II';
      const u16 = o => le ? b.readUInt16LE(t + o) : b.readUInt16BE(t + o), u32 = o => le ? b.readUInt32LE(t + o) : b.readUInt32BE(t + o);
      const ifd = o => Object.fromEntries(Array.from({ length: u16(o) }, (_, k) => o + 2 + k * 12).map(e => [u16(e), e]));
      const zero = ifd(u32(4)), sub = zero[0x8769] ? ifd(u32(zero[0x8769] + 8)) : {};
      return { orientation: zero[0x0112] ? u16(zero[0x0112] + 8) : null, gps: !!zero[0x8825], make: !!zero[0x010f], copyright: !!zero[0x8298],
        date: !!sub[0x9003], makerNote: !!sub[0x927c] };
    }
    if (marker === 0xda) break;
    i += 2 + length;
  }
  return null;
}

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
  const adb = (...args) => spawnSync(resolve(androidSdk(), 'platform-tools/adb'), ['-s', androidDevice(), ...args], { maxBuffer: 1 << 26 });
  const api = Number(adb('shell', 'getprop', 'ro.build.version.sdk').stdout.toString().trim());
  const where = `android ${api}`;
  /** One launch of the APK under the agent, with [more] flags: each op's JSON reply, and the journal's and the app's
   * lines. */
  const drive = (permissions, ops, more = []) => {
    const flags = [...more, ...Object.entries(permissions).flatMap(([name, state]) => ['--permission', `${name}=${state}`])];
    const r = spawnSync('bun', ['scripts/agent.mjs', 'android', '--reader', '--json', '--app', app, ...flags, ...ops], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 26 });
    const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
    if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
    const logs = replies.find(x => Array.isArray(x?.lines));
    replies.lines = [...logs?.lines ?? [], ...logs?.host ?? []];
    return replies;
  };
  const tree = t => JSON.stringify(t ?? '');
  const grid = t => [...tree(t).matchAll(/"photo-(\d+)"/g)].length;
  // 1: the launch's read waits on a ticket; allowed, six pictures drawn by the door; the first copied byte for byte.
  {
    const r = drive({ photos: 'prompt' }, ['clock +100', 'state pending', 'tap @recent allow', 'clock settle', 'tree', 'state extensions',
      'tap photo-1', 'clock settle', 'state copied', 'logs']);
    const copy = r.lines.map(l => /photos: copied (\S+) to (\S+) sha256 ([0-9a-f]{64})/.exec(l)).find(Boolean);
    check(where, '1: the launch read waits on a photos ticket', r[1]?.some?.(p => p.device?.args?.capability === 'photos'), JSON.stringify(r[1]));
    check(where, '1: six pictures', grid(r[4]) === 6, tree(r[4]).slice(0, 400));
    check(where, '1: each picture admitted, asked of the extension, none refused', (r[5]?.photos?.pictures?.admitted ?? 0) >= 6 && !r[5]?.photos?.pictures?.refused
      && r.lines.filter(l => /photos: picture agent-\d at \d+x\d+ cover/.test(l)).length >= 6, JSON.stringify(r[5]?.photos?.pictures));
    check(where, "1: photo-1 is agent-1's copy, byte for byte", copy?.[1] === 'agent-1' && copy?.[2] === 'app:/tmp/photos/1.jpg' && copy?.[3] === AGENT(1), JSON.stringify(copy));
  }
  // 2: a copy added back joins the library as the newest, readable as the app's own.
  {
    const r = drive({ photos: 'granted', 'photos-add': 'granted' }, ['clock settle', 'tap photo-3', 'clock settle', 'tap add', 'clock settle', 'state added', 'tree', 'logs']);
    const add = r.lines.map(l => /photos: added (\S+) from (\S+) sha256 ([0-9a-f]{64}) (\d+)x(\d+)/.exec(l)).find(Boolean);
    check(where, '2: the add is agent-7, the stored turned photo kept as it was', add?.[1] === 'agent-7' && add?.[3] === AGENT(3), JSON.stringify(add));
    check(where, '2: seven pictures, the add first', grid(r[6]) === 7, tree(r[6]).slice(0, 400));
  }
  // 3: limited access, the agent's selection of three; then a photo leaves it and its picture is withdrawn.
  {
    const r = drive({ photos: 'prompt' }, ['clock +100', 'tap @recent limited', 'clock settle', 'tree', 'prefer extension photos.selection=["agent-5"]', 'clock settle', 'tree', 'logs']);
    check(where, '3: limited shows three and Manage', grid(r[3]) === 3 && tree(r[3]).includes('"manage"'), tree(r[3]).slice(0, 400));
    check(where, '3: a smaller selection shows one, the others revoked', grid(r[6]) === 1 && r.lines.some(l => /photos: revoked photos:agent-1/.test(l)), r.lines.filter(l => /photos:/.test(l)).join(' | '));
  }
  // 4: the codec (D16), §11's image conformance and checklist step 9, in one launch: each call's record, then its
  // file, read from the app's directory (an emulator's adb runs as root) and decoded here by `sips`.
  {
    const cases = [
      ['upright', 'fixture:oriented-gps.jpg', 'transform', {}],
      ['rotate 90', 'fixture:oriented-gps.jpg', 'transform', { rotate: 90 }],
      ['flip', 'fixture:oriented-gps.jpg', 'transform', { flip: 'horizontal' }],
      ['crop', 'fixture:oriented-gps.jpg', 'transform', { crop: { x: 0, y: 0, width: 24, height: 64 } }],
      ['free turn png', 'fixture:oriented-gps.jpg', 'transform', { rotate: 30, type: 'image/png' }],
      ['free turn jpeg', 'fixture:oriented-gps.jpg', 'transform', { rotate: 30 }],
      ['compress', 'fixture:oriented-gps.jpg', 'compress', { maxDimension: 4000, maxBytes: 1000000, rotate: 90 }],
      ['keep', 'fixture:every-tag.jpg', 'transform', { metadata: 'keep' }],
      ['no-location', 'fixture:every-tag.jpg', 'transform', { metadata: 'no-location' }],
      ['none', 'fixture:every-tag.jpg', 'transform', { metadata: 'none' }],
      ['agent-3', 'app:/tmp/photos/1.jpg', 'transform', {}],
      ['agent-3 top half', 'app:/tmp/photos/1.jpg', 'transform', { crop: { x: 0, y: 0, width: 360, height: 240 } }],
      ['agent-3 top half at 24', 'app:/tmp/photos/1.jpg', 'transform', { crop: { x: 0, y: 0, width: 360, height: 240 }, resizeWidth: 24 }],
      ['gain map, a quarter', 'fixture:gainmap-iso.jpg', 'transform', { rotate: 90 }],
      ['gain map, 10°', 'fixture:gainmap-iso.jpg', 'transform', { rotate: 10 }],
    ];
    const ops = ['clock settle', 'tap photo-3', 'clock settle'];
    for (const [, source, call, options] of cases)
      ops.push(`type edit-source ${source}`, `type edit-options ${JSON.stringify(options)}`, `tap ${call}`, 'clock settle', 'state edited');
    // Under the agent, app storage is a named scratch store's.
    const r = drive({ photos: 'granted' }, [...ops, 'logs'], ['--storage', 'photos-codec']);
    const records = cases.map((_, i) => { try { return JSON.parse(r[3 + i * 5 + 4]?.record ?? 'null'); } catch { return null; } });
    // The launch's temporary directory: the newest holding these edits.
    adb('root');
    adb('wait-for-device');
    const dirs = adb('shell', 'ls -dt $(find /data/data/com.exact.photosfixture -type d -name edits)').stdout.toString().trim().split(/\s+/);
    const out = resolve(tmpdir(), `photos-android-${process.pid}`);
    mkdirSync(out, { recursive: true });
    const file = (record) => {
      if (!record?.path?.startsWith('app:/tmp/edits/')) return null;
      const local = resolve(out, `${api}-${basename(record.path)}`);
      return adb('pull', `${dirs[0]}/${record.path.slice('app:/tmp/edits/'.length)}`, local).status === 0 ? local : null;
    };
    const pixels = (path) => {
      const png = path.endsWith('.png') ? path : `${path}.png`;
      if (png !== path) spawnSync('sips', ['-s', 'format', 'png', path, '--out', png]);
      return decodePng(readFileSync(png));
    };
    const at = (img, x, y) => [...img.data.slice((y * img.width + x) * 4, (y * img.width + x) * 4 + 4)];
    const blue = ([r, g, b]) => b > 150 && r < 100, red = ([r, g, b]) => r > 150 && b < 100;
    const by = Object.fromEntries(cases.map(([name], i) => [name, { record: records[i], path: file(records[i]) }]));
    const sized = (name, w, h) => check(where, `4: ${name} is ${w}x${h}`, by[name].record?.width === w && by[name].record?.height === h, JSON.stringify(by[name].record));
    const img = name => by[name].path && pixels(by[name].path);
    sized('upright', 48, 64);
    const up = img('upright');
    check(where, '4: upright, blue on the left', up && blue(at(up, 6, 32)) && red(at(up, 41, 32)), up && JSON.stringify([at(up, 6, 32), at(up, 41, 32)]));
    check(where, '4: upright, orientation 1 and the GPS kept', exif(by.upright.path)?.orientation === 1 && exif(by.upright.path)?.gps, JSON.stringify(exif(by.upright.path)));
    sized('rotate 90', 64, 48);
    const turned = img('rotate 90');
    check(where, '4: rotate 90, blue on top', turned && blue(at(turned, 32, 6)) && red(at(turned, 32, 41)), turned && JSON.stringify([at(turned, 32, 6), at(turned, 32, 41)]));
    const flipped = img('flip');
    check(where, '4: flip, blue on the right', flipped && red(at(flipped, 6, 32)) && blue(at(flipped, 41, 32)), flipped && JSON.stringify([at(flipped, 6, 32), at(flipped, 41, 32)]));
    sized('crop', 24, 64);
    const cut = img('crop');
    check(where, '4: crop, all blue', cut && [[2, 2], [21, 2], [2, 61], [21, 61], [12, 32]].every(([x, y]) => blue(at(cut, x, y))), cut && JSON.stringify(at(cut, 21, 32)));
    const clear = img('free turn png'), white = img('free turn jpeg');
    check(where, '4: rotate 30, a PNG\'s corners are clear', clear && at(clear, 0, 0)[3] === 0 && at(clear, clear.width - 1, clear.height - 1)[3] === 0, clear && JSON.stringify(at(clear, 0, 0)));
    check(where, '4: rotate 30, a JPEG\'s corners are white', white && at(white, 0, 0).slice(0, 3).every(v => v > 240), white && JSON.stringify(at(white, 0, 0)));
    sized('compress', 64, 48);
    check(where, '4: compressImage turns and keeps no metadata', by.compress.path && !exif(by.compress.path), JSON.stringify(by.compress.path && exif(by.compress.path)));
    const keep = exif(by.keep.path), bare = exif(by['no-location'].path), none = exif(by.none.path);
    check(where, '4: keep keeps the GPS, the date, the camera, the rights and the maker note', keep?.gps && keep.date && keep.make && keep.copyright && keep.makerNote, JSON.stringify(keep));
    check(where, '4: no-location keeps the date, the camera and the rights, and no GPS or maker note', bare && !bare.gps && !bare.makerNote && bare.date && bare.make && bare.copyright, JSON.stringify(bare));
    check(where, '4: none writes no metadata', by.none.path && !none, JSON.stringify(none));
    sized('agent-3', 360, 480);
    sized('agent-3 top half', 360, 240);
    sized('agent-3 top half at 24', 24, 16);
    const half = img('agent-3 top half'), whole = img('agent-3');
    check(where, '4: agent-3\'s top half is the upright photo\'s', half && whole && [[30, 30], [180, 120], [330, 200]].every(([x, y]) => at(half, x, y).slice(0, 3).every((v, i) => Math.abs(v - at(whole, x, y)[i]) < 24)),
      half && whole && JSON.stringify([at(half, 180, 120), at(whole, 180, 120)]));
    check(where, '4: a free turn of a gain-map photo says hdr false', by['gain map, 10°'].record?.hdr === false, JSON.stringify(by['gain map, 10°'].record));
    // Recorded, not checked: whether API 34 keeps the gain map through a quarter turn (D16).
    console.log(`NOTE ${where} 4: gain map through a quarter turn: hdr ${by['gain map, a quarter'].record?.hdr}`);
  }
  // 5: MediaStore for real, the implementation bound (§11's conformance on an emulator): two photos pushed and
  // scanned, the read granted by `pm grant`; the list, a copy with its location, an add whose row is no longer
  // pending and joins the list. The app's own rows and the pushed files go first, so a rerun starts alike.
  {
    const id = 'com.exact.photosfixture', images = 'content://media/external/images/media';
    adb('shell', `content delete --uri ${images} --where "owner_package_name='${id}' OR _display_name LIKE 'photos-drive-%'"`);
    adb('shell', 'rm -f /sdcard/Pictures/photos-drive-*');
    for (const [from, name] of [[resolve(here, '../fixture/agent-3.jpg'), 'photos-drive-turned.jpg'], [resolve(root, 'scripts/fixtures/picker/oriented-gps.jpg'), 'photos-drive-gps.jpg']])
      adb('push', from, `/sdcard/Pictures/${name}`);
    adb('shell', api >= 30 ? 'content call --method scan_volume --uri content://media --arg external_primary'
      : 'am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Pictures/photos-drive-turned.jpg && am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Pictures/photos-drive-gps.jpg');
    const read = api >= 33 ? 'READ_MEDIA_IMAGES' : 'READ_EXTERNAL_STORAGE';
    for (const p of [read, 'ACCESS_MEDIA_LOCATION']) adb('shell', 'pm', 'grant', id, `android.permission.${p}`);
    // The launch's read is decided before the implementation is bound, so it waits on the runner's ticket.
    const shot = resolve(tmpdir(), `photos-android-${api}-mediastore.png`);
    const r = drive({}, ['clock +100', 'tap @recent allow', 'clock settle', 'tree', 'tap photo-1', 'clock settle', 'state copied', 'tap add',
      'clock settle', 'state added', 'clock settle', 'tree', `screenshot ${shot}`, 'layout copy-picture', 'logs'], ['--bind', 'photos']);
    const status = t => /"testId":"status","text":"([^"]*)"/.exec(tree(t))?.[1];
    check(where, '5: granted, full access, the two pushed photos listed', status(r[3]) === 'granted full granted' && grid(r[3]) >= 2, `${status(r[3])}, ${grid(r[3])} photos`);
    const copied = r[6], pushed = { 'photos-drive-turned.jpg': [360, 480, AGENT(3)], 'photos-drive-gps.jpg': [48, 64] };
    check(where, '5: the newest copied upright, its location kept', pushed[copied?.name] && copied.width === pushed[copied.name][0] && copied.height === pushed[copied.name][1]
      && copied.android?.locationRedacted === false, JSON.stringify(copied));
    const added = r[9]?.asset;
    const row = added ? adb('shell', `content query --uri ${added} --projection _display_name:is_pending:relative_path:owner_package_name`).stdout.toString() : '';
    check(where, '5: the add is a MediaStore row of the app\'s, no longer pending, under Pictures', /is_pending=0/.test(row) && row.includes(`owner_package_name=${id}`) && /relative_path=Pictures/.test(row), `${added} ${row}`);
    // The reader's own loader sizes a turned JPEG by its upright size (an `img` 48 wide, its height auto).
    const drawn = r[13]?.node?.absolute;
    check(where, '5: the copy drawn by the reader\'s loader is upright', copied && drawn?.w === 48 && Math.abs(drawn.h - 48 * copied.height / copied.width) < 1, JSON.stringify(drawn));
    check(where, '5: the add joins the list', grid(r[11]) === grid(r[3]) + 1, `${grid(r[3])} then ${grid(r[11])}; ${shot}`);
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
