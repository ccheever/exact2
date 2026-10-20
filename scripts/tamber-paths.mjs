// What a Tamber answer may say of this machine's files (tamber/1.15, a
// fix; round 1's rework): path metadata named through one rule, the one
// `contract/cli/src/tamber_uses.rs` `shown` keeps for the design server, and
// machine-written messages scrubbed at path boundaries. Authored values and
// file contents are never touched.
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, isAbsolute, relative, sep } from 'node:path';

const real = p => { try { return realpathSync(p); } catch { return p; } };
const slashed = p => p.split(sep).join('/');
const inside = (p, d) => p === d || p.startsWith(d.endsWith(sep) ? d : d + sep);

/** `file` as an answer names it, never with this machine's directories:
 * inside the app's directory `appDir` (as given or real), from it; else
 * inside a package (the nearest directory above the file whose
 * `package.json` has a name), `<name>/<path inside it>`, as D15 names a
 * package's file; else `<outside>/<its name>`. A relative path is the app's
 * already, and a string that is no path is kept. */
export function pathName(file, appDir) {
  if (typeof file !== 'string' || !isAbsolute(file)) return file;
  const fileReal = real(file);
  for (const [p, d] of [[file, appDir], [file, real(appDir)], [fileReal, real(appDir)]]) {
    if (inside(p, d) && p !== d) return slashed(relative(d, p));
  }
  for (let dir = dirname(fileReal); dir !== dirname(dir); dir = dirname(dir)) {
    let name = null;
    try { name = JSON.parse(readFileSync(`${dir}/package.json`, 'utf8')).name; } catch { /* none here */ }
    if (typeof name === 'string' && name) return `${name}/${slashed(relative(dir, fileReal))}`;
  }
  return `<outside>/${basename(fileReal)}`;
}

/** A copy of `value` with every `file` string in it (any depth) named by
 * [`pathName`]: a selection's source and chain, a row's place, a diagnostic.
 * Nothing else is read or changed. */
export function namedPaths(value, appDir) {
  if (Array.isArray(value)) return value.map(v => namedPaths(v, appDir));
  if (!value || typeof value !== 'object') return value;
  const out = {};
  for (const [k, v] of Object.entries(value)) out[k] = k === 'file' && typeof v === 'string' ? pathName(v, appDir) : namedPaths(v, appDir);
  return out;
}

/** A machine-written message (a read error, the design server's stderr, a
 * refusal's text) with this machine's directories taken out:
 * - every absolute path in it (a run from `/`, `~/` or a drive letter, up
 *   to white space, a quote, a backtick, `)`, or a `:` before a line
 *   number) that names a file or a directory there (it, or its directory,
 *   exists) is named by [`pathName`]: app-relative, its package's
 *   `<name>/<path inside>` (wherever the package is installed or linked,
 *   found from the file itself), or `<outside>/<name>`; a `:line:col` after
 *   it stays;
 * - then, for any other mention, the app's directory (as `<app>`) and the
 *   home directory (as `~`), as given and real, longest first, where a whole
 *   path stands. */
export function messageScrubber(appDir) {
  const escape = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const backstop = [...new Map([
    [appDir, '<app>'], [real(appDir), '<app>'], [homedir(), '~'], [real(homedir()), '~'],
  ].filter(([d]) => d && d.length > 1)).entries()]
    .sort((a, b) => b[0].length - a[0].length)
    .map(([d, to]) => [new RegExp(`(?<![\\w.~/-])${escape(d)}(?=$|/|[^\\w.~-])`, 'g'), to]);
  const token = /(?<![\w.~/-])(?:~\/|\/|[A-Za-z]:[\\/])[^\s"'`)]+?(?=:\d|[\s"'`)]|$)/g;
  const there = p => { try { return existsSync(p) || existsSync(dirname(p)); } catch { return false; } };
  return text => {
    if (typeof text !== 'string' || !/[/\\]/.test(text)) return text;
    const named = new Map();
    const out = text.replace(token, raw => {
      // A sentence's full stop or comma after a path is no part of it.
      const [, path, tail] = /^(.*?)([.,;]*)$/.exec(raw);
      const full = path.startsWith('~/') ? `${homedir()}${path.slice(1)}` : path;
      if (!there(full)) return raw;
      // The app's or the home directory itself is no file: the backstop's names.
      const r = real(full);
      if (r === real(appDir)) return `<app>${tail}`;
      if (r === real(homedir())) return `~${tail}`;
      if (!named.has(full)) named.set(full, pathName(full, appDir));
      return named.get(full) + tail;
    });
    return backstop.reduce((t, [re, to]) => t.replace(re, to), out);
  };
}

/** A copy of `value` with every `message` string inside an `error` (any
 * depth: a refusal's, its diagnostics') passed through `scrub`, and a
 * `down` event's: what this machine wrote. Nothing else (a commit's
 * message, a token's value) is read or changed. */
export function scrubbedMessages(value, scrub, inError = false) {
  if (Array.isArray(value)) return value.map(v => scrubbedMessages(v, scrub, inError));
  if (!value || typeof value !== 'object') return value;
  const machine = inError || value.event === 'down';
  const out = {};
  for (const [k, v] of Object.entries(value)) {
    if (k === 'message' && typeof v === 'string' && machine) out[k] = scrub(v);
    else out[k] = scrubbedMessages(v, scrub, inError || k === 'error');
  }
  return out;
}
