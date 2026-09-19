// Development-only immutable input/artifact receipts, LLP 1041.006 §6.
import {createHash} from 'node:crypto';
import {readFileSync, readdirSync, lstatSync} from 'node:fs';
import {resolve} from 'node:path';
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export function reloadInputs(files, trees = []) {
  const paths = new Set(files);
  const walk = path => {
    let stat;
    try { stat = lstatSync(path); } catch (error) { if (error.code === 'ENOENT') {paths.add(path); return;} throw error; }
    if (stat.isSymbolicLink()) throw new Error(`reload receipt cannot traverse symlink: ${path}`);
    if (stat.isDirectory()) for (const name of readdirSync(path).sort()) walk(resolve(path, name));
    else paths.add(path);
  };
  trees.forEach(walk);
  return [...paths].sort().map(path => {
    try { return {path, sha256:sha256(readFileSync(path))}; }
    catch (error) { if (error.code === 'ENOENT') return {path, sha256:null}; throw error; }
  });
}
export function verifyReloadInputs(before, after) {
  if (JSON.stringify(before) !== JSON.stringify(after)) {
    const old = new Map(before.map(row => [row.path,row.sha256]));
    const changes = after.filter(row=>old.get(row.path)!==row.sha256).map(row=>row.path);
    for (const row of before) if (!after.some(next=>next.path===row.path)) changes.push(row.path);
    throw new Error(`GPU build inputs changed during bake; candidate discarded: ${changes.slice(0,8).join(', ')}`);
  }
}
export function gpuArtifact(directory, version, inputs, timing, format = 'no-modules') {
  return {version, format, inputs:sha256(JSON.stringify(inputs)), timing,
    files:Object.fromEntries(['gpu.js','gpu_bg.wasm'].map(name=> {
      const bytes=readFileSync(resolve(directory,name));
      return [name,{bytes:bytes.length,sha256:sha256(bytes)}];
    }))};
}
