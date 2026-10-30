import { test, expect, mock, spyOn } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { moduleGrants, webCompiler } from './module.mjs';
import { spawnOwned } from '../../scripts/test-children.mjs';

test.each([
  ['empty', ''],
  ['network', 'net.fetch https://api.example'],
  ['storage', 'fs.read app:/data\nsqlite.open app:/data/notes.db'],
])('module metadata preserves grants: %s', async (_label, grants) => {
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/module-grants-')), path = resolve(dir, 'app.module.wasm');
  writeFileSync(path, new Uint8Array());
  const memory = new WebAssembly.Memory({ initial: 1 }), out = 1024, bytes = [];
  const u32 = n => bytes.push(n & 255, n >>> 8 & 255, n >>> 16 & 255, n >>> 24 & 255);
  const string = s => { const b = new TextEncoder().encode(s); u32(b.length); bytes.push(...b); };
  u32(4); bytes.push(0); string('test.grants'); string(grants);
  new Uint8Array(memory.buffer, out, bytes.length).set(bytes);
  const output = mock(() => out);
  const instantiate = spyOn(WebAssembly, 'instantiate').mockResolvedValue({ instance: { exports: {
    memory, exact_logic_abi: () => 4, exact_logic_create: () => 1, exact_logic_alloc: () => 0,
    exact_logic_call(_session, pointer, length) {
      const request = new DataView(memory.buffer, pointer, length);
      return length === 5 && request.getUint32(0, true) === 4 && request.getUint8(4) === 0 ? 0 : 1;
    },
    exact_logic_output: output, exact_logic_output_len: () => bytes.length,
  } } });
  try {
    const actual = await moduleGrants(path);
    expect(output).toHaveBeenCalledTimes(1);
    expect(actual).toBe(grants);
  } finally {
    instantiate.mockRestore();
    rmSync(dir, { recursive: true, force: true });
  }
});

