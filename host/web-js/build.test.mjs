import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { webCompiler } from './module.mjs';

test('production lazy host modules ignore agent storage and entropy parameters', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/web-js-agent-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Agent gate', id: 'test.agent-gate', app: { id: 'test.agent-gate', name: 'Agent gate' } }));
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    input type="file"\n');
    const build = (out, args = []) => {
      const result = spawnSync(process.execPath, ['host/web-js/build.mjs', 'agent-gate', '--out', out, '--render', 'none', ...args], {
        cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, encoding: 'utf8',
      });
      expect(result.status, result.stdout + result.stderr).toBe(0);
    };
    const dev = resolve(dir, 'dev'), prod = resolve(dir, 'prod');
    build(dev);
    build(prod, ['--production', '--plan', resolve(dev, 'app.plan')]);
    const development = await import(pathToFileURL(resolve(dev, 'storage-environment.js')).href);
    const production = await import(pathToFileURL(resolve(prod, 'storage-environment.js')).href);
    const appId = 'test.agent-gate', own = development.storageKey(appId, 'https://example.test/');
    expect(own).toMatch(/^test\.agent-gate@[a-f0-9]{12}$/);
    expect(development.storageKey(appId, 'https://example.test/?agent&storage=x')).toBe(`${own}/agent/x`);
    expect(development.storageKey(appId, 'https://example.test/?agent')).toBeNull();
    expect(development.agentSeed('http://localhost/?agent&seed=7')).toBe(7);
    for (const origin of ['https://example.test', 'http://localhost']) {
      for (const query of ['', '?agent', '?agent&storage=x', '?agent&storage=../invalid']) {
        expect(production.storageKey(appId, origin + '/' + query)).toBe(appId);
      }
      expect(production.agentSeed(origin + '/?agent&seed=7')).toBeNull();
    }
    const shipped = readdirSync(prod, { recursive: true }).filter(name => /\.m?js$/.test(name));
    expect(shipped).toContain('picker-glue.js');
    expect(shipped).toContain('storage-environment.js');
    expect(shipped).toContain('navigation.js');
    for (const name of shipped) {
      expect(readFileSync(resolve(prod, name), 'utf8'), name).not.toContain('const AGENT_ADMITTED = true;');
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);


test('Snapback mutation success types check final answers through helpers and preserve ordinary resource values', async () => {
  const {configure, check} = await import('../../js/bake/src/typescript.mjs');
  const root = resolve(import.meta.dir, '../..');
  const libraries = resolve(dirname(fileURLToPath(import.meta.resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`))), 'lib');
  const dir = mkdtempSync(resolve(root, 'target/snapback-refusals-'));
  const declarations = `export type MutationSource = "post" | "post-note" | "count";
export interface SourceMap {
  post: {args: []; result: {ok: boolean; message: string}};
  "post-note": {args: []; result: {ok: boolean; message: string}};
  status: {args: []; result: {ok: boolean; message: string}};
  count: {args: []; result: number};
}
export type Source = keyof SourceMap;
export type Result<S extends Source> = SourceMap[S]['result'];
export type Sources = {[S in Source]: () => Result<S> | Promise<Result<S>>};
export type Answer = <S extends Source>(source: S) => Result<S> | Promise<Result<S>>;
`;
  const imports = "import type {Answer, Sources, Result} from './app.contract.d.ts';\n";
  const source = body => imports + `function refused(why: {code:string;message:string}) {return new Error(why.message);}
const sources: Sources = {
  post: () => {${body}},
  "post-note": () => ({ok:true, message:'Posted'}),
  status: () => ({ok:false, message:'Loading'}),
  count: () => 1,
};
export const answer: Answer = (name) => sources[name]();
`;
  const compile = async (body, snapback = true) => {
    writeFileSync(resolve(dir, 'app.ts'), source(body));
    writeFileSync(resolve(dir, '__exact_entry.ts'), "export * from './app.ts';\n");
    writeFileSync(resolve(dir, 'app.contract.d.ts'), declarations);
    writeFileSync(resolve(dir, '__exact_paths.json'), JSON.stringify({app:dir, mounts:snapback ? [['adapter',resolve(root,'snapback4/online')]] : []}));
    configure(dir);
    return check(dir, resolve(libraries, 'tsc'), libraries);
  };
  try {
    for (const body of [
      "return {ok:false, message:'No room'};",
      "const value = {ok:false, message:'No room'}; const alias = value; return alias;",
      "const value = {ok:false, message:'No room'}; const {ok,message} = value; return {ok,message};",
      "function identity<T>(value:T) {return value;} return identity({ok:false, message:'No room'});",
      "return Promise.resolve({ok:false, message:'No room'});",
      "function helper() {return {ok:false, message:'No room'};} return helper();",
    ]) {
      let failure;
      try {await compile(body);} catch(error) {failure=error;}
      expect(failure?.message, body).toContain('throw refused(');
    }
    await compile("function helper() {return {ok:false, message:'Handled'};} const intermediate = helper(); return {ok:true, message:intermediate.message};");
    await compile("function helper(): Result<'post'> {return {ok:true, message:'Posted'};} return helper();");
    await compile("const result = {ok:true as const, message:'Posted'}; return result;");
    await compile("throw refused({code:'FULL', message:'No room'});");
    await compile("return {ok:false, message:'Ordinary app result'};", false);
    expect(readFileSync(resolve(dir,'app.contract.d.ts'),'utf8')).toBe(declarations);
  } finally {rmSync(dir, {recursive: true, force: true});}
}, 30_000);
