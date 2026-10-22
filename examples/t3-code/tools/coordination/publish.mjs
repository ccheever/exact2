import { spawnSync } from 'node:child_process';
import { command } from './state.mjs';
const [remote, branch, ...verify] = process.argv.slice(2);
function run(args) {
  const result = spawnSync(args[0], args.slice(1), { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${args[0]} failed: ${result.status}`);
}
try {
  if (!verify.length) throw new Error('affected verification command required');
  run(['git', 'fetch', '--', remote, `refs/heads/${branch}`]);
  const base = command(['git', 'rev-parse', 'FETCH_HEAD']);
  run(['git', 'rebase', base]);
  run(verify);
  if (command(['git', 'status', '--porcelain', '--untracked-files=all'])) throw new Error('verification changed source');
  run(['git', 'merge-base', '--is-ancestor', base, 'HEAD']);
  run(['git', 'push', '--', remote, `HEAD:refs/heads/${branch}`]);
} catch (error) { console.error(error.message); process.exitCode = 1; }
