// What a development menu shows about the build it is in: when and where it
// was built and with which Xcode, the exact2 commit (and the app's own commit
// and branch, when the app keeps its own git repository), each with whether its
// tree had uncommitted changes, the build's kind, the distribution's revision
// when the deploy script names it, and the app's release notes. Stamped into
// the bundle's Info.plist, so it is baked into the binary, for a normal build
// and an `--archive` alike. Every key is optional.
import { spawnSync } from 'node:child_process';
import { closeSync, existsSync, openSync, readSync } from 'node:fs';
import { hostname } from 'node:os';
import { resolve } from 'node:path';

const git = (dir, args) => {
  const r = spawnSync('git', ['-C', dir, ...args], { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : null;
};

/** `{ sha, dirty }` for the repository holding `dir`, or `null` outside one. */
export function commitOf(dir) {
  const sha = git(dir, ['rev-parse', 'HEAD']);
  return sha ? { sha, dirty: git(dir, ['status', '--porcelain', '--untracked-files=normal']) !== '' } : null;
}

/** The app's release notes: `release-notes.md` beside its `app.contract`,
 * UTF-8 plain text (Markdown is shown as written). At most `NOTES_MAX` bytes
 * are read; longer notes are cut at a character and end with `…`. Notes that
 * are not UTF-8 fail the build, naming the file. */
export const RELEASE_NOTES = 'release-notes.md', NOTES_MAX = 16384;

export function releaseNotes(dir) {
  const file = dir && resolve(dir, RELEASE_NOTES);
  if (!file || !existsSync(file)) return '';
  const fd = openSync(file, 'r'), bytes = Buffer.alloc(NOTES_MAX + 1);
  const read = (() => { try { return readSync(fd, bytes, 0, bytes.length, 0); } finally { closeSync(fd); } })();
  const cut = read > NOTES_MAX;
  // A cut can split a character: back off over its continuation bytes.
  let end = cut ? NOTES_MAX : read;
  if (cut) while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--;
  let text;
  try { text = new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(0, end)); }
  catch { throw new Error(`host/apple: ${file} is not UTF-8 text`); }
  return (cut ? text.trimEnd() + '\n…' : text).trim();
}

const memo = new Map(), once = (key, f) => (memo.has(key) ? memo.get(key) : memo.set(key, f()).get(key));
const tool = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : null;
};
// One probe a build, whatever the number of bundles it stamps.
const xcode = () => once('xcode', () => tool('xcodebuild', ['-version'])?.split('\n').join(' ') ?? null);
const sdkOf = (sdk) => once(`sdk ${sdk}`, () => tool('xcrun', ['--sdk', sdk, '--show-sdk-version']));

/** The Info.plist keys for `app` built from the exact2 checkout at `root`.
 * Its kind is `archive` (an `--archive`), `release` (production trust, or a
 * distributed build: Swift's release configuration) or `debug`. A tree is
 * dirty with a change to a tracked file or a new file git does not ignore.
 * The app's commit and branch are left out when the app lives in exact2's
 * repository (they are exact2's). `EXACT_DISTRIBUTION_REVISION` is the
 * revision a deploy script publishes the build as (AppDrop's), when it names
 * one. */
export function buildInfo(app, { root, archive = false, production = false, sdk = null, now = new Date(), env = process.env }) {
  const exact = commitOf(root);
  const top = (dir) => git(dir, ['rev-parse', '--show-toplevel']);
  const own = app.dir && top(app.dir) && top(app.dir) !== top(root) ? commitOf(app.dir) : null;
  const branchOf = (dir) => { const b = dir && git(dir, ['rev-parse', '--abbrev-ref', 'HEAD']); return b && b !== 'HEAD' ? b : null; };
  const branch = branchOf(root), appBranch = own && branchOf(app.dir);
  const notes = releaseNotes(app.dir);
  const tools = xcode(), sdkVersion = sdk && sdkOf(sdk);
  return {
    ExactBuildTime: now.toISOString(),
    ExactBuildKind: archive ? 'archive' : production ? 'release' : 'debug',
    ExactBuildHost: hostname(),
    ...(tools ? { ExactBuildXcode: sdkVersion ? `${tools} · ${sdk} ${sdkVersion}` : tools } : {}),
    ...(exact ? { ExactCommit: exact.sha, ExactCommitDirty: exact.dirty } : {}),
    ...(exact && branch ? { ExactBranch: branch } : {}),
    ...(own ? { ExactAppCommit: own.sha, ExactAppCommitDirty: own.dirty } : {}),
    ...(appBranch ? { ExactAppBranch: appBranch } : {}),
    ...(env.EXACT_DISTRIBUTION_REVISION ? { ExactDistributionRevision: env.EXACT_DISTRIBUTION_REVISION } : {}),
    ...(notes ? { ExactReleaseNotes: notes } : {}),
  };
}
