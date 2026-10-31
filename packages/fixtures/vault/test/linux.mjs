// @fixture/vault on the Linux host, headless, and on the web (LLP 1154 §6 M5): the package built with its library
// (library.mjs) and the fixture app depending on it (fixture.mjs). On a Mac the Linux host is built for the Mac's
// target and links the xcframework's macOS framework, as `bun exact.mjs linux` builds it.
//
// - A copy of the package that declares a wrong hash is refused by the build, both hashes named.
// - The bake joined the shim and wrote the factory; the build carries the library beside the executable.
// - With the fetched library deleted, the drive (fixture.mjs STEPS) gets the library's answer, an ungranted path
//   refused with the grant named, the file opened by the library's SQLite with its row count, and the app's own open
//   of it refused while the library holds it and admitted after; and nothing fetched the library again.
// - The web build serves the declared asset at /assets/@fixture/vault/web/device.wasm as application/wasm.
//
// usage: bun packages/fixtures/vault/test/linux.mjs [<dir>]   (a directory a run made is reused, its apps built again)
import { spawn } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { TARGET } from './library.mjs';
import { checker, drive, fetched, forget, makeFixture, refused, root, run, workDir, wrongHash } from './fixture.mjs';

const given = process.argv[2], dir = workDir(given);
const { pkg, library, wrong: wrongLibrary, app, wrongApp } = makeFixture(dir);
const { check, failed } = checker();
const target = process.platform === 'darwin' ? 'aarch64-apple-darwin' : TARGET;

const wrong = refused('bun', ['exact.mjs', 'linux'], wrongApp) ?? '';
check('linux', 'a declared hash the library does not have is refused, both named', wrong.includes(wrongHash(library, wrongLibrary, target)), wrong.slice(0, 2000));

const built = run('bun', ['exact.mjs', 'linux'], app).trim().split('\n');
const binary = built.at(-2);
// The bake's module.rs for the Linux crate, wherever Cargo put its OUT_DIR; the bake writes the installed shim's
// canonical path.
const modules = [];
const walk = (at, depth) => {
  for (const entry of (() => { try { return readdirSync(at, { withFileTypes: true }); } catch { return []; } })()) {
    const path = join(at, entry.name);
    if (entry.isDirectory() && depth > 0) walk(path, depth - 1);
    else if (entry.name === 'module.rs' && path.endsWith(join('out', 'module.rs'))) modules.push(path);
  }
};
walk(join(app, 'target'), 5);
const module = modules.map(file => readFileSync(file, 'utf8')).find(text => text.includes('exact_native')) ?? '';
check('linux', 'module.rs joins the shim and writes the factory', module.includes(`#[path = "${realpathSync(join(app, 'node_modules/@fixture/vault/native/lib.rs'))}"]`) && module.includes('Box::new(exact_native_shim::Vault::new(APP))'), modules.join(', ') || 'no module.rs found');
const carried = process.platform === 'darwin' ? 'vault.framework/Versions/A/vault' : 'libvault.so';
check('linux', `the build carries the library beside the executable (${carried})`, !!binary && existsSync(join(dirname(binary), carried)), binary);

forget(library);
drive(check, 'linux', app);
check('linux', 'the drive loaded the library the build carries: nothing fetched it again', !fetched(library), 'the cache holds it again');

// The web: the declared asset, captured with the package app.ts imports, served from the build.
run('bun', ['exact.mjs', 'web-build', '--out', 'dist'], app);
const asset = join(app, 'dist/assets/@fixture/vault/web/device.wasm'), bytes = readFileSync(join(pkg, 'web/device.wasm'));
check('web', 'the build holds the declared asset with the package\'s bytes', existsSync(asset) && readFileSync(asset).equals(bytes), asset);
const port = 20000 + Math.floor(Math.random() * 20000);
const server = spawn('bun', [resolve(root, 'host/web/serve.mjs'), join(app, 'dist'), '--port', String(port), '--loopback'], { stdio: 'ignore' });
try {
  let response;
  for (let i = 0; i < 200 && !response; i++) {
    response = await fetch(`http://127.0.0.1:${port}/assets/@fixture/vault/web/device.wasm`).catch(() => null);
    if (!response) await Bun.sleep(50);
  }
  const served = response?.ok ? Buffer.from(await response.arrayBuffer()) : null;
  check('web', 'serve.mjs serves it at /assets/<package>/<path> as application/wasm, a module the browser compiles', response?.headers.get('content-type') === 'application/wasm' && served?.equals(bytes) && WebAssembly.validate(served), `${response?.status} ${response?.headers.get('content-type')}`);
} finally { server.kill(); }

if (!given) rmSync(dir, { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
