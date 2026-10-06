// A failed app build's Contract errors, all of them (LLP 1054 L9). A host
// crate's build script compiles the Contract and panics on its first error,
// somewhere in Cargo's output; on a failure that can be one, ask the compiler
// for every error and say those alone, last.
import { spawnSync } from 'node:child_process';
import { existsSync, rmSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';

/** Whether a failed Cargo run can be the Contract's: a build that a build
 * script failed. A rustc error, a linker's, `cargo metadata`'s or a spawn's
 * is not, and asks no second compiler. With its stderr inherited, a
 * `--message-format=json` build says rustc's errors on stdout instead. */
export function contractMayHaveFailed(args, result) {
  if (result.error || !['build', 'rustc', 'check'].includes(args[0])) return false;
  if (typeof result.stderr === 'string') return result.stderr.includes('failed to run custom build command');
  return !(result.stdout ?? '').split('\n').some((line) => line.includes('"reason":"compiler-message"') && /"level":"error"/.test(line));
}

/** Throw the Contract's errors when `result`, a failed `cargo <args>` of
 * `app`, is the Contract's; return otherwise. The compiler is the
 * repository's tool, run as every script runs it: from `root` with
 * `process.env`, not the build's (a web nightly, an app's target). */
export function throwContractErrors(app, args, result, root) {
  const source = resolve(app.dir, 'app.contract');
  if (!existsSync(source) || !contractMayHaveFailed(args, result)) return;
  const scratch = resolve(tmpdir(), `exact-contract-check-${process.pid}.plan`);
  const checked = spawnSync('cargo', ['run', '-q', '--manifest-path', resolve(root, 'Cargo.toml'), '-p', 'contract', '--', 'build', source, '-o', scratch],
    { cwd: root, env: process.env, stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8' });
  rmSync(scratch, { force: true });
  const found = (checked.stderr ?? '').split('\n').filter((line) => /\.contract:\d+:\d+ \[[a-z0-9-]+\]/.test(line))
    .map((line) => line.replace(/^(\/\S+?\.contract)/, (file) => relative(process.cwd(), file) || file));
  if (checked.status !== 0 && found.length) {
    const error = new Error(`${app.name}: the Contract does not compile:\n  ${[...new Set(found)].join('\n  ')}`);
    error.stack = error.message; error.contract = true;
    throw error;
  }
}
