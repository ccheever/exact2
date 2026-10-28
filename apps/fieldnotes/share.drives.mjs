// LLP 1133.024 D9's share drives on Fieldnotes under the agent: the
// driver's `share:` form handed to the host's claim path, warm (`type
// fieldnotes "share:?…"`) and cold (`--url "share:?…"`). The web takes text
// only; the native hosts take files too.
//
// usage: bun apps/fieldnotes/share.drives.mjs <web|macos|ios|linux> [--sim <udid>]
//   (the app built for that host: host/web/build.mjs fieldnotes,
//   host/apple/build.mjs [--ios] fieldnotes-apple, or
//   cargo build --profile host-dev -p fieldnotes-linux)
import { mkdtempSync, rmSync, writeFileSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../packages/pedometer/test/fixture.mjs';

const root = resolve(import.meta.dir, '../..');
const host = process.argv[2] ?? 'web', extra = process.argv.slice(3);
const { check, failed } = checker();
const run = `share-${Date.now().toString(36)}`;
const linux = host === 'linux' ? ['EXACT_LINUX_BIN', resolve(root, 'target/host-dev/fieldnotes-linux')] : null;

/** One launch: each op's JSON reply, in order. */
function drive(storage, ops, { url } = {}) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'fieldnotes', '--storage', storage, '--reset-storage', ...(url ? ['--url', url] : []), ...extra, ...ops];
  const env = { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-', ...(linux && !process.env.EXACT_LINUX_BIN ? { [linux[0]]: linux[1] } : {}) };
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = tree => JSON.stringify(tree) ?? '';
/** A view's text, value or label by testId, from a `tree` reply. */
const shown = (tree, id) => (tree?.nodes ?? []).filter(n => n.props?.testId === id).map(n => n.props.value ?? n.props.text ?? n.text ?? '');
const lines = logs => (logs?.lines ?? []).join('\n');

const files = mkdtempSync(join(tmpdir(), 'fieldnotes-share-'));
const note = join(files, 'packing.md'), photo = resolve(root, 'scripts/fixtures/picker/photo.png'), odd = join(files, 'data.bin');
writeFileSync(note, '# Packing\nwater, map');
writeFileSync(odd, Buffer.from([0, 1, 2, 3]));
const enc = encodeURIComponent;

// 1: a warm share of text: its route, the title and body as sent; saving it makes a note and leaves the route.
{
  const r = drive(`fn-share-1-${run}`, ['clock settle', `type fieldnotes share:?title=${enc('Trip')}&text=${enc('Bring water')}`, 'clock settle', 'tree', 'tap save-share', 'clock settle', 'tree', 'logs']);
  check(host, '1: the share panel shows the title and text', shown(r[3], 'share-title').includes('Trip') && shown(r[3], 'share-body').includes('Bring water'), text(r[3]).slice(0, 400));
  check(host, '1: saving leaves the route and lists the note', !text(r[6]).includes('share-panel') && /"(aria-label|accessibilityLabel|label)":"Trip"/.test(text(r[6])), text(r[6]).slice(0, 400));
  check(host, '1: the journal names the members, never the text', host === 'web' || (/share: received title, text/.test(lines(r[7])) && !lines(r[7]).includes('Bring water')), lines(r[7]));
}
// 2: cold: the first frame is the share's route with its text.
{
  const r = drive(`fn-share-2-${run}`, ['tree', 'clock settle', 'tree'], { url: `share:?text=${enc('Cold hello')}` });
  check(host, '2: a cold share opens on its route with its text', shown(r[0], 'share-body').includes('Cold hello') || shown(r[2], 'share-body').includes('Cold hello'), text(r[2]).slice(0, 400));
}
if (host === 'web') {
  // 3: the web's member carries no files.
  const r = drive(`fn-share-3-${run}`, ['clock settle', `type fieldnotes share:?text=hi&files=${note}`]);
  check(host, '3: a file share is refused on the web', r.length < 2 || /service worker/.test(text(r[1])), text(r));
} else {
  // 3: files, warm: the text file's words join the body, the picture is listed, the file no member accepts is counted.
  const r = drive(`fn-share-3-${run}`, ['clock settle', `type fieldnotes share:?text=${enc('From a friend')}&files=${note}&files=${photo}&files=${odd}`, 'clock settle', 'tree', 'logs']);
  check(host, '3: the files are listed under their own names', shown(r[3], 'share-file').join(',') === 'packing.md,photo.png', shown(r[3], 'share-file').join(','));
  check(host, '3: the text file joins the body', (shown(r[3], 'share-body')[0] ?? '').includes('From a friend\n\n# Packing\nwater, map'), shown(r[3], 'share-body'));
  check(host, '3: the file no member accepts is counted', shown(r[3], 'share-dropped').includes("1 file couldn't be added."), shown(r[3], 'share-dropped'));
  const size = statSync(note).size;
  check(host, '3: the journal says the types and sizes', lines(r[4]).includes(`share: received text; files: text/markdown ${size} B, image/png`), lines(r[4]));
  // 4: files, cold: the route first, then the files land.
  const c = drive(`fn-share-4-${run}`, ['clock settle', 'tree'], { url: `share:?text=${enc('Cold files')}&files=${note}` });
  check(host, '4: a cold share with a file lands it', shown(c[1], 'share-file').join(',') === 'packing.md' && (shown(c[1], 'share-body')[0] ?? '').includes('# Packing'), text(c[1]).slice(0, 600));
}
rmSync(files, { recursive: true, force: true });
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
