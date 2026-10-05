// A game's art and declared level are data its module fetches, not code it
// compiles (LLP 1046.009 G2). An edit to either bakes into assets/ here: the
// art through the baker the GPU build script runs (`exact-game-bake --art`),
// a `.level.json` copied beside it as that script copies it. The assets/
// watcher then delivers the changed names to the running world, which takes
// them in place: nothing rebuilds and the world is kept. The next full build
// bakes the same bytes; both writers write only what changed.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';

const engine = resolve(import.meta.dir, '../../game');

export function gameData({ app, env, report }) {
  const art = () => resolve(app().dir, 'art');
  const baker = () => resolve(app().target, 'debug/exact-game-bake');
  let building = null, timer = null, baking = null, editedAt = null;
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
  const level = path => dirname(path) === app().dir && path.endsWith('.level.json');
  async function bake(paths) {
    const start = performance.now();
    for (const path of paths.filter(level)) {
      const bytes = readFileSync(path);
      JSON.parse(bytes.toString('utf8')); // the module checks its type on arrival
      const out = resolve(app().dir, 'assets', relative(app().dir, path));
      if (existsSync(out) && readFileSync(out).equals(bytes)) continue;
      mkdirSync(dirname(out), { recursive: true });
      writeFileSync(out, bytes);
    }
    if (paths.some(path => path.startsWith(art() + '/'))) {
      await build();
      await run(baker(), ['--art', app().dir], { cwd: app().dir, env });
    }
    return performance.now() - start;
  }
  function drain() {
    if (baking || !queued.size) return;
    const paths = [...queued]; queued.clear();
    const seen = editedAt === null ? '' : `, seen ${(Date.now() - editedAt).toFixed(0)} ms after the save`;
    baking = bake(paths)
      .then(ms => console.log(`edit → ${paths.map(p => relative(app().dir, p)).join(', ')} baked into assets/ in ${ms.toFixed(0)} ms${seen}; the running world takes it in place`))
      .catch(error => report(`${relative(app().dir, paths[0])}: ${error.message}`))
      .finally(() => { baking = null; drain(); });
  }
  return {
    /** Whether an input edit is this data, which never rebuilds the module. */
    owns: path => Boolean(app().manifest.game) && (path.startsWith(art() + '/') || level(path)),
    changed(path) { editedAt ??= existsSync(path) ? statSync(path).mtimeMs : Date.now(); queued.add(path); clearTimeout(timer); timer = setTimeout(drain, 50); },
    /** When the edit an asset generation carries was saved, once. */
    takeEditedAt() { const at = editedAt; editedAt = null; return at; },
    /** Build the baker in the background, so the first art edit is warm. */
    warm() { if (app().manifest.game && existsSync(art())) build().catch(error => report(error.message)); },
  };
}
