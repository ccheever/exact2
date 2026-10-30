// Tuning edits use the validator compiled into the game's existing GPU module.
import {readFileSync, writeFileSync, renameSync, watchFile, unwatchFile} from 'node:fs';
import {basename, dirname, resolve} from 'node:path';
import {createContext, runInContext} from 'node:vm';
import childProcess from 'node:child_process';
import {syncBuiltinESMExports} from 'node:module';

let validatorRevision = 0;
export const tableAsset = 'assets/exact-tables.json';
export async function tableValidator(directory, noModules = false) {
  const file = resolve(directory, 'gpu.js'), bytes = readFileSync(resolve(directory, 'gpu_bg.wasm'));
  let module;
  if (noModules) {
    const context = createContext({WebAssembly, TextDecoder, TextEncoder, console, URL, tableModuleBytes:bytes});
    runInContext(readFileSync(file, 'utf8'), context, {filename:file});
    runInContext('wasm_bindgen.initSync({module:tableModuleBytes})', context);
    module = runInContext('wasm_bindgen', context);
  } else {
    // Bun caches file modules by path even with a new query. Each primary build needs a new instance.
    module = await import(`data:text/javascript;base64,${Buffer.from(readFileSync(file, 'utf8') + `\n// validator ${++validatorRevision}`).toString('base64')}`);
    module.initSync({module:bytes});
  }
  const validate = module.exact_game_tables;
  return validate ? {names:JSON.parse(validate('')), validate} : {names:[], validate:()=>{throw new Error('rebuild the game to validate tuning tables');}};
}

// A tuning transaction is synchronous through publication. Guard the process APIs themselves,
// including calls from imported helpers, and restore them before yielding to other dev work.
export function tableOperation(operation) {
  const originals = [], started = performance.now();
  let processes = 0;
  try {
    for (const [owner, names] of [[childProcess, ['spawn', 'spawnSync', 'exec', 'execSync', 'execFile', 'execFileSync', 'fork']], [Bun, ['spawn', 'spawnSync']]]) {
      for (const name of names) {
        const original = owner[name]; originals.push([owner, name, original]);
        owner[name] = (...args) => { processes++; throw new Error(`table publication prohibited subprocess: ${name} ${String(args[0])}`); };
      }
    }
    syncBuiltinESMExports();
    operation();
    if (processes) throw new Error(`table publication prohibited subprocess (${processes} attempted)`);
    return {milliseconds:performance.now()-started, processes};
  } finally {
    for (const [owner, name, original] of originals) owner[name] = original;
    syncBuiltinESMExports();
  }
}

// The carrier as the Rust baker writes it (game/bake/src/files.rs `bake_game_tables`: names in
// byte order, each string quoted as game/engine/src/data/json.rs `quote_into` does), so an
// unchanged table is the same bytes from either writer and neither rewrites the other's.
const quote = text => '"' + [...text].map(c => c === '"' ? '\\"' : c === '\\' ? '\\\\' : c === '\n' ? '\\n'
  : c === '\r' ? '\\r' : c === '\t' ? '\\t' : c < ' ' ? `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}` : c).join('') + '"';
export const carrier = rows => '{' + Object.keys(rows).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
  .map(name => `${quote(name)}:${quote(rows[name])}`).join(',') + '}';

export async function watchTables({app, dist, publish, refused, reported = () => {}}) {
  const paths = new Set(), listeners = new Map();
  let validator, timer, closed = false;
  // An identical file is left alone, as the Rust bakers' `write_changed` does: a rename moves
  // its directory's mtime, and the app's build script watches assets/, so the next Rust edit
  // would rerun the bake and rebuild the app's wasm (Beacons: 34-58 s at `web`).
  const write = (path, text) => {
    try { if (readFileSync(path).equals(Buffer.from(text))) return; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const temporary = resolve(dirname(path), `.${basename(path)}.${process.pid}.tmp`);
    writeFileSync(temporary, text); renameSync(temporary, path);
  };
  const flush = () => {
    if (closed || !validator.names.length) return;
    let text, published;
    try {
      const timing = tableOperation(() => {
        const rows = Object.fromEntries(validator.names.map(name => [name, readFileSync(resolve(app, name), 'utf8')]));
        text = carrier(rows);
        text = validator.validate(text);
        write(resolve(app, tableAsset), text);
        write(resolve(dist, tableAsset), text);
        published = publish(tableAsset, Buffer.from(text));
      });
      if (published !== false) reported(timing);
    } catch (error) { refused(`table swap refused: ${error.message ?? error}`, text); }
  };
  const reload = async (directory = dist, noModules = false) => {
    const next = await tableValidator(directory, noModules);
    closed = false; clearTimeout(timer);
    for (const [path, listener] of listeners) unwatchFile(path, listener);
    paths.clear(); listeners.clear(); validator = next;
    for (const name of next.names) {
      const path = resolve(app,name); paths.add(path);
      const listener = () => { clearTimeout(timer); timer = setTimeout(flush,20); };
      listeners.set(path,listener); watchFile(path, {interval:20}, listener);
    }
    flush();
  };
  await reload();
  return { paths, reload, flush, close() { closed = true; clearTimeout(timer); for (const [path, listener] of listeners) unwatchFile(path,listener); } };
}
