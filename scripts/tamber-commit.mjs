// The design server's `commit` (LLP 1122 D9, LLP 1121 stage 3 commit 5):
// the files this session wrote, committed to git with a message naming each
// edit and its author, through a temporary index, never pushed.
//
// - **The baseline.** When the session first writes a file it records HEAD,
//   whether the file's index entry or its working tree (as the write read
//   it) differed from HEAD, and from then on every change it did not make.
// - **The guard.** A file is refused, named, when it differed from HEAD at
//   its baseline, HEAD has moved since, a change the server did not make was
//   seen (even one a later write overwrote), or the file now differs from
//   the server's last write. Such a file is committed by hand.
// - **The index.** HEAD plus exactly those files, in a temporary index; then
//   the real index's entries for those paths, and no other, take the new
//   blobs, so the developer's other staging stays.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative, resolve } from 'node:path';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');

/** git in `cwd`; its stdout, or a thrown error with its stderr. */
function git(cwd, args, { env = {}, input } = {}) {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8', input, env: { ...process.env, ...env } });
  if (r.error) throw new Error(`git: ${r.error.message}`);
  if (r.status !== 0) throw Object.assign(new Error((r.stderr || r.stdout || `git ${args[0]} failed`).trim()), { status: r.status });
  return r.stdout;
}
const tryGit = (cwd, args) => { try { return git(cwd, args).trim(); } catch { return null; } };

/** One app's session as `commit` sees it. Paths are the server's, from the
 * app's directory. */
export function designSession(dir) {
  // As git names it: a temporary directory's /var is /private/var to git.
  let appDir = dir;
  try { appDir = realpathSync(dir); } catch { /* not there yet */ }
  const files = new Map(); // path → { head, dirty, foreign, last, edits: [{client, tokens}] }
  const top = () => tryGit(appDir, ['rev-parse', '--show-toplevel']);

  /** A write the server made: `before` and `after` are the file's digests
   * as the write read it and as it left it. */
  function wrote(path, { before, after, client, tokens }) {
    let f = files.get(path);
    if (!f) {
      const root = top();
      const head = root && tryGit(root, ['rev-parse', '--verify', '-q', 'HEAD']);
      const repoPath = root && relative(root, resolve(appDir, path));
      let dirty = null;
      if (!head) dirty = 'the repository has no commit to start from';
      else {
        // Its text at HEAD, against the text the first write read.
        const atHead = spawnSync('git', ['show', `HEAD:${repoPath}`], { cwd: root });
        if (atHead.status !== 0 || sha(atHead.stdout) !== before) dirty = 'it differed from HEAD when the session first wrote it';
        else if (tryGit(root, ['diff', '--cached', '--quiet', 'HEAD', '--', repoPath]) === null) dirty = 'it was staged when the session first wrote it';
      }
      f = { head, dirty, foreign: false, last: before, edits: [] };
      files.set(path, f);
    }
    // A write that read other bytes than the last one left: a change
    // between them that was not the server's.
    if (f.last !== before) f.foreign = true;
    f.last = after;
    f.edits.push({ client, tokens });
  }

  /** Changes the server did not make (the watcher's): each a file's new
   * digest. One to a file the session wrote is foreign, for good. */
  function seen(changes) {
    for (const [path, d] of Object.entries(changes)) {
      const f = files.get(path);
      if (f && f.last !== d) f.foreign = true;
    }
  }

  /** Commit what the session wrote. `{ commit, files }`, or a refusal
   * naming each file and why. */
  function commit({ client, message }) {
    const root = top();
    if (!root) return { error: { message: 'the app is in no git repository' } };
    const head = tryGit(root, ['rev-parse', '--verify', '-q', 'HEAD']);
    const refused = [], chosen = [];
    for (const [path, f] of files) {
      const repoPath = relative(root, resolve(appDir, path));
      let now = '';
      try { now = sha(readFileSync(resolve(appDir, path))); } catch { /* gone */ }
      const why = f.dirty
        ?? (f.head !== head ? 'HEAD has moved since the session first wrote it' : null)
        ?? (f.foreign ? 'it was changed by someone other than this server' : null)
        ?? (now !== f.last ? "it differs from the server's last write" : null);
      if (why) refused.push({ file: path, why });
      else chosen.push({ path, repoPath, f });
    }
    if (refused.length) return { error: { message: `commit refused: ${refused.map(r => `${r.file}: ${r.why}`).join('; ')}`, refused } };
    if (!chosen.length) return { error: { message: 'the session has written nothing to commit' } };
    const scratch = mkdtempSync(join(tmpdir(), 'exact-tamber-index-'));
    const env = { GIT_INDEX_FILE: join(scratch, 'index') };
    try {
      git(root, ['read-tree', 'HEAD'], { env });
      const blobs = [];
      for (const { path, repoPath } of chosen) {
        const blob = git(root, ['hash-object', '-w', '--', resolve(appDir, path)]).trim();
        const mode = (tryGit(root, ['ls-tree', 'HEAD', '--', repoPath]) ?? '').split(/\s/)[0] || '100644';
        git(root, ['update-index', '--add', '--cacheinfo', `${mode},${blob},${repoPath}`], { env });
        blobs.push({ mode, blob, repoPath });
      }
      const tree = git(root, ['write-tree'], { env }).trim();
      if (tree === tryGit(root, ['rev-parse', 'HEAD^{tree}'])) return { error: { message: 'the session\'s files are as HEAD has them: nothing to commit' } };
      const lines = chosen.flatMap(({ path, f }) => f.edits.map(e => `- ${e.client}: ${e.tokens?.length ? e.tokens.join(', ') : 'edited'} (${path})`));
      const text = `${message || `design: ${lines.length} edit${lines.length === 1 ? '' : 's'}`}\n\n${lines.join('\n')}\n\nCommitted by the design server for ${client}.\n`;
      const made = git(root, ['commit-tree', tree, '-p', head], { input: text }).trim();
      // HEAD moves only from the HEAD the guard checked.
      git(root, ['update-ref', '-m', 'tamber commit', 'HEAD', made, head]);
      // The real index: those paths' entries, and no other.
      for (const { mode, blob, repoPath } of blobs) git(root, ['update-index', '--add', '--cacheinfo', `${mode},${blob},${repoPath}`]);
      // A new session begins for them at the new HEAD.
      for (const { path } of chosen) files.delete(path);
      return { ok: { commit: made, files: chosen.map(c => c.path) } };
    } catch (e) {
      return { error: { message: `commit: ${e.message}` } };
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  }

  /** The session's files and what stands in the way of committing each. */
  function state() {
    return Object.fromEntries([...files].map(([path, f]) => [path, { edits: f.edits.length, dirty: f.dirty, foreign: f.foreign }]));
  }

  return { wrote, seen, commit, state };
}
