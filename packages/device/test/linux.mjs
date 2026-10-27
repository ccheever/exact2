// @exact/device on the Linux host (LLP 1133.031 §12 stage 4), and the
// Windows implementation's compile:
//
// - The readers over recorded answers (`linux/readers.rs`, run by
//   `rustc --test`): a laptop's `/sys` tree, a desktop's with no battery and
//   one with a mouse's battery; rtnetlink route replies (an IPv6-only Wi-Fi
//   link, a WireGuard default route in its own table); UPower's states; the
//   D-Bus framing of a property read.
// - The fixture app made outside the repo, its Linux crate built with the
//   package's Rust composed by the build, and driven headless. Bound
//   (`--bind device`) the implementation answers: on a Mac, with no `/sys`
//   or rtnetlink, the specification's no-battery values and `type`
//   `unknown`; on Linux, this machine's values, checked by range. Under the
//   agent unbound the declaration answers.
// - With `--cross`, the package's Rust checked for the targets this Mac
//   cannot run, with Zig as the C compiler, as the async lane's tier 2
//   checks the Linux apps and Windows Desk: two crates added to the fixture,
//   each the fixture's plan and the build's composition of the package with
//   no data module, one over `exact_linux::run_composed` for
//   `x86_64-unknown-linux-gnu` (compiling `system.rs`), and one over
//   `exact_windows::run_composed` for x64 (`x86_64-pc-windows-gnu`) and
//   ARM64 (`aarch64-pc-windows-gnullvm`) (compiling `windows.rs`).
//
// usage: bun packages/device/test/linux.mjs [--cross] [<app dir>]
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, run } from './fixture.mjs';

const argv = process.argv.slice(2), cross = argv.includes('--cross');
const given = argv.find((a) => !a.startsWith('--')) && resolve(argv.find((a) => !a.startsWith('--')));
const { check, failed } = checker();

{
  const out = mkdtempSync(join(tmpdir(), 'exact-device-readers-'));
  run('rustc', ['--edition', '2021', '--test', resolve(pkg, 'linux/readers.rs'), '-o', join(out, 'readers')], out);
  const lines = run(join(out, 'readers'), [], out).split('\n').filter((l) => /^test .* \.\.\. /.test(l));
  for (const line of lines) check('readers', line.replace(/^test tests::| \.\.\. .*$/g, '').replaceAll('_', ' '), line.endsWith('ok'), line);
  check('readers', 'every recorded case ran', lines.length >= 5, lines.length);
  rmSync(out, { recursive: true, force: true });
}

const app = given && existsSync(given) ? given : makeFixture(...(given ? [given] : [])).app;
run('bun', ['exact.mjs', 'linux'], app);

