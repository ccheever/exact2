import {existsSync, readdirSync} from 'node:fs';
import {basename, dirname, relative, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';

export function gameNonInput(path) {
  path = path.replaceAll('\\', '/');
  return path === '.exact/status.json' || /^(shots|tools|repros)\//.test(path)
    || /(^|\/)(pins\.json|proof\.mjs|[^/]*\.test\.(?:mjs|js|ts|rs|contract)|[^/]*\.md)$/.test(path)
    || (/\.(?:m?js|sh|ops)$/.test(path) && !/^(logic|data|gpu|render|art|assets|deck)\//.test(path));
}
/// Whether a repository file is outside a game's deterministic build inputs:
/// other games, the bench and its probes, the twins, diaries, LLPs, apps, build
/// outputs. Source extensions, app assets, and Apple module inputs are admitted.
/// A game's own scripts (bench.mjs, a proxy, a probe) are tools, not bake inputs:
/// no bake reads a .mjs/.js outside its logic, data, gpu, render and asset folders.
export function proofInputExcluded(file, name, appPrefix = `game/games/${name}/`, root = resolve(import.meta.dir, '..')) {
  if (file.startsWith(appPrefix)) {
    const own = file.slice(appPrefix.length), parts = own.split('/').slice(0, -1);
    return gameNonInput(own)
      || /^(node_modules|target|\.shells|\.exact|dist|dist\.previous|dist-windows|artifacts)\//.test(own)
      || parts.some((part, i) => ['tests', 'examples'].includes(part) && existsSync(resolve(root, appPrefix, ...parts.slice(0,i), 'Cargo.toml')))
      || (!/\.(rs|toml|lock|contract|ts|js|mjs|wgsl|json|swift|h|c|html|css|modulemap)$/.test(own) && !/^(art|assets|deck)\//.test(own));
  }
  const gamePrefix = file.startsWith(appPrefix) ? appPrefix : file.startsWith('game/') ? 'game/' : null;
  const gameParts = gamePrefix === null ? [] : file.slice(gamePrefix.length).split('/').slice(0, -1);
  return (/^(issues|\.claude)\//.test(file) && !file.startsWith(appPrefix))
    || /(^|\/)(pins\.json|proof\.mjs|.*\.test\.mjs|.*\.md)$/.test(file)
    || (file.startsWith(appPrefix) && gameNonInput(file.slice(appPrefix.length)))
    || (!/\.(rs|toml|lock|contract|ts|js|mjs|wgsl|json|swift|h|c|html|css|modulemap)$/.test(file)
      && !(file.startsWith('host/apple/') && !basename(file).includes('.'))
      && !['art/', 'assets/', 'deck/'].some(dir => file.startsWith(appPrefix + dir)))
    || (/^(game\/(bench|twins|diaries|artifacts)\/|llp\/)/.test(file) && !file.startsWith(appPrefix))
    || (file.startsWith('game/games/') && !file.startsWith(appPrefix))
    || ['node_modules/', 'target/', '.shells/', 'dist/', 'dist.previous/', 'dist-windows/', 'artifacts/'].some(output => file.startsWith(output) || file.startsWith('game/' + output) || file.startsWith(appPrefix + output))
    || /^(host\/web\/dist(?:\.previous)?|host\/apple\/\.build|game\/render\/target)\//.test(file)
    || (file.startsWith('apps/') && !file.startsWith(appPrefix))
    || gameParts.some((part, i) => ['tests', 'examples'].includes(part)
      && existsSync(resolve(root, gamePrefix, ...gameParts.slice(0, i), 'Cargo.toml')));
}
export function proofInputFiles(root, app, inventory, ignoredDirectories = new Set()) {
  const top = inventory ? {status:0, stdout:root} : spawnSync('git', ['rev-parse','--show-toplevel'], {cwd:root, encoding:'utf8'});
  const repository = top.status === 0 ? top.stdout.trim() : root;
  const files = inventory ?? spawnSync('git', ['ls-files','-z','--cached','--others','--exclude-standard'], {cwd:repository, encoding:'utf8'});
  const walk = (dir, prefix = '') => ignoredDirectories.has(resolve(dir)) ? [] : readdirSync(dir, {withFileTypes:true}).flatMap(entry => {
    if (['.git','node_modules'].includes(entry.name) || (!prefix && ['target','.build','.shells','dist','dist.previous','dist-windows','artifacts'].includes(entry.name))) return [];
    const path = prefix + entry.name;
    return entry.isDirectory() ? walk(resolve(dir,entry.name), path + '/') : entry.isFile() ? [path] : [];
  });
  // Fleet exports have no Git index; external games are outside exact2's index.
  // Git-ignored files are outputs or private apparatus, never authored inputs.
  const sources = files.status === 0 ? files.stdout.split('\0').filter(Boolean) : walk(repository);
  const excluded = new Set();
  for (const cwd of new Set([repository, app])) {
    const ignored = spawnSync('git', ['ls-files', '-z', '--others', '--ignored', '--exclude-standard', '--directory'], {cwd, encoding:'utf8'});
    if (ignored.status === 0) for (const file of ignored.stdout.split('\0').filter(Boolean)) {
      if (file.endsWith('/')) { if (resolve(cwd,file) !== resolve(app)) ignoredDirectories.add(resolve(cwd, file)); }
      else excluded.add(relative(repository, resolve(cwd, file)).replaceAll('\\', '/'));
    }
  }
  sources.push(...walk(app).map(file => relative(repository, resolve(app, file)).replaceAll('\\', '/')));
  const prefix = relative(repository, app).replaceAll('\\', '/') + '/';
  return [...new Set(sources)].sort().filter(file => !excluded.has(file) &&
    ![...ignoredDirectories].some(dir => resolve(repository, file).startsWith(dir + '/') && (!resolve(repository,file).startsWith(resolve(app) + '/') || dir.startsWith(resolve(app) + '/'))) &&
    !proofInputExcluded(file, basename(app), prefix, repository) && existsSync(resolve(repository, file))).map(file => relative(root, resolve(repository, file)).replaceAll('\\', '/')).sort();
}
