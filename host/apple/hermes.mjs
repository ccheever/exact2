// The lean Hermes an iOS app links (LLP 1036.001 D5): its archives, and the
// once-per-machine build that provisions them. Split from build.mjs.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { hermesIos } from '../../scripts/app.mjs';

/** The archives js/build.rs links from each platform's CMake build. */
export const HERMES_IOS_ARCHIVES = ['lib/libhermesvmlean_a.a', 'jsi/libjsi.a', 'external/boost/boost_1_86_0/libs/context/libboost_context.a'];
// Provisions the source and the host compiler when this machine has neither
// (a pristine clone at the pin, hermesc for the host), checks them, then
// builds and publishes one platform; ibex's lock is held throughout.
const HERMES_IOS_SCRIPT = `set -eu
src=$1 pin=$2 root=$3 platform=$4 sdk=$5 arch=$6; shift 6
out=$root/$platform build=$root/.build-$platform
fix="ibex ./scripts/build-hermes.sh --vanilla $pin puts it back, or remove $src and build again"
if [ -e "$out" ]; then
  for a; do [ -f "$out/$a" ] || { echo "$out lacks $a: remove it and build again" >&2; exit 1; }; done
  exit 0
fi
if [ ! -d "$src/.git" ]; then
  echo "host/apple: cloning facebook/hermes at $pin into $src, once for this machine" >&2
  rm -rf "$src.clone" && git clone --quiet --filter=blob:none --no-checkout https://github.com/facebook/hermes.git "$src.clone"
  git -C "$src.clone" checkout --quiet --detach "$pin" && mv "$src.clone" "$src"
fi
head=$(git -C "$src" rev-parse HEAD)
[ "$head" = "$pin" ] || { echo "the Hermes source $src is at $head; js/build.rs pins $pin: $fix" >&2; exit 1; }
[ -z "$(git -C "$src" status --porcelain --untracked-files=no)" ] || { echo "the Hermes source $src is patched; the lean VM is pristine upstream: $fix" >&2; exit 1; }
if [ ! -f "$src/build_host_hermesc/ImportHostCompilers.cmake" ]; then
  echo "host/apple: building the host hermesc in $src/build_host_hermesc, once for this machine" >&2
  # ibex's host configure (build-hermes.sh): no test suite, which CMake 4 refuses.
  cmake -S "$src" -B "$src/build_host_hermesc" -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_SYSROOT=macosx \\
    -DCMAKE_OSX_ARCHITECTURES="$(uname -m)" -DHERMES_ENABLE_TEST_SUITE=false \\
    -DHAVE_CXX_ATOMICS_WITHOUT_LIB=ON -DHAVE_CXX_ATOMICS64_WITHOUT_LIB=ON >/dev/null
  cmake --build "$src/build_host_hermesc" --target hermesc -j "$(sysctl -n hw.ncpu)" >/dev/null
fi
cmake -S "$src" -B "$build" -DHERMES_APPLE_TARGET_PLATFORM="$sdk" -DCMAKE_OSX_ARCHITECTURES="$arch" \\
  -DCMAKE_OSX_DEPLOYMENT_TARGET=17.0 -DHERMES_ENABLE_DEBUGGER=OFF -DHERMES_ENABLE_INTL=ON \\
  -DHERMES_ENABLE_TEST_SUITE=OFF -DHERMES_ENABLE_BITCODE=OFF -DHERMES_BUILD_APPLE_FRAMEWORK=OFF \\
  -DHERMES_BUILD_SHARED_JSI=OFF -DIMPORT_HOST_COMPILERS="$src/build_host_hermesc/ImportHostCompilers.cmake" \\
  -DCMAKE_BUILD_TYPE=MinSizeRel
cmake --build "$build" --target hermesvmlean_a jsi boost_context -j "$(sysctl -n hw.ncpu)"
stage=$(mktemp -d "$root/.stage-$platform.XXXXXX") && chmod 755 "$stage"
for a; do mkdir -p "$stage/$(dirname "$a")"; cp "$build/$a" "$stage/$a"; done
mv "$stage" "$out"
rm -rf "$build"`;

/** An iOS app with an `app.ts` links lean Hermes for its platform. Missing
 * from the per-pin cache every checkout and outside app shares, it is built
 * here, once per machine: only that platform's three CMake targets, from
 * the pristine source cache ibex shares (cloned at the pin when absent) with
 * its host compiler (built when absent), under ibex's own source-build
 * lock, so neither build moves the checkout under the other. CMake is the
 * one prerequisite, named in one message when it is missing.
 * EXACT_HERMES_IOS_DIR's archives are provisioned elsewhere; js/build.rs
 * refuses missing ones. @ref LLP 1036.001 D5 */
export function provisionHermesIos(platform, env = process.env) {
  const { pin, root, cached } = hermesIos(env), out = resolve(root, platform);
  if (!cached || HERMES_IOS_ARCHIVES.every(a => existsSync(resolve(out, a)))) return;
  const cache = resolve(env.HOME ?? homedir(), '.cache/exact');
  // The one thing a first build cannot provision itself (shop F19): said
  // once, before anything is cloned or built.
  if (spawnSync('cmake', ['--version'], { env }).status !== 0) {
    throw new Error(`lean Hermes for ${platform} is built once on this machine (facebook/hermes ${pin.slice(0, 12)}, into ${out}) and needs CMake, which is not installed. Install it (brew install cmake) and build again; the source and the host compiler are fetched and built here. Or set EXACT_HERMES_IOS_DIR to a directory holding ${platform}/{${HERMES_IOS_ARCHIVES.join(',')}}, or EXACT_JS_ENGINE=stub for an app whose data module need not run.`);
  }
  console.error(`host/apple: building lean Hermes for ${platform} (facebook/hermes ${pin.slice(0, 12)}) into ${out}, once for this machine`);
  mkdirSync(root, { recursive: true });
  const { SDKROOT, ...clean } = env; // the platform names its own SDK
  const sdk = platform === 'ios' ? 'iphoneos' : 'iphonesimulator', arch = platform === 'ios' || process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const r = spawnSync('perl', ['-MFcntl=:flock', '-e', 'open(my $l, ">>", shift) or die "lock: $!\\n"; flock($l, LOCK_EX) or die "flock: $!\\n"; exit(system(@ARGV) == 0 ? 0 : 1)',
    resolve(cache, 'hermes-source-build.lock'), 'sh', '-c', HERMES_IOS_SCRIPT, 'hermes', resolve(cache, 'hermes/hermes-src'), pin, root, platform, sdk, arch, ...HERMES_IOS_ARCHIVES],
    { env: clean, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (r.status !== 0) throw new Error(`lean Hermes for ${platform} did not build (${r.error?.message ?? `exit ${r.status}`}):\n${`${r.stdout ?? ''}${r.stderr ?? ''}`.trim().split('\n').slice(-30).join('\n')}`);
}
