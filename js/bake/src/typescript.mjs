// The one TypeScript configuration for an app's data module (`app.ts`), and
// the check every producer runs with it: the native bake (one-shot and
// resident, js/bake/src), which copies this file into its stage as
// `__exact_config.mjs`, and the web build (host/web-js/build.mjs), which
// imports it from here. One app.ts builds on every host or on none, refused
// with the same diagnostics at the same point (calc F2, calendar F9/F11).
// It reads only captured configuration; the producer owns the stage.
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, realpathSync, rmdirSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { resolve, relative, dirname, join, sep, isAbsolute } from 'node:path';

/** Explicit build input, ahead of an ignored app-local development selection. */
export function backendOrigin(app, env=process.env) {
  const file=resolve(app,'.exact/backend-origin');
  const value=env.EXACT_BACKEND_ORIGIN ?? (existsSync(file) ? readFileSync(file,'utf8').trim() : null);
  if (value===null) return null;
  let url; try { url=new URL(value); } catch { throw new Error('EXACT_BACKEND_ORIGIN: expected an http(s) origin'); }
  if (!['http:','https:'].includes(url.protocol) || url.username || url.password || url.pathname!=='/' || url.search || url.hash)
    throw new Error('EXACT_BACKEND_ORIGIN: expected an http(s) origin without credentials, path, query or fragment');
  return url.origin;
}
/** Rewrite only the top-level literal origin and its exact net.fetch grant literals in the captured copy. */
export function injectBackendOrigin(code, origin, parse) {
  if (origin===null) return code;
  const parsed=parse('app.ts',code);
  if (parsed.errors.length) throw new Error('EXACT_BACKEND_ORIGIN: app.ts must parse before selecting a backend');
  const declarations=parsed.program.body.flatMap(s=>(s.type==='ExportNamedDeclaration' ? s.declaration : s)?.declarations ?? []);
  const targets=declarations.filter(d=>d.id.type==='Identifier' && d.id.name==='origin');
  const init=targets[0]?.init;
  const literal=init?.type==='Literal' && typeof init.value==='string' ? init.value
    : init?.type==='TemplateLiteral' && !init.expressions.length ? init.quasis[0].value.cooked : null;
  if (targets.length!==1 || !literal || !/^https?:\/\//.test(literal))
    throw new Error('EXACT_BACKEND_ORIGIN: app.ts needs one top-level literal const origin; use it in net.fetch ${origin}');
  const edits=[[init.start,init.end,JSON.stringify(origin)]];
  const previous=literal.replace(/\/+$/,'');
  const replace=value=>value.split('\n').map(line=>line===`net.fetch ${previous}` || line===`net.fetch ${previous}/` ? `net.fetch ${origin}` : line).join('\n');
  const walk=node=>{
    if (!node || typeof node!=='object') return;
    if (node.type==='Literal' && typeof node.value==='string') { const next=replace(node.value); if(next!==node.value) edits.push([node.start,node.end,JSON.stringify(next)]); return; }
    if (node.type==='TemplateLiteral' && !node.expressions.length) { const next=replace(node.quasis[0].value.cooked); if(next!==node.quasis[0].value.cooked) edits.push([node.start,node.end,JSON.stringify(next)]); return; }
    if (node.type==='TemplateElement') { if(replace(node.value.cooked)!==node.value.cooked) throw new Error('EXACT_BACKEND_ORIGIN: use net.fetch ${origin} in an interpolated grants template'); return; }
    for(const child of Object.values(node)) if(Array.isArray(child)) child.forEach(walk); else if(child && typeof child==='object') walk(child);
  };
  declarations.filter(d=>d.id.type==='Identifier' && d.id.name==='grants').forEach(d=>walk(d.init));
  for(const [start,end,value] of edits.sort((a,b)=>b[0]-a[0])) code=code.slice(0,start)+value+code.slice(end);
  return code;
}

// Rolldown IDs are file paths, not URL or slash-normalized strings. Both
// producers enforce the same captured-file boundary (LLP 1027 D5). A
// package's declarations are captured for the checker alone (`stagePackages`):
// nothing under the stage's `node_modules` runs, so loading one is refused by
// the file it was captured from.
export function assertCapturedModule(stage, id) {
  if (!isAbsolute(id)) throw new Error('module outside captured app: '+id);
  const root=realpathSync.native(stage), file=realpathSync.native(id), path=relative(root,file);
  if (!path || isAbsolute(path) || path==='..' || path.startsWith('..'+sep) || resolve(root,path)!==file || !statSync(file).isFile()) {
    throw new Error('module outside captured app: '+id);
  }
  if (path.split(sep).includes('node_modules')) throw new Error('module outside captured app: '+packageOrigin(root,file));
}

// Package declarations (LLP 1027 D5, "Type-only package imports"). A bare
// specifier names a package. Its declaration files (`.d.ts`, which emit
// nothing) and `package.json` files are captured into the stage's
// `node_modules`, at the place TypeScript looks for them, so the checker reads
// a package's types from the capture as it reads app.ts, and the resolved
// graph still stays inside the stage. Its JavaScript is never captured: an
// import that reaches a package at run time is refused (`packageImports`).
const DECLARATION=/\.d\.[cm]?ts$/;
// A string after `from`, `import`, `import(`, `require(`, `declare module` or
// `<reference types=`, comments between included. Matching more than an
// import costs at most a package's declarations, staged and unread.
const SPECIFIER=/(?:\bfrom|\bimport|\brequire|\bmodule|\btypes\s*=)(?:\s|\(|\/\*[\s\S]*?\*\/|\/\/[^\n]*)*(['"])([^'"\r\n]+)\1/g;
const PACKAGES='__exact_declarations.json';
// The package a specifier names, or null: relative, absolute, `#` subpath and
// scheme (`node:`, `exact:`) specifiers name none.
export function packageName(spec) {
  if (/^[./\\#]|^[A-Za-z][\w+.-]*:/.test(spec)) return null;
  const parts=spec.split('/'), name=spec.startsWith('@') ? parts.slice(0,2).join('/') : parts[0];
  return /^(?:@[\w.-]+\/)?[\w.-]+$/.test(name) && !/^\.+$/.test(name.split('/').at(-1)) ? name : null;
}
// Where a staged package file came from (`__exact_declarations.json`).
function packageOrigin(stage, file) {
  try {
    const { packages }=JSON.parse(readFileSync(resolve(stage,PACKAGES),'utf8'));
    const match=packages.map(([at,from])=>[resolve(stage,at),from]).sort((a,b)=>b[0].length-a[0].length)
      .find(([at])=>file===at || file.startsWith(at+sep));
    if (match) return resolve(match[1],relative(match[0],file));
  } catch { /* no packages staged */ }
  return file;
}
// The captured app's places: the app at the stage's root and each mount
// under its name, each with the directory it was captured from.
function capturedOrigins(stage) {
  const mapping=resolve(stage,'__exact_paths.json');
  const roots=existsSync(mapping) ? JSON.parse(readFileSync(mapping,'utf8')) : {app:stage,mounts:[]};
  return [...roots.mounts.map(([name,path])=>[resolve(stage,name),path]),[stage,roots.app]];
}
const within=(path,root)=>path===root || path.startsWith(root+sep);
/** Capture into the stage the declarations of every package the captured
 * TypeScript names, and of the packages those declarations name: each found
 * from where its file was captured, as Node and TypeScript find it (the
 * nearest `node_modules` upward; a package's `@types` companion too), and
 * placed where the checker, walking up from the staged file, finds it. A
 * `node_modules` inside a captured root is mirrored at its place; one above
 * it is placed at the root (the app's at the stage's top, a mount's under its
 * name); a package's dependency sits beside it unless nested inside it.
 * Writes `__exact_declarations.json`: each staged package, where it is
 * installed and the directory that is, and every file read where it is
 * installed, so a producer can watch them. */
export function stagePackages(stage) {
  const roots=capturedOrigins(stage);
  const staged=new Map(), files=new Map(), queue=[];
  let total=0;
  // Where `name`, needed by a file in `dir` on disk, is found and placed.
  const find=(name,dir,root,owner)=>{
    for (let at=dir;;) {
      const candidate=join(at,'node_modules',name);
      if (!within(candidate,stage) && !within(stage,candidate) && existsSync(join(candidate,'package.json'))) {
        const mirror=(from,to)=>within(at,from) ? resolve(to,relative(from,at),'node_modules') : null;
        const modules=owner ? mirror(owner.from,owner.at) ?? owner.modules : mirror(root[1],root[0]) ?? resolve(root[0],'node_modules');
        return {real:realpathSync(candidate),installed:candidate,place:join(modules,name),modules};
      }
      const up=dirname(at);
      if (up===at) return null;
      at=up;
    }
  };
  const need=(text,dir,root,owner)=>{
    for (const [, , spec] of text.matchAll(SPECIFIER)) {
      const name=packageName(spec);
      if (!name) continue;
      for (const wanted of [name,'@types/'+name.replace(/^@/,'').replace('/','__')]) {
        const found=find(wanted,dir,root,owner);
        if (!found) continue;
        const known=staged.get(found.place)?.real;
        if (known && known!==found.real) throw new Error(`package declarations: two copies of ${wanted} (${known}, ${found.real}); install one`);
        if (known) continue;
        staged.set(found.place,found);
        copy(found.real,found.place,{at:found.place,from:found.real,installed:found.installed,modules:found.modules});
      }
    }
  };
  const copy=(dir,place,owner)=>{
    for (const entry of readdirSync(dir,{withFileTypes:true})) {
      const name=entry.name, path=join(dir,name);
      if (name==='node_modules' || name.startsWith('.')) continue;
      const kind=entry.isSymbolicLink() ? statSync(path,{throwIfNoEntry:false}) : entry;
      if (kind?.isDirectory()) copy(path,join(place,name),owner);
      else if (kind?.isFile() && (name==='package.json' || DECLARATION.test(name))) {
        const bytes=readFileSync(path);
        total+=bytes.length;
        if (total>64<<20) throw new Error('package declarations exceed 64 MiB');
        // Recorded where it is installed: a link retargeted is a change there.
        files.set(join(place,name),[join(owner.installed,relative(owner.from,path)),bytes]);
        if (DECLARATION.test(name)) queue.push([bytes.toString('utf8'),dirname(realpathSync(path)),null,owner]);
      }
    }
  };
  // The captured sources, from where each was captured, and every staged
  // `node_modules` (the app's capture holds none).
  const mirrors=[];
  const sources=(dir)=>{
    for (const entry of readdirSync(dir,{withFileTypes:true})) {
      const path=join(dir,entry.name);
      if (entry.isDirectory() && entry.name==='node_modules') mirrors.push(path);
      else if (entry.isDirectory()) sources(path);
      else if (entry.isFile() && /\.[cm]?tsx?$/.test(entry.name) && !entry.name.startsWith('__exact_')) {
        const root=roots.find(([at])=>within(path,at));
        need(readFileSync(path,'utf8'),dirname(resolve(root[1],relative(root[0],path))),root,null);
      }
    }
  };
  sources(stage);
  while (queue.length) need(...queue.shift());
  // The staged trees are exactly this capture: a package no longer named,
  // or no longer installed, goes.
  const prune=(dir)=>{
    for (const entry of readdirSync(dir,{withFileTypes:true})) {
      const path=join(dir,entry.name);
      if (entry.isDirectory()) { prune(path); try { rmdirSync(path); } catch { /* not empty */ } }
      else if (!files.has(path)) unlinkSync(path);
    }
  };
  for (const dir of mirrors) { prune(dir); try { rmdirSync(dir); } catch { /* still staged */ } }
  for (const [path,[,bytes]] of files) {
    if (existsSync(path) && readFileSync(path).equals(bytes)) continue;
    mkdirSync(dirname(path),{recursive:true});
    writeFileSync(path,bytes);
  }
  const record=JSON.stringify({
    packages:[...staged].map(([at,{installed,real}])=>[relative(stage,at),installed,real]).sort(),
    files:[...files.values()].map(([path])=>path).sort(),
  });
  const recordPath=resolve(stage,PACKAGES);
  if (!existsSync(recordPath) || readFileSync(recordPath,'utf8')!==record) writeFileSync(recordPath,record);
}

/** Rolldown's `resolveId` for a captured graph: a bare import that reaches a
 * package at run time (a value import, never one TypeScript erases) is
 * refused as any module outside the capture is, by the file it would run. */
export function packageImports(stage) {
  // Only staged declarations sit under a `node_modules` in the stage.
  const staged=(id)=>within(id,stage) && relative(stage,id).split(sep).includes('node_modules');
  return async function (source, importer, options) {
    if (!importer || !packageName(source)) return null;
    // `rows/../../helper.ts` names no file of a package: it leaves it, to a
    // place that differs between the stage and the app's own directory.
    if (source.split(/[\\/]/).some(part => part === '.' || part === '..')) throw new Error('module outside captured app: '+source);
    const resolved=await this.resolve(source, importer, { ...options, skipSelf: true });
    if (resolved && !staged(resolved.id)) return resolved;
    // Where the import leads from the captured file's own place.
    const [at,origin]=capturedOrigins(stage).find(([at])=>within(importer,at)) ?? [stage,stage];
    const real=await this.resolve(source, resolve(origin,relative(at,importer)), { skipSelf: true }).catch(() => null);
    if (!resolved && !real) return null;
    throw new Error('module outside captured app: '+(real?.id ?? packageOrigin(stage,resolved.id)));
  };
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
// A literal bracket key names the same property as dot access. Dynamic
// keys and aliases stay with the runtime guard; do not evaluate them here.
const propertyName=(node)=>{
  if (node?.type!=='MemberExpression') return null;
  const property=bare(node.property);
  if (!node.computed) return property?.type==='Identifier' ? property.name : null;
  if (property?.type==='Literal' && typeof property.value==='string') return property.value;
  if (property?.type==='TemplateLiteral' && !property.expressions.length) return property.quasis[0].value.cooked;
  return null;
};
// The global `name`: the bare identifier, or a global object's property.
const ambientGlobal=(node,name,local)=>{
  node=bare(node);
  if (node?.type==='Identifier') return node.name===name && !local.has(name);
  return propertyName(node)===name
    && bare(node.object)?.type==='Identifier' && GLOBALS.includes(bare(node.object).name) && !local.has(bare(node.object).name);
};
const member=(node,object,property,local)=>{ node=bare(node); return propertyName(node)===property && ambientGlobal(node.object,object,local); };
function refusal(node,local) {
  if (node.type==='CallExpression') {
    const callee=node.callee;
    if (member(callee,'Date','now',local)) return ambient('Date.now()');
    if (member(callee,'performance','now',local)) return ambient('performance.now()');
    if (member(callee,'Math','random',local)) return 'Math.random() is unavailable in data sources; seed from crypto.getRandomValues in a source (docs/recipes/random-and-shuffle.md), or pass a seed as an argument';
    if (ambientGlobal(callee,'Date',local)) return ambient('Date()');
    const timer=TIMERS.find(name=>ambientGlobal(callee,name,local));
    if (timer) return timers(timer+'()');
  }
  if (node.type==='NewExpression' && ambientGlobal(node.callee,'Date',local) && !node.arguments.length) return ambient('new Date()');
  return null;
}
const VALUES=['FunctionDeclaration','VariableDeclaration','ClassDeclaration','ExpressionStatement','TSEnumDeclaration','TSImportEqualsDeclaration'];
function instantiated(ns) {
  const body=ns.body;
  if (!body) return false;
  if (body.type==='TSModuleDeclaration') return instantiated(body);
  return (body.body ?? []).some(statement=>{
    const s=['ExportNamedDeclaration','ExportDefaultDeclaration'].includes(statement.type) ? statement.declaration : statement;
    if (!s || s.declare || (s.type==='TSEnumDeclaration' && s.const)) return false;
    return VALUES.includes(s.type) || (s.type==='TSModuleDeclaration' && instantiated(s));
  });
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
    // A namespace emits a value only when it holds one (an empty or
    // type-only one is erased, and the global runs).
    if (node.type==='TSModuleDeclaration' && node.id?.type==='Identifier' && instantiated(node)) names.add(node.id.name);
    if (node.type==='TSImportEqualsDeclaration' && node.importKind!=='type') names.add(node.id.name);
    if (node.type==='ImportDeclaration') typeOnly=node.importKind==='type';
    if (['ImportSpecifier','ImportDefaultSpecifier','ImportNamespaceSpecifier'].includes(node.type) && !typeOnly && node.importKind!=='type') names.add(node.local.name);
    for (const key in node) if (key!=='parent') walk(node[key],typeOnly);
  };
  walk(ast,false);
  return names;
}
// A Snapback mutation's returned failure is still a successful source answer.
// Refuse the literal at build and name the throw that populates failure(slot).
function snapbackMutationRefusals(stage, file, ast) {
  if (relative(stage,file)!=='app.ts' || !ast.body.some(n=>n.type==='ImportDeclaration' && /(?:^|\/)snapback4\//.test(n.source.value))) return [];
  const app=capturedOrigins(stage).at(-1)[1], contract=resolve(app,'app.contract');
  if (!existsSync(contract)) return [];
  const names=new Set([...readFileSync(contract,'utf8').matchAll(/\bsend\s+[\w$]+\s*=\s*([\w$]+)\s*\(/g)].map(m=>m[1]));
  const roots=[], failed=new Set(), scopes=new WeakMap();
  const key=n=>n?.type==='Identifier' ? n.name : n?.type==='Literal' ? n.value : null;
  const lookup=(scope,name)=>{
    for(;scope;scope=scope.parent) if(scope.bindings.has(name)) return scope.bindings.get(name);
    return [];
  };
  const functionNode=node=>['FunctionDeclaration','ArrowFunctionExpression','FunctionExpression'].includes(node?.type);
  const scan=(node,scope)=>{
    if (!node || typeof node!=='object') return;
    if (Array.isArray(node)) {node.forEach(child=>scan(child,scope));return;}
    if (node.type==='FunctionDeclaration' && node.id) {
      scope.bindings.set(node.id.name,[node]);
      if(names.has(node.id.name)) roots.push(node);
    }
    if (node.type==='VariableDeclarator' && node.id.type==='Identifier') {
      scope.bindings.set(node.id.name,node.init ? [node.init] : []);
      if(names.has(node.id.name) && functionNode(bare(node.init))) roots.push(node.init);
    }
    if (node.type==='AssignmentExpression' && node.operator==='=' && node.left.type==='Identifier') lookup(scope,node.left.name).push(node.right);
    if (node.type==='Property' && names.has(key(node.key))) roots.push(node.value);
    if (functionNode(node) || node.type==='BlockStatement') {
      scope={parent:scope,bindings:new Map()};
      // Parameters shadow outer values even when their runtime value is unknown.
      for(const param of node.params ?? []) if(param.type==='Identifier') scope.bindings.set(param.name,[]);
    }
    scopes.set(node,scope);
    for (const child of Object.values(node)) scan(child,scope);
  };
  scan(ast,{parent:null,bindings:new Map()});
  const resolved=(node,seen=new Set())=>{
    node=bare(node);
    if(!node || seen.has(node)) return [];
    seen.add(node);
    if(node.type==='Identifier') return lookup(scopes.get(node),node.name);
    if(node.type==='MemberExpression') {
      const name=node.computed ? key(node.property) : node.property.name;
      const objects=object=>{
        object=bare(object);
        if(object?.type==='ObjectExpression') return object.properties.filter(p=>p.type==='Property' && key(p.key)===name).map(p=>p.value);
        return resolved(object,seen).flatMap(objects);
      };
      return objects(node.object);
    }
    return [];
  };
  const seenValues=new Set();
  const falseValue=node=>{
    node=bare(node);
    if(!node || seenValues.has(node)) return false;
    seenValues.add(node);
    if(node.type==='Literal') return node.value===false;
    return resolved(node).some(falseValue);
  };
  const seenReturned=new Set();
  const returned=node=>{
    node=bare(node);
    if(!node || seenReturned.has(node)) return;
    seenReturned.add(node);
    if(['Identifier','MemberExpression'].includes(node.type)) resolved(node).forEach(returned);
    if(node.type==='ObjectExpression') {
      const property=node.properties.findLast(p=>p.type==='Property' && key(p.key)==='ok');
      seenValues.clear();
      if(property && falseValue(property.value)) failed.add(node);
      if(!property) node.properties.filter(p=>p.type==='SpreadElement').forEach(p=>returned(p.argument));
    }
    if(node.type==='ConditionalExpression') {returned(node.consequent);returned(node.alternate);}
    if(node.type==='LogicalExpression') {returned(node.left);returned(node.right);}
    if(node.type==='AwaitExpression') returned(node.argument);
    if(node.type==='CallExpression' && propertyName(node.callee)==='resolve' && bare(node.callee.object)?.type==='Identifier' && bare(node.callee.object).name==='Promise') returned(node.arguments[0]);
  };
  const seen=new Set(), seenCallees=new Set();
  const callable=node=>{
    node=bare(node);
    if(!node || seenCallees.has(node)) return;
    seenCallees.add(node);
    if(['Identifier','MemberExpression'].includes(node.type)) resolved(node).forEach(callable); else visit(node);
  };
  const visit=node=>{
    node=bare(node);
    if (!node || typeof node!=='object' || seen.has(node)) return;
    seen.add(node);
    if (Array.isArray(node)) {node.forEach(visit);return;}
    if(node.type==='ReturnStatement') returned(node.argument);
    if(node.type==='ArrowFunctionExpression' && node.body.type!=='BlockStatement') returned(node.body);
    if(node.type==='CallExpression') callable(node.callee);
    for(const child of Object.values(node)) visit(child);
  };
  roots.forEach(callable);
  return [...failed];
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
  for(const node of snapbackMutationRefusals(stage,file,ast)) found.push(label+':'+at(node.start)+': a Snapback mutation returning {ok:false} leaves failure(slot) empty; throw refused({code: \'YOUR_CODE\', message: \'Explain the refusal\'}) instead');
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
  // Installed declaration internals belong to their package; app sources stay strict.
  skipLibCheck: true,
  target: 'ES2023',
  module: 'ESNext',
  moduleResolution: 'bundler',
  allowImportingTsExtensions: true,
  lib: ['ES2023', 'ES2024.Object', 'ES2024.Collection', 'ES2024.Promise', 'ES2024.String', 'WebWorker'],
  // No package's types are global unless a file names it: an installed
  // `@types/node` or `@types/bun` would declare timers and `process`, which
  // no executor has.
  types: [],
};
// The library files that configuration loads (ES2023 is ES5 through ES2023,
// with their parts and the decorators). A package's `/// <reference lib>`
// would add others, such as the DOM's, which native hosts cannot support.
const year=Math.max(...compilerOptions.lib.map(l=>/^ES(\d{4})$/i.exec(l)?.[1] ?? 0));
const configuredLibrary=(file)=>{
  const name=/^lib\.(.+)\.d\.ts$/.exec(file)?.[1]?.toLowerCase();
  if (!name) return false;
  const es=/^es(\d{4})(?:\.|$)/.exec(name);
  return ['es5','decorators','decorators.legacy','webworker'].includes(name)
    || compilerOptions.lib.some(l=>l.toLowerCase()===name)
    || Boolean(es && Number(es[1])<=year);
};

export function configure(stage, incremental=false, parse) {
  const app=capturedOrigins(stage).at(-1)[1], modulePath=resolve(stage,'app.ts');
  if (existsSync(modulePath)) {
    const before=readFileSync(modulePath,'utf8'), original=resolve(stage,'__exact_backend_source.json');
    const authored=existsSync(original) ? JSON.parse(readFileSync(original,'utf8')) : before;
    const after=injectBackendOrigin(authored,backendOrigin(app),parse);
    if (before!==after) writeFileSync(modulePath,after);
  }
  const inside = (path) => {
    if (path !== stage && !path.startsWith(stage+sep)) throw new Error('tsconfig: path outside captured app: '+path);
    return path;
  };
  const origins=capturedOrigins(stage);
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
  stagePackages(stage);
}

/** A name that is not a source (`Result<'posted'>`, a mutation's slot) fails
 * as "does not satisfy the constraint 'keyof SourceMap'", which lists none:
 * say which there are (sb4 farm wave 7, E3: 061 and 066, 15 minutes). */
function sourceNames(stage,diagnostics) {
  if(!/constraint 'keyof SourceMap'/.test(diagnostics))return '';
  let text='';
  try { text=readFileSync(resolve(stage,'app.contract.d.ts'),'utf8'); } catch { return ''; }
  const map=text.match(/export interface SourceMap \{\n([\s\S]*?)\n\}/)?.[1] ?? '';
  const names=[...map.matchAll(/^\s+("[^"]+"|[A-Za-z_$][\w$]*):/gm)].map(m=>m[1].replace(/^"|"$/g,''));
  return names.length ? `\nnote: Result<…> and Args<…> take a source's name, what the contract calls (\`resource x = name(…)\`, \`send slot = name(…)\`), never a resource's or mutation's own: ${names.map(n=>`'${n}'`).join(', ')}\n` : '';
}

// Short-lived native checks trade fewer GC cycles for a soft Go memory limit
// (not an RSS cap); explicit user settings always win.
const checkEnv={GOGC:'300',GOMEMLIMIT:'128MiB',GOMAXPROCS:'4',...process.env};

/** Type-check a configured stage with the native compiler `tsc` (whose
 * standard libraries are in `libraries`): its diagnostics, or the compiler's
 * actual resolved graph, type-only imports included, must stay inside the
 * stage (a package's declarations are in it: `stagePackages`) and the
 * configured libraries. An external declaration must not influence an
 * accepted app artifact. */
export function check(stage, tsc, libraries) {
  const allowed = path => { path=resolve(path);return path === stage || path.startsWith(stage+sep) || (within(path,libraries) && configuredLibrary(relative(libraries,path))); };
  // A package's JavaScript entry (`typescript/bin/tsc`, an `EXACT_TSC`
  // override) keeps its Node shebang: run it through Bun, as the one-shot
  // producer runs its tools (js/bake/src/lib.rs `run`).
  let head='';
  try { const fd=openSync(tsc,'r'), bytes=Buffer.alloc(128); head=bytes.toString('utf8',0,readSync(fd,bytes,0,128,0)).split('\n')[0]; closeSync(fd); } catch { /* not readable: execFile says so */ }
  const script=head.startsWith('#!') && head.split(/\s+/).some(word=>['node','bun'].includes(word.split('/').at(-1)));
  const args=['--project',resolve(stage,'__exact_tsconfig.json'),'--pretty','false','--listFiles'];
  return new Promise((done, fail) => execFile(script ? process.execPath : tsc,script ? [tsc,...args] : args,
    {cwd:stage,env:checkEnv,encoding:'utf8',maxBuffer:4*1024*1024}, (error, stdout, stderr) => {
      if (error) {
        const diagnostics=String(stdout??'').split(/\r?\n/).filter(line=>! /^(?:\/|[A-Za-z]:[\\/]).*\.[cm]?[jt]sx?$/.test(line)).join('\n');
        return fail(new Error(diagnostics+sourceNames(stage,diagnostics)+String(stderr??'') || String(error.message)));
      }
      for(const path of stdout.trim().split(/\r?\n/)) {
        if(path && !allowed(realpathSync(path)))return fail(new Error('module outside captured app: '+path));
      }
      done();
    }));
}

/** Contract build's early ambient check: walk reachable modules without executing application code. */
export async function ambientDiagnostics(app) {
  const {rolldown}=await import('rolldown');
  const found=[];
  let mounts={};
  try { mounts=JSON.parse(readFileSync(resolve(app,'app.json'),'utf8')).typescript?.sources ?? {}; } catch { /* plain Contract directory */ }
  const bundle=await rolldown({input:resolve(app,'app.ts'), logLevel:'silent', external:source=>!!packageName(source),
    plugins:[{name:'contract-ambient', async resolveId(source,importer) {
      if (!importer || !source.startsWith('.')) return null;
      const path=resolve(dirname(importer),source), child=relative(app,path).split(sep);
      return mounts[child[0]] ? this.resolve(resolve(app,mounts[child[0]],...child.slice(1)), importer, {skipSelf:true}) : null;
    }, transform(code,id) { found.push(...ambientRefusals(app,id,code,(c,o)=>this.parse(c,o))); return null; }}]});
  try { await bundle.generate({format:'esm'}); } finally { await bundle.close(); }
  return found.map(line=>{
    const [,file,row,col,message]=/^(.*?):(\d+):(\d+): ([\s\S]*)$/.exec(line);
    return {file:resolve(app,file),line:Number(row),col:Number(col),message};
  });
}
if (process.argv[2]==='--ambient' && process.argv[1]?.endsWith('/typescript.mjs')) {
  try { console.log(JSON.stringify(await ambientDiagnostics(resolve(process.argv[3])))); }
  catch(error) { console.error(error.message); process.exitCode=1; }
}
