// What a development menu shows about the build it is in: when it was built,
// the exact2 commit (and the app's own, when the app keeps its own git
// repository), each with whether its tree had uncommitted changes, and the
// build's kind. Stamped into the bundle's Info.plist, so it is baked into the
// binary, for a normal build and an `--archive` alike.
import { spawnSync } from 'node:child_process';

const git = (dir, args) => {
  const r = spawnSync('git', ['-C', dir, ...args], { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : null;
};

/** `{ sha, dirty }` for the repository holding `dir`, or `null` outside one. */
export function commitOf(dir) {
  const sha = git(dir, ['rev-parse', 'HEAD']);
  return sha ? { sha, dirty: git(dir, ['status', '--porcelain', '--untracked-files=normal']) !== '' } : null;
}

/** The Info.plist keys for `app` built from the exact2 checkout at `root`:
 * its kind is `archive` (an `--archive`), `release` (production trust, or a
 * distributed build: Swift's release configuration) or `debug`. A tree is
 * dirty with a change to a tracked file or a new file git does not ignore. The app's own commit is left out when the app lives in exact2's
 * repository (its commit is exact2's). */
export function buildInfo(app, { root, archive = false, production = false, now = new Date() }) {
  const exact = commitOf(root);
  const top = (dir) => git(dir, ['rev-parse', '--show-toplevel']);
  const own = app.dir && top(app.dir) && top(app.dir) !== top(root) ? commitOf(app.dir) : null;
  return {
    ExactBuildTime: now.toISOString(),
    ExactBuildKind: archive ? 'archive' : production ? 'release' : 'debug',
    ...(exact ? { ExactCommit: exact.sha, ExactCommitDirty: exact.dirty } : {}),
    ...(own ? { ExactAppCommit: own.sha, ExactAppCommitDirty: own.dirty } : {}),
  };
}
