// @ref LLP 1086 D12 — launchers use the pinned Bun independently of login-shell PATH.
/** Self-contained so generated app launchers carry the same resolver. */
export function ensureBun(pin) {
  const {existsSync, realpathSync} = require('node:fs');
  const {dirname, resolve, delimiter} = require('node:path');
  const {homedir} = require('node:os');
  const {spawnSync} = require('node:child_process');
  const versioned = resolve(homedir(), `.bun-${pin}/bin/bun`);
  const candidate = process.env.BUN || (existsSync(versioned) ? versioned : process.execPath);
  const executable = Bun.which(candidate);
  if (!executable) throw new Error(`BUN=${candidate} was not found; use ~/.bun-${pin}/bin/bun`);
  const selected = realpathSync(executable);
  const reply = selected === realpathSync(process.execPath) ? {status: 0, stdout: process.versions.bun}
    : spawnSync(selected, ['--version'], {encoding: 'utf8'});
  const have = String(reply.stdout ?? '').trim().split('.').map(Number), want = pin.split('.').map(Number);
  const different = want.findIndex((n, i) => n !== have[i]);
  if (reply.status !== 0 || have.length < 3 || have.some(n => !Number.isFinite(n)) || (different >= 0 && have[different] < want[different]))
    throw new Error(`Bun ${String(reply.stdout ?? '').trim() || candidate} is older than ${pin} or unavailable; set BUN=~/.bun-${pin}/bin/bun`);
  process.env.BUN = selected;
  process.env.PATH = [dirname(selected), ...(process.env.PATH ?? '').split(delimiter).filter(p => p !== dirname(selected))].join(delimiter);
  if (selected !== realpathSync(process.execPath)) {
    const child = spawnSync(selected, process.argv.slice(1), {stdio: 'inherit', env: process.env});
    if (child.error) throw child.error;
    process.exit(child.status ?? 1);
  }
  return selected;
}
