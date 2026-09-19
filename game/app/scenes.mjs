// LLP 1041.006 §7: typed content bake, independent of the game GPU/behavior artifact.
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync, readFileSync, statSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {gameDefaults, prepareGame} from './shells.mjs';
import {cargoReproducibilityFlags} from '../../scripts/app.mjs';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const appDir = app => typeof app === 'string' ? app : app.dir;

/** Initial builds compile the typed baker; content edits reuse that same executable.
 * Cargo reports fresh artifacts on content edits because JSON is never a Rust include.
 * No server or running app is touched. A failed bake leaves accepted output intact. */
export function bakeGameScene(app, {development = true, build = true} = {}) {
  const dir = appDir(app), scene = resolve(dir, 'scene.json');
  if (!existsSync(scene)) return null;
  if (typeof app === 'object' && app.prepare) app.prepare();
  else if (existsSync(resolve(dir, 'logic/src/lib.rs'))) prepareGame(dir, gameDefaults(dir).game);
  const manifest = app.manifest ?? gameDefaults(dir) ?? JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
  const crate = manifest.game?.crate;
  if (!crate?.endsWith('-logic')) throw new Error(`${dir}: scene needs game.crate`);
  const binary = crate.slice(0, -6) + '-scene';
  const workspace = typeof app === 'object' ? app.workspace : resolve(dir, '.shells');
  const target = typeof app === 'object' ? app.target : resolve(process.env.CARGO_TARGET_DIR ?? resolve(dir, 'target'));
  const env = {...process.env, CARGO_TARGET_DIR:target, EXACT_UPDATE_TRUST:development ? 'development' : 'production'};
  const compiled = [];
  let executable = resolve(target, 'debug', binary);
  if (build) {
    const result = spawnSync('cargo', ['build', ...cargoReproducibilityFlags({workspace, manifest}), '--manifest-path', resolve(workspace, 'Cargo.toml'), '-p', crate, '--bin', binary, '--message-format=json'], {cwd:workspace, env, encoding:'utf8', maxBuffer:32*1024*1024});
    for (const line of (result.stdout ?? '').split('\n').filter(Boolean)) {
      const event = JSON.parse(line);
      if (event.reason === 'compiler-artifact') {
        if (!event.fresh) compiled.push(event.target.name);
        if (event.target.name === binary && event.executable) executable = event.executable;
      }
      if (event.reason === 'compiler-message' && event.message?.rendered) process.stderr.write(event.message.rendered);
    }
    if (result.error || result.status !== 0) throw new Error(`scene baker build: ${result.error?.message ?? result.stderr}`);
  }
  const result = spawnSync(executable, [scene, resolve(dir, '.scene')], {cwd:dir, env, encoding:'utf8', maxBuffer:16*1024*1024});
  if (result.error || result.status !== 0) throw new Error(`scene content bake: ${result.error?.message ?? result.stderr}`);
  const content = readFileSync(resolve(dir, '.scene/scene.binhex'), 'utf8');
  const digest = sha(Buffer.from(content, 'hex').subarray(9));
  return {digest, compiled, content, output:result.stdout.trim(), inputs:sceneInputs(app)};
}

/** Includes shared fragments and declared asset inputs. No new watcher or daemon. */
export function sceneInputs(app) {
  const dir = appDir(app), scene = resolve(dir, 'scene.json');
  if (!existsSync(scene)) return [];
  const mapPath = resolve(dir, '.scene/scene.map.json');
  const files = new Set([scene]);
  if (existsSync(mapPath)) {
    const map = JSON.parse(readFileSync(mapPath, 'utf8'));
    for (const file of Object.keys(map.files)) files.add(file);
  }
  // Bounded iterative discovery also handles cyclic/very deep edited input.
  // The typed baker remains the source of authoring/depth diagnostics.
  const pending = [...files], visited = new Set();
  let remaining = 64 * 1024 * 1024;
  while (pending.length) {
    const file = pending.pop();
    if (visited.has(file)) continue;
    visited.add(file);
    if (visited.size > 4096) throw new Error('scene input discovery exceeds 4096 files');
    if (!existsSync(file)) continue;
    const size = statSync(file).size;
    if (size > Math.min(16 * 1024 * 1024, remaining)) throw new Error('scene input discovery exceeds byte limit');
    const bytes = readFileSync(file); remaining -= bytes.length;
    if (bytes.length > size || remaining < 0) throw new Error('scene input changed during discovery; rebake');
    let document;
    try { document = JSON.parse(bytes.toString('utf8')); } catch { continue; }
    for (const child of Object.values(document.fragments ?? {})) {
      if (typeof child !== 'string') continue;
      const path = resolve(dirname(file), child);
      files.add(path); pending.push(path);
    }
    for (const asset of Object.values(document.assets ?? {})) {
      if (typeof asset?.path === 'string') files.add(resolve(dirname(file), asset.path));
    }
  }
  return [...files].sort();
}

/** A driver may link only the inspected world's exact content and unchanged sources. */
export function sceneSource(app, digest, entity) {
  const path = resolve(appDir(app), '.scene/scene.map.json');
  if (!existsSync(path)) return {unavailable:'development scene map is absent'};
  const map = JSON.parse(readFileSync(path, 'utf8'));
  if (map.version !== 1 || map.sceneDigest !== digest) return {unavailable:'scene digest differs; world retains older content'};
  for (const [file, expected] of Object.entries(map.files)) {
    if (!existsSync(file) || sha(readFileSync(file)) !== expected) return {unavailable:`source changed since scene bake: ${file}`};
  }
  return map.entities[entity] ?? {unavailable:'not an authored entity; inspect GeneratedBy for generator and parameters'};
}