const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', ...args], app).trim().split('\n').map((l) => JSON.parse(l));
const texts = (tree) => {
  const byId = new Map((tree.nodes ?? []).map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? (n.children ?? []).map((c) => text(byId.get(c))).join('');
  return Object.fromEntries((tree.nodes ?? []).filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
const here = process.platform === 'linux' ? 'linux' : `linux host on ${process.platform}`;

{
  const [, , tree, , , after, logs] = drive('--bind', 'device', 'clock settle', 'clock +1500 real', 'tree', 'tap toggle', 'clock settle', 'tree', 'logs');
  const t = texts(tree);
  console.log(`RECORD ${here}: battery "${t.battery}", network "${t.network}", power "${t.power}"`);
  if (process.platform === 'linux') {
    const level = Number(t.battery?.split(' ')[0]);
    check(here, 'bound, the battery answers a level in hundredths', level >= 0 && level <= 1 && /(true|false)$/.test(t.battery), t.battery);
    check(here, 'bound, the network answers a route\'s link', /^(wifi|ethernet|cellular|other|none) false$/.test(t.network), t.network);
    check(here, 'bound, low power answers and no thermal state', /^(true|false) none$/.test(t.power), t.power);
  } else {
    check(here, 'bound, no /sys: the specification\'s no-battery values', t.battery === '1 true', t.battery);
    check(here, 'bound, no rtnetlink: type unknown', t.network === 'unknown false', t.network);
    check(here, 'bound, no portal or daemon: no low power, no thermal state', t.power === 'false none', t.power);
  }
  const lines = JSON.stringify(logs);
  check(here, 'the runner bound it here', lines.includes('modules bound here: device'), lines.slice(0, 400));
  check(here, 'the last reader gone, getBattery and connection are released', texts(after).battery === 'off' && ['getBattery', 'connection'].every((n) => lines.includes(`release device.${n}`)), lines.slice(-600));
}
{
  const [, tree, , , pref] = drive('clock settle', 'tree', 'prefer extension device.powerState={"lowPowerMode":true}', 'clock settle', 'tree');
  check('agent', 'unbound, the declaration answers', texts(tree).battery === '1 true' && texts(tree).network === 'unknown false' && texts(tree).power === 'false none', JSON.stringify(texts(tree)));
  check('agent', 'a preferred value reaches the reader', texts(pref).power === 'true none', texts(pref).power);
}
if (cross) {
  // A crate per OS beside the fixture's own, which composes the package as the host's build does but runs no data
  // module: Hermes has no Windows build, and its Linux bundle is not one this Mac needs. The bake still runs (its
  // `packages.rs` is what composes `linux/lib.rs`), so the package's Rust is compiled for each target.
  const name = JSON.parse(readFileSync(join(app, 'linux/Cargo.toml'), 'utf8').match(/^name = (".*")$/m)[1]).replace(/-linux$/, '');
  const exact2 = readFileSync(join(app, 'linux/Cargo.toml'), 'utf8').match(/^exact-linux = \{ path = "(.*)\/host\/linux" \}$/m)[1];
  const run_composed = { linux: 'exact_linux::run_composed::<NoData>(PLAN, COMPAT, composed)', windows: 'exact_windows::run_composed::<NoData>("Fixture", Default::default(), PLAN, COMPAT, composed)' };
  for (const os of ['linux', 'windows']) {
    const dir = join(app, `check-${os}`);
    mkdirSync(join(dir, 'src'), { recursive: true });
    writeFileSync(join(dir, 'Cargo.toml'), `[package]
name = "${name}-check-${os}"
version.workspace = true
edition.workspace = true
publish = false
build = "build.rs"

[dependencies]
exact-linux = { path = "${exact2}/host/linux" }
exact-runner = { path = "${exact2}/runner" }
${os === 'windows' ? `exact-windows = { path = "${exact2}/host/windows" }\n` : ''}
[build-dependencies]
exact-js-bake = { path = "${exact2}/js/bake" }
`);
    writeFileSync(join(dir, 'build.rs'), readFileSync(join(app, 'linux/build.rs'), 'utf8').replace('"linux"', `"${os}"`));
    writeFileSync(join(dir, 'src/main.rs'), `//! The fixture's plan and packages for ${os}, with no data module: a compile check.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
include!(concat!(env!("OUT_DIR"), "/packages.rs"));

#[derive(Default)]
struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(&mut self, source: &str, _: &[exact_runner::Value]) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

fn main() {
    let composed = exact_linux::Composed { packages: exact_packages(), ..Default::default() };
    std::process::exit(${run_composed[os]});
}
`);
  }
  const workspace = readFileSync(join(app, 'Cargo.toml'), 'utf8');
  if (!workspace.includes('"check-linux"')) writeFileSync(join(app, 'Cargo.toml'), workspace.replace(/members = \[([^\]]*)\]/, (_, m) => `members = [${m}, "check-linux", "check-windows"]`));
  for (const [os, triple, zig] of [['linux', 'x86_64-unknown-linux-gnu', 'x86_64-linux-gnu'], ['windows', 'x86_64-pc-windows-gnu', 'x86_64-windows-gnu'], ['windows', 'aarch64-pc-windows-gnullvm', 'aarch64-windows-gnu']]) {
    const t = triple.replaceAll('-', '_');
    const env = { ...process.env, CRATE_CC_NO_DEFAULTS: '1', [`CC_${t}`]: `zig cc -target ${zig}`, [`CXX_${t}`]: `zig c++ -target ${zig}` };
    let ok = true, out = '';
    try { run('cargo', ['check', '--keep-going', '--target', triple, '-p', `${name}-check-${os}`], app, env); } catch (e) { ok = false; out = e.message.split('\n').filter((l) => /^(error|warning)|^\s+-->/.test(l)).join('\n').slice(-3000) || e.message.slice(-3000); }
    check(os, `the package's Rust compiles, composed by the build for ${triple}`, ok, out);
  }
}
if (!given) rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
