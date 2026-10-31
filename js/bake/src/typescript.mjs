// The one TypeScript configuration for an app's data module (`app.ts`), and
// the check every producer runs with it: the native bake (one-shot and
// resident, js/bake/src), which copies this file into its stage as
// `__exact_config.mjs`, and the web build (host/web-js/build.mjs), which
// imports it from here. One app.ts builds on every host or on none, refused
// with the same diagnostics at the same point (calc F2, calendar F9/F11).
// It reads only captured configuration; the producer owns the stage.
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, realpathSync, rmdirSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { builtinModules, createRequire } from 'node:module';
import { resolve, relative, dirname, basename, join, sep, isAbsolute, toNamespacedPath } from 'node:path';
// The bake writes the binding's generator beside this file in its stage (js/bake/src/resident.rs `BINDING`).
import { moduleBinding } from './__exact_binding.mjs';
// And the settings' reader beside it (resident.rs `ENV`).
import { inlineSettings, strayReads } from './__exact_env.mjs';

export { moduleBinding };

function httpOrigin(value, label) {
  let url; try { url=new URL(value); } catch { throw new Error(`${label}: expected an http(s) origin`); }
  if (!['http:','https:'].includes(url.protocol) || url.username || url.password || url.pathname!=='/' || url.search || url.hash)
    throw new Error(`${label}: expected an http(s) origin without credentials, path, query or fragment`);
  return url.origin;
}

/** Explicit backend, saved development selection, then the checkout's declared fixture default. */
export function backendOriginSelection(app, env=process.env) {
  const file=resolve(app,'.exact/backend-origin');
  const value=env.EXACT_BACKEND_ORIGIN ?? (existsSync(file) ? readFileSync(file,'utf8').trim() : null);
  if (value===null) return fixtureBackendSelection(app,env);
  return { origin: httpOrigin(value,'EXACT_BACKEND_ORIGIN'), source: env.EXACT_BACKEND_ORIGIN != null ? 'EXACT_BACKEND_ORIGIN' : '.exact/backend-origin' };
}
export const backendOrigin = (app, env=process.env) => backendOriginSelection(app, env)?.origin ?? null;

/** A declared fixture gets a stable development port from its app directory, shared by builds and drives. */
export function fixtureBackendSelection(app, env=process.env) {
  if (!developmentTrust(env)) return null;
  let manifest, code;
  try { manifest=JSON.parse(readFileSync(resolve(app,'app.json'),'utf8')); code=readFileSync(resolve(app,'app.ts'),'utf8'); } catch { return null; }
  const argv=manifest.commands?.server;
  if (!Array.isArray(argv) || !/(^|\/)bun$/.test(argv[0])) return null;
  const entry=argv.findIndex((a,i)=>i>0 && !a.startsWith('-'));
  if (entry<0 || !/(^|[/\\])fixtures?[/\\]/.test(argv[entry])) return null;
  const literal=/\bconst\s+origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(code)?.[1];
  if (!literal || !loopback(literal)) return null;
  const args=argv.slice(entry+1), at=args.indexOf('--port'), given=at<0 ? env.PORT : args[at+1];
  // Rust canonicalizes Windows paths with a namespace prefix; server drives must hash the same spelling.
  const port=given===undefined ? 4790+parseInt(createHash('sha256').update(toNamespacedPath(realpathSync(app))).digest('hex').slice(0,8),16)%1000 : Number(given);
  if (!Number.isInteger(port) || port<1 || port>65535) throw new Error(`${at<0 ? 'PORT' : 'commands.server --port'}: a fixture port must be an integer from 1 to 65535`);
  const origin=new URL(literal); origin.port=String(port);
  return { origin:httpOrigin(origin.href,'fixture origin'), source:given===undefined ? 'checkout fixture port' : at<0 ? 'PORT' : 'commands.server --port' };
}

// The bound on a type check that never ends (the design tool's `build.mjs --check` spun 4.7 days, found 2026-10-26):
// 10 minutes, a bound for "never", not a target (the clone's check takes seconds). A test may shorten it
// (EXACT_CHECK_BOUND_MS), read only under EXACT_TEST_TIMING=1, as the design server's clocks are
// (scripts/tamber-timing.mjs); a value not shorter, not a whole number of ms from 50, or set without the switch is
// ignored. Here, not in a module of its own: the native bake copies this file alone into its stage.
export const CHECK_BOUND_MS = 10 * 60_000;

export function checkBound(env = process.env) {
  if (env.EXACT_TEST_TIMING !== '1') return CHECK_BOUND_MS;
  const said = env.EXACT_CHECK_BOUND_MS, ms = /^\d+$/.test(said ?? '') ? Number(said) : NaN;
  return ms >= 50 && ms < CHECK_BOUND_MS ? ms : CHECK_BOUND_MS;
}

/** The bound in words: `10 min`, `1.5 s`. */
export const boundWords = ms => (ms >= 60_000 ? `${ms / 60_000} min` : `${ms / 1000} s`);

/** Explain when an app-local selection takes precedence over the authored origin. */
export function backendOriginWarning(code, selection, parse) {
  if (selection?.source !== '.exact/backend-origin') return null;
  const parsed=parse('app.ts',code);
  if (parsed.errors.length) return null;
  const declarations=parsed.program.body.flatMap(s=>(s.type==='ExportNamedDeclaration' ? s.declaration : s)?.declarations ?? []);
  const init=declarations.find(d=>d.id.type==='Identifier' && d.id.name==='origin')?.init;
  const literal=init?.type==='Literal' && typeof init.value==='string' ? init.value
    : init?.type==='TemplateLiteral' && !init.expressions.length ? init.quasis[0].value.cooked : null;
  if (!literal || literal.replace(/\/+$/,'')===selection.origin) return null;
  return `.exact/backend-origin selects ${selection.origin}, overriding the app.ts origin literal ${JSON.stringify(literal)}. Edit or remove .exact/backend-origin to use the changed literal; EXACT_BACKEND_ORIGIN overrides both.`;
}
/** The socket origin at an http(s) origin's host: ws for http, wss for https. */
const socketOrigin = (origin) => origin.replace(/^http/, 'ws');
// Validate every entry, including those this bake does not select. No value
// in this form goes through the constant evaluator or the plain form's grant
// URL rewrite. The entry is the selected origin's own; in a development build
// only, an origin at a private-network address with no entry takes the entry
// of the server at loopback, its fetch and socket
// lines moved to that address: a phone reaching the fixture on this Mac's LAN.
// A production build refuses it as any missing entry.
function keyedGrants(node, origin, at, development) {
  const entries=new Map();
  const refuse=(node, why)=>{ throw new Error(`app.ts:${at(node.start)}: grants: ${why}`); };
  const line=(node, place)=>{
    if (node?.type!=='Literal' || typeof node.value!=='string')
      refuse(node ?? place, `expected a string literal grant line, got ${node?.type ?? 'array hole'}`);
    if (/[\r\n\u2028\u2029]/.test(node.value)) refuse(node,'each string literal must contain one grant line');
    return node.value;
  };
  for (const property of node.properties) {
    if (property.type==='SpreadElement') refuse(property,'spread entries are not allowed');
    if (property.computed) refuse(property.key,'computed keys are not allowed');
    if (property.method || property.kind!=='init') refuse(property,'methods and accessors are not allowed');
    const key=property.key;
    if (key.type!=='Literal' || typeof key.value!=='string') refuse(key,`expected a string literal origin key, got ${key.type}`);
    // A key is written as the origin it means, so two spellings of one origin cannot both appear.
    const canonical=httpOrigin(key.value,`app.ts:${at(key.start)}: grants key ${JSON.stringify(key.value)}`);
    if (canonical!==key.value) refuse(key,`write the origin key ${JSON.stringify(key.value)} as ${JSON.stringify(canonical)}`);
    if (entries.has(key.value)) refuse(key,`duplicate key ${JSON.stringify(key.value)}`);
    const value=property.value;
    entries.set(key.value,value.type==='ArrayExpression' ? value.elements.map(n=>line(n,value)).join('\n') : line(value,value));
  }
  const wanted=new URL(origin).origin;
  if (entries.has(wanted)) return entries.get(wanted);
  const candidates=development && (privateNetwork(wanted) || loopback(wanted)) ? [...entries.keys()].filter(key=>sameServer(key,wanted)) : [];
  const local=candidates.filter(key=>new URL(key).port===new URL(wanted).port);
  if (!local.length) local.push(...candidates);
  if (local.length>1) refuse(node,`origin ${JSON.stringify(wanted)} is the server of more than one loopback key: ${local.map(k=>JSON.stringify(k)).join(', ')}`);
  if (!local.length) refuse(node,`no entry for origin ${JSON.stringify(wanted)}; keys: ${[...entries.keys()].map(k=>JSON.stringify(k)).join(', ') || '(none)'}`);
  return movedLines(entries.get(local[0]),local[0],wanted);
}

/** A build that is not production (`EXACT_UPDATE_TRUST` unset or `development`, as `bake/src/compat.rs` reads it). */
const developmentTrust = (env) => env.EXACT_UPDATE_TRUST === undefined || env.EXACT_UPDATE_TRUST === 'development';

