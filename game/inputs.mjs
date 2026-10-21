import {createHash} from 'node:crypto';
import {readFileSync, existsSync, readdirSync, statSync, mkdirSync, writeFileSync} from 'node:fs';
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

// Generated assets are runtime inputs even when Git excludes the bake's outputs.
export function executionInputFiles(root, app, inventory, ignoredDirectories, authored) {
  const files = [...(authored ?? proofInputFiles(root, app, inventory, ignoredDirectories))];
  const walk = dir => {
    if (existsSync(dir)) for (const entry of readdirSync(dir,{withFileTypes:true})) {
      const path = resolve(dir,entry.name);
      if (entry.isDirectory()) walk(path); else if (entry.isFile()) files.push(relative(root,path));
    }
  };
  walk(resolve(app,"assets"));
  const manifest = resolve(app, '.baked-assets.json');
  if (existsSync(manifest)) {
    files.push(relative(root, manifest));
    for (const name of Object.keys(JSON.parse(readFileSync(manifest, 'utf8')))) {
      const path = resolve(app, 'assets', name);
      if (!path.startsWith(resolve(app, 'assets') + '/')) throw new Error(`invalid baked asset path: ${name}`);
      files.push(relative(root, path));
    }
  }
  return [...new Set(files)].sort();
}

/** Missing modes/profiles mean legacy full coverage; hosts are never inferred. */
export function releasePinGaps(pins = {}) {
  const gaps = [];
  if (!(pins.hosts ?? []).includes('web')) gaps.push('web not pinned');
  if (!(pins.hosts ?? []).some(host => ['linux','windows'].includes(host))) gaps.push('native host not pinned');
  for (const mode of ['0', '1', 'fresh-game']) if (!(pins.modes ?? ['0', '1', 'fresh-game']).includes(mode)) gaps.push(`mode ${mode} not pinned`);
  for (const profile of ['gpu-dev', 'release']) if (!(pins.profiles ?? ['gpu-dev', 'release']).includes(profile)) gaps.push(`${profile === 'release' ? 'release' : 'development'} not pinned`);
  return gaps;
}
export function requireReleasePins(app) {
  if (!app.manifest?.game) return;
  let pins;
  try { pins = JSON.parse(readFileSync(resolve(app.dir, 'pins.json'), 'utf8')); } catch { /* Refuse below. */ }
  const gaps = releasePinGaps(pins);
  if (!Object.keys(pins?.ticks ?? {}).length || !Object.keys(pins?.saves ?? {}).length) gaps.push('release not pinned');
  if (gaps.length) throw new Error(`game delivery refused: ${gaps.join('; ')}; run prove --release (with --repin for existing pins)`);
}

