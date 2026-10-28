// LLP 1133.024 §20 stage 3's drive: shares sent to Fieldnotes on Android as
// another app sends them, an `ACTION_SEND` intent from `adb shell am start`,
// read back under the agent on the Canvas reader. Warm, the intent resolves
// by the filters the build derived (`-p`, no component) and must reach the
// running activity in its one task; cold, it is the launch intent. A file is
// a MediaStore URI the shell grants, as a sender's provider grants it.
//
// usage: bun apps/fieldnotes/share-android.drives.mjs [--emulator <serial>]
//   Builds nothing: build the APK first (`bun scripts/agent-android.mjs build fieldnotes --apk`).
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checker } from '../../packages/pedometer/test/fixture.mjs';

const root = resolve(import.meta.dir, '../..');
const argv = process.argv.slice(2);
Object.assign(process.env, { EXACT_ANDROID_READER: '1' }, argv.includes('--emulator') ? { ANDROID_SERIAL: argv[argv.indexOf('--emulator') + 1] } : {});
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const host = 'android', id = 'com.exact.fieldnotes';
const adb = (...args) => spawnSync('adb', [...(process.env.ANDROID_SERIAL ? ['-s', process.env.ANDROID_SERIAL] : []), ...args], { encoding: 'utf8' });
const q = (s) => `'${String(s).replace(/'/g, "'\\''")}'`;
const shown = (tree, testId) => (tree?.nodes ?? []).filter((n) => n.props?.testId === testId).map((n) => n.props.value ?? n.props.text ?? n.text ?? '');
const text = (tree) => JSON.stringify(tree) ?? '';
/** The app's activities in the system's tasks: a share must not stack a second one. */
const activities = () => new Set([...adb('shell', 'dumpsys', 'activity', 'activities').stdout.matchAll(/ActivityRecord\{([0-9a-f]+) u\d+ com\.exact\.fieldnotes\/dev\.exact\.app\.MainActivity/g)].map((m) => m[1])).size;
const send = (words) => { const r = adb('shell', ['am', 'start', '-W', ...words].join(' ')); if (/Error/.test(r.stdout + r.stderr)) throw new Error(`am start: ${r.stdout}${r.stderr}`); };
/** The share's intent words: text, and a file as a MediaStore URI with its type. */
const intent = ({ title, body, file }) => ['-a', 'android.intent.action.SEND', '-t', file ? file.type : 'text/plain',
  ...(title ? ['--es', 'android.intent.extra.SUBJECT', q(title)] : []), ...(body ? ['--es', 'android.intent.extra.TEXT', q(body)] : []),
  // The shell grants the URI as the intent's data; the stream extra names the same one.
  ...(file ? ['-d', file.uri, '--eu', 'android.intent.extra.STREAM', file.uri, '--grant-read-uri-permission'] : [])];
/** Waits, on the wall clock, until the tree shows `testId`. */
async function until(s, testId) {
  let tree;
  for (let i = 0; i < 40; i++) {
    await s.clock('settle');
    tree = await s.tree();
    if (shown(tree, testId).length) return tree;
    await s.clock('+250 real');
  }
  return tree;
}

// The fixture file, pushed and scanned into MediaStore, so a grant can carry it.
const dir = mkdtempSync(join(tmpdir(), 'fieldnotes-share-android-'));
const name = `packing-${Date.now().toString(36)}.md`, local = join(dir, name), remote = `/sdcard/Download/${name}`;
writeFileSync(local, '# Packing\nwater, map');
adb('push', local, remote);
adb('shell', 'am', 'broadcast', '-a', 'android.intent.action.MEDIA_SCANNER_SCAN_FILE', '-d', `file://${remote}`);
let media = null;
for (let i = 0; i < 20 && !media; i++) {
  media = adb('shell', `content query --uri content://media/external/file --projection _id --where "_display_name='${name}'"`).stdout.match(/_id=(\d+)/)?.[1];
  if (!media) await new Promise((r) => setTimeout(r, 250));
}
const file = media ? { uri: `content://media/external/file/${media}`, type: 'text/markdown' } : null;
check(host, 'the fixture file is in MediaStore', !!file, remote);

try {
  // Warm: text, then a file, each by the derived filters, into the running app's one activity.
  {
    const s = await open({ host, app: 'fieldnotes', storage: `fn-share-android-${Date.now().toString(36)}` });
    try {
      await s.clock('settle');
      send([...intent({ title: 'Trip', body: 'Bring water' }), '-p', id]);
      const t = await until(s, 'share-body');
      check(host, 'warm: a text share opens its route with the title and text', shown(t, 'share-title').includes('Trip') && shown(t, 'share-body').includes('Bring water'), text(t).slice(0, 400));
      check(host, 'warm: the share brought the app forward, one activity', activities() === 1, `${activities()} activities`);
      await s.tap('save-share');
      await s.clock('settle');
      const saved = await s.tree();
      check(host, 'warm: saving leaves the route and lists the note', !text(saved).includes('share-panel') && /"(aria-label|accessibilityLabel|label)":"Trip"/.test(text(saved)), text(saved).slice(0, 400));
      if (file) {
        send([...intent({ body: 'From a friend', file }), '-p', id]);
        const f = await until(s, 'share-file');
        check(host, 'warm: the file is listed under its own name', shown(f, 'share-file').join(',') === name, shown(f, 'share-file').join(','));
        check(host, 'warm: the text file joins the body', (shown(f, 'share-body')[0] ?? '').includes('From a friend\n\n# Packing\nwater, map'), shown(f, 'share-body'));
      }
      const logs = ((await s.logs()).lines ?? []).join('\n');
      check(host, 'warm: the journal names members, never the text', /share: received title, text/.test(logs) && !logs.includes('Bring water'), logs.split('\n').filter((l) => l.includes('share')).join(' | '));
    } finally { await s.close(); }
  }
  // Cold: the share is the launch intent; its first frame is the share's route.
  for (const [label, share] of [['text', { body: 'Cold hello' }], ...(file ? [['a file', { body: 'Cold files', file }]] : [])]) {
    process.env.EXACT_AGENT_INTENT = intent(share).join(' ');
    const s = await open({ host, app: 'fieldnotes', storage: `fn-share-android-cold-${Date.now().toString(36)}` });
    delete process.env.EXACT_AGENT_INTENT;
    try {
      const t = await until(s, share.file ? 'share-file' : 'share-body');
      check(host, `cold, ${label}: the launch opens the share's route`, (shown(t, 'share-body')[0] ?? '').includes(share.body), text(t).slice(0, 400));
      if (share.file) check(host, 'cold, a file: it lands after the route', shown(t, 'share-file').join(',') === name && (shown(t, 'share-body')[0] ?? '').includes('# Packing'), text(t).slice(0, 600));
    } finally { await s.close(); }
  }
} finally {
  adb('shell', 'rm', '-f', remote);
  if (media) adb('shell', 'content', 'delete', '--uri', `content://media/external/file/${media}`);
  rmSync(dir, { recursive: true, force: true });
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