/** A `net.fetch local-network` or `net.websocket local-network` line, as the native bake's entry matches it too. */
export const LOCAL_NETWORK_LINE = /^([ \t]*net\.(?:fetch|websocket)[ \t]+local-network)[ \t]*$/gm;
/** In a development build each such line gains the word that admits loopback as well (LLP 1133.036 D5), so a
 * drive's fixture on this machine stands in for a device on the LAN. A production build's lines stay as written,
 * and its bake refuses the word. */
export const developmentLocalNetwork = (spec, env=process.env) => developmentTrust(env) ? spec.replace(LOCAL_NETWORK_LINE, '$1 development') : spec;

/** Whether an origin's host is a private-network address: RFC 1918 IPv4, link-local IPv4 (RFC 3927),
 * unique-local IPv6 (fc00::/7, RFC 4193) or link-local IPv6 (fe80::/10, RFC 4291). */
export function privateNetwork(origin) {
  const host=new URL(origin).hostname;
  const v4=host.match(/^(\d+)\.(\d+)\.\d+\.\d+$/);
  if (v4) {
    const [a,b]=[Number(v4[1]),Number(v4[2])];
    return a===10 || (a===172 && b>=16 && b<=31) || (a===192 && b===168) || (a===169 && b===254);
  }
  if (!host.startsWith('[')) return false;
  const first=host.slice(1).split(':')[0];
  if (!first) return false;
  const bits=parseInt(first,16);
  return (bits & 0xfe00)===0xfc00 || (bits & 0xffc0)===0xfe80;
}

const loopback = origin => ['127.0.0.1','localhost','[::1]'].includes(new URL(origin).hostname);
/** A loopback grant key with the selected origin's scheme; a development fixture may move ports. */
function sameServer(key, origin) {
  const [k,o]=[new URL(key),new URL(origin)];
  return loopback(key) && k.protocol===o.protocol;
}

/** A loopback entry's lines for the same server at `to`: each `net.fetch` and `net.websocket` line at the
 * loopback origin moves to `to`, keeping its path; every other line stays as written. */
function movedLines(lines, from, to) {
  const move=(target, a, b)=>target===a || target.startsWith(a+'/') ? b+target.slice(a.length) : target;
  return lines.split('\n').map(line=>{
    // A development build's only line (`developmentGrants`) moves too.
    const m=line.match(/^((?:[ \t]*development[ \t]+)?)(net\.fetch|net\.websocket) (\S+)(.*)$/);
    if (!m) return line;
    const [a,b]=m[2]==='net.fetch' ? [from,to] : [socketOrigin(from),socketOrigin(to)];
    return `${m[1]}${m[2]} ${move(m[3],a,b)}${m[4]}`;
  }).join('\n');
}

/** Select literal keyed grants, or move the plain form's matching fetch and
 * socket grant lines with the origin. Only the captured copy is rewritten. */
