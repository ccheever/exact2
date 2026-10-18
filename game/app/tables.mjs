// Tuning edits use the validator compiled into the game's existing GPU module.
import {readFileSync, writeFileSync, renameSync, watchFile, unwatchFile} from 'node:fs';
import {basename, dirname, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createContext, runInContext} from 'node:vm';

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
    module = await import(`${pathToFileURL(file)}?tables=${Date.now()}`);
    module.initSync({module:bytes});
  }
  const validate = module.exact_game_tables;
  return validate ? {names:JSON.parse(validate('')), validate} : {names:[], validate:()=>{throw new Error('rebuild the game to validate tuning tables');}};
}

export async function watchTables({app, dist, processCount, publish, refused}) {
  const paths = new Set(), listeners = new Map();
  let validator, timer, closed = false;
  const write = (path, text) => {
    const temporary = resolve(dirname(path), `.${basename(path)}.${process.pid}.tmp`);
    writeFileSync(temporary, text); renameSync(temporary, path);
  };
  const flush = () => {
    if (closed || !validator.names.length) return;
    const started = performance.now(), processes = processCount();
    let text;
    try {
      const rows = Object.fromEntries(validator.names.map(name => [name, readFileSync(resolve(app, name), 'utf8')]));
      text = JSON.stringify(rows);
      text = validator.validate(text);
      write(resolve(app, tableAsset), text);
      write(resolve(dist, tableAsset), text);
      publish(tableAsset, {milliseconds:performance.now()-started, processes:processCount()-processes});
    } catch (error) { refused(`table swap refused: ${error.message ?? error}`, text); }
  };
  const reload = async (directory = dist, noModules = false) => {
    const next = await tableValidator(directory, noModules);
    closed = false;
    for (const [path, listener] of listeners) unwatchFile(path, listener);
    paths.clear(); listeners.clear(); validator = next;
    for (const name of next.names) {
      const path = resolve(app,name); paths.add(path);
      const listener = () => { clearTimeout(timer); timer = setTimeout(flush,20); };
      listeners.set(path,listener); watchFile(path, {interval:20}, listener);
    }
  };
  await reload();
  return { paths, reload, close() { closed = true; clearTimeout(timer); for (const [path, listener] of listeners) unwatchFile(path,listener); } };
}
