// Cargo's exact unit dep-info, including the nightly's copied products.
import { basename, dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { existsSync, statSync, readFileSync, readdirSync } from 'node:fs';
const under = (root, file) => {
  const child = relative(root, file);
  return child === '' || (!isAbsolute(child) && child !== '..' && !child.startsWith('..' + sep));
};

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
// rustc records relative inputs against Cargo's workspace root, even when
// EXACT_APP_DIR makes the invoking directory a nested app in that workspace.
export function unitDepInfo(message, workspace, metadata) {
  // Cargo reports copied roots in target_directory and units in build_directory.
  // When the directories nest, the more specific root identifies the file.
  const intermediate = file => metadata && under(metadata.target_directory, file)
      && (!under(metadata.build_directory, file) || metadata.target_directory.length > metadata.build_directory.length)
    ? resolve(metadata.build_directory, relative(metadata.target_directory, file)) : file;
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
      const dep = readFileSync(candidate, 'utf8');
      const output = dep.slice(0, dep.indexOf(': '));
      // rustc leaves the single output path raw, but escapes dependency paths.
      if ((resolve(workspace, output) === candidate || compilerPaths('unit: ' + output, workspace).includes(candidate))
          && compilerPaths(dep, workspace).includes(resolve(message.target.src_path))
          && sameBytes(resolve(dirname(candidate), basename(file)), file)) return candidate;
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
      const dep = readFileSync(candidate, 'utf8');
      const output = dep.slice(0, dep.indexOf(': '));
      // rustc leaves the single output path raw, but escapes dependency paths.
      if ((resolve(workspace, output) === candidate || compilerPaths('unit: ' + output, workspace).includes(candidate))
          && compilerPaths(dep, workspace).includes(resolve(message.target.src_path))) matches.push(candidate);
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
        const dep = readFileSync(candidate, 'utf8'), output = dep.slice(0, dep.indexOf(': '));
        if ((resolve(workspace, output) === candidate || compilerPaths('unit: ' + output, workspace).includes(candidate))
            && compilerPaths(dep, workspace).includes(resolve(message.target.src_path))) matches.push(candidate);
      }
    }
    if (matches.length === 1) return matches[0];
    if (matches.length > 1) throw new Error(`ambiguous rustc unit dep-info for ${message.target.name}`);
  }
  throw new Error(`no matching rustc unit dep-info for ${message.target.name}; rebuild the stale Cargo unit or use a private target directory`);
}
