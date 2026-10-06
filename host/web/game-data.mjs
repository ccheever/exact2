// A game's art and declared data are what its module fetches, not code it
// compiles (LLP 1046.009 G2). Its `.level.json` files are authored in assets/
// and reach the running world from there as any asset does. Its art bakes
// into assets/ here, through the baker the GPU build script runs
// (`exact-game-bake --art`); a game whose art is written by TypeScript (a
// generator: `art.mjs`, or `art-src/gen.mjs`) has it written again first when
// the generator, a module it imports, or a level it reads changes. The assets/
// watcher then delivers the changed names to the running world, which takes
// them in place: nothing rebuilds and the world is kept. The next full build
// bakes the same bytes; both writers write only what changed.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, statSync, unwatchFile, watchFile } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';

const engine = resolve(import.meta.dir, '../../game');
const GENERATORS = ['art.mjs', 'art-src/gen.mjs'];

/** The generator and the modules it imports inside the game, by path. */
function generatorSources(dir) {
  const entry = GENERATORS.map(name => resolve(dir, name)).find(existsSync);
  const sources = new Set();
  const visit = path => {
    if (sources.has(path) || !path.startsWith(dir + '/') || !existsSync(path)) return;
    sources.add(path);
    for (const [, spec] of readFileSync(path, 'utf8').matchAll(/\bfrom\s+['"](\.{1,2}\/[^'"]+)['"]/g)) visit(resolve(dirname(path), spec));
  };
  if (entry) visit(entry);
  return { entry, sources };
}

export function gameData({ app, env, report }) {
  const art = () => resolve(app().dir, 'art');
  const baker = () => resolve(app().target, 'debug/exact-game-bake');
  let building = null, timer = null, baking = null, editedAt = null, generator = { entry: null, sources: new Set() };
  // Until then, art/ changes are the generator's own writes, which its bake took.
  let ownWritesUntil = 0;
  const queued = new Set();
  const run = (command, args, options) => new Promise((ok, fail) => {
    const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    child.stdout.on('data', data => { output += data; });
    child.stderr.on('data', data => { output += data; });
    child.on('error', fail);
    child.on('exit', code => code === 0 ? ok(output) : fail(new Error(output.trim() || `${command} exited ${code}`)));
  });
  // The baker is built from the engine's workspace, where it is a member: in a
  // game's generated one it is only a build dependency, and Cargo's resolver
  // panics selecting it with -p there. It shares the game's target directory.
  const build = () => existsSync(baker()) ? Promise.resolve() : building ??= run('cargo',
    ['build', '-q', '--locked', '--offline', '-p', 'exact-game-bake', '--bin', 'exact-game-bake'],
    { cwd: engine, env: { ...env, CARGO_TARGET_DIR: app().target } })
    .finally(() => { building = null; });
  // A level the generator's sources name: its tables colour or shape the art.
  const readByGenerator = path => path.endsWith('.level.json') && dirname(path) === resolve(app().dir, 'assets')
    && [...generator.sources].some(source => readFileSync(source, 'utf8').includes(basename(path)));
  const generates = path => generator.sources.has(path) || readByGenerator(path);
  // The directory itself too: its build-script input reports the whole tree.
  const inArt = path => path === art() || path.startsWith(art() + '/');
  async function bake(paths) {
    const start = performance.now();
    const generating = generator.entry && paths.some(generates);
    try {
      if (generating) { ownWritesUntil = Infinity; await run(process.execPath, [generator.entry], { cwd: app().dir, env }); }
      if (paths.some(path => inArt(path) || generates(path))) {
        await build();
        await run(baker(), ['--art', app().dir], { cwd: app().dir, env });
      }
    } finally {
      // Watchers poll every 100 ms: what they report next is still these writes.
      if (generating) ownWritesUntil = Date.now() + 300;
    }
    return performance.now() - start;
  }
  function drain() {
    if (baking || !queued.size) return;
    const paths = [...queued]; queued.clear();
    const seen = editedAt === null ? '' : `, seen ${(Date.now() - editedAt).toFixed(0)} ms after the save`;
    const written = generator.entry && paths.some(generates) ? `${relative(app().dir, generator.entry)} wrote art/, ` : '';
    baking = bake(paths)
      .then(ms => console.log(`edit → ${paths.map(p => relative(app().dir, p)).join(', ')}: ${written}baked into assets/ in ${ms.toFixed(0)} ms${seen}; the running world takes it in place`))
      .catch(error => report(`${relative(app().dir, paths[0])}: ${error.message}`))
      .finally(() => { baking = null; drain(); });
  }
  function changed(path) {
    if (inArt(path) && Date.now() < ownWritesUntil) return;
    editedAt ??= existsSync(path) ? statSync(path).mtimeMs : Date.now();
    queued.add(path); clearTimeout(timer); timer = setTimeout(drain, 50);
  }
  return {
    /** Whether an input edit is this data, which never rebuilds the module. */
    owns: path => Boolean(app().manifest.game) && (inArt(path) || generator.sources.has(path)),
    changed,
    /** An assets/ edit, delivered as it is; a level the generator reads also writes the art again. */
    assetChanged(path) { if (app().manifest.game && readByGenerator(path)) changed(path); },
    /** When the edit an asset generation carries was saved, once. */
    takeEditedAt() { const at = editedAt; editedAt = null; return at; },
    /** Watch the art generator's sources (no build reads them) and build the
     * baker in the background, so the first art edit is warm. */
    start() {
      if (!app().manifest.game) return;
      for (const source of generator.sources) unwatchFile(source);
      generator = generatorSources(app().dir);
      for (const source of generator.sources) watchFile(source, { interval: 100 }, (now, before) => { if (now.mtimeMs !== before.mtimeMs || now.size !== before.size) changed(source); });
      if (existsSync(art())) build().catch(error => report(error.message));
    },
  };
}