test.each([false, true])('optional hook warnings are confined to authored imports (authored: %s)', authored => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/web-js-warnings-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Warnings', app: { id: 'test.warnings', name: 'Warnings' } }));
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "Warnings"\n');
    writeFileSync(resolve(dir, 'app.ts'), `${authored ? "import { value } from './ts-data.js';" : "const value = 'ok';"}
export const appId = 'test.warnings';
export const grants = '';
export const answer = () => value as never;
`);
    // The author's file has the glue's basename and probes the same names.
    writeFileSync(resolve(dir, 'ts-data.js'), "import * as source from './empty.js';\nexport const value = [source.kept, source.overlay, source.other].join(',');\n");
    writeFileSync(resolve(dir, 'empty.js'), 'export {};\n');
    const out = resolve(dir, 'dist');
    const result = spawnSync(process.execPath, ['host/web-js/build.mjs', 'warnings', '--out', out], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, encoding: 'utf8',
    });
    expect(result.status, result.stdout + result.stderr).toBe(0);
    expect(readFileSync(resolve(out, '.gen/server.js'), 'utf8')).toContain('__start');
    const stderr = result.stderr.replace(/\x1b\[[0-9;]*m/g, '');
    if (authored) {
      for (const name of ['kept', 'overlay', 'other']) {
        expect(stderr).toContain(`BuildMessage: Import "${name}" will always be undefined`);
        expect(stderr).toContain(`[IMPORT_IS_UNDEFINED] Import \`${name}\` will always be undefined`);
      }
      expect(stderr).toContain('.gen/typescript/ts-data.js:');
      expect(stderr).not.toContain('.gen/ts-data.js:');
      expect(stderr.match(/BuildMessage: Import /g)).toHaveLength(3);
    } else {
      expect(stderr).not.toContain('IMPORT_IS_UNDEFINED');
      expect(stderr).not.toContain('will always be undefined');
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

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
    // A release over a bake's plan, and one with no bake (an app that names no native platform, LLP 1153 D3.6).
    const dev = resolve(dir, 'dev'), prod = resolve(dir, 'prod'), webOnly = resolve(dir, 'web-only');
    build(dev);
    build(prod, ['--production', '--plan', resolve(dev, 'app.plan')]);
    build(webOnly, ['--production']);
    const development = await import(pathToFileURL(resolve(dev, 'storage-environment.js')).href);
    const appId = 'test.agent-gate', own = development.storageKey(appId, 'https://example.test/');
    expect(own).toMatch(/^test\.agent-gate@[a-f0-9]{12}$/);
    expect(development.storageKey(appId, 'https://example.test/?agent&storage=x')).toBe(`${own}/agent/x`);
    expect(development.storageKey(appId, 'https://example.test/?agent')).toBeNull();
    expect(development.agentSeed('http://localhost/?agent&seed=7')).toBe(7);
    for (const release of [prod, webOnly]) {
      const production = await import(pathToFileURL(resolve(release, 'storage-environment.js')).href);
      for (const origin of ['https://example.test', 'http://localhost']) {
        for (const query of ['', '?agent', '?agent&storage=x', '?agent&storage=../invalid']) {
          expect(production.storageKey(appId, origin + '/' + query)).toBe(appId);
        }
        expect(production.agentSeed(origin + '/?agent&seed=7')).toBeNull();
      }
      const shipped = readdirSync(release, { recursive: true }).filter(name => /\.m?js$/.test(name));
      expect(shipped).toContain('picker-glue.js');
      expect(shipped).toContain('storage-environment.js');
      expect(shipped).toContain('navigation.js');
      for (const name of shipped) {
        expect(readFileSync(resolve(release, name), 'utf8'), name).not.toContain('const AGENT_ADMITTED = true;');
      }
    }
    // With no bake, the page links its web manifest and no envelope, and the install pages name a release.
    const page = readFileSync(resolve(webOnly, 'index.html'), 'utf8');
    expect(page).toContain('<link rel="manifest" href="./manifest.json">');
    expect(page).not.toContain('application/vnd.exact.envelope+json');
    expect(readdirSync(webOnly)).not.toContain('exact.json');
    expect(readFileSync(resolve(webOnly, '.exact/install.json'), 'utf8')).toContain('"mode": "Release build"');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);


// LLP 1153 D3: `web-build --out` writes a site any static host serves (its pages, the shell, the route table and
// 404.html, none of the build's intermediates but the server bundle), ends saying how a host serves it, and serve.mjs
// serves it so; `serve` (serve.mjs as the verb runs it) renders a `render=request` route's page for each request and
// keeps a `cached` one's.
test('web-build --out writes a site that serve.mjs serves by the hosting rule, rendering request and cached routes', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-site-')), app = resolve(dir, 'site'), out = resolve(dir, 'dist');
  let server, serve;
  try {
    mkdirSync(app);
    writeFileSync(resolve(app, 'app.json'), JSON.stringify({ name: 'Site', app: { id: 'test.site', name: 'Site' } }));
    writeFileSync(resolve(app, 'app.contract'), 'shape Note\n  text: string\n\nroutes nav\n  home "/" render=build\n  item "/item/:id"\n  live "/live" render=request\n  feed "/feed" render=cached\n  notfound render=build\n\n'
      + 'component App\n  resource note = note("rendered") as shape Note\n  view\n    column\n      text top(nav).url\n      text note.text\n');
    writeFileSync(resolve(app, 'app.ts'), "import type { Answer, Result, Sources } from './app.contract.d.ts';\nexport const appId = 'test.site';\nexport const grants = '';\n"
      + "const sources: Sources = { note: ([what]): Result<'note'> => ({ text: `The note, ${what}.` }) };\n"
      + 'export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);\n');
    const built = spawnSync(process.execPath, ['host/web/build.mjs', app, '--out', out], { cwd: root, env: { ...process.env, EXACT_APP_DIR: app }, encoding: 'utf8' });
    expect(built.status, built.stdout + built.stderr).toBe(0);
    expect(built.stdout).toContain('site: no site-render entry, so its pages render under Bun (--render js)');
    expect(built.stdout).toContain('/live: render=request, so serve renders its page for each request; a static host serves its shell');
    expect(built.stdout).toContain('/feed: render=cached, so serve renders its page and keeps it 60 s; a static host serves its shell');
    expect(built.stdout.trimEnd().split('\n').slice(-3).join('\n')).toMatch(/dist: the site, for any static host\. bun host\/web\/serve\.mjs .*dist serves it[^]*shell\.html with status 200; else 404\.html with status 404\.\n {2}A host with a single fallback page serves shell\.html/);
    expect(JSON.parse(readFileSync(resolve(out, 'routes.json'), 'utf8')).routes).toEqual([
      { name: 'home', pattern: '/', render: 'build', notfound: false },
      { name: 'item', pattern: '/item/:id', render: 'client', notfound: false },
      { name: 'live', pattern: '/live', render: 'request', notfound: false },
      { name: 'feed', pattern: '/feed', render: 'cached', notfound: false },
      { name: 'notfound', pattern: '', render: 'build', notfound: true },
    ]);
    const files = readdirSync(out);
    for (const name of ['index.html', 'shell.html', '404.html', 'routes.json', '.exact-build.json']) expect(files).toContain(name);
    expect(readdirSync(resolve(out, '.gen'))).toEqual(['server.js']);
    expect(files).not.toContain('app.plan.map.json');
    expect(JSON.parse(readFileSync(resolve(out, '.exact-build.json'), 'utf8')).inputs).toBeUndefined();
    expect(readdirSync(dir).sort()).toEqual(['dist', 'site']);
    // As a static host serves it: a request or cached route gets the shell, which has none of the data.
    const { serveBuildTree } = await import('../web/serve.mjs');
    server = createServer((req, res) => serveBuildTree(out, req, res));
    await new Promise(ok => server.listen(0, '127.0.0.1', ok));
    const page = (name) => readFileSync(resolve(out, name), 'utf8');
    expect(page('shell.html')).not.toContain('The note, rendered.');
    for (const [path, status, file] of [['/', 200, 'index.html'], ['/item/7', 200, 'shell.html'], ['/live', 200, 'shell.html'], ['/no/such/route', 404, '404.html'], ['/nope.png', 404, '404.html']]) {
      const r = await fetch(`http://127.0.0.1:${server.address().port}${path}`);
      // (A served document names its build's generation in its base.)
      expect([path, r.status, (await r.text()).replace(/<base href="[^"]*">/, '<base href="/">')]).toEqual([path, status, page(file)]);
    }
    // As `serve` serves it: each route's page rendered with its data, the live one per request, the cached one once.
    const probe = createServer();
    await new Promise(ok => probe.listen(0, '127.0.0.1', ok));
    const port = probe.address().port;
    await new Promise(ok => probe.close(ok));
    serve = spawnOwned(process.execPath, ['host/web/serve.mjs', out, '--port', String(port), '--loopback'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
    let said = '';
    const renders = (path) => (said.match(new RegExp(`render ${path} 200 `, 'g')) ?? []).length;
    // What serve has said, once `done` holds of it (its output is the event; bounded so a failure names it).
    const heard = (done) => new Promise((ok, no) => {
      const timer = setTimeout(() => { serve.stdout.off('data', check); no(new Error(`serve did not say it: ${said}`)); }, 30_000);
      const check = () => { if (done(said)) { clearTimeout(timer); serve.stdout.off('data', check); ok(); } };
      serve.stdout.on('data', check); check();
    });
    serve.stdout.on('data', (data) => { said += data; });
    serve.stderr.on('data', (data) => { said += data; });
    await heard((text) => text.includes(`http://127.0.0.1:${port}/`));
    const get = async (path) => { const r = await fetch(`http://127.0.0.1:${port}${path}`); return { status: r.status, cache: r.headers.get('cache-control'), body: await r.text() }; };
    const feed = [await get('/feed'), await get('/feed')], live = [await get('/live'), await get('/live')];
    for (const r of [...feed, ...live]) expect([r.status, r.body.includes('The note, rendered.'), r.body.includes('<div id="exact-root"></div>')]).toEqual([200, true, false]);
    expect(feed.map((r) => r.cache)).toEqual(['public, max-age=0, s-maxage=60', 'public, max-age=0, s-maxage=60']);
    expect(feed[1].body).toBe(feed[0].body);
    expect(live.map((r) => r.cache)).toEqual(['no-store', 'no-store']);
    // Each render is said before its page is sent: once the second live one is said, the cached page was rendered once.
    await heard(() => renders('/live') === 2);
    expect(renders('/feed')).toBe(1);
    expect(await get('/item/7')).toMatchObject({ status: 200, body: expect.not.stringContaining('The note, rendered.') });
    expect((await get('/no/such/route')).status).toBe(404);
  } finally {
    await serve?.stop(5_000);
    if (server) { server.closeAllConnections(); await new Promise(ok => server.close(ok)); }
    rmSync(dir, { recursive: true, force: true });
  }
}, 90000);

test('Snapback mutation success types check final answers through helpers and preserve ordinary resource values', async () => {
  const {configure, check} = await import('../../js/bake/src/typescript.mjs');
  const {parseSync} = await import('rolldown/utils');
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
    configure(dir, false, parseSync); // as build.mjs configures a stage
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
