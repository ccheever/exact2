// The design server's `commit` (LLP 1122 D9, LLP 1121 stage 3 commit 5):
// the files this session wrote, committed to git with a message naming each
// edit and its author, through a temporary index, never pushed.
//
// - **The baseline.** Before the session first writes a file it records
//   HEAD and whether the file's index entry or its working tree (content
//   or mode) differed from HEAD; the write's own reading of the file must
//   then be HEAD's text. From then on every change it did not make.
// - **The guard.** A file is refused, named, when it differed from HEAD at
//   its baseline, HEAD has moved since, a change the server did not make was
//   seen (even one a later write overwrote), or the file now differs from
//   the server's last write. Such a file is committed by hand.
// - **The index.** HEAD plus exactly those files, in a temporary index; then
//   the real index's entries for those paths, and no other, take the new
//   blobs, so the developer's other staging stays. The real index moves
//   before HEAD does (and is put back if HEAD cannot move), so a failure
//   never leaves HEAD ahead of an index that would undo it.
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
  const pending = new Map(); // path → { head, dirty, atHead }, read before a first write
  const top = () => tryGit(appDir, ['rev-parse', '--show-toplevel']);

  /** Before a write that may touch `paths`: the git facts of each the
   * session has not written yet, as they are now, read in three git
   * processes however many paths. A path already prepared for a write
   * still in flight keeps its facts (that write may be on disk already);
   * each prepare is ended by `wrote` or `release`. The paths it holds. */
  function prepare(paths) {
    // A name git cannot take (a NUL) is no file of the design; the server
    // refuses it.
    const named = [...new Set(paths)].filter(path => typeof path === 'string' && !path.includes('\0'));
    const fresh = named.filter(path => !files.has(path) && !pending.has(path));
    // Every git read first: one that throws leaves no hold taken.
    const facts = fresh.length ? read(fresh) : [];
    const held = named.filter(path => !files.has(path));
    for (const path of held) if (pending.has(path)) pending.get(path).holds++;
    fresh.forEach((path, i) => pending.set(path, { ...facts[i], holds: 1 }));
    return held;
  }

  /** The git facts of `paths` now: three git processes however many. A
   * read that fails marks every path dirty, never clean. */
  function read(paths) {
    const root = top();
    const head = root && tryGit(root, ['rev-parse', '--verify', '-q', 'HEAD']);
    if (!head) return paths.map(() => ({ head, dirty: 'the repository has no commit to start from', atHead: null }));
    const repoPaths = paths.map(path => relative(root, resolve(appDir, path)));
    const failed = paths.map(() => ({ head, dirty: 'git could not say whether it differed from HEAD', atHead: null }));
    // XY per path: X the index against HEAD, Y the working tree against the
    // index. No renames, so each record is one path.
    const status = new Map();
    const listed = spawnSync('git', ['status', '--porcelain=v1', '-z', '--no-renames', '--untracked-files=all', '--', ...repoPaths], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20 });
    if (listed.error || listed.status !== 0) return failed;
    for (const record of listed.stdout.split('\0').filter(Boolean)) status.set(record.slice(3), record.slice(0, 2));
    // HEAD's text of each clean path, by one cat-file, NUL-separated (a path
    // may hold a newline): `<oid> blob <size>\0<bytes>\0`, or
    // `<name> missing\0`.
    const atHead = new Map();
    const clean = repoPaths.filter(r => !status.has(r));
    if (clean.length) {
      const shown = spawnSync('git', ['cat-file', '--batch', '-Z'], { cwd: root, input: clean.map(r => `HEAD:${r}\0`).join(''), maxBuffer: 1 << 30 });
      if (shown.error || shown.status !== 0) return failed;
      const out = shown.stdout;
      let at = 0;
      for (const r of clean) {
        const end = out.indexOf(0, at);
        if (end < 0) return failed;
        const header = out.subarray(at, end).toString();
        if (header.endsWith(' missing')) { at = end + 1; continue; }
        const size = Number(header.split(' ')[2]);
        if (!Number.isInteger(size)) return failed;
        atHead.set(r, sha(out.subarray(end + 1, end + 1 + size)));
        at = end + 1 + size + 1;
      }
    }
    return repoPaths.map(r => {
      const xy = status.get(r);
      const dirty = xy && xy[0] !== ' ' && xy[0] !== '?' ? 'it was staged when the session first wrote it'
        : xy ? 'it differed from HEAD when the session first wrote it'
        : null;
      return { head, dirty, atHead: atHead.get(r) ?? null };
    });
  }

  /** A prepared write that wrote nothing to `paths`: its hold on their
   * facts ends, and the last hold's end forgets them. */
  function release(paths) {
    for (const path of paths) {
      const facts = pending.get(path);
      if (facts && --facts.holds <= 0) pending.delete(path);
    }
  }

  /** A write the server made: `before` and `after` are the file's digests
   * as the write read it and as it left it. */
  function wrote(path, { before, after, client, tokens }) {
    let f = files.get(path);
    if (!f) {
      const facts = pending.get(path) ?? { head: null, dirty: 'the session did not read it before writing it', atHead: null };
      // The text the write read is HEAD's, or something moved in between.
      const dirty = facts.dirty ?? (facts.atHead !== before ? 'it differed from HEAD when the session first wrote it' : null);
      f = { head: facts.head, dirty, foreign: false, last: before, edits: [] };
      files.set(path, f);
    }
    release([path]);
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
      // Read once: these bytes are the ones checked and the ones committed.
      let bytes = null;
      try { bytes = readFileSync(resolve(appDir, path)); } catch { /* gone */ }
      const why = f.dirty
        ?? (f.head !== head ? 'HEAD has moved since the session first wrote it' : null)
        ?? (f.foreign ? 'it was changed by someone other than this server' : null)
        ?? (!bytes || sha(bytes) !== f.last ? "it differs from the server's last write" : null);
      if (why) refused.push({ file: path, why });
      else chosen.push({ path, repoPath, f, bytes });
    }
    if (refused.length) return { error: { message: `commit refused: ${refused.map(r => `${r.file}: ${r.why}`).join('; ')}`, refused } };
    if (!chosen.length) return { error: { message: 'the session has written nothing to commit' } };
    const scratch = mkdtempSync(join(tmpdir(), 'exact-tamber-index-'));
    const env = { GIT_INDEX_FILE: join(scratch, 'index') };
    try {
      git(root, ['read-tree', 'HEAD'], { env });
      const blobs = [];
      for (const { repoPath, bytes } of chosen) {
        const blob = git(root, ['hash-object', '-w', '--stdin'], { input: bytes }).trim();
        const mode = (tryGit(root, ['ls-tree', 'HEAD', '--', repoPath]) ?? '').split(/\s/)[0] || '100644';
        git(root, ['update-index', '--add', '--cacheinfo', `${mode},${blob},${repoPath}`], { env });
        blobs.push({ mode, blob, repoPath });
      }
      const tree = git(root, ['write-tree'], { env }).trim();
      if (tree === tryGit(root, ['rev-parse', 'HEAD^{tree}'])) return { error: { message: 'the session\'s files are as HEAD has them: nothing to commit' } };
      const lines = chosen.flatMap(({ path, f }) => f.edits.map(e => `- ${e.client}: ${e.tokens?.length ? e.tokens.join(', ') : 'edited'} (${path})`));
      const text = `${message || `design: ${lines.length} edit${lines.length === 1 ? '' : 's'}`}\n\n${lines.join('\n')}\n\nCommitted by the design server for ${client}.\n`;
      const made = git(root, ['commit-tree', tree, '-p', head], { input: text }).trim();
      // The real index first: those paths' entries, and no other, all in
      // one update (it fails whole, with HEAD unmoved).
      const saved = git(root, ['ls-files', '-s', '-z', '--', ...blobs.map(b => b.repoPath)]);
      // NUL-separated: a path may hold any byte but NUL.
      git(root, ['update-index', '-z', '--add', '--index-info'], { input: blobs.map(b => `${b.mode} ${b.blob}\t${b.repoPath}\0`).join('') });
      // HEAD moves only from the HEAD the guard checked; if it cannot, the
      // index entries go back as they were (an entry not there before is
      // removed: mode 0 and a zero name as long as the repository's).
      try { git(root, ['update-ref', '-m', 'tamber commit', 'HEAD', made, head]); } catch (e) {
        const records = saved.split('\0').filter(Boolean);
        const before = new Set(records.map(r => r.slice(r.indexOf('\t') + 1)));
        const absent = blobs.filter(b => !before.has(b.repoPath)).map(b => `0 ${'0'.repeat(b.blob.length)}\t${b.repoPath}\0`);
        try { git(root, ['update-index', '-z', '--index-info'], { input: records.map(r => `${r}\0`).join('') + absent.join('') }); } catch (back) {
          throw new Error(`${e.message}; and the index could not be put back (${back.message}): those paths stay staged at the session's text`);
        }
        throw e;
      }
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

  return { prepare, release, wrote, seen, commit, state };
}
