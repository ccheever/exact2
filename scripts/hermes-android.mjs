// The Hermes bundle for exact2's Android host (LLP 1107): a TypeScript data
// module runs on Android in the same lean Hermes VM as on every native host,
// but the pinned Ibex release ships no `aarch64-linux-android` bundle yet.
// This builds one with Ibex's own release script, whose Android target is the
// pending Ibex patch (`aarch64-linux-android` in
// scripts/build-hermes-vanilla-release.sh): the pinned Hermes commit, the NDK,
// Unicode Lite and no Intl (no JNI, no ICU), and the receipt the
// `hermes-lean-sys` resolver verifies. The bundle unpacks into
// ~/.cache/exact/hermes-android/<archive sha256>/, `current` names the newest,
// and `agent-android.mjs build` hands it to Cargo as
// HERMES_LEAN_SYS_DIR_aarch64_linux_android, so the host's own pinned bundle is
// untouched.
//
//   bun scripts/hermes-android.mjs build     IBEX_DIR: an Ibex checkout with the Android target
//   bun scripts/hermes-android.mjs path      the installed bundle, or nothing
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readlinkSync, renameSync, rmSync, symlinkSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

export const HERMES_ANDROID_CACHE = resolve(process.env.EXACT_HERMES_ANDROID_CACHE ?? resolve(homedir(), '.cache/exact/hermes-android'));

/** The installed bundle's directory, or null. */
export function hermesAndroidBundle() {
  const current = resolve(HERMES_ANDROID_CACHE, 'current');
  if (!existsSync(current)) return null;
  const root = resolve(HERMES_ANDROID_CACHE, readlinkSync(current));
  return existsSync(resolve(root, 'hermes-input-receipt.json')) ? root : null;
}

function build() {
  const candidates = [process.env.IBEX_DIR, resolve(homedir(), 'projects/ibex2-android-hermes'), resolve(homedir(), 'projects/ibex2')].filter(Boolean);
  const ibex = candidates.find((dir) => {
    const script = resolve(dir, 'scripts/build-hermes-vanilla-release.sh');
    return existsSync(script) && readFileSync(script, 'utf8').includes('aarch64-linux-android)');
  });
  if (!ibex) throw new Error(`no Ibex checkout whose scripts/build-hermes-vanilla-release.sh has the aarch64-linux-android target (set IBEX_DIR; looked in ${candidates.join(', ')})`);
  mkdirSync(HERMES_ANDROID_CACHE, { recursive: true });
  const archive = resolve(HERMES_ANDROID_CACHE, 'hermes-vanilla-aarch64-linux-android.tar.gz');
  const built = spawnSync('bash', ['scripts/build-hermes-vanilla-release.sh', 'aarch64-linux-android', archive], { cwd: ibex, stdio: 'inherit' });
  if (built.status !== 0) process.exit(built.status ?? 1);
  const digest = createHash('sha256').update(readFileSync(archive)).digest('hex');
  const root = resolve(HERMES_ANDROID_CACHE, digest), staging = `${root}.partial`;
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  const unpacked = spawnSync('tar', ['-xzf', archive, '-C', staging], { stdio: 'inherit' });
  if (unpacked.status !== 0) process.exit(unpacked.status ?? 1);
  rmSync(root, { recursive: true, force: true });
  renameSync(staging, root);
  const current = resolve(HERMES_ANDROID_CACHE, 'current');
  rmSync(current, { force: true });
  symlinkSync(digest, current);
  console.log(root);
}

if (import.meta.main) {
  const verb = process.argv[2];
  if (verb === 'build') build();
  else if (verb === 'path') console.log(hermesAndroidBundle() ?? '');
  else { console.error('usage: bun scripts/hermes-android.mjs build | path'); process.exit(2); }
}
