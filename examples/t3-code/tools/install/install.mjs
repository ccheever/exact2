#!/usr/bin/env bun
// Builds the clone and installs it for this user, in one command from the Exact repository root:
//
//   bun examples/t3-code/tools/install/install.mjs
//
// It installs `/Applications/T3 Code (Exact).app`, a distribution bundle (`--bundle --distribution`), whose server
// keeps its data in `~/.t3-exact` (Info.plist `LSEnvironment.T3CODE_HOME`, the user's absolute path), apart from
// T3 Code's own `~/.t3`. Then it signs the bundle again, since the plist changed, with the identity the build
// signed it with, and registers it with LaunchServices so the `t3code` links open it.
//
// Its steps are the README's "Build and run" ones: dependencies, generated sources, the terminal page, the
// embedded server (downloaded and verified once, tools/stage-runtime), then the bundle.
//
// For a trial install (a build check; launch it only on a Mac without the real install):
//   --to <dir>       the Applications folder (default /Applications)
//   --id <bundle id> the bundle identifier (default app.json's), so LaunchServices and the Keychain keep the two apart.
//                    Exact keeps an app's saved state under app.json's id whatever the bundle id is, so a launched
//                    copy shares the installed app's (its drafts, its server record: its launch stops that app's
//                    server as one left behind)
//   --home <dir>     the server's data folder (default ~/.t3-exact)
//   --port <n>       a fixed server port (LSEnvironment.T3CODE_PORT; default: the app scans from 3773)
//   --no-build       installs the bundle the last build made
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const root = resolve(app, '..', '..');
const manifest = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
const NAME = `${manifest.app.name}.app`;
const PLIST = '/usr/libexec/PlistBuddy';
const LSREGISTER = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister';

export function parseArgs(argv, home = homedir()) {
  const options = { to: '/Applications', id: manifest.app.id, home: join(home, '.t3-exact'), port: null, build: true };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i], value = () => {
      const next = argv[++i];
      if (next === undefined || next.startsWith('--')) throw new Error(`${flag} needs a value`);
      return next;
    };
    if (flag === '--to') options.to = resolve(value());
    else if (flag === '--id') options.id = value();
    else if (flag === '--home') options.home = resolve(value().replace(/^~(?=$|\/)/, home));
    else if (flag === '--port') options.port = Number(value());
    else if (flag === '--no-build') options.build = false;
    else throw new Error(`unknown option ${flag}`);
  }
  if (!/^[A-Za-z0-9.-]+$/.test(options.id)) throw new Error(`--id ${options.id}: a bundle identifier is letters, digits, dots and hyphens`);
  if (options.port !== null && !(Number.isInteger(options.port) && options.port > 0 && options.port < 65536)) throw new Error('--port must be a TCP port');
  // T3 Code's own data folder belongs to T3 Code; two servers on one folder would contend for it.
  if (options.home === join(home, '.t3')) throw new Error(`--home ${options.home} is T3 Code's own data folder`);
  if (!isAbsolute(options.home)) throw new Error('--home must be an absolute path');
  return options;
}

const run = (command, args, env = process.env) => {
  console.log(`install: ${[command, ...args].join(' ')}`);
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', env });
  if (result.status !== 0) throw new Error(`${command} ${args.join(' ')} failed (exit ${result.status ?? result.signal})`);
};
const read = (command, args) => spawnSync(command, args, { encoding: 'utf8' });

/** The bundle `host/apple/build.mjs --bundle` made: `<target>/clients/<source key>/<app id>/macos/<name>.app`, the newest one. */
export function builtBundle(target = join(app, 'target')) {
  const clients = join(target, 'clients');
  const found = existsSync(clients) ? readdirSync(clients).map(key => join(clients, key, manifest.app.id, 'macos', NAME)).filter(path => existsSync(path)) : [];
  found.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (!found[0]) throw new Error(`no ${NAME} under ${clients}; build it first`);
  return found[0];
}

/** The identity a bundle is signed with: its leaf `Authority=` name and that identity's SHA-1 here, or ad hoc. */
function signingIdentity(bundle) {
  const name = /^Authority=(.*)$/m.exec(read('codesign', ['-dvv', bundle]).stderr)?.[1];
  if (!name) return { name: 'an ad hoc signature', sign: '-' };
  const listed = read('security', ['find-identity', '-v', '-p', 'codesigning']).stdout ?? '';
  const sha1 = [...listed.matchAll(/^\s*\d+\) ([0-9A-F]{40}) "(.*)"$/gm)].find(match => match[2] === name)?.[1];
  if (!sha1) throw new Error(`the bundle is signed by ${name}, which is not a signing identity in this keychain`);
  return { name, sign: sha1 };
}
const escapeRegExp = text => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

function install(options) {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') throw new Error('the clone builds for macOS on Apple silicon only');
  if (options.build) {
    const env = { ...process.env, EXACT_APP_DIR: app };
    run(process.execPath, ['install', '--frozen-lockfile'], env);
    run(process.execPath, ['examples/t3-code/tools/generate/generate.mjs'], env);
    run(process.execPath, ['examples/t3-code/terminal-host/build.mjs'], env);
    run(process.execPath, ['examples/t3-code/tools/stage-runtime/stage-runtime.mjs'], env);
    run(process.execPath, ['host/apple/build.mjs', 't3-code-macos', '--bundle', '--distribution'], env);
  }
  const built = builtBundle(), identity = signingIdentity(built), installed = join(options.to, NAME);
  if (read('pgrep', ['-f', escapeRegExp(`${installed}/Contents/MacOS/`)]).status === 0) throw new Error(`${installed} is running; quit it first`);
  mkdirSync(options.to, { recursive: true });
  rmSync(installed, { recursive: true, force: true });
  run('ditto', [built, installed]);
  const plist = join(installed, 'Contents', 'Info.plist');
  const set = (key, type, value) => {
    if (read(PLIST, ['-c', `Print :${key}`, plist]).status === 0) run(PLIST, ['-c', `Set :${key} ${value}`, plist]);
    else run(PLIST, ['-c', `Add :${key} ${type} ${value}`, plist]);
  };
  if (options.id !== manifest.app.id) set('CFBundleIdentifier', 'string', options.id);
  if (read(PLIST, ['-c', 'Print :LSEnvironment', plist]).status !== 0) run(PLIST, ['-c', 'Add :LSEnvironment dict', plist]);
  set('LSEnvironment:T3CODE_HOME', 'string', options.home);
  if (options.port !== null) set('LSEnvironment:T3CODE_PORT', 'string', String(options.port));
  // The plist is part of the signature. Re-signing keeps the build's entitlements, flags and requirements; a new
  // identifier needs requirements of its own, which the default designated requirement gives.
  const keep = options.id === manifest.app.id ? 'entitlements,flags,requirements' : 'entitlements,flags';
  run('codesign', ['--force', `--preserve-metadata=${keep}`, '--sign', identity.sign, installed]);
  run('codesign', ['--verify', '--strict', installed]);
  run(LSREGISTER, ['-f', installed]);
  console.log(`install: ${installed} (${options.id}, signed by ${identity.name}); its server keeps its data in ${options.home}`);
}

if (import.meta.main) {
  try { install(parseArgs(process.argv.slice(2))); }
  catch (error) { console.error(`install: ${error.message}`); process.exit(1); }
}