export function injectBackendOrigin(code, origin, parse, env=process.env) {
  const parsed=parse('app.ts',code);
  if (parsed.errors.length) {
    if (origin===null) return code;
    throw new Error('EXACT_BACKEND_ORIGIN: app.ts must parse before selecting a backend');
  }
  const statements=parsed.program.body.map(s=>s.type==='ExportNamedDeclaration' ? s.declaration : s);
  const declarations=statements.flatMap(s=>s?.declarations ?? []);
  const grants=declarations.filter(d=>d.id.type==='Identifier' && d.id.name==='grants');
  const keyed=grants.filter(d=>bare(d.init)?.type==='ObjectExpression');
  if (origin===null && !keyed.length) return code;
  const targets=declarations.filter(d=>d.id.type==='Identifier' && d.id.name==='origin');
  const init=targets[0]?.init;
  const literal=init?.type==='Literal' && typeof init.value==='string' ? init.value
    : init?.type==='TemplateLiteral' && !init.expressions.length ? init.quasis[0].value.cooked : null;
  if (targets.length!==1 || !literal || !/^https?:\/\//.test(literal))
    throw new Error(keyed.length ? `app.ts:${lineColumn(code)(init?.start ?? keyed[0].start)}: grants keyed by origin need one top-level literal const origin`
      : 'EXACT_BACKEND_ORIGIN: app.ts needs one top-level literal const origin; use it in net.fetch ${origin}');
  const edits=origin===null ? [] : [[init.start,init.end,JSON.stringify(origin)]];
  const at=lineColumn(code);
  if (keyed.length) {
    for (const d of [...targets,...keyed]) if (!statements.some(s=>s?.kind==='const' && s.declarations.includes(d)))
      throw new Error(`app.ts:${at(d.start)}: grants keyed by origin require const ${d.id.name}`);
    httpOrigin(origin ?? literal,`app.ts:${at(init.start)}: grants origin`);
    for (const d of keyed) edits.push([d.init.start,d.init.end,JSON.stringify(keyedGrants(bare(d.init),origin ?? literal,at,developmentTrust(env)))]);
  }
  const previous=literal.replace(/\/+$/,'');
  // A development build's only line (`developmentGrants`) keeps its marker and moves too.
  const moved=line=>line===`net.fetch ${previous}` || line===`net.fetch ${previous}/` ? `net.fetch ${origin}`
    : line===`net.websocket ${socketOrigin(previous)}` || line===`net.websocket ${socketOrigin(previous)}/` ? `net.websocket ${socketOrigin(origin)}` : line;
  const replace=value=>value.split('\n').map(line=>{ const [,mark,rest]=line.match(/^([ \t]*development[ \t]+|)(.*)$/s); return mark+moved(rest); }).join('\n');
  const walk=node=>{
    if (!node || typeof node!=='object') return;
    if (node.type==='Literal' && typeof node.value==='string') { const next=replace(node.value); if(next!==node.value) edits.push([node.start,node.end,JSON.stringify(next)]); return; }
    if (node.type==='TemplateLiteral' && !node.expressions.length) { const next=replace(node.quasis[0].value.cooked); if(next!==node.quasis[0].value.cooked) edits.push([node.start,node.end,JSON.stringify(next)]); return; }
    if (node.type==='TemplateElement') { if(replace(node.value.cooked)!==node.value.cooked) throw new Error('EXACT_BACKEND_ORIGIN: use net.fetch ${origin} in an interpolated grants template, and write a net.websocket line at its host as a string of its own'); return; }
    for(const child of Object.values(node)) if(Array.isArray(child)) child.forEach(walk); else if(child && typeof child==='object') walk(child);
  };
  if (origin!==null) grants.filter(d=>!keyed.includes(d)).forEach(d=>walk(d.init));
  for(const [start,end,value] of edits.sort((a,b)=>b[0]-a[0])) code=code.slice(0,start)+value+code.slice(end);
  return code;
}

/** A grant line that begins with the word `development` is a development
 * build's only: a production bake leaves it out, any other build keeps it
 * without the word (Harmony's test call, `development device.calls
 * purpose.calls`, LLP 1133.033 §13 question 4). No grant family is a bare
 * word, so the marker is never a grant's own; the bake's trailing
 * `local-network development` is another thing. Only the captured copy is
 * rewritten, so the app's own `grants` is the build's at run time too. Run
 * after `injectBackendOrigin`, which has moved a marked line with the
 * backend's origin and made keyed grants the selected entry's string. */
export function developmentGrants(code, parse, env=process.env) {
  const parsed=parse('app.ts',code);
  if (parsed.errors.length) return code;
  const marker=/^([ \t]*)development[ \t]+(?=\S)/;
  const grants=staticFacts(code,parse).grants;
  if (grants===undefined || !grants.split('\n').some(line=>marker.test(line))) return code;
  // The binding exported as `grants`, as `staticFacts` reads it.
  const body=parsed.program.body;
  const local=body.flatMap(s=>s.type==='ExportNamedDeclaration' && !s.source ? (s.specifiers ?? []) : [])
    .find(x=>(x.exported.name ?? x.exported.value)==='grants')?.local;
  const name=local ? local.name ?? local.value : 'grants';
  const declaration=body.map(s=>s.type==='ExportNamedDeclaration' ? s.declaration : s)
    .flatMap(s=>s?.type==='VariableDeclaration' ? s.declarations : []).find(d=>d.id.type==='Identifier' && d.id.name===name);
  const development=developmentTrust(env);
  const lines=grants.split('\n').flatMap(line=>!marker.test(line) ? [line] : development ? [line.replace(marker,'$1')] : []);
  return code.slice(0,declaration.init.start)+JSON.stringify(lines.join('\n'))+code.slice(declaration.init.end);
}

// Rolldown IDs are file paths, not URL or slash-normalized strings. Both
// producers enforce the same captured-file boundary (LLP 1027 D5). A
// package's declarations are captured for the checker alone (`stagePackages`)
// and never load; a package's code loads only from where `capturedSources`
// captured it, which admits it before this check.
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
// graph still stays inside the stage. Its JavaScript is captured apart, file
// by file as the bundle reaches it (`capturedSources`).
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
// A path as the disk names it (a symlinked stage's real place); one that isn't there, an importer from outside the stage
// a bundler names, stays as written, which `within` then leaves alone.
const physicalId=id=>{ if (!isAbsolute(id)) return id; try { return realpathSync.native(id); } catch { return id; } };
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
      else if (entry.isFile() && /\.[cm]?[jt]sx?$/.test(entry.name) && !entry.name.startsWith('__exact_') && path!==resolve(stage,'app.js')) {
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

// Package code (LLP 1027 D5, "Package code"). A package's JavaScript is
// captured file by file as the bundle reaches it: each import is resolved
// from where its importer really is (Node's lookup, a package's `exports`,
// `main` and `browser` fields, as a browser bundle resolves them), and the
// file it leads to is copied into the stage under `node_modules/.packages/
// <name>@<version>/`, with every `package.json` between it and the package's
// root (their `type` and `sideEffects`). The bundle loads only those copies.
// `__exact_packages.json` records each file by where it was read and its
// SHA-256, each import's resolution, and each package's installed paths
// (the links an install retargets), so a bake is a function of what it
// captured and every later bundle replays it (`replayCaptured`). A Node
// built-in, a native addon, an import that is not installed, and an
// unguarded use of the DOM or of Node's globals are refused by package and
// file: no executor has them.
const RECORD='__exact_packages.json';
const BUILTIN=new Set(builtinModules);
const builtin=(spec)=>spec.startsWith('node:') || BUILTIN.has(spec) || BUILTIN.has(spec.split('/')[0]);
// npm's package names (with the old capitals) and versions: a place in the
// stage is made of them, so nothing else is taken.
const NAME=/^(?:@[a-z0-9~][\w.~-]*\/)?[a-z0-9~][\w.~-]*$/i, VERSION=/^(?:[0-9a-z][\w.+-]*)?$/i;
// The package a file belongs to: the nearest `package.json` that names one
// (a nested `{"type":"module"}` names none).
function packageOf(file) {
  for (let dir=dirname(file);;) {
    let json=null;
    try { json=JSON.parse(readFileSync(join(dir,'package.json'),'utf8')); } catch { /* none here, or not JSON */ }
    if (typeof json?.name==='string' && json.name) {
      const version=typeof json.version==='string' ? json.version : '';
      if (!NAME.test(json.name) || !VERSION.test(version)) throw new Error(`${join(dir,'package.json')}: not an npm package name and version (${JSON.stringify(json.name)}, ${JSON.stringify(version)})`);
      return {root:dir,key:json.name+'@'+version,name:json.name,json,installed:new Set()};
    }
    const up=dirname(dir);
    if (up===dir) return null;
    dir=up;
  }
}
// Where Node finds package `name` from `dir`: the nearest `node_modules/<name>`
// upward, as installed (a link stays a link).
function installedAt(name, dir) {
  for (let at=dir;;) {
    const candidate=join(at,'node_modules',name);
    if (statSync(candidate,{throwIfNoEntry:false})) return candidate;
    const up=dirname(at);
    if (up===at) return null;
    at=up;
  }
}
const slash=(path)=>path.split(sep).join('/');
const kindOf=(options)=>options?.kind==='require-call' ? 'require' : 'import';
// Every bundle of the module resolves packages as a browser bundle does (the
// `browser` condition and field first), as the page's own bundle does, so
// the page and Hermes run the same file of a package; `process.env.NODE_ENV`
// is the constant packages test.
export const bundleOptions={platform:'browser',transform:{define:{'process.env.NODE_ENV':'"production"'}}};
// An async function resumes under the answer it began in (LLP 1133.002 D14),
// so a call it makes after an `await` is that answer's, even when it awaited
// a promise another answer made. No engine carries context across a native
// `await`, so a data module's bundles lower `async` to generators
// (`lowered`) and drive each step through `asyncContextHelper`, which puts
// back the answer current when the function was called and restores the one
// before after the step. The runtime that runs the module owns
// `__exact_async_context` (js/src/prelude.js, host/web-js/ts-data.js); a
// realm without one runs the steps as the stock helper would.
export const lowered={target:'es2016'};
const ASYNC_HELPER=/@oxc-project\+runtime@[^/]*\/helpers\/esm\/asyncToGenerator\.js$/;
export const asyncContextHelper=`export default function _asyncToGenerator(fn) {
  return function () {
    var self = this, args = arguments, context = globalThis.__exact_async_context, at = context ? context.current() : undefined;
    return new Promise(function (resolve, reject) {
      var gen = fn.apply(self, args);
      function step(key, arg) {
        // A live answer's step that a module turn settled runs in that answer's own turn (LLP 1146 D2)
        if (context && context.route && context.route(at, step, key, arg)) return;
        var before = context ? context.swap(at) : undefined, info;
        try { info = gen[key](arg); } catch (error) { if (context) context.swap(before); reject(error); return; }
        if (context) context.swap(before);
        if (info.done) resolve(info.value);
        else Promise.resolve(info.value).then(function (v) { step("next", v); }, function (e) { step("throw", e); });
      }
      step("next", undefined);
    });
  };
}
`;
/** Rolldown resolves the helpers its lowering names itself, never through a
 * plugin's `resolveId`, but passes them through `transform`: this replaces
 * `asyncToGenerator` there. */
export function asyncContext() {
  return { name:'exact-async-context', transform(code,id) { return ASYNC_HELPER.test(id) ? { code:asyncContextHelper, map:null } : null; } };
}
/** The native bundle of a data module, the same for the one-shot producer
 * and the resident compiler. */
export const dataBundle=(stage)=>({...bundleOptions, transform:{...bundleOptions.transform, ...lowered},
  plugins:[asyncContext(), capturedSources(stage)]});
/** The captured graph's Rolldown plugin, the same in every producer: app
 * files load only from the stage, package code only from its capture, and
 * the clock (`ambientRefusals`) or what no executor has (`packageRefusals`)
 * is refused by file and line, into `refusals` when given, else thrown. */
export function capturedSources(stage, refusals=null) {
  stage=realpathSync.native(stage);
  const files=new Map(), roots=new Map(), edges=new Map(), area=join(stage,'node_modules','.packages');
  let total=0;
  // Staged declarations and captured package code sit under a `node_modules` in the stage.
  const staged=(id)=>within(id,stage) && relative(stage,id).split(sep).includes('node_modules');
  const label=(id,at='')=>{
    const file=files.get(id);
    if (!file) return slash(relative(stage,id))+at;
    return `${file.package.key} (${slash(relative(file.package.root,file.real))}${at})`;
  };
  const copy=(from,to,pkg)=>{
    if (files.has(to)) return;
    if (!within(to,area) || to===area) throw new Error('module outside captured app: '+from);
    const bytes=readFileSync(from);
    total+=bytes.length;
    if (total>64<<20) throw new Error('package code exceeds 64 MiB');
    if (!existsSync(to) || !readFileSync(to).equals(bytes)) { mkdirSync(dirname(to),{recursive:true}); writeFileSync(to,bytes); }
    files.set(to,{real:from,package:pkg,sha256:createHash('sha256').update(bytes).digest('hex')});
  };
  // The files a package's exact.json declares in `web.assets` (LLP 1154 D2), with that declaration: captured with its
  // code, so its digest covers their bytes and the web build serves them from the capture (host/web-js/build.mjs).
  const assets=(pkg)=>{
    let declared;
    try { declared=JSON.parse(readFileSync(join(pkg.root,'exact.json'),'utf8')).web?.assets; } catch { return; }
    if (declared===undefined) return;
    if (!Array.isArray(declared) || declared.some(p=>typeof p!=='string')) throw new Error(`${pkg.key}: exact.json web.assets is a list of paths in the package`);
    const base=join(area,...pkg.key.split('/'));
    copy(join(pkg.root,'exact.json'),join(base,'exact.json'),pkg);
    for (const path of declared) {
      const from=resolve(pkg.root,path), real=existsSync(from) ? realpathSync.native(from) : null;
      if (isAbsolute(path) || real!==from || real===pkg.root || !within(real,pkg.root) || !statSync(real).isFile()) throw new Error(`${pkg.key}: exact.json web.assets names ${JSON.stringify(path)}, which is not a file in the package (a link is not one)`);
      copy(real,join(base,relative(pkg.root,real)),pkg);
    }
  };
  const capture=(real,who,installed)=>{
    const found=packageOf(real);
    if (!found) throw new Error('module outside captured app: '+real);
    const pkg=roots.get(found.key) ?? found;
    if (pkg.root!==found.root) throw new Error(`two copies of ${found.key} (${pkg.root}, ${found.root}); install one`);
    if (!roots.has(pkg.key)) assets(pkg);
    roots.set(pkg.key,pkg);
    if (installed) pkg.installed.add(installed);
    if (/\.node$/.test(real) || pkg.json.gypfile || existsSync(join(pkg.root,'binding.gyp')))
      throw new Error(`${who} imports ${pkg.key}, a native addon: a data module runs JavaScript alone`);
    const base=join(area,...pkg.key.split('/')), place=join(base,relative(pkg.root,real));
    copy(real,place,pkg);
    for (let dir=dirname(real); within(dir,pkg.root); dir=dirname(dir)) {
      if (existsSync(join(dir,'package.json'))) copy(join(dir,'package.json'),join(base,relative(pkg.root,dir),'package.json'),pkg);
      if (dir===pkg.root) break;
    }
    return place;
  };
  const report=(why)=>{ if (!why.length) return; if (refusals) refusals.push(...why); else throw new Error(why.join('\n')); };
  return {
    name:'captured-sources',
    async resolveId(source, importer, options) {
      if (!importer || importer.startsWith('\0')) return null;
      importer=physicalId(importer);
      const from=files.get(importer), name=packageName(source), kind=kindOf(options);
      const record=(id)=>edges.set([slash(relative(stage,importer)),source,kind].join('\0'),[slash(relative(stage,importer)),source,kind,id ? slash(relative(stage,id)) : '']);
      // The app's own imports (a relative or absolute path) resolve as they
      // would, and are recorded, so a later bundle replays every one.
      if (!from && !name && !builtin(source)) {
        if (!within(importer,stage)) return null;
        const result=await this.resolve(source, importer, { ...options, skipSelf: true });
        const resolved=result && !result.external ? {...result,id:physicalId(result.id)} : result;
        if (resolved && !resolved.external && !resolved.id.startsWith('\0')) record(resolved.id);
        return resolved;
      }
      if (!from && name) {
        // `rows/../../helper.ts` names no file of a package: it leaves it, to a
        // place that differs between the stage and the app's own directory.
        if (source.split(/[\\/]/).some(part => part === '.' || part === '..')) throw new Error('module outside captured app: '+source);
        // A path mapping into the capture.
        const result=await this.resolve(source, importer, { ...options, skipSelf: true });
        const resolved=result && !result.external ? {...result,id:physicalId(result.id)} : result;
        if (resolved && !resolved.external && within(resolved.id,stage) && !staged(resolved.id)) { record(resolved.id); return resolved; }
      }
      // Where the import leads from the importer's own place.
      const [at,origin]=capturedOrigins(stage).find(([at])=>within(importer,at)) ?? [stage,stage];
      const place=from ? from.real : resolve(origin,relative(at,importer));
      const real=await this.resolve(source, place, { ...options, skipSelf: true }).catch(() => null);
      const who=label(importer);
      if (!real || real.external) {
        if (builtin(source)) throw new Error(`${who} imports Node's built-in '${source}': a data module runs on Hermes and in the browser, which have no Node modules; import a part of the package that needs none, or another package`);
        throw new Error(`${who} imports '${source}', which is not installed`);
      }
      // A `browser` field's `false`: Rolldown's empty module.
      if (real.id.startsWith('\0')) { record(null); return real; }
      const id=capture(physicalId(real.id), who, name && installedAt(name, dirname(place)));
      record(id);
      return { ...real, id };
    },
    load(id) {
      id=physicalId(id);
      if (!files.has(id) && !id.startsWith('\0rolldown/empty.js')) assertCapturedModule(stage, id);
      return null;
    },
    transform(code, id) {
      id=physicalId(id);
      const parse=(c,o)=>this.parse(c,o), js=/\.[cm]?js$/.test(id) && within(id,stage);
      // The app's own JavaScript is checked as package code is, and for the clock as the app's TypeScript is.
      report([...(files.has(id) ? [] : ambientRefusals(stage,id,code,parse)), ...strayReads(id,code,parse,(at)=>label(id,at),files.has(id)), ...(js ? packageRefusals(code,(at)=>label(id,at),parse) : [])]);
      return null;
    },
    buildEnd() {
      const listed=[...files.values()].map(({real,package:{key,root},sha256})=>[key,slash(relative(root,real)),real,sha256])
        .sort((a,b)=>a[0]+'\0'+a[1]<b[0]+'\0'+b[1] ? -1 : 1);
      const resolved=[...edges.values()].sort((a,b)=>a.join('\0')<b.join('\0') ? -1 : 1);
      // A package's digest: its files' paths and bytes, and every import that reached it.
      const digests={};
      for (const key of [...roots.keys()].sort()) {
        const prefix='node_modules/.packages/'+key+'/';
        const lines=[...listed.filter(f=>f[0]===key).map(f=>f[1]+'\0'+f[3]), ...resolved.filter(e=>e[3].startsWith(prefix)).map(e=>e.join('\0'))];
        digests[key]=createHash('sha256').update(lines.join('\n')).digest('hex');
      }
      const record=JSON.stringify({
        packages:[...roots].map(([key,{root,installed}])=>[key,root,[...installed].sort()]).sort((a,b)=>a[0]<b[0] ? -1 : 1),
        files:listed, edges:resolved, digests,
      });
      const path=resolve(stage,RECORD);
      if (!existsSync(path) || readFileSync(path,'utf8')!==record) writeFileSync(path,record);
    },
  };
}
/** The web assets a capture holds (`capturedSources`): each package's name, the asset's path in it, and the file. */
export function capturedAssets(stage) {
  const out=[];
  for (const [key] of JSON.parse(readFileSync(resolve(stage,RECORD),'utf8')).packages) {
    const base=join(stage,'node_modules','.packages',...key.split('/'));
    let declared;
    try { declared=JSON.parse(readFileSync(join(base,'exact.json'),'utf8')).web?.assets; } catch { continue; }
    for (const path of declared ?? []) out.push({ name:key.slice(0,key.lastIndexOf('@')), path:slash(relative(base,resolve(base,path))), file:resolve(base,path) });
  }
  return out;
}
/** A later bundle of the checked capture (the web page's and its server
 * render's): every import from a captured file resolves to what
 * `capturedSources` resolved it to, and any other is refused, so nothing is
 * read again from the app's directory or where a package is installed. The
 * one exception is `allowed`: the host runtime's own module the web build
 * injects into the app's code. `rolldown` is a Rolldown plugin; `bun` a
 * Bun.build plugin's setup. Imports from outside the stage are left alone. */
export function replayCaptured(stage, allowed=[]) {
  stage=realpathSync.native(stage);
  const record=JSON.parse(readFileSync(resolve(stage,RECORD),'utf8'));
  const edges=new Map(record.edges.map(([importer,source,kind,target])=>[[resolve(stage,importer),source,kind].join('\0'),target && resolve(stage,target)]));
  const host=new Set(allowed);
  const lookup=(source,importer,kind)=>{
    if (importer) importer=physicalId(importer);
    if (!importer || !within(importer,stage)) return undefined;
    const key=[importer,source,kind].join('\0');
    if (edges.has(key)) return edges.get(key);
    if (host.has(source)) return undefined;
    throw new Error(`${slash(relative(stage,importer))} imports '${source}', which the checked capture did not resolve; build again`);
  };
  const EMPTY='module.exports = {};';
  return {
    rolldown: { name:'captured-replay',
      resolveId(source,importer,options) { const id=lookup(source,importer,kindOf(options)); return id===undefined ? null : id || '\0exact-empty'; },
      load(id) { return id==='\0exact-empty' ? EMPTY : null; } },
    bun(build) {
      build.onResolve({ filter: /.*/ }, (args) => {
        const id=lookup(args.path,args.importer,args.kind==='require-call' ? 'require' : 'import');
        return id===undefined ? undefined : id ? { path: id } : { path: args.path, namespace: 'exact-empty' };
      });
      build.onLoad({ filter: /.*/, namespace: 'exact-empty' }, () => ({ contents: EMPTY, loader: 'js' }));
    },
  };
}
// What package code, and the app's own JavaScript, may not use: the DOM,
// which no data module has, Node's globals, and code made from a string.
// Both executors lack the first two alike (js/src/prelude.js defines none;
// host/web-js/ts-fetch.js hides the page's), so a test of one (`typeof
// window`) answers the same everywhere; a use that test does not guard is
// refused here, by name. This keeps a module portable; it is no security
// boundary (LLP 1027 D5): on the web the module runs in the page's realm.
const DOM=['window','frames','parent','top','opener','document','navigator','location','history','localStorage','sessionStorage','indexedDB','HTMLElement','Element','getComputedStyle','customElements','alert'];
// The global object's names for itself, through which a member reaches the page.
const HOSTS=['globalThis','self','window','frames','parent','top','opener'];
// Code from a string: the bundle cannot capture it, and on the page it runs outside the module's bindings.
const CODE=['Function','eval'];
const NODE=['process','Buffer','global','__dirname','__filename','require'];
const SCOPES=['Program','BlockStatement','StaticBlock','SwitchStatement','ForStatement','ForInStatement','ForOfStatement','CatchClause','ClassExpression'];
const NOT_REFERENCES={MemberExpression:'property',Property:'key',MethodDefinition:'key',PropertyDefinition:'key',LabeledStatement:'label',BreakStatement:'label',ContinueStatement:'label',ImportSpecifier:'imported',ExportSpecifier:'exported',MetaProperty:'property'};
const FUNCTIONS=['FunctionDeclaration','FunctionExpression','ArrowFunctionExpression'];
// The names a scope node declares itself: a function's parameters and its
// hoisted `var`s and functions, a block's lexical declarations, a catch
// clause's parameter, a named function or class expression's own name.
function declares(node) {
  const names=new Set();
  const pattern=(p)=>{
    if (!p) return;
    if (p.type==='Identifier') names.add(p.name);
    else if (p.type==='ObjectPattern') p.properties.forEach(q=>pattern(q.type==='RestElement' ? q.argument : q.value));
    else if (p.type==='ArrayPattern') p.elements.forEach(pattern);
    else if (p.type==='AssignmentPattern') pattern(p.left);
    else if (p.type==='RestElement') pattern(p.argument);
  };
  const lexical=(statements)=>{
    for (let s of statements ?? []) {
      if (['ExportNamedDeclaration','ExportDefaultDeclaration'].includes(s.type)) s=s.declaration ?? s;
      if (s.type==='VariableDeclaration' && s.kind!=='var') s.declarations.forEach(d=>pattern(d.id));
      if (['FunctionDeclaration','ClassDeclaration'].includes(s.type) && s.id) names.add(s.id.name);
      if (s.type==='ImportDeclaration') s.specifiers.forEach(x=>names.add(x.local.name));
    }
  };
  const vars=(n)=>{
    if (!n || typeof n!=='object') return;
    if (Array.isArray(n)) { n.forEach(vars); return; }
    if (FUNCTIONS.includes(n.type) || n.type==='ClassExpression' || n.type==='ClassDeclaration') return;
    if (n.type==='VariableDeclaration' && n.kind==='var') n.declarations.forEach(d=>pattern(d.id));
    for (const key in n) if (key!=='parent' && n[key] && typeof n[key]==='object') vars(n[key]);
  };
  if (node.type==='Program') { lexical(node.body); vars(node.body); }
  else if (FUNCTIONS.includes(node.type)) {
    node.params.forEach(pattern);
    if (node.type==='FunctionExpression' && node.id) names.add(node.id.name);
    if (node.body?.type==='BlockStatement') { lexical(node.body.body); vars(node.body.body); }
  }
  else if (node.type==='ClassExpression' && node.id) names.add(node.id.name);
  else if (['BlockStatement','StaticBlock'].includes(node.type)) lexical(node.body);
  else if (node.type==='SwitchStatement') node.cases.forEach(c=>lexical(c.consequent));
  else if (['ForStatement','ForInStatement','ForOfStatement'].includes(node.type)) { const d=node.init ?? node.left; if (d?.type==='VariableDeclaration' && d.kind!=='var') d.declarations.forEach(x=>pattern(x.id)); }
  else if (node.type==='CatchClause') pattern(node.param);
  return names;
}
/** The DOM and Node globals a JavaScript file uses, as `label(at)`'s
 * `…: why`: each use of a name no enclosing scope binds, not guarded by a
 * `typeof` test of it that dominates the use (`typeof x !== 'undefined' &&
 * x.y`, its `if` or `?:`, or an earlier `if (typeof x …) return`), and each
 * such member of `globalThis`, `self`, `window` or `global`. Static: a use
 * behind an alias still reaches an executor without the name. */
export function packageRefusals(code, label, parse) {
  const ast=parse(code,{lang:'js'}), found=[], at=lineColumn(code);
  // Whether `node` tests `typeof name` (or `typeof globalThis.name`).
  const tests=(node,name)=>{
    if (!node || typeof node!=='object') return false;
    if (Array.isArray(node)) return node.some(n=>tests(n,name));
    if (node.type==='UnaryExpression' && node.operator==='typeof') {
      const a=bare(node.argument);
      return a?.type==='Identifier' ? a.name===name : propertyName(a)===name;
    }
    if (FUNCTIONS.includes(node.type)) return false;
    for (const key in node) if (key!=='parent' && tests(node[key],name)) return true;
    return false;
  };
  const exits=(s)=>s && (['ReturnStatement','ThrowStatement'].includes(s.type) || (s.type==='BlockStatement' && exits(s.body.at(-1))));
  // Whether a use of `name`, under the ancestors `path` (outermost first), is guarded.
  const guarded=(name,path)=>path.some(([node,key,index],i)=>{
    if (['IfStatement','ConditionalExpression'].includes(node.type) && key!=='test' && tests(node.test,name)) return true;
    if (node.type==='LogicalExpression' && key==='right' && tests(node.left,name)) return true;
    // An earlier statement of an enclosing block leaves when the name is absent.
    const list=key==='body' || key==='consequent' ? node[key] : null;
    return Array.isArray(list) && list.slice(0,index).some(s=>s.type==='IfStatement' && tests(s.test,name) && exits(s.consequent));
  });
  const refuse=(name,node)=>{
    const where=label(':'+at(node.start));
    if (CODE.includes(name) || name==='import()') found.push(where+` loads code from a string ('${name}'), which a bundle cannot capture`);
    else if (HOSTS.includes(name) && !DOM.includes(name)) found.push(where+` uses the page's '${name}' through the global object: a data module has no page`);
    else if (DOM.includes(name)) found.push(where+` uses the DOM's '${name}': a data module has no DOM`);
    else if (name==='require') found.push(where+' loads a module by a computed name, which a bundle cannot capture');
    else found.push(where+` uses Node's '${name}': a data module runs on Hermes and in the browser, which have no Node APIs`);
  };
  const scopes=[], path=[];
  const free=(name)=>!scopes.some(s=>s.has(name));
  const walk=(node,parent,key)=>{
    if (!node || typeof node!=='object') return;
    if (Array.isArray(node)) { node.forEach((n,i)=>{ path.push([parent,key,i]); walk(n,parent,key); path.pop(); }); return; }
    if (node.type==='UnaryExpression' && node.operator==='typeof') return;
    const pushed=SCOPES.includes(node.type) || FUNCTIONS.includes(node.type);
    if (pushed) scopes.push(declares(node));
    if (node.type==='Identifier' && !(NOT_REFERENCES[parent?.type]===key && !parent.computed) && free(node.name)) {
      const name=node.name, grand=path.at(-2)?.[0];
      const constant=name==='process' && parent?.type==='MemberExpression' && key==='object' && propertyName(parent)==='env' && grand?.type==='MemberExpression' && propertyName(grand)==='NODE_ENV';
      const literal=name==='require' && parent?.type==='CallExpression' && key==='callee'
        && ['Literal','TemplateLiteral'].includes(parent.arguments[0]?.type) && !parent.arguments[0].expressions?.length;
      if ((DOM.includes(name) || NODE.includes(name)) && !constant && !literal && !guarded(name,path)) refuse(name,node);
      // `Function`, and `eval` direct or not, however reached as a name.
      else if (CODE.includes(name)) refuse(name,node);
    }
    if (node.type==='MemberExpression') {
      const object=bare(node.object), name=propertyName(node);
      if (object?.type==='Identifier' && HOSTS.includes(object.name) && free(object.name)
        && (DOM.includes(name) || NODE.includes(name) || HOSTS.includes(name) || CODE.includes(name)) && !guarded(name,path)) refuse(name,node);
    }
    // `import(x)` of a computed name, like a computed `require`.
    if (node.type==='ImportExpression' && !(['Literal','TemplateLiteral'].includes(node.source?.type) && !node.source.expressions?.length)) refuse('import()',node);
    for (const k in node) if (k!=='parent' && node[k] && typeof node[k]==='object') {
      if (!Array.isArray(node[k])) path.push([node,k]);
      walk(node[k],node,k);
      if (!Array.isArray(node[k])) path.pop();
    }
    if (pushed) scopes.pop();
  };
  walk(ast,null,null);
  return found;
}

// Time and seeds are source arguments (LLP 1027.000): every executor refuses the clock, randomness and every
// timer but an answer's own `setTimeout` (D1) when they are used, with these words (js/src/prelude.js,
// js/src/binary.js, host/web-js/ts-fetch.js). A direct use in the app's own code is refused here too, at
// build, by file and line, so a test that runs the module under Bun, which has no such guard, cannot hide it.
const ambient=(api)=>api+' is unavailable in data sources; pass time or a random seed as an argument';
const timers=(api)=>api+' is unavailable in data sources: an answer waits with setTimeout; pass time as an argument';
const TIMERS=['setInterval','requestAnimationFrame','requestIdleCallback'];
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
    if (member(callee,'Math','random',local)) return 'Math.random() is unavailable in data sources; seed from crypto.getRandomValues in a source (docs/recipes/random-and-shuffle.md), or pass a seed as an argument; a roll in the view needs no source (docs/recipes/random-rolls.md)';
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
// `line:column` of an offset in `code`, by ECMAScript's line terminators:
// LF, CR (CRLF is one), U+2028, U+2029.
function lineColumn(code) {
  const starts=[0];
  for (let i=0;i<code.length;i++) {
    const c=code.charCodeAt(i);
    if (c===10 || c===0x2028 || c===0x2029 || (c===13 && code.charCodeAt(i+1)!==10)) starts.push(i+1);
  }
  return (offset)=>{ let lo=0, hi=starts.length; while (lo+1<hi) { const mid=(lo+hi)>>1; if (starts[mid]<=offset) lo=mid; else hi=mid; } return (lo+1)+':'+(offset-starts[lo]+1); };
}
/** The direct uses of the clock, randomness and timers in the captured
 * module `file`, as `path:line:col: why` with its path in the app: `parse`
 * is Rolldown's (a plugin's `this.parse`, or `parseAst`), given TypeScript.
 * An early warning a Bun test cannot give, not the guard: an alias still
 * reaches the runtime's refusal. */
export function ambientRefusals(stage, file, code, parse) {
  if (!isAbsolute(file) || /\.d\.[cm]?ts$/.test(file) || !/\.[cm]?[jt]sx?$/.test(file)) return [];
  const lang=/\.[cm]?tsx?$/.test(file) ? (file.endsWith('x') ? 'tsx' : 'ts') : 'js';
  const label=relative(stage,file).split(sep).join('/'), found=[], at=lineColumn(code);
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
  // Installed declaration internals belong to their package; app sources stay strict.
  skipLibCheck: true,
  target: 'ES2023',
  module: 'ESNext',
  moduleResolution: 'bundler',
  allowImportingTsExtensions: true,
  // The app's own `.js` files are typed from their code, not checked; a
  // package's JavaScript is never read for types (`maxNodeModuleJsDepth` 0).
  allowJs: true,
  lib: ['ES2023', 'ES2024.Object', 'ES2024.Collection', 'ES2024.Promise', 'ES2024.String', 'WebWorker'],
  // No package's types are global unless a file names it: an installed
  // `@types/node` or `@types/bun` would declare Node's timers and `process`, which
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

/** On Snapback, a successful mutation record says ok:true; refusals throw.
 * Narrow the captured seam, so TypeScript checks aliases and helpers without
 * interpreting application code. Ordinary resource records retain boolean ok. */
export function snapbackMutationTypes(declarations) {
  const original="export type Result<S extends Source> = SourceMap[S]['result'];";
  if(!declarations.includes(original)) return declarations;
  if(!declarations.includes('export type MutationSource =')) throw new Error('Snapback mutation types need regenerated Contract declarations; rebuild the Contract compiler');
  return declarations.replace(original,
    "type SnapbackMutationSuccess<R> = R extends {ok: boolean} ? R & {ok: true} : R;\n"+
    "export type Result<S extends Source> = S extends MutationSource ? SnapbackMutationSuccess<SourceMap[S]['result']> : SourceMap[S]['result'];");
}

/** `appId`, `grants` and the export names of the module `code` (app.ts),
 * read from its syntax: no build runs app or package code to learn them. Each
 * of `appId` and `grants` is a constant: a string, a template, `+` or an
 * array's `.join(…)` of constants, or a top-level `const` holding one; any
 * other is refused by name and place. Keyed grants have already become a
 * string in `injectBackendOrigin`. An absent one is `undefined`. */
export function staticFacts(code, parse) {
  const parsed=parse('app.ts',code);
  if (parsed.errors?.length) return { appId: undefined, grants: undefined, exported: [] };
  const body=parsed.program.body, at=lineColumn(code), consts=new Map(), exported=new Map();
  for (const statement of body) {
    const s=['ExportNamedDeclaration','ExportDefaultDeclaration'].includes(statement.type) ? statement.declaration ?? statement : statement;
    if (s.type==='VariableDeclaration') for (const d of s.declarations) {
      if (d.id.type!=='Identifier') continue;
      if (s.kind==='const') consts.set(d.id.name,d.init);
      if (statement.type==='ExportNamedDeclaration') exported.set(d.id.name,d.id.name);
    }
    if (statement.type==='ExportNamedDeclaration' && s.id?.name) exported.set(s.id.name,s.id.name);
    if (statement.type==='ExportNamedDeclaration' && !statement.source) for (const x of statement.specifiers ?? []) exported.set(x.exported.name ?? x.exported.value,x.local.name ?? x.local.value);
    if (statement.type==='ExportDefaultDeclaration') exported.set('default','default');
  }
  const value=(name)=>{
    if (!exported.has(name)) return undefined;
    const seen=new Set();
    const constant=(node)=>{
      node=bare(node);
      const refuse=()=>{ throw new Error(`app.ts:${at(node?.start ?? 0)}: ${name} must be a constant the build reads without running the module: a string, a template or + of constants, an array of them .join('\\n'), or a top-level const holding one`); };
      if (!node) refuse();
      if (node.type==='Literal' && ['string','number'].includes(typeof node.value)) return node.value;
      if (node.type==='TemplateLiteral') return node.quasis.map((q,i)=>q.value.cooked+(i<node.expressions.length ? constant(node.expressions[i]) : '')).join('');
      if (node.type==='BinaryExpression' && node.operator==='+') return constant(node.left)+constant(node.right);
      if (node.type==='Identifier' && consts.has(node.name) && !seen.has(node.name)) { seen.add(node.name); const v=constant(consts.get(node.name)); seen.delete(node.name); return v; }
      if (node.type==='CallExpression' && node.arguments.length<=1 && propertyName(bare(node.callee))==='join' && bare(bare(node.callee).object)?.type==='ArrayExpression')
        return bare(bare(node.callee).object).elements.map(constant).join(node.arguments.length ? constant(node.arguments[0]) : ',');
      return refuse();
    };
    // A number is refused: native evaluation reads `grants: string`, and an app id is a string too.
    const v=constant(consts.get(exported.get(name)) ?? null);
    if (typeof v!=="string") throw new Error(`app.ts: ${name} must be a string; it is the number ${v}`);
    return v;
  };
  return { appId: value('appId'), grants: value('grants'), exported: [...exported.keys()] };
}

export function configure(stage, incremental=false, parse) {
  const app=capturedOrigins(stage).at(-1)[1], modulePath=resolve(stage,'app.ts');
  if (existsSync(modulePath)) {
    const before=readFileSync(modulePath,'utf8'), original=resolve(stage,'__exact_backend_source.json');
    const authored=existsSync(original) ? JSON.parse(readFileSync(original,'utf8')) : before;
    const selection=backendOriginSelection(app), warning=backendOriginWarning(authored,selection,parse);
    writeFileSync(resolve(stage,'__exact_backend_warning.json'), JSON.stringify(warning));
    // `import.meta.env` first: a `const origin` it sets is a literal the backend selection may then override.
    const settings=inlineSettings(authored,app,parse);
    writeFileSync(resolve(stage,'__exact_env.json'), JSON.stringify(settings.record));
    const after=developmentGrants(injectBackendOrigin(settings.code,selection?.origin ?? null,parse),parse);
    if (before!==after) writeFileSync(modulePath,after);
    // Every producer refuses alike what the web build cannot read without running it.
    if (parse) staticFacts(after,parse);
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
  for (const [name,file] of moduleBindings(stage,app)) paths[name] ??= [file];
  const config={compilerOptions:{...compilerOptions,paths,
    ...(incremental ? {incremental:true,tsBuildInfoFile:resolve(stage,'__exact_build.tsbuildinfo')} : {})},files:['__exact_entry.ts'],
    // The app's own declarations, as an editor's project has them (an
    // untyped package's `declare module`); a package's are read when named.
    include:['**/*.d.ts'],exclude:['**/node_modules']};
  const path=resolve(stage,'__exact_tsconfig.json'), bytes=JSON.stringify(config);
  if (!existsSync(path) || readFileSync(path,'utf8')!==bytes) writeFileSync(path,bytes);
  if(capturedOrigins(stage).slice(0,-1).some(([,from])=>/(?:^|[/\\])snapback4[/\\](?:ts|online)$/.test(from))) {
    const declaration=resolve(stage,'app.contract.d.ts');
    const before=readFileSync(declaration,'utf8'), after=snapbackMutationTypes(before);
    if(before!==after) writeFileSync(declaration,after);
  }
  stagePackages(stage);
}

// A native module's names in data code (LLP 1133.002 D13): an `import … from
// "@exact/geolocation"` reaches a binding generated here from the package's
// `exact.json`, never committed, so a published package and a `file:` one
// read alike. Each interface installed above the app, the nearest first, is
// written into the stage as `__exact_modules/<name>.ts` and mapped by
// `paths`, which the checker and Rolldown share. A call goes through the
// runner's admission: the web runtime's `__exact_module`, else a native
// request (`exact-native:`), whose answer the runner gives as the module's
// declaration says, an error as `{code, message}` (D1).
// Members other interfaces declare on `name`'s names, by name, with their interface's shapes (LLP 1133.014 D10 item 1).
const membersOf=(name, interfaces)=>interfaces.flatMap(i=>Object.entries(i.declaration.members ?? {}).filter(([o])=>o.split('#')[0]===name).flatMap(([o,m])=>Object.entries(m ?? {}).map(([option,spec])=>[o.split('#')[1],{ option, type: spec.type, answers: spec.answers, shapes: i.declaration.shapes ?? {} }]))).reduce((out,[fn,m])=>((out[fn] ??= []).push(m), out),{});
function moduleBindings(stage, app) {
  // The target the build compiles, as an implementation's `targets` names it, or none (a loop serving several).
  const mapping=resolve(stage,'__exact_paths.json'), named=existsSync(mapping) ? JSON.parse(readFileSync(mapping,'utf8')).target : null;
  const target=['web','ios','macos','tvos','android','windows','linux'].includes(named) ? named : null, out=new Map();
  const interfaces=moduleInterfaces(app); for (const {name,root,declaration,implementations} of interfaces) {
    const file='__exact_modules/'+name.replace(/^@/,'').replace(/\//g,'__')+'.ts';
    mkdirSync(resolve(stage,'__exact_modules'),{recursive:true});
    // An interface the executor implements carries its own binding (LLP 1133.007 D12).
    const own=Array.isArray(declaration.executor) && typeof declaration.binding==='string' ? readFileSync(resolve(root,declaration.binding),'utf8') : null;
    const text=own ?? moduleBinding(name,declaration,implementations.map(i=>i.json),names=>domDeclarations(app,names),membersOf(name,interfaces),target), at=resolve(stage,file);
    if (!existsSync(at) || readFileSync(at,'utf8')!==text) writeFileSync(at,text);
    out.set(name,'./'+file);
  }
  return out;
}

/** Each module interface installed above `app`, the nearest first: its
 * package's name and directory, its declaration, and the implementations
 * its package carries (`exports` subpaths whose target is an
 * implementation's `exact.json`). */
export function moduleInterfaces(app) {
  const out=new Map(), read=(path)=>{ try { return JSON.parse(readFileSync(path,'utf8')); } catch { return null; } };
  const entry=(exports, key)=>{
    const t=exports && typeof exports==='object' ? (Object.keys(exports).some(k=>k.startsWith('.')) ? exports[key] : key==='.' ? exports : null) : key==='.' ? exports : null;
    return typeof t==='string' ? t : t && typeof t==='object' ? (t.contract ?? t.default ?? null) : null;
  };
  for (let dir=app; ; dir=dirname(dir)) {
    const modules=join(dir,'node_modules');
    if (existsSync(modules)) {
      const names=readdirSync(modules).flatMap(n=>n.startsWith('@') ? (statSync(join(modules,n)).isDirectory() ? readdirSync(join(modules,n)).map(m=>n+'/'+m) : []) : [n]);
      for (const name of names) {
        if (out.has(name)) continue;
        const root=join(modules,name), pkg=read(join(root,'package.json')), main=entry(pkg?.exports,'.');
        if (!pkg || !main || !/(^|\/)exact\.json$/.test(main)) continue;
        const declaration=read(resolve(root,main));
        if (declaration?.kind!=='interface') continue;
        const implementations=Object.keys(pkg.exports).filter(k=>k!=='.').map(k=>entry(pkg.exports,k))
          .filter(t=>t && /\/exact\.json$/.test(t)).map(t=>({dir:dirname(resolve(root,t)),json:read(resolve(root,t))})).filter(d=>d.json?.kind==='implementation');
        out.set(name,{name,root,declaration,implementations});
      }
    }
    if (dirname(dir)===dir) break;
  }
  return [...out.values()];
}

/** The implementation bound to interface `iface` for `target` (LLP 1133.001
 * D4): the one `app.json` names for the target, else the app's one direct
 * dependency that implements it there, else the one the interface carries;
 * `null` for none. Each choice is checked against the interface's version. */
export function implementationFor(app, iface, target) {
  const read=(path)=>{ try { return JSON.parse(readFileSync(path,'utf8')); } catch { return null; } };
  const located=(name)=>{
    for (let dir=app; ; dir=dirname(dir)) {
      const root=join(dir,'node_modules',name), pkg=read(join(root,'package.json'));
      const main=typeof pkg?.exports==='string' ? pkg.exports : pkg?.exports?.['.'];
      const at=typeof main==='string' ? main : main?.contract ?? main?.default;
      if (pkg && typeof at==='string' && /(^|\/)exact\.json$/.test(at)) { const json=read(resolve(root,at)); return json && {name,dir:dirname(resolve(root,at)),json}; }
      if (dirname(dir)===dir) return null;
    }
  };
  const covers=(i)=>i?.json?.kind==='implementation' && String(i.json.implements ?? '').replace(/@[^@]*$/,'')===iface.name && (i.json.targets ?? []).includes(target);
  const checked=(i)=>{
    const range=String(i.json.implements).slice(iface.name.length+1), version=read(join(iface.root,'package.json'))?.version;
    if (range && version && !Bun.semver.satisfies(version,range)) throw new Error(`bake-implementation-mismatch: ${i.name ?? iface.name} implements ${i.json.implements}, and the installed ${iface.name} is ${version}`);
    return i;
  };
  const manifest=read(join(app,'app.json')), named=manifest?.implementations?.[iface.name]?.[target];
  if (named) {
    const i=located(named);
    if (!covers(i)) throw new Error(`bake-implementation-missing: app.json names ${named} for ${iface.name} on ${target}, and ${i ? `it does not implement it for ${target}` : 'it is not installed'}`);
    return checked(i);
  }
  const deps=Object.keys(read(join(app,'package.json'))?.dependencies ?? {}).filter(n=>n!==iface.name).map(located).filter(covers);
  if (deps.length>1) throw new Error(`bake-implementation-ambiguous: ${deps.map(d=>d.name).join(' and ')} each implement ${iface.name} for ${target}; name one in app.json: "implementations": { "${iface.name}": { "${target}": "${deps[0].name}" } }`);
  if (deps.length) return checked(deps[0]);
  const carried=iface.implementations.find(i=>(i.json.targets ?? []).includes(target));
  return carried ? checked({name:iface.name,...carried}) : null;
}

/** The app's Rust data sources' text (`data/src`), where a generated binding's call writes its literal options. */
function rustSources(app) {
  const walk=(d)=>{ try { return readdirSync(d,{withFileTypes:true}).flatMap(e=>e.isDirectory() ? walk(join(d,e.name)) : e.name.endsWith('.rs') ? [readFileSync(join(d,e.name),'utf8')] : []); } catch { return []; } };
  return walk(join(app,'data','src')).join('\n');
}

/** The app's own source text that can name an interface: `app.ts`, and every
 * `.contract` file in the app's tree, since a root `app.contract` may import
 * the components that use one from a subdirectory. */
function appText(app) {
  const contracts=(d)=>{ try { return readdirSync(d,{withFileTypes:true}).flatMap(e=>e.isDirectory() ? (/^(\.|node_modules$|target$)/.test(e.name) ? [] : contracts(join(d,e.name))) : e.name.endsWith('.contract') ? [join(d,e.name)] : []); } catch { return []; } };
  return [join(app,'app.ts'),...contracts(app)].map(f=>{ try { return readFileSync(f,'utf8'); } catch { return ''; } }).join('\n');
}

/** The packages the app imports, wherever it imports them (LLP 1133.004
 * stage 0, LLP 1133 D6 rule 6): every Contract file `app.contract` reaches
 * through relative `use … from "./…"`, and every TypeScript file `app.ts`
 * reaches through relative imports, each bare specifier taken to its
 * package's name. `text` is those files' text, where a literal option a
 * build reads (an Info.plist key) is found. */
export function usedPackages(app) {
  const names=new Set(), seen=new Set(), texts=[];
  const packageName=(spec)=>{ const parts=spec.split('/'); return spec.startsWith('@') ? parts.slice(0,2).join('/') : parts[0]; };
  // A TypeScript import may name the emitted file (`./x.js` for `./x.ts`).
  const tsFile=(base)=>[...['', '.ts', '.tsx', '.mts', '.js', '.mjs', '/index.ts', '/index.js'].map(e=>base+e), base.replace(/\.m?js$/,'.ts'), base.replace(/\.m?js$/,'.tsx')].find(f=>{ try { return statSync(f).isFile(); } catch { return false; } });
  const visit=(file, contract)=>{
    if (!file || seen.has(file) || file.split(sep).includes('node_modules')) return;
    seen.add(file);
    let text='';
    try { text=readFileSync(file,'utf8'); } catch { return; }
    texts.push(text);
    // Comments name nothing, and a type-only import links nothing.
    const code=text.replace(/\/\*[\s\S]*?\*\//g,'').replace(/(^|[^:'"\\])\/\/.*$/gm,'$1');
    const specs=contract
      ? [...code.matchAll(/^\s*use\b[^\n]*?\bfrom\s+"([^"]+)"/gm)].map(m=>m[1])
      : [...code.matchAll(/\b(import|export)(\s+type\b)?[^'";]*?\bfrom\s*(['"])([^'"\n]+)\3|\bimport\s*\(?\s*(['"])([^'"\n]+)\5/g)]
          .filter(m=>!m[2]).map(m=>m[4] ?? m[6]);
    for (const spec of specs) {
      if (spec.startsWith('.')) visit(contract ? resolve(dirname(file),spec) : tsFile(resolve(dirname(file),spec)), contract);
      else if (!spec.startsWith('exact:') && !spec.startsWith('node:')) names.add(packageName(spec));
    }
  };
  visit(join(app,'app.contract'), true);
  visit(tsFile(join(app,'app')), false);
  return { names, text: texts.join('\n') };
}

/** The Swift implementations the app's interfaces carry for `target` (`ios`,
 * `macos`, `tvos`), of the interfaces the app names in `app.contract` or
 * `app.ts` (LLP 1133.001 D4): each extension, its own Swift module's name,
 * its class, its source files, and the system frameworks it needs from an OS
 * version (`frameworks`, LLP 1133.016 D15). */
export function appleImplementations(app, target) {
  const { names: used, text } = usedPackages(app), named = `${text}\n${appText(app)}`;
  return moduleInterfaces(app).filter(i=>used.has(i.name) || usesInterface(i,named)).flatMap(i=>{
    const impl=implementationFor(app,i,target);
    const files=impl?.json.platform==='apple' && impl.json.sources?.swift;
    if (!impl || !files?.length || !impl.json.class) return [];
    const extension=i.name.split('/').at(-1);
    // An Info.plist dictionary keyed by each literal strings key the app passes at `name.option` (LLP 1133.002 D10).
    const plist=Object.entries(impl.json.plist ?? {}).map(([key,at])=>{
      const [name,option]=at.split('.'), keys=new Set();
      for (const [,inside] of named.matchAll(new RegExp(`\\b${name}\\s*\\(([^)]*)\\)`,'g')))
        for (const [,,value] of inside.matchAll(new RegExp(`\\b${option}\\s*[=:]\\s*(["'])([^"']+)\\1`,'g'))) keys.add(value);
      // A Rust data source's generated binding takes the option as a snake_case field (`purpose_key: "…"`).
      const snake=option.replace(/[A-Z]/g,c=>'_'+c.toLowerCase());
      for (const [,value] of rustSources(app).matchAll(new RegExp(`\\b${snake}\\s*:\\s*(?:Some\\()?"([^"]+)"`,'g'))) keys.add(value);
      return {key,keys:[...keys].sort()};
    }).filter(p=>p.keys.length);
    // An interface that declares an authenticator entry (LLP 1133.010 D13): its class serves it in process.
    const authenticator=!!i.declaration?.authenticator?.apple;
    const events=(i.declaration.names ?? []).some(n=>n.event);
    return [{extension,module:'ExactPkg_'+extension.replace(/\W/g,'_'),className:impl.json.class,sources:files.map(f=>resolve(impl.dir,f)),plist,views:packageViewTags(i,impl),authenticator,start:i.declaration?.start==='load',frameworks:impl.json.frameworks??{},activity:impl.json.apple?.activity??null}];
  });
}

/** The view tags an implementation serves (LLP 1133.001 D1): every view its interface declares, and its own. */
function packageViewTags(iface, impl) {
  const tags=v=>Array.isArray(v) ? v.filter(t=>typeof t==='string') : v && typeof v==='object' ? Object.keys(v) : [];
  return [...new Set([...tags(iface.declaration.views), ...tags(impl?.json.views)])].sort();
}

/** Whether `app` uses interface `i`: names it in a `use` or an import, or places a view it declares. */
function usesInterface(i, named) {
  if (named.includes(`"${i.name}"`) || named.includes(`'${i.name}'`)) return true;
  return packageViewTags(i, null).some(tag => new RegExp(`(^|[^\\w-])${tag}([^\\w-]|$)`).test(named));
}

/** The Kotlin implementations bound for Android (LLP 1133.001 D4) of the interfaces the app names: each extension,
 * its class, its source directories, which join the APK's Kotlin, and the Maven coordinates it names, which join the
 * one locked Gradle graph (LLP 1133.008 D8 item 5). */
export function androidImplementations(app) {
  const { names: used, text } = usedPackages(app), named = `${text}\n${appText(app)}`;
  return moduleInterfaces(app).filter(i=>used.has(i.name) || usesInterface(i,named)).flatMap(i=>{
    const impl=implementationFor(app,i,'android'), dirs=impl?.json.platform==='android' && impl.json.sources?.kotlin;
    if (!impl || !dirs?.length || !impl.json.class) return [];
    return [{extension:i.name.split('/').at(-1),className:impl.json.class,dirs:dirs.map(d=>resolve(impl.dir,d)),views:packageViewTags(i,impl),maven:Array.isArray(impl.json.maven) ? impl.json.maven : [],manifest:impl.json.manifest ?? null,start:i.declaration?.start==='load'}];
  });
}

/** Each used interface's package configuration for `target` (LLP 1133.021 §9 item 7): extension name to the
 * values `app.json` gives under `packages.<interface>.<target>`, for the keys the interface's `config` declares. A
 * key typed `env` names a variable (`{"env": "NAME"}`) whose value is read here and never written to the repo. A
 * required key that is absent throws its declared code; one required only at `release` throws only for a release. */
export function packageConfig(app, target, { release = false } = {}) {
  const { names: used, text } = usedPackages(app), named = `${text}\n${appText(app)}`;
  let manifest = {};
  try { manifest = JSON.parse(readFileSync(resolve(app, 'app.json'), 'utf8')); } catch { /* a plain Contract directory */ }
  const out = new Map();
  for (const i of moduleInterfaces(app).filter(i => used.has(i.name) || usesInterface(i, named))) {
    const declared = i.declaration?.config?.[target];
    if (!declared) continue;
    const given = manifest.packages?.[i.name]?.[target] ?? {}, config = {};
    for (const [key, d] of Object.entries(declared)) {
      let value = given[key];
      if (d.type === 'env' && value !== undefined) value = typeof value?.env === 'string' ? process.env[value.env] : undefined;
      const required = d.required === true || (d.required === 'release' && release);
      if ((value === undefined || value === '') && required) {
        const how = d.type === 'env' ? `{"env": "<VARIABLE>"} naming a variable that is set` : `a ${d.type}`;
        throw new Error(`${d.missing ?? 'bake-config-missing'}: app.json "packages": {"${i.name}": {"${target}": {"${key}": …}}} needs ${how}`);
      }
      if (value !== undefined && value !== '') config[key] = value;
    }
    out.set(i.name.split('/').at(-1), config);
  }
  return out;
}

/** Each extension's web implementation, as bound (LLP 1133.001 D4):
 * extension name to the implementation's JavaScript entry. */
export function webImplementations(app) {
  const out=new Map();
  for (const iface of moduleInterfaces(app)) {
    const web=implementationFor(app,iface,'web'), file=web?.json.sources?.javascript?.[0];
    if (file) out.set(iface.name.split('/').at(-1),resolve(web.dir,file));
  }
  return out;
}

/** The declarations of the DOM's `names` (WebAuthn's JSON dictionaries, LLP
 * 1133.011 D10), and of each type they name, copied from the TypeScript
 * compiler's own `lib.dom.d.ts`, which the data module's libraries leave
 * out: the binding is in the stage, so the check reads them there. A name
 * the library lacks is an opaque object. */
function domDeclarations(app, names) {
  let text='';
  // The compiler above the app, else exact2's own (an app outside the repo installs none).
  for (const from of [resolve(app,'package.json'), import.meta.url]) {
    try {
      const pkg=createRequire(from).resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`);
      text=readFileSync(resolve(dirname(pkg),'lib','lib.dom.d.ts'),'utf8');
      break;
    } catch { /* neither has one: each name is opaque */ }
  }
  const found=new Map(), queue=[...names];
  const declared=n=>new RegExp(`^(interface ${n}\\b[^{]*\\{\\n[\\s\\S]*?\\n\\}|type ${n} = [^;]*;)`,'m').exec(text)?.[1];
  while (queue.length) {
    const n=queue.shift();
    if (found.has(n)) continue;
    const decl=declared(n);
    found.set(n, decl ? 'export '+decl : `export type ${n} = Record<string, unknown>;`);
    if (decl) for (const [ref] of decl.slice(decl.indexOf(n)+n.length).matchAll(/\b[A-Z][A-Za-z0-9]*\b/g)) if (!found.has(ref) && declared(ref)) queue.push(ref);
  }
  return [...found.values()];
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
  // TypeScript expands Windows short paths; compare the same physical names
  // for the capture, libraries and files, including directory junctions.
  stage=realpathSync.native(stage);
  libraries=realpathSync.native(libraries);
  const allowed = path => within(path,stage) || (within(path,libraries) && configuredLibrary(relative(libraries,path)));
  // A package's JavaScript entry (`typescript/bin/tsc`, an `EXACT_TSC`
  // override) keeps its Node shebang: run it through Bun, as the one-shot
  // producer runs its tools (js/bake/src/lib.rs `run`).
  let head='';
  try { const fd=openSync(tsc,'r'), bytes=Buffer.alloc(128); head=bytes.toString('utf8',0,readSync(fd,bytes,0,128,0)).split('\n')[0]; closeSync(fd); } catch { /* not readable: execFile says so */ }
  const script=head.startsWith('#!') && head.split(/\s+/).some(word=>['node','bun'].includes(word.split('/').at(-1)));
  const args=['--project',resolve(stage,'__exact_tsconfig.json'),'--pretty','false','--listFiles'];
  return new Promise((done, fail) => execFile(script ? process.execPath : tsc,script ? [tsc,...args] : args,
    {cwd:stage,env:checkEnv,encoding:'utf8',maxBuffer:4*1024*1024,timeout:checkBound(),killSignal:'SIGKILL'}, (error, stdout, stderr) => {
      // (A tsc that never returns is stopped at the bound: checkBound above.)
      if (error?.killed && error.signal === 'SIGKILL') return fail(new Error(`the TypeScript check did not finish in ${boundWords(checkBound())} (tsc stopped)`));
      if (error) {
        const diagnostics=String(stdout??'').split(/\r?\n/).filter(line=>! /^(?:\/|[A-Za-z]:[\\/]).*\.[cm]?[jt]sx?$/.test(line)).join('\n');
        const mutationFailure=/Type '(?:false|boolean)' is not assignable to type 'true'|SnapbackMutationSuccess/.test(diagnostics)
          && readFileSync(resolve(stage,'app.contract.d.ts'),'utf8').includes('type SnapbackMutationSuccess<');
        const refusal=mutationFailure ? "\nnote: a Snapback mutation returning {ok:false} leaves failure(slot) empty; throw refused({code: 'YOUR_CODE', message: 'Explain the refusal'}) instead. Annotate successful helpers with Result<'source'>, or keep ok:true literal with `as const`.\n" : '';
        return fail(new Error(diagnostics+sourceNames(stage,diagnostics)+refusal+String(stderr??'') || String(error.message)));
      }
      for(const path of stdout.trim().split(/\r?\n/)) {
        if(path && !allowed(realpathSync.native(path)))return fail(new Error('module outside captured app: '+path));
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
if (process.argv[2]==='--ambient' && basename(process.argv[1] ?? '')==='typescript.mjs') {
  try { console.log(JSON.stringify(await ambientDiagnostics(resolve(process.argv[3])))); }
  catch(error) { console.error(error.message); process.exitCode=1; }
}
