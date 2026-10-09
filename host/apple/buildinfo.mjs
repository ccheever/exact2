// What a development menu shows about the build it is in: when and where it
// was built and with which Xcode, the exact2 commit (and the app's own commit
// and branch, when the app keeps its own git repository), each with whether its
// tree had uncommitted changes, the build's kind, the distribution's revision
// when the deploy script names it, and the app's release notes. Stamped into
// the bundle's Info.plist, so it is baked into the binary, for a normal build
// and an `--archive` alike. Every key is optional.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
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
 * plain text (Markdown is shown as written), at most this many characters. */
export const RELEASE_NOTES = 'release-notes.md', NOTES_MAX = 8000;

const xcode = () => {
  const r = spawnSync('xcodebuild', ['-version'], { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim().split('\n').join(' ') : null;
};

/** The Info.plist keys for `app` built from the exact2 checkout at `root`.
 * Its kind is `archive` (an `--archive`), `release` (production trust, or a
 * distributed build: Swift's release configuration) or `debug`. A tree is
 * dirty with a change to a tracked file or a new file git does not ignore.
 * The app's commit and branch are left out when the app lives in exact2's
 * repository (they are exact2's). `EXACT_DISTRIBUTION_REVISION` is the
 * revision a deploy script publishes the build as (AppDrop's), when it names
 * one. */
export function buildInfo(app, { root, archive = false, production = false, now = new Date(), env = process.env }) {
  const exact = commitOf(root);
  const top = (dir) => git(dir, ['rev-parse', '--show-toplevel']);
  const own = app.dir && top(app.dir) && top(app.dir) !== top(root) ? commitOf(app.dir) : null;
  const branch = own && git(app.dir, ['rev-parse', '--abbrev-ref', 'HEAD']);
  const notesFile = app.dir && resolve(app.dir, RELEASE_NOTES);
  const notes = notesFile && existsSync(notesFile) ? readFileSync(notesFile, 'utf8').trim().slice(0, NOTES_MAX) : '';
  const tools = xcode();
  return {
    ExactBuildTime: now.toISOString(),
    ExactBuildKind: archive ? 'archive' : production ? 'release' : 'debug',
    ExactBuildHost: hostname(),
    ...(tools ? { ExactBuildXcode: tools } : {}),
    ...(exact ? { ExactCommit: exact.sha, ExactCommitDirty: exact.dirty } : {}),
    ...(own ? { ExactAppCommit: own.sha, ExactAppCommitDirty: own.dirty } : {}),
    ...(branch && branch !== 'HEAD' ? { ExactAppBranch: branch } : {}),
    ...(env.EXACT_DISTRIBUTION_REVISION ? { ExactDistributionRevision: env.EXACT_DISTRIBUTION_REVISION } : {}),
    ...(notes ? { ExactReleaseNotes: notes } : {}),
  };
}
