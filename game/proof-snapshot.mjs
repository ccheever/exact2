import {spawnSync} from 'node:child_process';
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
  const git = args => spawnSync('git', args, {cwd:sdk, encoding:'utf8', maxBuffer:32*1024*1024});
  const listed = git(['ls-files','-s','-z']), status = git(['status','--porcelain=v1','-z','--untracked-files=no','--no-renames']);
  const blobs = new Map((listed.stdout ?? '').split('\0').flatMap(row => { const match = /^\d+ ([a-f0-9]+) 0\t(.*)$/s.exec(row); return match ? [[match[2],match[1]]] : []; }));
  const dirty = new Set((status.stdout ?? '').split('\0').filter(row => row && row[1] !== ' ').map(row => row.slice(3)));
  return Object.fromEntries(proofInputFiles(sdk, app).map(file => {
    const path = resolve(sdk, file), own = relative(app, path);
    let hash = listed.status === 0 && status.status === 0 && !dirty.has(file) ? blobs.get(file) : null;
    if (!hash) { const bytes = readFileSync(path); hash = createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex'); }
    return [!own.startsWith('../') ? `app/${own}` : `sdk/${file}`, hash];
  }).sort(([a],[b]) => a < b ? -1 : a > b ? 1 : 0));
}
export function assertInventory(before, after) {
  for (const key of [...new Set([...Object.keys(before), ...Object.keys(after)])].sort())
    if (before[key] !== after[key]) throw new Error(`continue refused: ${key} changed since the snapshot`);
}
// Proof dependencies have their own identity: assertions and test data are not bake inputs.
function proofFiles(sdk, app) {
  const files = new Set();
  const visit = path => {
    if (files.has(path)) return;
    files.add(path);
    if (!/\.(mjs|js|ts)$/.test(path)) return;
    const source = readFileSync(path, 'utf8');
    for (const match of source.matchAll(/(?:from\s*|import\s*\(\s*|import\s*)['"](\.[^'"\n]+)['"]/g)) {
      const target = resolve(dirname(path), match[1]);
      if (existsSync(target)) visit(target);
    }
    // Static declarations avoid executing a proof merely to discover its data.
    const data = /export const proofData = (\[[^;]*\]);/.exec(source);
    if (/export\s+const\s+proofData\b/.test(source) && !data) throw new Error(`${path}: unparseable proofData; use a JSON array declaration`);
    if (data) {
      let names; try { names = JSON.parse(data[1]); } catch { throw new Error(`${path}: unparseable proofData; use a JSON array declaration`); }
      if (!Array.isArray(names) || names.some(name => typeof name !== 'string')) throw new Error(`${path}: proofData must be an array of paths`);
      for (const file of names) visit(resolve(dirname(path), file));
    }
  };
  visit(resolve(app, 'proof.mjs'));
  return [...files].sort().map(path => [path, path.startsWith(app + '/') ? 'app/' + relative(app,path) : 'sdk/' + relative(sdk,path)]);
}
const proofInventory = (sdk, app) => Object.fromEntries(proofFiles(sdk,app).map(([path,key]) => [key,digest(readFileSync(path))]));
function copyInput(app, src, file) {
  const from = resolve(app, file), to = resolve(src, file);
  mkdirSync(dirname(to), {recursive:true});
  if (/\.(mjs|js|ts)$/.test(file)) {
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
  const program = proofInventory(sdk, app), programs = Object.keys(program).filter(key => key.startsWith('app/')).map(key => key.slice(4));
  const files = new Set([...Object.keys(inputs).filter(key => key.startsWith('app/')).map(key => key.slice(4)), ...programs, 'pins.json']);
  for (const file of ['.gitignore','rust-toolchain.toml']) if (existsSync(resolve(app,file))) files.add(file);
  for (const file of files) copyInput(app, src, file);
  const initialized = spawnSync('git', ['init','-q',src], {encoding:'utf8'});
  if (initialized.status !== 0) throw new Error(`snapshot Git initialization failed: ${initialized.stderr}`);
  const tracked = spawnSync('git', ['add','.'], {cwd:src, encoding:'utf8'});
  if (tracked.status !== 0) throw new Error(`snapshot input tracking failed: ${tracked.stderr}`);
  assertInventory(inputs, inventory(sdk, app));
  assertInventory(program, proofInventory(sdk,app));
  const copied = Object.fromEntries([...files].sort().map(file => [file, digest(readFileSync(resolve(src,file)))]));
  return {src, inputs, copied, digest:digest(JSON.stringify({inputs,program})), program};
}
export function validateSnapshot(record, sdk, app) {
  assertInventory(record.inputs, inventory(sdk, app));
  validateProof(record, sdk, app);
}
export function validateProof(record, sdk, app) {
  assertInventory(record.program, proofInventory(sdk,app));
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
