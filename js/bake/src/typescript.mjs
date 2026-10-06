// The one TypeScript configuration for an app's data module (`app.ts`), and
// the check every producer runs with it: the native bake (one-shot and
// resident, js/bake/src), which copies this file into its stage as
// `__exact_config.mjs`, and the web build (host/web-js/build.mjs), which
// imports it from here. One app.ts builds on every host or on none, refused
// with the same diagnostics at the same point (calc F2, calendar F9/F11).
// It reads only captured configuration; the producer owns the stage.
import { existsSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { resolve, relative, dirname, sep, isAbsolute } from 'node:path';

// Rolldown IDs are file paths, not URL or slash-normalized strings. Both
// producers enforce the same captured-file boundary (LLP 1027 D5).
export function assertCapturedModule(stage, id) {
  if (!isAbsolute(id)) throw new Error('module outside captured app: '+id);
  const root=realpathSync.native(stage), file=realpathSync.native(id), path=relative(root,file);
  if (!path || isAbsolute(path) || path==='..' || path.startsWith('..'+sep) || resolve(root,path)!==file || !statSync(file).isFile()) {
    throw new Error('module outside captured app: '+id);
  }
}

// Time and seeds are source arguments (LLP 1027.000): every executor refuses
// the clock, randomness and timers when they are used, with these words
// (js/src/prelude.js, host/web-js/ts-fetch.js). A direct use in the app's own
// code is refused here too, at build, by file and line, so a test that runs
// the module under Bun, which has no such guard, cannot hide it.
const ambient=(api)=>api+' is unavailable in data sources; pass time or a random seed as an argument';
const timers=(api)=>api+' is unavailable in data sources: there are no timers; pass time as an argument';
const TIMERS=['setTimeout','setInterval','requestAnimationFrame','requestIdleCallback'];
const GLOBALS=['globalThis','self','window','global'];
// `x!`, `(x)` and `x?.y` as written, down to the expression they wrap.
const bare=(node)=>{ while (node && ['TSNonNullExpression','ParenthesizedExpression','ChainExpression','TSAsExpression','TSSatisfiesExpression','TSTypeAssertion'].includes(node.type)) node=node.expression; return node; };
// The global `name`: the bare identifier, or a global object's property.
const ambientGlobal=(node,name,local)=>{
  node=bare(node);
  if (node?.type==='Identifier') return node.name===name && !local.has(name);
  return node?.type==='MemberExpression' && !node.computed && node.property?.name===name
    && bare(node.object)?.type==='Identifier' && GLOBALS.includes(bare(node.object).name) && !local.has(bare(node.object).name);
};
const member=(node,object,property,local)=>{ node=bare(node); return node?.type==='MemberExpression' && !node.computed && node.property?.name===property && ambientGlobal(node.object,object,local); };
function refusal(node,local) {
  if (node.type==='CallExpression') {
    const callee=node.callee;
    if (member(callee,'Date','now',local)) return ambient('Date.now()');
    if (member(callee,'performance','now',local)) return ambient('performance.now()');
    if (member(callee,'Math','random',local)) return 'Math.random() is unavailable in data sources; pass time or a random seed as an argument, or use crypto.getRandomValues';
    if (ambientGlobal(callee,'Date',local)) return ambient('Date()');
    const timer=TIMERS.find(name=>ambientGlobal(callee,name,local));
    if (timer) return timers(timer+'()');
  }
  if (node.type==='NewExpression' && ambientGlobal(node.callee,'Date',local) && !node.arguments.length) return ambient('new Date()');
  return null;
}
// The names the module binds itself, anywhere (a parameter, a variable, a
// function, class or import): such a `Date` is not the guarded global, so
// it is left alone; the runtime's refusal still guards the global behind it.
function bound(ast) {
  const names=new Set();
  const pattern=(p)=>{
    p=bare(p);
    if (!p) return;
    if (p.type==='Identifier') names.add(p.name);
    else if (p.type==='ObjectPattern') p.properties.forEach(q=>pattern(q.type==='RestElement' ? q.argument : q.value));
    else if (p.type==='ArrayPattern') p.elements.forEach(pattern);
    else if (p.type==='AssignmentPattern') pattern(p.left);
    else if (p.type==='RestElement') pattern(p.argument);
    else if (p.type==='TSParameterProperty') pattern(p.parameter);
  };
  // Only bindings that exist at run time: a `declare` or type-only one is
  // erased, and the global is what runs.
  const walk=(node,typeOnly)=>{
    if (!node || typeof node!=='object') return;
    if (Array.isArray(node)) { node.forEach(n=>walk(n,typeOnly)); return; }
    if (node.declare) return;
    if (node.type==='VariableDeclarator') pattern(node.id);
    if (['FunctionDeclaration','FunctionExpression','ClassDeclaration','ClassExpression'].includes(node.type) && node.id) names.add(node.id.name);
    if (['FunctionDeclaration','FunctionExpression','ArrowFunctionExpression'].includes(node.type)) node.params?.forEach(pattern);
    if (node.type==='CatchClause') pattern(node.param);
    // A namespace with a body emits a value (a type-only one is rare in a data module).
    if (node.type==='TSModuleDeclaration' && node.id?.type==='Identifier' && node.body) names.add(node.id.name);
    if (node.type==='TSImportEqualsDeclaration' && node.importKind!=='type') names.add(node.id.name);
    if (node.type==='ImportDeclaration') typeOnly=node.importKind==='type';
    if (['ImportSpecifier','ImportDefaultSpecifier','ImportNamespaceSpecifier'].includes(node.type) && !typeOnly && node.importKind!=='type') names.add(node.local.name);
    for (const key in node) if (key!=='parent') walk(node[key],typeOnly);
  };
  walk(ast,false);
  return names;
}
/** The direct uses of the clock, randomness and timers in the captured
 * module `file`, as `path:line:col: why` with its path in the app: `parse`
 * is Rolldown's (a plugin's `this.parse`, or `parseAst`), given TypeScript.
 * An early warning a Bun test cannot give, not the guard: an alias still
 * reaches the runtime's refusal. */
export function ambientRefusals(stage, file, code, parse) {
  if (!isAbsolute(file) || /\.d\.[cm]?ts$/.test(file) || !/\.[cm]?[jt]sx?$/.test(file)) return [];
  const lang=/\.[cm]?tsx?$/.test(file) ? (file.endsWith('x') ? 'tsx' : 'ts') : 'js';
  const label=relative(stage,file).split(sep).join('/'), found=[], starts=[0];
  // ECMAScript's line terminators: LF, CR (CRLF is one), U+2028, U+2029.
  for (let i=0;i<code.length;i++) {
    const c=code.charCodeAt(i);
    if (c===10 || c===0x2028 || c===0x2029 || (c===13 && code.charCodeAt(i+1)!==10)) starts.push(i+1);
  }
  const at=(offset)=>{ let lo=0, hi=starts.length; while (lo+1<hi) { const mid=(lo+hi)>>1; if (starts[mid]<=offset) lo=mid; else hi=mid; } return (lo+1)+':'+(offset-starts[lo]+1); };
  const ast=parse(code,{lang}), local=bound(ast);
  const walk=(node)=>{
    if (!node || typeof node!=='object') return;
    if (Array.isArray(node)) { node.forEach(walk); return; }
    if (typeof node.type==='string') { const why=refusal(node,local); if (why) found.push(label+':'+at(node.start)+': '+why); }
    for (const key in node) if (key!=='parent') walk(node[key]);
  };
  walk(ast);
  return found;
}

// The standard library is what every executor runs: the browser, and Hermes
// natively (js/src/standard.js fills what it lacks of ES2023). ES2024's
// ArrayBuffer resizing, shared memory and the RegExp `v` flag are not in
// Hermes, so they are not in the library. `WebWorker` is the web APIs a data
// module may name, not the DOM's UI types; docs/reference.md lists which of
// them each executor has. `.ts` import paths are the web's own (Bun, Deno,
// TypeScript 5 all take them), and Rolldown resolves them.
export const compilerOptions = {
  noEmit: true,
  strict: true,
  target: 'ES2023',
  module: 'ESNext',
  moduleResolution: 'bundler',
  allowImportingTsExtensions: true,
  lib: ['ES2023', 'ES2024.Object', 'ES2024.Collection', 'ES2024.Promise', 'ES2024.String', 'WebWorker'],
};

export function configure(stage, incremental=false) {
  const inside = (path) => {
    if (path !== stage && !path.startsWith(stage+sep)) throw new Error('tsconfig: path outside captured app: '+path);
    return path;
  };
  const mapping=resolve(stage,'__exact_paths.json');
  const roots=existsSync(mapping) ? JSON.parse(readFileSync(mapping,'utf8')) : {app:stage,mounts:[]};
  const within=(path,root)=>path===root || path.startsWith(root+sep);
  const origins=[...roots.mounts.map(([name,path])=>[resolve(stage,name),path]),[stage,roots.app]];
  const source = path => {
    const [at,from]=origins.find(([at])=>within(path,at));
    return resolve(from,relative(at,path));
  };
  const captured = path => {
    const match=[...origins].sort((a,b)=>b[1].length-a[1].length).find(([,from])=>within(path,from));
    if (!match) throw new Error('tsconfig: path outside captured app: '+path);
    return inside(resolve(match[0],relative(match[1],path)));
  };
  const local = path => './'+relative(stage,captured(path)).split(sep).join('/');
  const object = (value, label) => {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('tsconfig: '+label+' must be an object');
    return value;
  };
  const chain = new Set();
  function read(path) {
    inside(path);
    if (chain.has(path)) throw new Error('tsconfig: cyclic extends: '+path);
    chain.add(path);
    const json=object(Bun.JSONC.parse(readFileSync(path,'utf8')),path), dir=source(dirname(path));
    let inherited={};
    for (const parent of json.extends === undefined ? [] : Array.isArray(json.extends) ? json.extends : [json.extends]) {
      if (typeof parent !== 'string' || !parent.startsWith('.')) throw new Error('tsconfig: extends must name a captured relative config');
      let target=captured(resolve(dir,parent));
      if (!existsSync(target) && !target.endsWith('.json')) target+='.json';
      inherited={...inherited,...read(target)};
    }
    const options=object(json.compilerOptions ?? {},'compilerOptions');
    if (options.baseUrl !== undefined) {
      if (typeof options.baseUrl !== 'string') throw new Error('tsconfig: baseUrl must be a string');
      inherited.base=resolve(dir,options.baseUrl);
      captured(inherited.base);
    }
    if (options.paths !== undefined) inherited.paths={map:object(options.paths,'paths'),dir};
    chain.delete(path);
    return inherited;
  }
  const entry=resolve(stage,'tsconfig.json');
  const {base,paths:declared}=existsSync(entry) ? read(entry) : {};
  const paths={};
  for (const [key,values] of Object.entries(declared?.map ?? {})) {
    if (key.split('*').length>2 || !Array.isArray(values) || !values.length || values.some(v=>typeof v!=='string' || v.split('*').length>2 || isAbsolute(v))) {
      throw new Error('tsconfig: paths.'+key+' must name relative paths with at most one wildcard');
    }
    paths[key]=values.map(value=>local(resolve(base ?? declared.dir,value)));
    // TypeScript 7 removed baseUrl. Lower its fallback to paths, shared with
    // Rolldown, so existing editor configuration retains the same resolution.
    if (base) paths[key].push(local(resolve(base,key)));
  }
  if (base && !paths['*']) paths['*']=[local(resolve(base,'*'))];
  const config={compilerOptions:{...compilerOptions,paths,
    ...(incremental ? {incremental:true,tsBuildInfoFile:resolve(stage,'__exact_build.tsbuildinfo')} : {})},files:['__exact_entry.ts']};
  const path=resolve(stage,'__exact_tsconfig.json'), bytes=JSON.stringify(config);
  if (!existsSync(path) || readFileSync(path,'utf8')!==bytes) writeFileSync(path,bytes);
}

// Short-lived native checks trade fewer GC cycles for a soft Go memory limit
// (not an RSS cap); explicit user settings always win.
const checkEnv={GOGC:'300',GOMEMLIMIT:'128MiB',GOMAXPROCS:'4',...process.env};

/** Type-check a configured stage with the native compiler `tsc` (whose
 * standard libraries are in `libraries`): its diagnostics, or the compiler's
 * actual resolved graph, type-only imports included, must stay inside the
 * stage. An external declaration must not influence an accepted app artifact. */
export function check(stage, tsc, libraries) {
  const allowed = path => { path=resolve(path);return path === stage || path.startsWith(stage+sep) || path === libraries || path.startsWith(libraries+sep); };
  return new Promise((done, fail) => execFile(tsc,['--project',resolve(stage,'__exact_tsconfig.json'),'--pretty','false','--listFiles'],
    {cwd:stage,env:checkEnv,encoding:'utf8',maxBuffer:4*1024*1024}, (error, stdout, stderr) => {
      if (error) {
        const diagnostics=String(stdout??'').split(/\r?\n/).filter(line=>! /^(?:\/|[A-Za-z]:[\\/]).*\.[cm]?[jt]sx?$/.test(line)).join('\n');
        return fail(new Error(diagnostics+String(stderr??'') || String(error.message)));
      }
      for(const path of stdout.trim().split(/\r?\n/)) {
        if(path && !allowed(realpathSync(path)))return fail(new Error('module outside captured app: '+path));
      }
      done();
    }));
}
