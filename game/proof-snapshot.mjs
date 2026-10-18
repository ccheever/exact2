import {createHash} from 'node:crypto';
import {copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, writeFileSync} from 'node:fs';
import {dirname, relative, resolve} from 'node:path';
import {proofInputFiles} from './inputs.mjs';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export function writeJSON(path, value) {
  const temp = path + '.tmp';
  writeFileSync(temp, JSON.stringify(value, null, 2) + '\n');
  renameSync(temp, path);
}
export function inventory(sdk, app) {
  return Object.fromEntries(proofInputFiles(sdk, app).map(file => {
    const path = resolve(sdk, file), own = relative(app, path);
    return [!own.startsWith('../') ? `app/${own}` : `sdk/${file}`, digest(readFileSync(path))];
  }).sort(([a],[b]) => a.localeCompare(b)));
}
export function assertInventory(before, after) {
  for (const key of [...new Set([...Object.keys(before), ...Object.keys(after)])].sort())
    if (before[key] !== after[key]) throw new Error(`continue refused: ${key} changed since the snapshot`);
}
// Proof programs are copied too, but remain outside the game's build identity.
function proofFiles(app, file = 'proof.mjs', files = new Set()) {
  if (files.has(file)) return [...files].sort();
  files.add(file);
  const source = readFileSync(resolve(app,file), 'utf8');
  for (const match of source.matchAll(/(?:from\s*|import\s*\(\s*|import\s*)['"](\.[^'"\n]+)['"]/g)) {
    const path = resolve(app, dirname(file), match[1]);
    if (path.startsWith(app + '/') && existsSync(path)) proofFiles(app, relative(app,path), files);
  }
  return [...files].sort();
}
function copyInput(app, src, file) {
  const from = resolve(app, file), to = resolve(src, file);
  mkdirSync(dirname(to), {recursive:true});
  if (/\.(mjs|js)$/.test(file)) {
    // SDK imports keep pointing at the SDK; game-local helpers stay in the snapshot.
    const source = readFileSync(from, 'utf8').replace(/((?:from\s*|import\s*\(\s*|import\s*)['"])(\.[^'"\n]+)(['"])/g, (all, head, path, tail) => {
      const target = resolve(dirname(from), path);
      return target.startsWith(app + '/') ? all : head + target + tail;
    });
    writeFileSync(to, source);
  } else copyFileSync(from, to);
}
export function snapshot(sdk, app, root) {
  const src = resolve(root, 'src'), inputs = inventory(sdk, app);
  mkdirSync(src, {recursive:true});
  const programs = proofFiles(app), files = new Set([...Object.keys(inputs).filter(key => key.startsWith('app/')).map(key => key.slice(4)), ...programs, 'pins.json']);
  for (const file of ['.gitignore','rust-toolchain.toml']) if (existsSync(resolve(app,file))) files.add(file);
  for (const file of files) copyInput(app, src, file);
  assertInventory(inputs, inventory(sdk, app));
  const copied = Object.fromEntries([...files].sort().map(file => [file, digest(readFileSync(resolve(src,file)))]));
  return {src, inputs, copied, digest:digest(JSON.stringify(inputs)), program:Object.fromEntries(programs.map(file => [file,digest(readFileSync(resolve(app,file)))]))};
}
export function validateSnapshot(record, sdk, app) {
  assertInventory(record.inputs, inventory(sdk, app));
  assertInventory(record.program, Object.fromEntries(proofFiles(app).map(file => [file,digest(readFileSync(resolve(app,file)))])));
  for (const [file, hash] of Object.entries(record.copied)) {
    const path = resolve(record.src, file);
    if (!existsSync(path) || digest(readFileSync(path)) !== hash) throw new Error(`continue refused: snapshot ${file} changed`);
  }
}
export function reusableLeg(leg, root, inputDigest) {
  if (!leg || leg.state !== 'complete' || leg.digest !== inputDigest || ![0,3].includes(leg.exitCode)) return null;
  const path = resolve(root, leg.summaryPath);
  if (!existsSync(path) || digest(readFileSync(path)) !== leg.summaryDigest) throw new Error(`continue refused: corrupt receipt ${leg.summaryPath}`);
  const summary = JSON.parse(readFileSync(path, 'utf8'));
  if (summary.failures?.length || !['PASS','UNVERIFIED'].includes(summary.status)) throw new Error(`continue refused: failed receipt ${leg.summaryPath}`);
  return summary;
}
