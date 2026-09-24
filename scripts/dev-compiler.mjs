// Captured native Contract compilers for the web dev loop. @ref LLP 1007 §6.
// Cargo still builds every new source/configuration identity. Warm invocations
// hash the recorded inputs and run immutable bytes without starting Cargo.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { homedir } from 'node:os';
import { assertOwnTarget, cargoDefaultBinary, compilerPaths, unitDepInfo } from './app.mjs';
import { filesystemLock } from './filesystem.mjs';

const ROOT = resolve(import.meta.dir, '..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const VALIDATOR = hash(readFileSync(import.meta.path));
const under = (root, path) => path === root || path.startsWith(root + '/');
const flags = workspace => existsSync(resolve(workspace, 'Cargo.lock')) ? ['--locked', '--offline'] : [];
const pause = ms => new Promise(ok => setTimeout(ok, ms));

function eligible(workspaces, env) {
  if (env.RUSTC || env.CARGO_BUILD_RUSTC || env.CARGO_BUILD_TARGET || env.CARGO_BUILD_BUILD_DIR
    || Object.keys(env).some(name => /^(?:RUSTC|CARGO_BUILD_RUSTC).*WRAPPER$/.test(name) && env[name])) return false;
  const configs = new Set([resolve(env.CARGO_HOME ?? resolve(homedir(), '.cargo'))]);
  for (let workspace of workspaces) for (;;) {
    configs.add(resolve(workspace, '.cargo'));
    if (dirname(workspace) === workspace) break;
    workspace = dirname(workspace);
  }
  if ([...configs].some(path => ['config','config.toml'].some(name => existsSync(resolve(path, name))))) return false;
  // A new wildcard workspace member can change resolution without editing an
  // already recorded manifest. Leave such workspaces to Cargo.
  return workspaces.every(workspace => !(Bun.TOML.parse(readFileSync(resolve(workspace, 'Cargo.toml'), 'utf8')).workspace?.members ?? [])
    .some(member => /[*?[{]/.test(member)));
}
function metadata(workspace, env) {
  const result = spawnSync('cargo', ['metadata', ...flags(workspace), '--format-version', '1'], {cwd:workspace, env, encoding:'utf8', maxBuffer:128*1024*1024});
  if (result.status !== 0) throw new Error(result.stderr || 'compiler cargo metadata failed');
  return JSON.parse(result.stdout);
}
function select(request, env) {
  const requested = metadata(request.workspace, env);
  const pkg = requested.packages.find(p => p.name === request.package);
  const bin = cargoDefaultBinary(pkg);
  const workspace = bin ? request.workspace : ROOT;
  const graph = workspace === request.workspace ? requested : metadata(workspace, env);
  return {workspace, package:bin ? request.package : 'exact-web', bin:bin ?? 'exact-dev',
    profile:Bun.TOML.parse(readFileSync(resolve(workspace, 'Cargo.toml'), 'utf8')).profile?.['logic-dev'] ? 'logic-dev' : 'release',
    graph, requested, selector:pkg ? [pkg.manifest_path, resolve(dirname(pkg.manifest_path), 'src')] : []};
}
function versions(workspace, env) {
  return ['rustc', 'cargo'].map(command => {
    const result = spawnSync(command, ['-Vv'], {cwd:workspace, env, encoding:'utf8'});
    if (result.status !== 0) throw new Error(`${command} did not report its version`);
    const executable = Bun.which(command, {PATH:env.PATH});
    if (!executable) throw new Error(`${command} executable is missing`);
    return {version:result.stdout, executable, digest:hash(readFileSync(executable))};
  });
}
function signature(request, selection, inputs, env, tools, output) {
  const source = createHash('sha256'), visited = new Set();
  const visit = path => {
    path = resolve(path);
    if (visited.has(path)) return;
    visited.add(path);
    if (under(output, path) || under(path, output)) throw new Error('compiler source watch contains build output');
    source.update(JSON.stringify(path));
    let info;
    try { info = lstatSync(path); } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      source.update('missing'); return;
    }
    if (info.isSymbolicLink()) throw new Error('linked compiler input');
    if (info.isDirectory()) {
      const names = readdirSync(path).sort(); source.update(JSON.stringify(names));
      for (const name of names) visit(resolve(path, name));
    } else if (info.isFile()) source.update(hash(readFileSync(path)));
    else throw new Error('nonregular compiler input');
  };
  for (const path of inputs.paths) visit(path);
  const bytes = source.digest('hex');
  const names = [...new Set([...inputs.names, ...Object.keys(env).filter(name =>
    /^(?:CARGO|RUST|CC|CXX|AR|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|SDKROOT|MACOSX_DEPLOYMENT_TARGET|PATH$|HOME$)/.test(name))])].sort();
  return {source:bytes, key:hash(JSON.stringify({request, selection, tools, source:bytes, validator:VALIDATOR,
    environment:names.map(name => [name, env[name] ?? null])}))};
}
function inputsOf(selection, messages, output) {
  const paths = new Set(selection.selector), names = new Set(), packages = new Map(selection.graph.packages.map(p => [p.id, p]));
  // All resolution manifests count, not just the packages linked this time.
  for (const graph of [selection.requested, selection.graph]) {
    for (const pkg of graph.packages) paths.add(resolve(pkg.manifest_path));
    for (let at = graph.workspace_root;; at = dirname(at)) {
      for (const name of ['Cargo.toml','Cargo.lock','rust-toolchain','rust-toolchain.toml']) paths.add(resolve(at, name));
      if (dirname(at) === at) break;
    }
  }
  const remember = path => { path = resolve(path); if (!under(output, path)) paths.add(path); };
  for (const message of messages) {
    if (message.reason === 'compiler-artifact') {
      const pkg = packages.get(message.package_id);
      if (!pkg) throw new Error('compiler artifact outside metadata graph');
      let dep;
      if (message.target.kind.includes('custom-build')) {
        const directory = dirname(message.executable ?? message.filenames[0]);
        const candidates = readdirSync(directory).filter(name => name.endsWith('.d')).map(name => readFileSync(resolve(directory, name), 'utf8'))
          .filter(text => compilerPaths(text, selection.workspace).includes(resolve(message.target.src_path)));
        if (candidates.length !== 1) throw new Error('ambiguous build-script dep-info');
        dep = candidates[0];
      } else dep = readFileSync(unitDepInfo(message, selection.workspace, selection.graph), 'utf8');
      for (const path of compilerPaths(dep, selection.workspace)) remember(path);
      for (const match of dep.matchAll(/^# env-dep:([^=\r\n]+)(?:=[^\r\n]*)?$/gm)) names.add(match[1]);
    } else if (message.reason === 'build-script-executed') {
      const pkg = packages.get(message.package_id);
      if (!pkg) throw new Error('compiler build script outside metadata graph');
      // External native tool/link inputs need their own admission; Cargo remains
      // authoritative for those configurations instead of guessing their closure.
      if (message.linked_libs.length || message.linked_paths.length) throw new Error('compiler links external native artifacts');
      const text = readFileSync(resolve(message.out_dir, '../output'), 'utf8');
      const changed = [...text.matchAll(/^cargo::?rerun-if-changed=(.*)$/gm)];
      for (const [,path] of changed) {
        const input = resolve(dirname(pkg.manifest_path), path);
        if (under(output, input)) throw new Error('build script watches generated output');
        paths.add(input);
      }
      if (!changed.length) paths.add(dirname(pkg.manifest_path));
      for (const match of text.matchAll(/^cargo::?rerun-if-env-changed=(.*)$/gm)) names.add(match[1]);
    }
  }
  return {paths:[...paths].sort(), names:[...names].sort()};
}
async function build(selection, env, output) {
  const child = spawn('cargo', ['build', '-q', ...flags(selection.workspace), '--profile', selection.profile,
    '-p', selection.package, '--bin', selection.bin, '--target-dir', output, '--message-format=json'],
  {cwd:selection.workspace, env, stdio:['ignore','pipe','inherit']});
  const stop = () => child.kill('SIGKILL'); process.once('exit', stop);
  let text = '';
  child.stdout.on('data', chunk => { text += chunk; if (text.length > 128*1024*1024) child.kill('SIGKILL'); });
  try {
    const code = await new Promise((ok, fail) => { child.once('error', fail); child.once('exit', ok); });
    const messages = text.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
    for (const m of messages) if (m.reason === 'compiler-message' && m.message.rendered) process.stderr.write(m.message.rendered);
    if (code !== 0) throw new Error(`native compiler build exited ${code}`);
    const artifact = messages.find(m => m.reason === 'compiler-artifact' && m.target.name === selection.bin && m.target.kind.includes('bin') && m.executable);
    if (!artifact || !under(output, resolve(artifact.executable))) throw new Error('Cargo did not produce the requested private compiler');
    return {messages, executable:artifact.executable};
  } finally { process.removeListener('exit', stop); }
}
async function claim(directory, use) {
  mkdirSync(directory, {recursive:true});
  const start = Date.now();
  for (;;) {
    let entered = false;
    try { return await filesystemLock(directory, 'build/.lock', () => { entered = true; return use(); }); }
    catch (error) {
      if (entered || !error.message.includes('locked by another')) throw error;
      if (Date.now() - start > 60000) throw new Error(`native compiler build busy (${directory})`);
      await pause(50);
    }
  }
}

/** A command whose runtime arguments are the Contract source and output plan. */
export async function compilerCommand(request, runtimeEnv = process.env) {
  request = {workspace:resolve(request.workspace), target:resolve(request.target), package:request.package};
  assertOwnTarget(request.target, request.workspace);
  const directory = resolve(request.target, 'dev-compilers', hash(JSON.stringify(request)).slice(0, 24));
  const output = resolve(directory, 'cargo'), receiptPath = resolve(directory, 'captured.json');
  const env = {...runtimeEnv, CARGO_TARGET_DIR:output};
  let tools, previous, before;
  const reuse = () => {
    previous = before = undefined;
    try {
      previous = JSON.parse(readFileSync(receiptPath, 'utf8'));
      if (previous.version !== 1 || JSON.stringify(previous.request) !== JSON.stringify(request)
        || !eligible([request.workspace, previous.selection.workspace], env)) return;
      tools = versions(previous.selection.workspace, env);
      before = signature(request, previous.selection, previous.inputs, env, tools, output);
      if (previous.signature !== before.key || !/^[a-f0-9]{64}$/.test(previous.digest)) return;
      const executable = resolve(directory, previous.digest);
      if (hash(readFileSync(executable)) === previous.digest) return {command:executable, args:[], cwd:previous.selection.workspace};
    } catch { /* Missing, changed, damaged or unsupported captures use Cargo. */ }
  };
  const warm = reuse(); if (warm) return warm;
  return claim(directory, async () => {
    const warm = reuse(); if (warm) return warm;
    let selected = select(request, env);
    const fallback = {command:'cargo', args:['run','-q',...flags(selected.workspace),'--release','-p',selected.package,'--bin',selected.bin,'--'], cwd:selected.workspace};
    if (!eligible([request.workspace, selected.workspace], env)) return fallback;
    tools = versions(selected.workspace, env);
    // The target is private and locked. Clear Cargo fingerprints on every miss:
    // otherwise a preserved-mtime source edit could bless stale Cargo output.
    let result;
    for (let attempt = 0; attempt < 2; attempt++) {
      if (attempt) selected = select(request, env);
      const selection = {workspace:selected.workspace, package:selected.package, bin:selected.bin, profile:selected.profile};
      rmSync(resolve(output, selected.profile, '.fingerprint'), {recursive:true, force:true});
      const built = await build(selected, env, output);
      const bytes = readFileSync(built.executable), digest = hash(bytes), executable = resolve(directory, digest);
      const temporary = `${executable}.${process.pid}.tmp`;
      try { writeFileSync(temporary, bytes, {mode:0o755}); renameSync(temporary, executable); }
      finally { rmSync(temporary, {force:true}); }
      result = {command:executable, args:[], cwd:selected.workspace};
      let inputs, after;
      try {
        if (!eligible([request.workspace, selected.workspace], env)) return result;
        inputs = inputsOf(selected, built.messages, output);
        tools = versions(selected.workspace, env);
        after = signature(request, selection, inputs, env, tools, output);
      }
      catch (error) { console.error(`native compiler capture unavailable: ${error.message}`); return result; }
      const verified = before && before.key === after.key && JSON.stringify(previous.inputs) === JSON.stringify(inputs)
        && JSON.stringify(previous.selection) === JSON.stringify(selection);
      const receipt = {version:1, request, selection, inputs, source:after.source, signature:verified ? after.key : null, digest};
      const pending = `${receiptPath}.${process.pid}.tmp`;
      try { writeFileSync(pending, JSON.stringify(receipt)); renameSync(pending, receiptPath); }
      finally { rmSync(pending, {force:true}); }
      if (verified) return result;
      // Discovery seeds the complete closure. Only a subsequent build with
      // identical inputs on both sides can promote it to a reusable capture.
      previous = receipt; before = after;
    }
    return result;
  });
}

if (import.meta.main) {
  try {
    const [configuration, ...args] = process.argv.slice(2);
    const command = await compilerCommand(JSON.parse(configuration));
    const child = spawn(command.command, [...command.args, ...args], {cwd:command.cwd, env:process.env, stdio:'inherit'});
    process.once('exit', () => child.kill('SIGKILL'));
    child.once('error', error => { console.error(error.message); process.exit(1); });
    child.once('exit', code => process.exit(code ?? 1));
  } catch (error) { console.error(error.message); process.exit(1); }
}