// Clean tracked bytes are identified by Git blobs. Dirty app inputs and
// products reuse their digest only while their full stat tuple is unchanged.
// Nanosecond timestamps and inode identity catch edits restored before the next scan.
export const statKey = path => { const s=statSync(path, {bigint:true}); return [s.dev,s.ino,s.size,s.mtimeNs,s.ctimeNs].join(':'); };
export function proofInputs(root, app, cachePath) {
  const git = args => spawnSync('git',args,{cwd:root,encoding:'utf8',maxBuffer:32*1024*1024});
  const listed=git(['ls-files','-s','-z','--cached','--others','--exclude-standard']);
  const status=git(['status','--porcelain=v1','-z','--untracked-files=no','--no-renames']);
  const blobs=new Map(), paths=[];
  for(const row of listed.stdout?.split('\0').filter(Boolean) ?? []) {
    const match=/^\d+ ([a-f0-9]+) 0\t(.*)$/s.exec(row);
    const path=match ? match[2] : row; paths.push(path);
    if(match) blobs.set(path,match[1]);
  }
  const dirty=new Set((status.stdout??'').split('\0').filter(row=>row && row[1]!==' ').map(row=>row.slice(3)));
  let cache={}; try {cache=JSON.parse(readFileSync(cachePath,'utf8'));} catch { /* First proof. */ }
  const next={}, rows=[], prefix=relative(root,app).replaceAll('\\','/')+'/', groups={all:[],gpu:[],host:[],authored:[]}, authoredState=[], assetState=[]; let reads=0;
  const manifest = resolve(app, '.baked-assets.json');
  const generated = new Set([prefix + 'assets/exact-tables.json', ...(existsSync(manifest)
    ? Object.keys(JSON.parse(readFileSync(manifest, 'utf8'))).map(name => prefix + 'assets/' + name) : [])]);
  const ignoredDirectories = new Set();
  const authoredFiles = new Set(proofInputFiles(root,app,listed.status===0 ? {status:0,stdout:paths.join('\0')} : undefined, ignoredDirectories));
  const files=executionInputFiles(root,app,undefined,ignoredDirectories,authoredFiles);
  // File stats miss sources created, consumed and removed between scans. Walk
  // shared and app source directories, including empty ones, to retain edits.
  const directories = new Set();
  const sourceDirectories = dir => {
    if (!existsSync(dir) || directories.has(dir) || ignoredDirectories.has(resolve(dir))) return;
    const path = relative(root, dir).replaceAll('\\', '/'), file = path ? path + '/' : '';
    if (proofInputExcluded(file + 'input.rs', basename(app), prefix, root)) return;
    directories.add(dir);
    for (const entry of readdirSync(dir, {withFileTypes:true}).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
      if (entry.isDirectory() && !['.git', 'node_modules'].includes(entry.name)) sourceDirectories(resolve(dir, entry.name));
  };
  for(const file of files) {
    const path=resolve(root,file), key=statKey(path); let digest=blobs.get(file);
    if(!digest || status.status!==0 || dirty.has(file)) {
      digest=cache[file]?.key===key ? cache[file].digest : null;
      if(!digest) {const bytes=readFileSync(path); reads++; digest=createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');}
      next[file]={key,digest};
    }
    rows.push([file,digest,key]);
  }
  for(const row of rows) {
    const [file,digest,key]=row, input=[file.startsWith(prefix) ? 'app/' + file.slice(prefix.length) : 'sdk/' + file,digest]; groups.all.push(input);
    if (authoredFiles.has(file) && file !== prefix + '.baked-assets.json' && !generated.has(file)) {
      // Source provenance survives moving the app relative to the SDK. Keep
      // execution paths and the in-flight file/directory checks unchanged.
      groups.authored.push([file.startsWith(prefix) ? 'app/' + file.slice(prefix.length) : 'sdk/' + file, digest]);
      authoredState.push([file,key]);
    }
    // Host bake depends on the declaration, not the game's implementation.
    // Shared compiler crates remain conservative inputs of both graphs.
    const own=file.startsWith(prefix) ? file.slice(prefix.length) : null;
    const gpuOnly=own!==null ? /^(logic|gpu|render|art|assets|deck)\//.test(own) : /^game\/(engine|render|physics|audio|bake)\//.test(file);
    const hostOnly=own!==null ? /\.contract$/.test(own) : /^host\//.test(file);
    if(!hostOnly) groups.gpu.push(input);
    if(!gpuOnly) groups.host.push(input);
  }
  if(cachePath) {mkdirSync(resolve(cachePath,'..'),{recursive:true});writeFileSync(cachePath,JSON.stringify(next));}
  // Snapshot after writing our cache so its first creation is not a build edit.
  sourceDirectories(resolve(root));
  // The repository walk excludes other apps at their shared parent, and an
  // external app may be outside root. Always scan this app's whole input tree.
  sourceDirectories(resolve(app));
  // A Git-ignored directory can contain tracked sources. New untracked entries
  // there are ignored too; only its identity and the tracked files' full stats
  // matter. Other directories retain timestamps to catch transient sources.
  const ignoredMixed = new Set();
  for (const cwd of new Set([resolve(root), resolve(app)])) {
    const paths = [...directories].filter(dir => dir === cwd || dir.startsWith(cwd + '/'));
    const ignored = spawnSync('git', ['check-ignore', '--no-index', '-z', '--stdin'], {
      cwd, input:paths.map(dir => dir + '/').join('\0') + '\0', encoding:'utf8',
    });
    if (ignored.status === 0) for (const path of ignored.stdout.split('\0').filter(Boolean)) ignoredMixed.add(resolve(path));
  }
  for (const dir of directories) {
    const path = relative(root, dir).replaceAll('\\', '/'), file = path ? path + '/' : '';
    // Generated manifest/asset entry changes still require a rerun.
    const state = file === prefix || file.startsWith(prefix + 'assets/') ? assetState : authoredState;
    const key = statKey(dir);
    state.push([file, ignoredMixed.has(dir) ? key.split(':').slice(0, 2).join(':') : key]);
  }
  groups.authored.sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0);
  return {...Object.fromEntries(Object.entries({...groups,authoredState,assetState}).map(([kind,rows])=>[kind,createHash('sha256').update(JSON.stringify(rows)).digest('hex')])),reads,assetRows:assetState};
}
// One reader per proof. Directory changes require a rerun, even for baked assets.
export function proofInputReader(root, app, cachePath) {
  let previous;
  return () => {
    const inputs = proofInputs(root, app, cachePath);
    // Content identity alone misses an edit that is reverted while the build reads it.
    if (previous && previous.authoredState !== inputs.authoredState)
      throw new Error('authored proof inputs changed during build; rerun the proof');
    if (previous && previous.assetState !== inputs.assetState) {
      const before = new Map(previous.assetRows), after = new Map(inputs.assetRows);
      const changed = [...new Set([...before.keys(), ...after.keys()])].filter(dir => before.get(dir) !== after.get(dir));
      throw new Error(`app or asset directories changed during build: ${changed.join(', ')}; rerun the proof (generated creation, pruning and atomic replacement also require a rerun)`);
    }
    previous = inputs;
    return inputs;
  };
}
