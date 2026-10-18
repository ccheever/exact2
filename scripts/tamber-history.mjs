// What the design server reads of git for `history` and `revert` (LLP 1122
// D9, T1): the commits that touched the design's files, and each commit's
// versions of each such file as text, which `contract tamber serve` compares
// as token-level edits (contract/cli/src/tamber_history.rs). Reads only.
//
// A design file is a `.tamber` file in the app's directory, or the design
// app.json names wherever it is, or one of the app's own Contract sources
// the server has now (LLP 1121 stage 8 D: their state parts are the
// design's; the files `contract tamber serve` edits).
// A merge is read against its first parent; renames are a removal and an
// addition.
import { spawnSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { relative, resolve } from 'node:path';

// git's empty tree: what a root commit's files are compared with.
const EMPTY_TREE = '4b825dc642cb6eb9a060e54bf8d69288fbee4904';

function git(cwd, args, input) {
  const r = spawnSync('git', args, { cwd, input, maxBuffer: 1 << 30 });
  if (r.error) throw new Error(`git: ${r.error.message}`);
  if (r.status !== 0) throw new Error((r.stderr.toString() || `git ${args[0]} failed`).trim());
  return r.stdout;
}

/** The repository's top and the app's directory as git names it, or null
 * when the app is in no repository. */
function repository(appDir) {
  let dir = appDir;
  try { dir = realpathSync(appDir); } catch { /* not there */ }
  try { return { root: git(dir, ['rev-parse', '--show-toplevel']).toString().trim(), dir }; } catch { return null; }
}

/** The pathspecs of the design's files: `.tamber` beside the app, the
 * design (`designFile`, from the app's directory) wherever it is, and each
 * of `contracts` (from the app's directory), or with `'all'` every
 * `.contract` in the app outside `node_modules` (a revert's: one the app
 * no longer reads is refused by name, not left out). */
function pathspecs(root, dir, designFile, contracts = []) {
  const app = relative(root, dir);
  const glob = s => s.replace(/[*?[\\]/g, c => `\\${c}`);
  const under = app ? `${glob(app)}/` : '';
  const sources = contracts === 'all'
    ? [`:(glob)${under}**/*.contract`, `:(glob,exclude)${under}**/node_modules/**`]
    : contracts.map(f => `:(literal)${relative(root, resolve(dir, f))}`);
  return [`:(glob)${under}*.tamber`, `:(literal)${relative(root, resolve(dir, designFile))}`, ...sources];
}

/** Each `<rev>:<path>`'s bytes as text, or null where it is missing: one
 * cat-file, NUL-separated (a path may hold a newline). */
function texts(root, names) {
  if (!names.length) return [];
  const out = git(root, ['cat-file', '--batch', '-Z'], names.map(n => `${n}\0`).join(''));
  const found = [];
  let at = 0;
  for (let i = 0; i < names.length; i++) {
    const end = out.indexOf(0, at);
    const header = out.subarray(at, end).toString();
    if (header.endsWith(' missing') || header.endsWith(' ambiguous')) { found.push(null); at = end + 1; continue; }
    const size = Number(header.split(' ')[2]);
    found.push(out.subarray(end + 1, end + 1 + size).toString('utf8'));
    at = end + 1 + size + 1;
  }
  return found;
}

/** One commit as history shows it: its facts, and each design file it
 * changed (against its first parent) with its text before and after. */
function commitOf(root, dir, sha, specs) {
  const [parents, author, email, date, ...message] = git(root, ['show', '-s', '--format=%P%x00%an%x00%ae%x00%aI%x00%B', sha]).toString().split('\0');
  const parent = parents.split(' ').filter(Boolean)[0] ?? null;
  const listed = git(root, ['diff-tree', '-r', '-z', '--no-renames', '--name-status', parent ?? EMPTY_TREE, sha, '--', ...specs]).toString().split('\0').filter(Boolean);
  const changed = [];
  for (let i = 0; i + 1 < listed.length; i += 2) changed.push({ status: listed[i], repoPath: listed[i + 1] });
  const versions = texts(root, changed.flatMap(c => [parent ? `${parent}:${c.repoPath}` : null, `${sha}:${c.repoPath}`]).filter(n => n !== null));
  let k = 0;
  const files = changed.map(c => ({
    file: relative(dir, resolve(root, c.repoPath)),
    status: c.status,
    repoPath: c.repoPath,
    before: parent ? versions[k++] : null,
    after: versions[k++],
  }));
  return { commit: sha, parent, author, email, date, message: message.join('\0').trimEnd(), files };
}

/** The diff of each of `files` in commit `c`, by app path: one git diff a
 * file, by its literal path, so a name git would quote is no matter. */
function diffs(root, c) {
  return Object.fromEntries(c.files.map(f => [f.file, git(root, ['diff', '--no-color', '--no-renames', '--no-ext-diff', c.parent ?? EMPTY_TREE, c.commit, '--', `:(literal)${f.repoPath}`]).toString()]));
}

/** Up to `limit` commits that touched the design's files, newest first,
 * after skipping `skip`; `more` when there are older ones. */
export function designLog(appDir, designFile, { limit = 20, skip = 0, contracts = [] } = {}) {
  const repo = repository(appDir);
  if (!repo) return { error: { message: 'the app is in no git repository' } };
  const { root, dir } = repo;
  const specs = pathspecs(root, dir, designFile, contracts);
  let shas;
  try {
    shas = git(root, ['log', '--format=%H', `--max-count=${limit + 1}`, `--skip=${skip}`, '--', ...specs]).toString().split('\n').filter(Boolean);
  } catch (e) {
    // A repository with no commit yet has no history.
    if (/does not have any commits|bad default revision/.test(e.message)) return { ok: { commits: [], more: false } };
    return { error: { message: `git log: ${e.message}` } };
  }
  const commits = shas.slice(0, limit).map(sha => {
    const c = commitOf(root, dir, sha, specs);
    const diff = diffs(root, c);
    return { ...c, files: c.files.map(({ repoPath, ...f }) => ({ ...f, diff: diff[f.file] ?? '' })) };
  });
  return { ok: { commits, more: shas.length > limit } };
}

/** Commit `rev`'s design files, before and after it. */
export function designCommit(appDir, designFile, rev, contracts = []) {
  const repo = repository(appDir);
  if (!repo) return { error: { message: 'the app is in no git repository' } };
  if (typeof rev !== 'string' || !rev || rev.startsWith('-') || /[\0\s]/.test(rev)) return { status: 400, error: { message: '`commit` names a commit' } };
  const { root, dir } = repo;
  let sha;
  try { sha = git(root, ['rev-parse', '--verify', '-q', '--end-of-options', `${rev}^{commit}`]).toString().trim(); } catch { return { status: 404, error: { message: `no commit \`${rev}\`` } }; }
  const c = commitOf(root, dir, sha, pathspecs(root, dir, designFile, contracts));
  return { ok: { ...c, files: c.files.map(({ repoPath, ...f }) => f) } };
}
