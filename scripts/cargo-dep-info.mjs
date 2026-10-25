// Cargo's exact unit dep-info, including the nightly's copied products.
import { basename, dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { existsSync, lstatSync, mkdirSync, statSync, readFileSync, readdirSync, readlinkSync, realpathSync, renameSync, writeFileSync } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
const under = (root, file) => {
  const child = relative(root, file);
  return child === '' || (!isAbsolute(child) && child !== '..' && !child.startsWith('..' + sep));
};

function outputPath(path) {
  try { return realpathSync(path); }
  catch (error) {
    if (!['ENOENT', 'ENOTDIR'].includes(error.code) || dirname(path) === path) throw error;
    return resolve(outputPath(dirname(path)), basename(path));
  }
}

// Cargo follows directory links. Track their contents without reading outputs
// through an alias or opening special files the baker may never use.
function watchedDigest(paths, outputs) {
  const hash = createHash('sha256');
  const excluded = outputs.map(outputPath), ancestors = new Set();
  const row = (...values) => hash.update(JSON.stringify(values) + '\n');
  const visit = path => {
    let info;
    try {
      info = lstatSync(path);
      if (info.isSymbolicLink()) {
        row(path, 'link', readlinkSync(path));
        info = statSync(path);
      }
      const real = realpathSync(path);
      if (excluded.some(root => under(root, real))) { row(path, 'output'); return; }
      if (info.isDirectory()) {
        if (ancestors.has(real)) { row(path, 'cycle'); return; }
        const names = readdirSync(path).sort(); row(path, 'directory', names);
        ancestors.add(real);
        try { for (const name of names) visit(resolve(path, name)); }
        finally { ancestors.delete(real); }
      } else if (info.isFile()) row(path, 'file', createHash('sha256').update(readFileSync(path)).digest('hex'));
      else row(path, info.isSocket() ? 'socket' : info.isFIFO() ? 'fifo'
        : info.isBlockDevice() ? 'block-device' : info.isCharacterDevice() ? 'character-device' : 'other');
    } catch (error) {
      if (error.code === 'ELOOP') row(path, 'cycle');
      else if (error.code === 'ENOENT' || error.code === 'ENOTDIR') row(path, 'missing');
      // Directory watches include unused files. The baker decides which must be readable.
      else if (error.code === 'EACCES' || error.code === 'EPERM')
        row(path, 'unreadable', info?.mode, info?.size, info?.mtimeMs, info?.ctimeMs);
      else throw error;
    }
  };
  for (const path of paths) visit(path);
  return hash.digest('hex');
}

/** Supply Cargo with a content revision for the selected bake, then retain
 * its actual watches after success. File watches still serve direct Cargo. */
export function cargoBakeInputs(pkg, target, profile, env) {
  if (!pkg.targets.some(t => t.kind.includes('custom-build')) || env.EXACT_BAKED) return () => {};
  const key = createHash('sha256').update(JSON.stringify([pkg.manifest_path, target, profile])).digest('hex');
  const file = resolve(env.CARGO_TARGET_DIR, 'bake-inputs', key + '.json');
  let previous;
  try { previous = JSON.parse(readFileSync(file, 'utf8')); }
  catch (error) { if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error; }
  if (previous?.version !== 2) previous = null;
  const outputs = [env.CARGO_TARGET_DIR, env.CARGO_BUILD_BUILD_DIR, env.EXACT_BAKE_OUTPUT, env.OUT_DIR].filter(Boolean).map(path => resolve(path));
  const digest = previous ? watchedDigest(previous.paths, [...outputs, ...previous.outputs]) : null;
  // Keep the revision Cargo consumed when the bake discovers new watches.
  // A new revision must not be a content digest: inputs can change during Cargo,
  // then be restored to the pre-build bytes after Cargo consumed that revision.
  const revision = digest !== null && digest === previous.digest ? previous.revision : randomUUID();
  env.EXACT_BAKE_INPUTS = revision;
  return messages => {
    const script = messages.find(m => m.reason === 'build-script-executed' && m.package_id === pkg.id);
    if (!script) return;
    const stdout = [resolve(script.out_dir, '../output'), resolve(script.out_dir, '../run/stdout')].find(existsSync);
    const paths = [...new Set(readFileSync(stdout, 'utf8').split('\n').flatMap(line => {
      const path = /^cargo::?rerun-if-changed=(.*)$/.exec(line)?.[1];
      return path === undefined ? [] : [resolve(dirname(pkg.manifest_path), path)];
    }))].sort();
    const outDirs = [...new Set(messages.filter(m => m.reason === 'build-script-executed').map(m => m.out_dir))].sort();
    const record = {version:2, paths, outputs:outDirs, digest:watchedDigest(paths, [...outputs, ...outDirs]), revision};
    mkdirSync(dirname(file), {recursive:true});
    writeFileSync(`${file}.${process.pid}.tmp`, JSON.stringify(record));
    renameSync(`${file}.${process.pid}.tmp`, file);
  };
}

export function compilerPaths(text, workspace) {
  const first = text.replace(/\\\r?\n/g, '').split('\n')[0];
  const at = first.indexOf(': ');
  if (at < 0) throw new Error('rustc dep-info has no dependency rule');
  const paths = []; let word = '', escape = false;
  const dependencies = first.slice(at + 2);
  // Nearly every rule has no escape in it, and a receipt reads three hundred of them, each twice.
  if (!dependencies.includes('\\')) return dependencies.split(/\s+/).filter(Boolean).map(path => resolve(workspace, path));
  for (let index = 0; index < dependencies.length; index++) {
    const ch = dependencies[index];
    if (escape) { word += ch; escape = false; }
    // rustc escapes spaces in dep-info but leaves Windows path separators raw.
    else if (ch === '\\' && (process.platform !== 'win32' || /[\s#]/.test(dependencies[index + 1] ?? ''))) escape = true;
    else if (/\s/.test(ch)) { if (word) { paths.push(resolve(workspace, word)); word = ''; } }
    else word += ch;
  }
  if (escape) throw new Error('rustc dep-info has a truncated escape');
  if (word) paths.push(resolve(workspace, word));
  return paths;
}
// One file under two spellings (macOS's /tmp and /private/tmp, a linked
// checkout) is one file: a path that differs as written is compared as the
// file system resolves it (LLP 1143 D6), and only then, since a receipt reads
// hundreds of rules.
// rustc records relative inputs against Cargo's workspace root, even when
// EXACT_APP_DIR makes the invoking directory a nested app in that workspace.
export function unitDepInfo(message, workspace, metadata) {
  const realPaths = new Map();
  const real = path => { let r = realPaths.get(path); if (r === undefined) realPaths.set(path, r = outputPath(path)); return r; };
  const holds = (paths, want) => paths.includes(want) || paths.some(path => real(path) === real(want));
  // Cargo reports copied roots in target_directory and units in build_directory.
  // When the directories nest, the more specific root identifies the file.
  const targetDir = metadata && real(metadata.target_directory), buildDir = metadata && real(metadata.build_directory);
  const intermediate = file => metadata && under(targetDir, real(file))
      && (!under(buildDir, real(file)) || targetDir.length > buildDir.length)
    ? resolve(metadata.build_directory, relative(targetDir, real(file))) : file;
  const source = resolve(message.target.src_path);
  // rustc leaves the single output path raw, but escapes dependency paths.
  const names = (dep, candidate) => {
    const output = dep.slice(0, dep.indexOf(': '));
    return (holds([resolve(workspace, output)], candidate) || holds(compilerPaths('unit: ' + output, workspace), candidate))
      && holds(compilerPaths(dep, workspace), source);
  };
  const sameBytes = (product, file) => product === file || existsSync(product)
    && statSync(product).size === statSync(file).size && readFileSync(product).equals(readFileSync(file));
  for (const file of message.filenames) {
    const stem = basename(file).replace(/\.[^.]+$/, '').replace(/^lib/, '');
    // Selected libraries are copied out of deps; Cargo's sibling summary .d
    // omits env-dep rows. Read rustc's exact unit file, also on a cache hit,
    // when the product beside it is the reported file's bytes: another
    // toolchain building into this profile directory (the JS target's GPU
    // crate on stable; the web nightly's units sit under build/) leaves a
    // deps/<stem>.d for a unit this build did not produce.
    for (const candidate of [resolve(dirname(intermediate(file)), 'deps', stem + '.d'), resolve(dirname(intermediate(file)), stem + '.d')]) {
      if (!existsSync(candidate)) continue;
      if (names(readFileSync(candidate, 'utf8'), candidate) && sameBytes(resolve(dirname(candidate), basename(file)), file)) return candidate;
    }
  }
  // Cargo copies libraries and executables out of deps without reporting their
  // hashed unit filenames. Match the copied bytes; refuse ambiguous evidence.
  for (const file of message.filenames.filter(path => /\.(rlib|a)$/.test(path) || path === message.executable)) {
    const directory = resolve(dirname(intermediate(file)), 'deps');
    if (!existsSync(directory)) continue;
    const executable = file === message.executable;
    const extension = executable ? (file.endsWith('.exe') ? '.exe' : '') : file.slice(file.lastIndexOf('.'));
    const stem = executable ? message.target.name.replaceAll('-', '_') : basename(file, extension);
    const bytes = readFileSync(file), matches = [];
    for (const name of readdirSync(directory)) {
      if (!name.startsWith(stem + '-') || (!executable && !name.endsWith(extension))) continue;
      const unit = executable ? (extension ? name.slice(0, -extension.length) : name) : name.slice(3, -extension.length);
      if (!/^[a-f0-9]+$/.test(unit.slice(unit.lastIndexOf('-') + 1))) continue;
      const product = resolve(directory, name);
      if (statSync(product).size !== bytes.length || !readFileSync(product).equals(bytes)) continue;
      const candidate = resolve(directory, unit + '.d');
      if (!existsSync(candidate)) continue;
      if (names(readFileSync(candidate, 'utf8'), candidate)) matches.push(candidate);
    }
    if (matches.length === 1) return matches[0];
    if (matches.length > 1) throw new Error(`ambiguous rustc unit dep-info for ${message.target.name}`);
  }
  // The pinned nightly puts units under build/<package>/<hash>/out,
  // reporting only the copied product. Watch archives use that layout too.
  const pkg = metadata?.packages?.find(p => p.id === message.package_id);
  for (const file of message.filenames.filter(path => /\.(wasm|a|rlib)$/.test(path))) {
    if (!pkg) continue;
    const directory = resolve(dirname(intermediate(file)), 'build', pkg.name);
    if (!existsSync(directory)) continue;
    const bytes = readFileSync(file), matches = [], extension = file.slice(file.lastIndexOf('.'));
    for (const hash of readdirSync(directory)) {
      if (!/^[a-f0-9]+$/.test(hash)) continue;
      const out = resolve(directory, hash, 'out');
      if (!existsSync(out)) continue;
      const stem = basename(file, extension);
      for (const name of readdirSync(out).filter(name => name === basename(file) || name === `${stem}-${hash}${extension}`)) {
        const product = resolve(out, name);
        const candidate = resolve(out, name.replace(/\.[^.]+$/, '.d').replace(extension === '.wasm' ? /^$/ : /^lib/, ''));
        if (!existsSync(candidate) || statSync(product).size !== bytes.length || !readFileSync(product).equals(bytes)) continue;
        if (names(readFileSync(candidate, 'utf8'), candidate)) matches.push(candidate);
      }
    }
    if (matches.length === 1) return matches[0];
    if (matches.length > 1) throw new Error(`ambiguous rustc unit dep-info for ${message.target.name}`);
  }
  const searched = metadata ? `; searched target directory ${metadata.target_directory}${buildDir !== targetDir ? ` and build directory ${metadata.build_directory}` : ''}` : '';
  throw new Error(`no matching rustc unit dep-info for ${message.target.name}${searched}; rebuild the stale Cargo unit or use a private target directory`);
}
