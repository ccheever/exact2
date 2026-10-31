// The dev loop on the JS target (LLP 1071): what `host/web/dev.mjs` runs for
// an app the JS target takes, so a developer runs the runtime the app ships.
// An edit — the Contract, the app's TypeScript or Rust data, its assets, or
// this runtime and compiler — rebuilds the app (`host/web-js/build.mjs
// --render none`, into a stage renamed over dist/, so a request sees the old
// build or the new one; what did not change is not rebuilt: module.mjs
// `fresh`) and every open page reloads: the plan is compiled ahead of time,
// so a new plan is a new program. The old page checkpoints the wasm loop's
// carried set to this server immediately before that reload; the next
// document consumes it once. A build that fails
// shows its errors in the page and the page keeps the last good build.
// `host/web/dev.mjs --wasm` is the resident wasm loop (a plan restarts in
// place in ~20 ms, state carried).
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { createServer, request } from 'node:http';
import { copyFileSync, existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, renameSync, rmSync, statSync, utimesSync, watch, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { acceptsHTML, appDocumentPath, webRequestURL } from '../../scripts/origin.mjs';
import { compilerWarnings, INPUT_TREE, OUTPUT, listenOn } from '../../scripts/agent-launch.mjs';
import { builtSettings } from '../../js/bake/src/__exact_env.mjs';
import { alsoOnIPv6Loopback, appManifestDigest, buildFileCards, listenDelay, replaceBuild, saveTrace, sendStaticBody, watchLauncher, webContentType } from '../web/serve.mjs';
import { generationFile, generationPath, servedFile } from '../web/generations.mjs';
import { besidePattern } from '../web/beside.mjs';
import { localInstaller } from '../web/local-install.mjs';
import { designPath, frameOnlyByTool, tamberServer } from '../../scripts/tamber-server.mjs';
import { pluginOnLoopback } from '../../scripts/tamber-remote.mjs';
import { openRemoteCanvas } from '../../scripts/tamber-canvas.mjs';
import { mapCard, pullMap } from '../../scripts/tamber-inspect.mjs';
import { serveCommands, serveReport, streamFrame } from '../../scripts/tamber-builds.mjs';
import { fresh, freshCompiler, webCompiler } from './module.mjs';
import { INSPECT_EVENTS } from './inspect-dev.js';
import { galleryLoop } from './dev-gallery.mjs';
import { inlineWhy as inlineWhyOf } from './dev-inline.mjs';
import { DEV_STATUS_TYPE, FAILED, PHASE, gist } from '../web/dev-phase.mjs';

const CHECKPOINT_BYTES = 16 * 1024 * 1024, CHECKPOINTS = 8;

const root = resolve(new URL('../..', import.meta.url).pathname);
// (What a build in `dist` keeps beside it, `dist.gen` and the rest, is no source: beside.mjs.)
const skipped = new RegExp(String.raw`(^|\/)(target|dist|${besidePattern()}|node_modules|conformance)(\/|$)|(^|\/)\.|\.md$`);
/** The files beside app.json a setting is read from (`import.meta.env`, LLP 1153 D4): inputs, though dot files. */
const settingsFile = (name) => /^\.env(?:\.local)?$/.test(String(name));
/** Whether `path` changed from `since` (ms) until now, by the entry's own times (lstat: a link replaced by one to an older
 * file is new, though its target is not): its modification, or its status change, which alone moves when an older file
 * is renamed over it. A time past now is a file dated ahead (an archive's, a clock's), which a build read as it is, so it is no change. Throws
 * for a path that is gone, which each caller reads its own way. */
const changedSince = (path, since) => { const s = lstatSync(path), now = Date.now() + 1; return [s.mtimeMs, s.ctimeMs].some((t) => t >= since && t <= now); };
/** Whether a watcher's event for `path` is an edit since `since`: macOS reports a build's reads of a tree (the assets it
 * copies) as events too, and a path unchanged since then is none. A path that is gone is an edit. */
const movedSince = (path, since) => { try { return changedSince(path, since); } catch { return true; } };
/** The inputs that moved while a build ran, as its set-aside line names them (stage 15 D1.5): each by its path under
 * the app (a package's as it lies), three at most and how many more; "an input" when none was seen by name. */
const movedWords = (paths, dir) => {
  const names = [...new Set(paths)].map((p) => (relative(dir, p).startsWith('..') ? p : relative(dir, p)) || '.');
  if (!names.length) return 'an input';
  const shown = names.slice(0, 3).join(', ');
  return names.length > 3 ? `${shown} and ${names.length - 3} more` : shown;
};

let building = null; // the build in flight, stopped with the server
/** Stop the build in flight and what it runs (its own process group). */
const stopBuild = (signal = 'SIGTERM') => {
  if (!building) return;
  const pid = building.pid;
  try { process.kill(-pid, signal); } catch { building.kill(signal); }
  // One that does not stop is killed in 5 s, its group with it (not once
  // it is gone: its group id could be another's by then).
  if (signal !== 'SIGKILL') building.escalate ??= setTimeout(() => { try { process.kill(-pid, 'SIGKILL'); } catch { /* gone */ } }, 5000).unref();
};
// The sources the last build read that were there as the next began (one
// absent when consulted is no source gone).
let lastSources = [];

/** The Contract packages a build read (LLP 1091 D10), outside the app and
 * its skipped `node_modules`: the dev loop watches them too. */
function devPackages(stage) {
  try {
    const sources = JSON.parse(readFileSync(resolve(stage, '.gen', 'dev-sources.json'), 'utf8'));
    return { trees: sources.packages ?? [], shallow: sources.shallow ?? [], sources: sources.sources ?? [] };
  } catch { return { trees: [], shallow: [], sources: [] }; }
}

/** The development build's design (`exact-web-js --dev-reload`'s
 * `dev-design.json`: structure, digest, tokens), or null. */
function devDesign(stage) {
  try { return JSON.parse(readFileSync(resolve(stage, '.gen', 'dev-design.json'), 'utf8')); } catch { return null; }
}

/** The SHA-256 of the design file a build in `dir` was compiled from, as
 * its source map records it (LLP 1122 D5), or null. */
function mapDesignSource(app, dir) {
  try {
    const file = realpathSync(designPath(app.dir));
    return JSON.parse(readFileSync(resolve(dir, 'app.plan.map.json'), 'utf8')).sources?.find((s) => { try { return realpathSync(s.file) === file; } catch { return false; } })?.sha256 ?? null;
  } catch { return null; }
}

/** One build of `app` into `dist` through a stage under the app's ignored
 * target/ (as the wasm build stages); resolves to null or the build's
 * errors (what the JS target refused, or a compile error). `keep(design,
 * logic)`, when it says so, keeps the build in `dist` (a value-only design
 * edit, LLP 1121 stage 4: the page takes a batch instead). `superseded()`,
 * when it says so as the build ends (an input moved since it began, or it
 * was stopped for one), or a source it read moved since, leaves `dist` as
 * it was and resolves `{superseded}`: what it read is older than the files.
 * With none, it is published whatever moved. */
function build(app, dist, keep = () => false, compiler = null, superseded = null) {
  // One stage per dist: two loops on one target/ (two apps outside a workspace, or one CARGO_TARGET_DIR) never
  // build into each other's.
  const stage = resolve(app.target, 'web-js-dev-stage', createHash('sha256').update(resolve(dist)).digest('hex').slice(0, 16));
  rmSync(stage, { recursive: true, force: true });
  const began = Date.now();
  // The app's own inputs beside its Contract sources, and which were there.
  const own = [designPath(app.dir) ?? resolve(app.dir, 'app.tamber'), resolve(app.dir, 'app.ts'), resolve(app.dir, 'app.json'), resolve(app.dir, '.exact/backend-origin'), resolve(app.dir, '.env.local'), resolve(app.dir, '.env')];
  const there = new Set(own.filter((path) => existsSync(path)));
  const known = new Set(lastSources.filter((path) => existsSync(path)));
  return new Promise((done) => {
    // The JS target's build itself, rendering no pages (every route is the
    // shell), with the completion marker host/web/build.mjs writes.
    const child = building = spawn(process.execPath, [resolve(root, 'host/web-js/build.mjs'), app.name, '--out', stage, '--render', 'none', '--dev'],
      { cwd: root, env: compiler ? { ...process.env, EXACT_WEB_COMPILER: compiler } : process.env, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
    let log = '';
    child.stdout.on('data', (d) => { log += d; });
    child.stderr.on('data', (d) => { log += d; });
    // EXACT_DEV_BUILD_HOLD_MS (a test's): the build's end taken that much later, as a slow one ends.
    const hold = Number(process.env.EXACT_DEV_BUILD_HOLD_MS) || 0;
    // Its end is its whole group's (cargo and the compilers it ran, which
    // outlive a stopped leader for a moment, holding Cargo's lock): waited
    // for, 5 s at most before they are killed.
    const groupGone = (then, until = Date.now() + 5000) => {
      try { process.kill(-child.pid, 0); } catch (e) { if (e.code === 'ESRCH') { clearTimeout(child.escalate); return then(); } }
      if (Date.now() > until) { try { process.kill(-child.pid, 'SIGKILL'); } catch { /* gone */ } }
      setTimeout(() => groupGone(then, until), 20);
    };
    child.on('exit', (code) => groupGone(() => setTimeout(() => {
      building = null;
      const packages = devPackages(stage);
      // Or a source it read moved since it began (a watcher's event can come
      // after the build ends): its Contract sources (one gone is moved), and
      // the design, app.ts, app.json, the backend's origin and the settings
      // files .env.local and .env (one come or gone is moved), by their times. Asked again just before it is
      // published or kept, after the work before that.
      const since = (path) => changedSince(path, began);
      lastSources = packages.sources;
      const newer = () => packages.sources.some((path) => { try { return since(path); } catch { return known.has(path); } })
        || own.some((path) => { try { lstatSync(path); } catch { return there.has(path); } return !there.has(path) || since(path); });
      const setAside = () => { rmSync(stage, { recursive: true, force: true }); done({ superseded: true, packages }); };
      const moved = () => superseded && (superseded() || newer());
      // A failed one too: its errors may be of files since edited.
      if (moved()) return setAside();
      // The one line a build says when it rewrote the types app.ts imports, built or not (LLP 1116.001 D4.5).
      const types = /^app\.contract\.d\.ts: .*$/m.exec(log)?.[0];
      if (types) console.log(types);
      if (code !== 0) { rmSync(stage, { recursive: true, force: true }); return done({ packages, error: log.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l) && !compilerWarnings(l).length).slice(-12).join('\n') || `build exited ${code}` }); }
      const logic = readFileSync(resolve(stage, '.exact-dev-logic.json'), 'utf8').trim();
      const design = devDesign(stage), designSource = mapDesignSource(app, stage);
      if (keep(design, logic)) {
        return setImmediate(() => {
          if (moved()) return setAside();
          rmSync(stage, { recursive: true, force: true }); done({ error: null, logic, packages, design, designSource, kept: true, warnings: compilerWarnings(log) });
        });
      }
      writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', app: { id: app.id, name: app.displayName },
        manifestSha256: appManifestDigest(app), files: buildFileCards(stage), ...builtSettings(stage) }) + '\n');
      // Once more after the watchers' events queued meanwhile are taken.
      setImmediate(() => {
        if (moved()) return setAside();
        replaceBuild(stage, dist);
        done({ error: null, logic, packages, design, designSource, warnings: compilerWarnings(log) });
      });
    }, hold)));
  });
}

const mtimeMs = (path) => { try { return statSync(path).mtimeMs; } catch { return null; } };

/** Copy the compiler at `path`, and its dep-info, to `copy`, dated `built`
 * (ms), the date it must have before and after, as its dep-info must keep
 * its own (Cargo rewrites it in place): each file written whole, dated,
 * then renamed. Where that fails, the copy stays as it was. */
function copyCompiler(path, copy, built) {
  const temp = `${copy}.${process.pid}.tmp`, tempDep = `${temp}.d`;
  try {
    const dep = mtimeMs(`${path}.d`);
    if (mtimeMs(path) !== built || dep === null) throw new Error('built again');
    copyFileSync(path, temp);
    utimesSync(temp, new Date(built), new Date(built));
    copyFileSync(`${path}.d`, tempDep);
    if (mtimeMs(path) !== built || mtimeMs(`${path}.d`) !== dep || !readFileSync(tempDep, 'utf8').endsWith('\n')) throw new Error('built again');
    renameSync(tempDep, `${copy}.d`);
    renameSync(temp, copy);
  } catch { rmSync(temp, { force: true }); rmSync(tempDep, { force: true }); }
}

/** Build `app` on the JS target and serve it with reload, until the process ends. */
export async function devJs({ app, dist, port, host, origins, gate, lan, allowHosts = [], remoteAsked = { remote: null, tunnel: null, plugin: false, hosts: [] }, residentPort = 0, pairTtlMs }) {
  const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
  const t0 = Date.now();
  // A first build that fails (a refusal or a compile error) serves its
  // errors in the page and builds again at the next edit. One an app source
  // moved during (the watchers come after it) is built again before the
  // loop serves anything.
  // At most three times: a build that writes one of its own inputs (a
  // build script's generated file) is served after that, and built again
  // once the watchers are on.
  let built, startMoved = false;
  for (let moved = false, tries = 1; ; moved = false, tries++) {
    const input = (name) => name && ((!skipped.test(String(name)) && String(name) !== 'app.contract.d.ts') || /^\.exact(?:[\\/]backend-origin)?$/.test(String(name)) || settingsFile(name));
    const begun = Date.now();
    const early = [watch(app.dir, { recursive: true }, (_, name) => { if (!moved && input(name) && movedSince(resolve(app.dir, String(name)), begun)) moved = true; }), watch(resolve(root, 'host/web-js'), { recursive: true }, (_, name) => { if (!moved && name && !skipped.test(String(name)) && movedSince(resolve(root, 'host/web-js', String(name)), begun)) moved = true; })];
    try { built = await build(app, dist, () => false, null, tries < 3 ? () => moved : null); } finally { for (const w of early) w.close(); }
    startMoved = moved;
    if (!built.superseded) break;
    console.log('a source moved while the first build ran: building again');
  }
  let error = built.error, logicRevision = built.logic ?? '';
  // The page's own plan's source map (LLP 1122 T2b, the JS target), kept
  // after each build by that plan's digest (the SHA-256 of dist's app.plan,
  // which the page says as `planDigest` and the map must name), the 8 last:
  // a selection on the page is joined to it, and an outline drawn from it.
  const pageMaps = new Map();
  const keepPageMap = () => {
    try {
      const digest = createHash('sha256').update(readFileSync(resolve(dist, 'app.plan'))).digest('hex'), bytes = readFileSync(resolve(dist, 'app.plan.map.json'));
      if (JSON.parse(bytes).digest !== digest) return;
      pageMaps.delete(digest); pageMaps.set(digest, bytes);
      for (const old of [...pageMaps.keys()].slice(0, Math.max(0, pageMaps.size - 8))) pageMaps.delete(old);
    } catch { /* no map: a selection on that plan is read-only */ }
  };
  // (What dist serves, built now or kept from before a failed build.)
  keepPageMap();
  // The design the builds run now, with the SHA-256 of the design file's
  // bytes it was compiled from (LLP 1122 D5: an inspected token's edit is
  // offered only while they are the file's): a value-only edit's, or a
  // build's (a native on the resident producer may keep its plan and take
  // it as a batch); null at start, for the plan's own design (the join
  // reads the map's).
  let served = null;
  // The compiler's warnings (LLP 1116.004 D7) when they change, not again at each save that leaves them.
  let warned = '';
  const warn = (warnings) => {
    if (!warnings || warnings.join('\n') === warned) return;
    if (warnings.length) console.log(warnings.join('\n'));
    else console.log('the compiler warns of nothing now');
    warned = warnings.join('\n');
  };
  if (error) console.log(`JS target: ${app.name} does not build; its errors show in the page\n${error}`);
  else console.log(`JS target: ${app.name} built in ${Date.now() - t0} ms`);
  warn(built.warnings);
  // Ready lines wait for the server to answer (and its doors), as a test or an agent connects on them.
  let listening = false; const held = [], sayReady = (line) => { if (listening) console.log(line); else held.push(line); };
  // The design server's wire, once made below (a rebuild may come first).
  let tamber = null, doors = null;
  const clients = new Set();
  // A batch to a page behind waits, then the latest design's notice goes instead (tamber-builds.mjs).
  const push = (m) => { for (const res of clients) if (m.kind === 'tamber-batch') streamFrame(res, JSON.stringify(m), () => snapshot && notice(snapshot)); else res.write(`data: ${JSON.stringify(m)}\n\n`); };
  // The live table (LLP 1121 stage 4): the design the page in dist was built
  // with, and the one the pages run after value-only edits since.
  let design = built.design ?? null, snapshot = null;
  // The SHA-256 of the design file's bytes `design` was compiled from, which
  // a scrub's frame must have been folded over to be patched on it.
  let designFrom = built.designSource ?? null;
  // A snapshot's notice keeps a scrub frame's mark (*1.11*).
  const notice = (s) => ({ kind: 'tamber-batch', structure: s.structure, base: s.design, design: s.design, tokens: [], ...(s.scrub ? { scrub: s.scrub } : {}) });
  // A batch larger than this, in UTF-8 bytes, goes as the snapshot's notice
  // instead, as the resident loop sends it (host/web/dev.mjs DESIGN_FRAME):
  // a native host's event frames are bounded (PlanURL.swift's 64 KiB).
  const framed = (batch) => (Buffer.byteLength(JSON.stringify(batch)) > 32 * 1024 ? notice(snapshot) : batch);
  const same = (a, b) => a.value === b.value && a.slices.length === b.slices.length && a.slices.every((s, i) => s === b.slices[i]);
  // A scrub (LLP 1121 stage 4, commit 6): the design the pages run while it
  // previews, and the snapshot it put aside; the next build, or its end,
  // sends them on from it.
  // `released`: that scrub is being written; the build of what it writes
  // goes from it, and ends it.
  let scrubbed = null, aside = null, released = false;
  // The released scrub's build carries the mark its release took (above its
  // frames, *1.11*), so a late fetch of one of them never shows over it; the
  // scrub ends with it.
  const releasedMark = () => { const mark = released?.id ? released : undefined; scrubbed = null; aside = null; released = false; return mark; };
  // A design (`exact-web-js design`), read from the compiler with no build:
  // a value-only edit's and a scrub's. Optimised (host-dev), built as the
  // loop starts and again after any edit that is not a design's, so it is
  // the compiler the builds use; until it is, the debug compiler answers.
  const tool = { path: resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'host-dev/exact-web-js'), ready: false, built: null, child: null, reading: null, cleaner: null };
  // Cargo and the compilers it runs, as a group (stopped together).
  const stopGroup = (child) => { if (!child) return; try { process.kill(-child.pid, 'SIGTERM'); } catch { child.kill(); } };
  // What a failed build said: each error with the lines that explain it (a
  // linker's `ld:` and `Undefined symbols` lines, its notes, the command line
  // cut short), then its last lines.
  const readerFailure = (text) => {
    const lines = text.trim().split('\n'), out = [];
    const cut = (l) => (l.length > 300 ? `${l.slice(0, 300)} …` : l);
    lines.forEach((l, i) => {
      if (!/^error|^\s*(= note|note:)|ld: |Undefined symbols|undefined reference|duplicate symbol|referenced from|clang: error|linker command failed/.test(l)) return;
      out.push(cut(l));
      // The symbols and where they were wanted.
      if (/Undefined symbols/.test(l)) out.push(...lines.slice(i + 1, i + 9).map(cut));
    });
    out.push(...lines.slice(-4).map(cut));
    return [...new Set(out)].slice(0, 60).join('\n');
  };
  // A link that wants `.llvm.<hash>` symbols of a crate's own objects is
  // that crate's incremental state gone stale (the checkout's sources moved
  // under it, as a branch switch does, host-dev being incremental): each
  // workspace crate whose object or archive wanted such a symbol (macOS
  // ld's `referenced from`, LLD's `referenced by`, GNU ld's `in function`,
  // read per symbol) is cleaned from host-dev and the reader built again,
  // once. Cargo's lock on target/ keeps the clean from
  // running under another build there.
  const workspace = () => {
    const meta = spawnSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20 });
    try { return new Map(JSON.parse(meta.stdout).packages.map((p) => [p.name.replaceAll('-', '_'), p.name])); } catch { return new Map(); }
  };
  const staleCrates = (text, packages) => {
    const crates = new Set(), lines = text.split('\n');
    const collect = (l) => { for (const m of l.matchAll(/\b(?:lib)?([a-z_][a-z0-9_]*)-[0-9a-f]{16}\b/g)) crates.add(m[1]); };
    const header = /"([^"]+)", referenced from:|undefined (?:hidden )?symbol: (\S+)/;
    lines.forEach((l, i) => {
      // macOS ld's and LLD's: a missing symbol, then the lines that want it.
      const block = header.exec(l);
      if (block) {
        if (!/\.llvm\.\d+/.test(block[1] ?? block[2])) return;
        for (let j = i + 1; j < lines.length && lines[j].trim() && !header.test(lines[j]); j++) collect(lines[j]);
        return;
      }
      // GNU ld's: the object on the line, or the `in function` line above.
      const gnu = /undefined reference to [`'"]([^`'"]+)/.exec(l);
      if (gnu && /\.llvm\.\d+/.test(gnu[1])) {
        collect(l);
        if (i > 0 && /in function/.test(lines[i - 1])) collect(lines[i - 1]);
      }
    });
    return [...crates].map((c) => packages.get(c)).filter(Boolean);
  };
  let cleaned = false, readerHolds = 0;
  const buildTool = () => {
    tool.ready = false;
    stopGroup(tool.child);
    const started = new Date();
    const child = tool.child = spawn('cargo', ['build', '-q', '--profile', 'host-dev', '-p', 'exact-web-js'], { cwd: root, stdio: ['ignore', 'ignore', 'pipe'], detached: true });
    let err = '';
    child.stderr.on('data', (d) => { err = (err + d).slice(-64000); });
    child.on('error', (e) => { if (tool.child === child) { tool.child = null; console.log(`the design reader did not build (design edits take the full build): ${e.message}`); resumeAfterReader(); } });
    // EXACT_DEV_READER_HOLD_MS (a test's): the loop's first reader build taken that much later, as a slow one ends
    // (only the first: one built again because a cold build left its inputs newer is not held twice).
    const hold = readerHolds++ === 0 ? Number(process.env.EXACT_DEV_READER_HOLD_MS) || 0 : 0;
    child.on('exit', (code, signal) => setTimeout(() => {
      if (tool.child !== child) return;
      tool.child = null;
      tool.ready = code === 0 && existsSync(tool.path);
      // Dated to the build's start, as Cargo dates its fingerprints: a
      // source saved while it compiled makes it stale (`fresh`).
      if (tool.ready) { try { utimesSync(tool.path, started, started); } catch { tool.ready = false; } }
      // For the builds' copy: the date this build gave it, while it still has
      // it (not on a filesystem that keeps a coarser one: the debug compiler).
      tool.built = tool.ready && mtimeMs(tool.path) === started.getTime() ? started.getTime() : null;
      const stale = !tool.ready && !signal && !cleaned && !stopping ? staleCrates(err, workspace()) : [];
      if (stale.length) {
        cleaned = true;
        console.log(`the design reader did not link (stale incremental state in ${stale.join(', ')}): cleaning ${stale.length === 1 ? 'it' : 'them'} from host-dev and building it again`);
        // A group of its own, which the loop's stop ends too.
        const clean = tool.cleaner = spawn('cargo', ['clean', '-q', '--profile', 'host-dev', ...stale.flatMap((c) => ['-p', c])], { cwd: root, stdio: 'ignore', detached: true });
        clean.on('exit', (code) => {
          if (tool.cleaner !== clean) return;
          tool.cleaner = null;
          if (stopping) return;
          if (code === 0) buildTool();
          else { console.log(`the design reader did not build (design edits take the full build): cleaning ${stale.join(', ')} failed`); resumeAfterReader(); }
        });
        return;
      }
      if (!tool.ready && !signal) console.log(`the design reader did not build (design edits take the full build):\n${readerFailure(err)}`);
      if (tool.ready) console.log('design reader ready'); // (until then a live proposal is taken as a rebuild, dev-inline.mjs)
      resumeAfterReader();
    }, hold));
  };
  // Ready, and built from the compiler's sources as they are (a compiler
  // edit the loop does not watch, under contract/ or kernel/, rebuilds it).
  const toolFresh = () => {
    if (!tool.ready) return false;
    if (fresh(tool.path, `${tool.path}.d`)) return true;
    buildTool();
    return false;
  };
  // The same, starting nothing: for what a request below write scope reaches
  // (`propose`'s class, stage 10 A1), which may not start a build.
  const toolReady = () => tool.ready && fresh(tool.path, `${tool.path}.d`);
  buildTool();
  // The page's inspect table (LLP 1122 T2b, inspect.rs), made for dist's
  // plan when the page first asks (seconds for a large app, so no build
  // makes it), kept by the plan's digest, the 4 last; it names that plan,
  // and one made while dist moved on is not served.
  const tables = new Map();
  const inspectTable = (digest, plan = resolve(dist, 'app.plan')) => {
    if (!tables.has(digest)) {
      tables.set(digest, new Promise((done) => {
        // (this loop's own: another loop on one target/ may make the same plan's table, and removes its own)
        const out = resolve(app.target, `inspect-${digest}-${process.pid}.json`);
        const compiler = toolFresh() ? { cmd: tool.path, pre: [], done() {} } : webCompiler();
        const child = spawn(compiler.cmd, [...compiler.pre, 'inspect-table', plan, out], { cwd: root, stdio: ['ignore', 'ignore', 'pipe'] });
        let err = '';
        child.stderr.on('data', (d) => { err += d; });
        child.on('exit', (code) => {
          compiler.done();
          let bytes = null;
          try { bytes = code === 0 ? readFileSync(out) : null; if (bytes && JSON.parse(bytes).plan !== digest) bytes = null; } catch { bytes = null; }
          rmSync(out, { force: true });
          if (!bytes) { tables.delete(digest); if (err.trim()) console.log(`inspect table: ${err.trim().split('\n').slice(-3).join('\n')}`); }
          done(bytes);
        });
      }));
      for (const old of [...tables.keys()].slice(0, Math.max(0, tables.size - 4))) tables.delete(old);
    }
    return tables.get(digest);
  };
  // `readOnly` (what a request below write scope reaches): the built reader
  // only, as it is, never a build or the `cargo run` fallback.
  const designOf = (text, { readOnly = false } = {}) => new Promise((done) => {
    if (readOnly && !toolReady()) return done({ error: 'the design reader is not built' });
    const compiler = readOnly ? { cmd: tool.path, pre: [] } : toolFresh() ? { cmd: tool.path, pre: [] } : webCompiler();
    // A group: the debug fallback is a `cargo run`, stopped with its compilers.
    const child = tool.reading = spawn(compiler.cmd, [...compiler.pre, 'design', resolve(app.dir, 'app.contract'), '-'], { cwd: root, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
    child.on('error', (e) => done({ error: e.message }));
    child.stdin.on('error', () => {});
    child.stdin.end(text);
    let out = '', err = '';
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => { err += d; });
    // `close`: its output read whole.
    child.on('close', (code) => {
      if (tool.reading === child) tool.reading = null;
      try { done(code === 0 ? JSON.parse(out) : { error: err.trim().split('\n').slice(-6).join('\n') || `the compiler exited ${code}` }); } catch (e) { done({ error: e.message }); }
    });
  });
  // The design's shape and the plan's structure (a profile colour's value
  // is in it) as a kept build requires, and each token's accent.
  // A token the page writes inline (`inline`: no property sets it in place,
  // LLP 1121 stage 6) builds when its value moves.
  const sameShape = (from, next) => from && next && from.shape === next.shape && from.structure === next.structure && from.tokens.length === next.tokens.length
    && next.tokens.every((t, i) => t.name === from.tokens[i].name && String(t.accent) === String(from.tokens[i].accent) && (!t.inline || same(t, from.tokens[i])));
  // A fast batch is checked by the full build after it (its verifying build): the
  // design's module inputs and its compile; a value-only edit during that
  // build is sent at once and checked after it.
  let designing = false, stopping = false;
  // Input changes seen, counted: a build that began before the last one
  // read older files, and is set aside (`superseded`), never published.
  // An edit stops the build in flight, once in a burst and only while it
  // is young (under 2 s, and under half the last build's time): one past
  // that finishes, set aside, so sustained edits never starve a build.
  // Builds set aside in a row past three are said: an input that moves in
  // every build (a build that writes one of its own) is never published.
  let inputs = 0, buildBegan = 0, lastBuildMs = 0, stopped = false, asideRun = 0;
  // The paths that moved since the full build in flight began: what its set-aside line names (stage 15 D1.5).
  const movedSinceBuild = new Set();
  // A value-only edit waiting for the design reader to build again (it does after every other edit): it reads the
  // edit then, in seconds, where the full build it would otherwise take is minutes on a large app, said nowhere until
  // it ended (the third hands-on pass: 343 s, the canvas on the old value). A reader that fails gives it the full build.
  let readerWait = false;
  const resumeAfterReader = () => { if (!readerWait || stopping) return; readerWait = false; rebuild(); };
  const stopYoungBuild = () => {
    const age = Date.now() - buildBegan;
    if (!building || building.exitCode !== null || building.signalCode !== null || stopped || age >= 2000 || (lastBuildMs && age >= lastBuildMs / 2)) return;
    stopped = true;
    console.log(`edit → the build in flight stopped after ${age} ms (aborted): an input moved`);
    stopBuild();
  };
  // The files edited since the last build, by path: only design files may
  // be a value-only edit.
  const edited = new Set(), appeared = new Set();
  let seq = 1, saved = 0, again = false, timer = null, since = t0;
  // `.exact/backend-origin`'s bytes when the last build began (null: none).
  const backendOf = () => { try { return readFileSync(resolve(app.dir, '.exact/backend-origin'), 'utf8'); } catch { return null; } };
  let backendSeen = backendOf();
  const pending = new Map(); // seq -> the save it answers
  // The variant gallery (dev-gallery.mjs), started by its first page: told of each publication after it is out.
  const gallery = galleryLoop({ app, root, appDist: dist, channel: () => tamber?.channel, design: () => design?.digest ?? null, idleMs: Number(process.env.EXACT_DEV_GALLERY_IDLE_MS) || undefined,
    // (The reader's binary while it is fresh, asked without building it or the reader again; else no table yet.)
    tableTool: () => (tool.ready && tool.path && fresh(tool.path, `${tool.path}.d`) ? { cmd: tool.path, pre: [] } : null),
    compiler: () => { const copy = resolve(app.target, `web-js-dev-compiler-${app.name.replace(/[^\w.-]/g, '_')}`); return freshCompiler(copy) ? copy : null; },
    client: (n) => client(n, null, { base: '/__dev/gallery', stream: '/__dev/gallery/stream', page: 'gallery' }) });
  const rebuild = async () => {
    // A burst a build already took (its timer still armed) is no edit.
    if (!edited.size) return;
    const at = saved, t = Date.now();
    // A path that existed during the save and is gone by now (an editor's
    // or sed's temporary file) edited nothing; one already gone when its
    // event came (a deleted asset) is an edit.
    const designs = [...edited].filter((path) => path.endsWith('.tamber'));
    const designOnly = designs.length > 0 && [...edited].every((path) => path.endsWith('.tamber') || (!existsSync(path) && appeared.has(path)));
    // A value-only design edit (LLP 1121 stage 4): its design from the
    // compiler, no build, sent as a batch; the page in dist stays, as a kept
    // build's does; the full build after it checks what the read cannot (the
    // modules' inputs, the whole compile). One that moves the design's shape
    // (or fails) builds. Only the app's own design: another .tamber may be
    // any build's input.
    const file = designPath(app.dir);
    const fast = designOnly && design && designs.every((path) => path === file);
    // A value-only edit reads its design and sends its batch whatever builds
    // meanwhile (that build is set aside); any other waits for it.
    if (designing || (building && !fast)) { again = true; return; }
    if (fast && !toolFresh() && (tool.child || tool.cleaner)) {
      if (!readerWait) console.log('edit → waiting for the design reader, building again, to read it');
      readerWait = true;
      return;
    }
    // Kept until a build lands: a failed one's edits are the next build's too.
    const pendingEdits = [...edited], pendingAppeared = [...appeared];
    edited.clear(); appeared.clear();
    // Whatever waited for the reader is this pass's now (a build that fails keeps it for the next edit, as any).
    readerWait = false;
    let checking = false; // the build below checks a batch the read sent
    if (fast && toolFresh()) {
      designing = true;
      let next;
      let text = null;
      const readAt = inputs;
      try { next = existsSync(file) ? await designOf(text = readFileSync(file, 'utf8')) : { error: 'no design' }; } catch (e) { next = { error: e.message }; } finally { designing = false; }
      // The loop stopped while it read: no build follows.
      if (stopping) return;
      // An edit came while it read (its event, or the file's bytes, which an
      // event can trail): this read is behind it, and the next one, with
      // that edit's, runs now (not after a build in flight).
      let still = false;
      try { still = text === null ? !existsSync(file) : readFileSync(file, 'utf8') === text; } catch { /* gone */ }
      if (inputs !== readAt || !still) {
        edited.add(file);
        for (const p of pendingEdits) edited.add(p);
        for (const p of pendingAppeared) appeared.add(p);
        again = false;
        return rebuild();
      }
      if (!next.error && sameShape(design, next)) {
        checking = true;
        const from = scrubbed ?? design;
        design = next;
        served = { digest: design.digest ?? null, source: designFrom = createHash('sha256').update(text).digest('hex') };
        const keep = scrubbed && released ? releasedMark() : undefined;
        const committed = { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens, scrub: keep };
        if (scrubbed) aside = committed; else { snapshot = committed; const kept = design.digest; setImmediate(() => gallery.designKept(kept)); } // (the gallery: a kept design, never a scrub's, once the batch below is out)
        const tokens = design.tokens.filter((tk, i) => !same(tk, from.tokens[i]));
        if (tokens.length && !scrubbed) {
          console.log(`edit → batch ready ${Date.now() - at} ms (design read in ${Date.now() - t} ms, no build) · ${tokens.length} token${tokens.length === 1 ? '' : 's'} to ${clients.size} page${clients.size === 1 ? '' : 's'}, no reload`);
          const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens, scrub: keep });
          push(batch);
          tamber?.channel.broadcast(batch);
        }
        // A check already under way is set aside, and done again after it.
        if (building) { for (const p of pendingEdits) edited.add(p); for (const p of pendingAppeared) appeared.add(p); again = true; return; }
      }
    }
    if (building) { for (const p of pendingEdits) edited.add(p); for (const p of pendingAppeared) appeared.add(p); again = true; return; }
    // An edit (or a set-aside build's edits) came while its design read:
    // the next pass takes them all, and starts the one build.
    if (again || edited.size) {
      for (const p of pendingEdits) edited.add(p);
      for (const p of pendingAppeared) appeared.add(p);
      again = false;
      return rebuild();
    }
    // The design reader's compiler is this build's too: a copy (Cargo
    // replaces even a fresh binary), taken while it is done and fresh, then
    // kept while it is fresh, so a build while the reader builds again (as
    // it does below, after every edit) takes it as well.
    // The app's own (apps in the tree share a target/).
    const copy = resolve(app.target, `web-js-dev-compiler-${app.name.replace(/[^\w.-]/g, '_')}`);
    // Only the binary this loop's own build left (another loop on this
    // target/ may have linked one since, not yet dated to its start).
    if (tool.ready && tool.built !== null && freshCompiler(tool.path)) copyCompiler(tool.path, copy, tool.built);
    const compiler = freshCompiler(copy) ? copy : null;
    // Anything else may move the compiler: the design tool is built again.
    if (!designOnly) buildTool();
    // What a full build reads from disk is what changed since it began.
    since = t;
    backendSeen = backendOf();
    const hadError = error, began = inputs;
    movedSinceBuild.clear();
    const keep = (next, logic) => designOnly && design && next && logic === logicRevision
      && next.structure === design.structure && next.tokens.length === design.tokens.length
      && next.tokens.every((t, i) => t.name === design.tokens[i].name && String(t.accent) === String(design.tokens[i].accent) && (!t.inline || same(t, design.tokens[i])));
    buildBegan = Date.now();
    console.log(`edit → building${checking ? ' (checking the batch)' : ''}`);
    built = await build(app, dist, keep, compiler, () => inputs !== began);
    if (built.superseded && ++asideRun === 3) console.log('edit → three builds in a row set aside: an input moves during every build (does one write its own input?)');
    if (!built.superseded) { stopped = false; lastBuildMs = Date.now() - buildBegan; asideRun = 0; }
    // An input moved while it built (a value-only edit's batch included):
    // what it read is behind the files, and nothing of it is published (not
    // dist, which a fresh page loads, nor the design or a reload). The
    // edits it took are the next build's, with those since.
    if (built.superseded) {
      console.log(`edit → a build superseded after ${Date.now() - t} ms, set aside: ${movedWords([...movedSinceBuild], app.dir)} moved while it built`);
      watchPackages(built.packages);
      for (const p of pendingEdits) edited.add(p);
      for (const p of pendingAppeared) appeared.add(p);
      if (!timer) { again = false; rebuild(); }
      return;
    }
    error = built.error;
    // The design the pages run now (a scrub's preview, or its end, may have
    // come while it built).
    const from = scrubbed ?? design;
    watchPackages(built.packages);
    // A repair clears the page's error.
    if (!error && hadError) push({ error: null });
    if (error) {
      for (const p of pendingEdits) edited.add(p);
      for (const p of pendingAppeared) appeared.add(p);
      console.log(`build failed in ${Date.now() - t} ms; the page keeps the last good build\n${error}`); push({ error });
    }
    else if (built.kept) {
      const tokens = built.design.tokens.filter((t, i) => !same(t, from.tokens[i]));
      design = built.design;
      served = { digest: design.digest ?? null, source: designFrom = built.designSource };
      // While a scrub previews the pages stay on it: this changes only what
      // its end restores. A released scrub's build goes from it, and ends it.
      const keep = scrubbed && released ? releasedMark() : undefined;
      const committed = { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens, scrub: keep };
      if (scrubbed) aside = committed; else { snapshot = committed; const kept = design.digest; setImmediate(() => gallery.designKept(kept)); }
      // A batch sent before this build: it checked out, and stands.
      if (fast && !tokens.length) console.log(`edit → checked in ${Date.now() - t} ms · the batch stands`);
      if (tokens.length && !scrubbed) {
        console.log(`edit → batch ready ${Date.now() - at} ms (built in ${Date.now() - t} ms) · ${tokens.length} token${tokens.length === 1 ? '' : 's'} to ${clients.size} page${clients.size === 1 ? '' : 's'}, no reload`);
        const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens, scrub: keep });
        push(batch);
        // The one source of batches: the resident loop's devices and Linux
        // take them through the channel's polled streams.
        tamber?.channel.broadcast(batch);
      }
    }
    else {
      // A reload ends a scrub's preview: the pages boot the committed design.
      // The scrub ends too: a frame folded on its sources would leave this edit out.
      tamber?.compete();
      scrubbed = null; aside = null; released = false;
      const next = built.design ?? null;
      keepPageMap();
      // The page reloads; a native build on the resident producer, whose
      // plan this edit may leave as it was (an accent's companion rules are
      // the JS target's own), takes the values as a batch when the design's
      // shape held, and the snapshot stays its to fetch.
      const shaped = from && next && from.shape === next.shape && from.tokens.length === next.tokens.length && next.tokens.every((tk, i) => tk.name === from.tokens[i].name);
      design = next;
      designFrom = built.designSource ?? null;
      snapshot = shaped ? { kind: 'tamber-snapshot', structure: next.shape, base: null, design: next.digest, tokens: next.tokens } : null;
      // A native build on the resident producer keeps its plan and takes
      // this design as a batch only when its shape held: then it is served,
      // from the bytes compiled; else no build runs it but on its own plan.
      served = shaped ? { digest: next.digest ?? null, source: built.designSource } : null;
      const changedTokens = shaped ? next.tokens.filter((tk, i) => !same(tk, from.tokens[i])) : [];
      if (changedTokens.length) tamber?.channel.broadcast(framed({ kind: 'tamber-batch', structure: next.shape, base: from.digest, design: next.digest, tokens: changedTokens }));
      logicRevision = built.logic;
      pending.set(++seq, at);
      sayReady(`edit → plan ready ${Date.now() - at} ms (rebuilt in ${Date.now() - t} ms) · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      warn(built.warnings);
      push({ reload: seq, revision: logicRevision }); gallery.appChanged(); // after the app's reload is published
    }
    if (again) { again = false; rebuild(); }
  };
  // An editor's temporary name for a `.tamber` file beside it: the file's
  // name with `.tmp` (and a hex or numeric suffix, a pid or a hash), `.swp`,
  // `.swx`, `~`, JetBrains' or Emacs's marks; anchored, so no other file
  // named after it (`app.tamber.tmp.png`) is one.
  const TEMPORARY = /^(?:\.?(.+\.tamber)(?:\.tmp(?:[.-][0-9a-f]+)?|\.sw[px]|~|___jb_(?:tmp|old)___)|#(.+\.tamber)#|\.#(.+\.tamber))$/;
  const temporaryOfDesign = (path) => {
    const m = TEMPORARY.exec(basename(path));
    const design = m && (m[1] ?? m[2] ?? m[3]);
    return !!design && existsSync(resolve(dirname(path), design));
  };
  // A build reading a tree (the assets it copies) is reported too on macOS:
  // a path not modified since the last build started is no edit.
  const changed = (base, skip = skipped, fresh = true) => (_, name) => {
    // A Contract source the compile read is an input even where the app's
    // watcher skips (a dot directory).
    if (name && skip.test(String(name)) && !contractInput(resolve(base, String(name)))
      && !(base === app.dir && (/^\.exact(?:[\\/]backend-origin)?$/.test(String(name)) || settingsFile(name)))) return;
    // `.exact/` is an input only for `backend-origin`: the loop's own design
    // server makes the directory for its lock, which is none. An event for
    // either is an edit when that file's bytes (or its being there) differ
    // from the last build's, whatever its time says.
    let backendMoved = false;
    if (base === app.dir && /^\.exact(?:[\\/]backend-origin)?$/.test(String(name))) {
      if (backendOf() === backendSeen) return;
      backendMoved = true;
    }
    // The declarations a build writes beside app.ts, for an editor.
    if (base === app.dir && String(name) === 'app.contract.d.ts') return;
    // An editor's temporary for a design beside it (an atomic save writes
    // `app.tamber.tmp` and renames it over `app.tamber`; its event can come
    // before or after the rename): never an input, whenever it comes.
    if (name && temporaryOfDesign(resolve(base, String(name)))) return;
    // A screenshot, film, log or note written into the app folder, outside the input trees and not named by
    // app.json, does not reload a page under a test (authoring bench).
    if (base === app.dir && OUTPUT.test(String(name)) && !INPUT_TREE.test(String(name)) && !JSON.stringify(app.manifest ?? {}).includes(String(name))) return;
    // A package's events are all edits: a link replaced by one to an older
    // file is newer than nothing, and its target's time says nothing of it.
    if (fresh && name && !backendMoved && !movedSince(resolve(base, String(name)), since)) return;
    if (!timer) saved = Date.now();
    const path = resolve(base, String(name ?? ''));
    edited.add(path);
    // A resident start that failed is tried again at the next request after
    // an edit: any, once it has exited; one to the app, which it builds
    // again, while it runs.
    if (!residentChild || base === app.dir) starting.failure = null;
    // A build in flight read older files: set aside, and stopped while young.
    inputs++; movedSinceBuild.add(path);
    stopYoungBuild();
    // Made since the last build began (a temporary), not an input it had:
    // only such a file gone again edited nothing.
    try { if (statSync(path).birthtimeMs >= since) appeared.add(path); } catch { /* gone */ }
    clearTimeout(timer);
    // An editor's save is one burst of events, well inside 5 ms; an event
    // after the build starts builds again (`again`).
    timer = setTimeout(() => { timer = null; rebuild(); }, 5);
  };
  // The resident loop's start as a native client sees it (dtool2's pass 3
  // #8): the step it is on (its `phase: ` lines), whether it is up, and the
  // error a start that failed ended with, until an edit.
  const starting = { phase: null, ready: false, failure: null, waiting: new Set() };
  let resident = null, residentChild = null;
  // The app's sources, and the runtime and compiler it builds with.
  const watchers = [watch(app.dir, { recursive: true }, changed(app.dir)), watch(resolve(root, 'host/web-js'), { recursive: true }, changed(resolve(root, 'host/web-js')))];
  for (const f of ['navigation.js', 'index.html', 'neutral.css', 'look.mjs']) watchers.push(watch(resolve(root, 'host/web', f), changed(resolve(root, 'host/web'))));
  // Each Contract package the app uses, wherever it is installed or linked.
  const packages = new Set();
  const contractSources = new Set();
  // A source, or a directory holding one (its event names the directory).
  const contractInput = path => contractSources.has(path) || [...contractSources].some(s => s.startsWith(path + '/') || s.startsWith(path + '\\'));
  const watchPackages = ({ trees = [], shallow = [], sources = [] } = {}) => {
    for (const source of sources) contractSources.add(source);
    for (const dir of trees) {
      if (packages.has(dir) || !existsSync(dir)) continue;
      packages.add(dir);
      // A package's dot directories are its sources too; only its own
      // node_modules is not.
      watchers.push(watch(dir, { recursive: true }, changed(dir, /(^|\/)node_modules(\/|$)/, false)));
    }
    // One entry of a directory: a node_modules not made yet, or an install
    // that is a link. Its making, or its retargeting, is an edit.
    for (const [dir, entry] of shallow) {
      const key = `shallow:${dir}:${entry}`;
      if (packages.has(key) || !existsSync(dir)) continue;
      packages.add(key);
      watchers.push(watch(dir, (_, name) => { if (String(name) === entry) changed(dir, /^$/, false)(_, name); }));
    }
  };
  watchPackages(built.packages);
  // The first build was served though a source moved during its last try:
  // built again now, its edits seen by the watchers from here.
  if (startMoved) { edited.add(resolve(app.dir, 'app.contract')); rebuild(); }
  // The one TypeScript configuration app.ts is checked with (js/bake/src/typescript.mjs).
  watchers.push(watch(resolve(root, 'js/bake/src/typescript.mjs'), changed(resolve(root, 'js/bake/src'))));
  // The page's side: reload on a new build, the errors of a failed one in an
  // overlay, and a beacon when the reloaded page's runtime is up. Its build
  // channel (as host/web/dev.js's): a pin's scheme is the system's, a rule
  // beneath the app's `setScheme`; its status is what paints, sent again
  // once a turn when the app's own commands move it.
  // A checkpoint the server refuses (403: a canvas below `write`, LLP 1121
  // stage 10 C) reloads afresh, so a reader still sees the new build.
  // A gallery page's (dev-gallery.mjs): its own stream, design and checkpoint routes, `page: 'gallery'`, no `navigate`.
  const client = (n, revision, { base = '/__dev', stream = '/__dev/page', page = null } = {}) => `<script>(()=>{const fetch=globalThis.fetch.bind(globalThis),seq=${n},logicRevision=${JSON.stringify(revision)},es=new EventSource('${stream}');let o,reloading=false;
const show=t=>{if(!t){o?.remove();o=null;return}o??=document.body.appendChild(Object.assign(document.createElement('pre'),{style:'position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647',onclick:()=>show()}));o.textContent=t+'\\n(click to dismiss)'};
const encode=v=>typeof v==='number'&&(!Number.isFinite(v)||Object.is(v,-0))?{$exactNumber:Object.is(v,-0)?'-0':String(v)}:Array.isArray(v)?v.map(encode):v;
const children=e=>[...(e?.children??[])].filter(x=>x.hasAttribute('data-carry-type')||x.hasAttribute('data-listitemkey'));
const capture=logic=>{const x=globalThis.exact,active=document.activeElement?.closest?.('[data-carry-type]');let focus=null;if(active){const path=[];for(let el=active,p;el&&el.id!=='exact-root';el=p){p=el.parentElement;path.unshift(children(p).indexOf(el))}if(!path.includes(-1))focus={path,type:active.getAttribute('data-carry-type')}}return{time:x.clock.now,slots:[...x.state[0],...[...(globalThis.__exactDevPanels?.live??[])].filter(s=>!s.n.gone)].map(s=>[s.n.devName,s.n.devType,encode(s())]).concat(globalThis.__exactDevPanels?.left()??[]),answers:logic?[]:x.resources.filter(r=>!r.owned&&!r.failed&&!r.ticket&&!r.waiting&&!r.ov&&r.settled!==undefined).map(r=>[r.name,r.source,encode(r.settled),encode(r.value),r.devType,!!r.store]),carryAnswers:!logic,modes:ch.before?.mode??x.modes?.read()??null,focus}};
const restored=document.querySelector('script[type="application/vnd.exact.dev-checkpoint"]'),q=new URLSearchParams(location.search),devKeys=new Set(['agent','seed','locale','formatLocale','timeZone','epoch','storage']),admission=q.has('agent')?[...q].filter(([k])=>devKeys.has(k)):null;
if(restored){const restoredSeq=Number(restored.dataset.seq);console.info('exact dev reload: restored',restoredSeq);const t=setInterval(()=>{const b=document.getElementById('exact-root')?.dataset.bootMs;if(b!=null){clearInterval(t);Promise.resolve(globalThis.exact?.ready).then(()=>{console.info('exact dev reload: runtime up',restoredSeq);fetch('${base}/reloaded?seq='+restoredSeq+'&boot='+b+'&at='+Date.now(),{method:'POST'})})}},2)}
const ready=()=>globalThis.exact?Promise.resolve(globalThis.exact.ready):new Promise(ok=>{const t=setInterval(()=>{if(globalThis.exact){clearInterval(t);Promise.resolve(globalThis.exact.ready).then(ok)}},2)});
let liveQ=Promise.resolve(),liveStatus=null;const live=m=>{liveQ=liveQ.then(async()=>{const x=await ready().then(()=>globalThis.exact),was=x?.designDigest;let sc=m.scrub;const out=x?.design?.(m);if(out?.live==='stale'){const r=await fetch('${base}/design',{cache:'no-store'});if(r.status===200){const f=await r.json();sc=f.scrub;x.design(f)}}if(x?.designDigest!==was&&ch.nonce){clearTimeout(liveStatus);if(sc)liveStatus=setTimeout(status,150);else status()}}).catch(e=>console.error('exact dev:',e))};
const ch={nonce:null,registered:null,revision:null,before:null,told:null},cp=window!==parent&&/^exact-copy-\\d{1,2}$/.test(window.name),bk=${page ? `(cp?'exact.dev.build:'+window.name:'exact.dev.build')+':${page}'` : "cp?'exact.dev.build:'+window.name:'exact.dev.build'"},bid=(async()=>{const fresh=()=>'web-js-'+crypto.getRandomValues(new Uint32Array(2)).join('');let i;try{i=sessionStorage.getItem(bk)}catch{return fresh()}const t=typeof BroadcastChannel==='function'?new BroadcastChannel('exact.dev.build'):null;if(i&&t){const mine=i;if(await new Promise(done=>{t.onmessage=e=>{if(e.data==='taken:'+mine)done(true)};t.postMessage('who:'+mine);setTimeout(()=>done(false),60)}))i=null}i??=fresh();if(t){const own=i;t.onmessage=e=>{if(e.data==='who:'+own)t.postMessage('taken:'+own)}}try{sessionStorage.setItem(bk,i)}catch{}return i})(),post=(n,b,ms)=>fetch('/__dev/report',{method:'POST',body:JSON.stringify({nonce:n,...b}),...(ms?{signal:AbortSignal.timeout(ms)}:{})}),rep=b=>{const{nonce:n,registered:r}=ch;if(!n)return Promise.resolve();if(ch.queue?.n!==n)ch.queue={n,tail:Promise.resolve()};const q=ch.queue;return q.tail=q.tail.then(async()=>{if(!(await r)||ch.nonce!==n||ch.registered!==r)return;await post(n,b,10000).catch(()=>{})})};
const here=()=>{const at=location.pathname+location.search+location.hash;return at.length<=2048&&/^\\/(?![/\\\\])[\\x21-\\x5b\\x5d-\\x7e]*$/.test(at)?at:null};
const asked=()=>{const p=ch.pin,k=globalThis.exact?.asked??{};if(!cp||!p)return null;const o={},val=f=>k[f]?.value;if(p.scheme&&val('scheme')!=null&&val('scheme')!==p.scheme)o.scheme=String(val('scheme')).slice(0,32);if(p.rootFontSize&&val('rootFontSize')!=null&&val('rootFontSize')!==p.rootFontSize)o.rootFontSize=String(val('rootFontSize')).slice(0,64);const md=Object.fromEntries(Object.keys(p.mode??{}).filter(a=>val('mode:'+a)!=null&&val('mode:'+a)!==p.mode[a]&&a.length<=64&&String(val('mode:'+a)).length<=64).map(a=>[a,val('mode:'+a)]));if(Object.keys(md).length&&new TextEncoder().encode(JSON.stringify(md)).length<=800)o.mode=md;return Object.keys(o).length?o:null};
const facts=()=>{const s=getComputedStyle(document.documentElement),cs=s.colorScheme,x=globalThis.exact;return{kind:'status',scheme:/dark/.test(cs)&&!/light/.test(cs)?'dark':/light/.test(cs)&&!/dark/.test(cs)?'light':x?.systemScheme??(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'),contrast:matchMedia('(prefers-contrast: more)').matches?'more':matchMedia('(prefers-contrast: less)').matches?'less':'no-preference',mode:x?.modes?.read?.()??null,rootFontSize:parseFloat(s.fontSize),plan:null,planDigest:x?.plan??null,design:x?.designDigest??null,revision:ch.revision,location:here(),app:asked()??undefined}},status=()=>{clearTimeout(liveStatus);liveStatus=null;const b=facts();ch.told=JSON.stringify(b);rep(b)};for(const q of['(prefers-color-scheme: dark)','(prefers-contrast: more)','(prefers-contrast: less)'])matchMedia(q).addEventListener?.('change',()=>{if(ch.nonce)status()});
let gate=null,held=(()=>{try{return sessionStorage.getItem('exact.dev.inspect${page ? `:${page}` : ''}')!=null}catch{return false}})();const early=()=>{for(const id of['exact-inspect-early','exact-inspect-early-shield'])document.getElementById(id)?.remove()},letGo=exit=>{held=false;early();try{const k=sessionStorage.getItem('exact.dev.inspect${page ? `:${page}` : ''}');if(exit&&k)sessionStorage.setItem('exact.dev.inspect${page ? `:${page}` : ''}.left',k);sessionStorage.removeItem('exact.dev.inspect${page ? `:${page}` : ''}')}catch{}};if(held){const sh=document.head.appendChild(Object.assign(document.createElement('style'),{id:'exact-inspect-early-shield',textContent:'iframe, frame, embed, object { pointer-events: none !important; }'}));const c=Object.assign(document.createElement('div'),{id:'exact-inspect-early'});c.style.cssText='position:fixed;inset:0;z-index:2147483644;background:transparent;touch-action:none';(document.body??document.documentElement).append(c)}for(const ty of ${JSON.stringify([...INSPECT_EVENTS, 'pointermove', 'focusin'])})addEventListener(ty,e=>{if(gate)return gate(e);if(!held)return;if(ty==='keydown'&&e.key==='Escape')letGo(true);if((ty==='focusin'||ty==='blur')&&document.activeElement?.tagName==='IFRAME')document.activeElement.blur();e.preventDefault();e.stopImmediatePropagation()},{capture:true,passive:false});
const inspector=ready().then(()=>import('/__dev/inspect.js')).then(m=>{m.install({${page ? `key:'exact.dev.inspect:${page}',` : ''}fetch,report:rep,status:async()=>status(),facts:()=>({plan:null,design:globalThis.exact?.designDigest??null,revision:ch.revision}),nonce:()=>ch.nonce});gate=m.gate;return m}).catch(e=>{letGo(false);throw e});
let due=false;const moved=()=>{if(due||!ch.nonce)return;due=true;setTimeout(()=>{due=false;const b=facts();if(JSON.stringify(b)!==ch.told){ch.told=JSON.stringify(b);rep(b)}},0)},seen=new MutationObserver(moved);if(cp)ready().then(()=>{globalThis.exact.onAsked=moved});seen.observe(document.documentElement,{attributes:true,attributeFilter:['style','data-x-modes']});seen.observe(document.head,{childList:true,subtree:true,characterData:true});for(const k of['pushState','replaceState']){const w=history[k];history[k]=function(...a){const o=w.apply(this,a);moved();return o}}addEventListener('popstate',moved);addEventListener('hashchange',moved);
const nav=m=>{liveQ=liveQ.then(async()=>{await ready();const mine=()=>m.stream===ch.nonce;if(!mine())return;const x=globalThis.exact,u=new URL(m.location,location.origin),to=u.pathname+u.search,no=d=>{if(mine())rep({kind:'unsupported',what:'navigate',detail:String(d).slice(0,1024),revision:m.seq})};if(u.origin!==location.origin||u.pathname.startsWith('//'))return no("the location is no path on this page's origin");const there=()=>history.state?.url===to||location.pathname+location.search===to,before=history.state?.url;try{if(!there()){const why=typeof x?.goTo==='function'?x.goTo(to):'this page has no router';if(why)return no(why)}for(let i=0;i<100&&!there()&&history.state?.url===before;i++)await new Promise(ok=>setTimeout(ok,10));if(mine()&&there()&&u.hash!==location.hash)history.replaceState(history.state,'',location.origin+location.pathname+location.search+u.hash)}catch(e){return no(String(e))}moved()}).catch(e=>console.error('exact dev:',e))};
const pin=m=>{liveQ=liveQ.then(async()=>{await ready();const r=document.documentElement.style,p=m.pin,x=globalThis.exact,was=x.held;let rm=0;x.held=cp&&p?Object.fromEntries([['scheme',!!p.scheme],['rootFontSize',!!p.rootFontSize],...Object.keys(p.mode??{}).map(a=>['mode:'+a,true])]):null;ch.before??={fontSize:r.fontSize,hostFont:r.getPropertyValue('--exact-host-font-size'),mode:x?.modes?.read?.()??{}};const sh=document.getElementById('exact-dev-pin')??document.head.appendChild(Object.assign(document.createElement('style'),{id:'exact-dev-pin'}));sh.textContent=p?.scheme?':root{color-scheme:'+p.scheme+(cp?'!important':'')+'}':'';if((x.systemScheme??null)!==(p?.scheme??null)){x.systemScheme=p?.scheme??null;dispatchEvent(new Event('resize'))}r.removeProperty('font-size');if(cp&&p?.rootFontSize)r.setProperty('font-size',p.rootFontSize+'px','important');else r.fontSize=p?.rootFontSize?p.rootFontSize+'px':ch.before.fontSize;if(p?.rootFontSize)r.setProperty('--exact-host-font-size',r.fontSize);else if(ch.before.hostFont)r.setProperty('--exact-host-font-size',ch.before.hostFont);else r.removeProperty('--exact-host-font-size');const last=ch.pin?.mode??{},back=Object.fromEntries(Object.entries(ch.before.mode??{}).filter(([a])=>a in last&&!(a in(p?.mode??{}))&&!(cp&&x.asked?.['mode:'+a]))),mode=Object.entries(p?(cp?{...back,...(p.mode??{})}:{...ch.before.mode,...(p.mode??{})}):back);if(mode.length&&x?.modes?.set){const why=x.modes.set(mode);if(why){rm=1;rep({kind:'unsupported',what:'pin.mode',detail:why,revision:m.revision});if(!cp)x.modes.set(Object.entries(ch.before.mode));if(x.held){for(const k of Object.keys(x.held))if(k.startsWith('mode:'))delete x.held[k];for(const [k,on]of Object.entries(was??{}))if(on&&k.startsWith('mode:'))x.held[k]=true}}dispatchEvent(new Event('resize'))}if(p?.contrast)rep({kind:'unsupported',what:'pin.contrast',detail:"a web page cannot pin the browser's contrast",revision:m.revision});for(const [f,on]of Object.entries(was??{}))if(on&&!x.held?.[f])x.asked?.[f]?.apply();if(!p)ch.before=null;ch.pin=p?(cp&&rm?{...p,mode:last}:p):null;ch.revision=m.revision;status()}).catch(e=>console.error('exact dev:',e))};
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.kind==='tamber-batch'||m.kind==='tamber-snapshot'){live(m);return}if(m.kind==='pin'){pin(m);return}if(m.kind==='navigate'){m.stream=ch.nonce;nav(m);return}if(m.kind==='inspect'||m.kind==='outline'||m.kind==='hold'){m.stream=ch.nonce;inspector.then(i=>i.frame(m)).catch(e=>rep({kind:'unsupported',what:m.kind,detail:String(e)}));return}if(m.nonce&&m.nonce!==ch.nonce){const n=ch.nonce=m.nonce;ch.registered=ready().then(async()=>{const id=await bid,got=await post(n,{kind:'register',id,host:'web-js',capabilities:${page ? `['pin','inspect','copies','hold'],page:'${page}'` : "['pin','inspect','copies','hold','navigate']"},plan:null,planDigest:globalThis.exact?.plan??null,design:globalThis.exact?.designDigest??null}),ok=got.ok;if(!ok)console.warn('exact dev: the dev loop refused the register of this page ('+got.status+'), so no tool sees it');if(ok&&window!==parent)parent.postMessage({type:'exact-build',id},'*');return ok}).catch(e=>{console.warn('exact dev: this page could not register with the dev loop, so no tool sees it: '+e);return false});status()}if(m.error!==undefined)show(m.error);if(m.reload>seq&&!reloading){reloading=true;if(!globalThis.exact){location.reload();return}ready().then(async()=>{const checkpoint=capture(m.revision!==logicRevision),id=crypto.getRandomValues(new Uint32Array(4)).join('-');console.info('exact dev reload: checkpoint',m.reload);const saved=await fetch('${base}/checkpoint?id='+id+'&seq='+m.reload+'&revision='+encodeURIComponent(m.revision??''),{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(checkpoint)});if(saved.status===403){console.info('exact dev reload: no checkpoint at this scope; a fresh load');location.reload();return}if(!saved.ok)throw new Error('checkpoint handoff answered '+saved.status);const next=new URL(location.href);next.searchParams.set('__exactDev',id);if(admission)for(const [k,v]of admission)if(!next.searchParams.has(k))next.searchParams.set(k,v);location.replace(next.href)}).catch(e=>{reloading=false;console.error('exact dev reload: current page retained; checkpoint handoff failed',e)})}}})()</script>`;
  const checkpoints = new Map();
  /** A scrub's design: the compiler's, with its text in place of the design file's; written nowhere. */
  const scrubDesign = designOf;
  // A scrub's frames patched (tamber-scrub.mjs `patch`), by the optimised
  // reader resident (`exact-web-js design-serve`): the app compiled once per
  // committed design, each frame's folded rows set on that plan, compiling
  // nothing. Started at a scrub's first frame, again after the reader
  // builds again; a frame it cannot patch is compiled whole instead.
  const patcher = { child: null, built: null, asks: new Map(), n: 0 };
  const patcherAsk = (request) => new Promise((done) => {
    if (!toolFresh()) return done({ fallback: true });
    if (!patcher.child || patcher.built !== tool.built) {
      stopGroup(patcher.child);
      const child = patcher.child = spawn(tool.path, ['design-serve', resolve(app.dir, 'app.contract')], { cwd: root, stdio: ['pipe', 'pipe', 'ignore'], detached: true });
      patcher.built = tool.built;
      let buffered = '';
      child.stdin.on('error', () => {});
      child.stdout.on('data', (d) => {
        buffered += d;
        const lines = buffered.split('\n');
        buffered = lines.pop();
        for (const line of lines) { try { const r = JSON.parse(line); patcher.asks.get(r.id)?.(r); patcher.asks.delete(r.id); } catch { /* not a reply */ } }
      });
      child.on('exit', () => {
        if (patcher.child === child) patcher.child = null;
        for (const answer of patcher.asks.values()) answer({ fallback: true });
        patcher.asks.clear();
      });
    }
    const id = ++patcher.n;
    patcher.asks.set(id, done);
    patcher.child.stdin.write(`${JSON.stringify({ id, ...request })}\n`);
  });
  /** A scrub's frame patched: `{fallback}` when it must be compiled instead. */
  async function scrubPatch(values, frame, files) {
    // Folded over files the committed design was not built from (a write
    // whose build has not landed): compiled instead.
    if (!design || files?.[relative(app.dir, designPath(app.dir))] !== designFrom) return { fallback: true };
    const base = design, next = await patcherAsk({ key: `${base.shape} ${base.digest}`, patch: values.map(({ name, css }) => [name, css]) });
    if (!next.tokens || design !== base) return { fallback: true };
    // The committed design's rows, those it patched set again.
    const rows = new Map(next.tokens.map((t) => [t.name, t]));
    return scrubShow({ ...base, digest: next.digest, structure: next.structure ?? base.structure, tokens: base.tokens.map((t) => rows.get(t.name) ?? t) }, frame);
  }
  /** The patched frame shown, against its texts compiled whole: `{mismatch}`
   * naming the first token that differs. */
  async function scrubVerify(texts) {
    const file = relative(app.dir, designPath(app.dir)), shown = scrubbed;
    if (!(file in texts) || !shown) return {};
    const next = await scrubDesign(texts[file]);
    if (scrubbed !== shown) return {};
    if (next.error) return { error: next.error };
    const i = next.tokens.findIndex((t, i) => JSON.stringify(t) !== JSON.stringify(shown.tokens[i]));
    const mismatch = i >= 0 ? `\`${next.tokens[i].name}\`: compiled ${JSON.stringify(next.tokens[i])}, patched ${JSON.stringify(shown.tokens[i])}`
      : next.tokens.length !== shown.tokens.length || next.digest !== shown.digest || next.shape !== shown.shape || next.structure !== shown.structure ? 'the designs differ' : null;
    if (mismatch) console.log(`scrub: the patched frame is not its compile (${mismatch}); the pages get the committed design back`);
    return mismatch ? { mismatch } : {};
  }
  /** Preview a scrub's texts: a batch from the design the pages run, as a
   * value-only edit sends one, and its snapshot for a page that asks. */
  async function scrubPreview(texts, frame) {
    const file = relative(app.dir, designPath(app.dir));
    // Texts that leave the design file as it is: the committed design.
    if (!(file in texts)) { scrubRestore(frame); return {}; }
    const next = await scrubDesign(texts[file]);
    if (next.error) return next;
    return scrubShow(next, frame);
  }
  /** Show a scrub's design `next` on the pages, its batch and snapshot
   * marked `frame` (*1.10*). */
  function scrubShow(next, frame) {
    const from = scrubbed ?? design;
    if (!from || from.shape !== next.shape || from.tokens.length !== next.tokens.length
      || next.tokens.some((t, i) => t.name !== from.tokens[i].name || String(t.accent) !== String(from.tokens[i].accent))) return { error: 'it changes the design\'s shape: a rebuild applies on release' };
    // The plan's structure as a kept build needs it (the base scale's
    // declarations among it, `modes::structure`): a frame that moves it would
    // name what the page's sheet lacks.
    if (from.structure !== next.structure) return { error: 'it changes the plan\'s structure (the base scale\'s declarations): a rebuild applies on release' };
    // A token the page writes inline has no property to preview through.
    const inline = next.tokens.find((t, i) => t.inline && !same(t, from.tokens[i]));
    if (inline) return { error: `\`${inline.name}\` is written inline on this page (${inline.writer ?? 'a font weight, an SVG attribute'}): a rebuild applies on release` };
    if (!scrubbed) aside = snapshot;
    // A new preview is no released scrub's.
    scrubbed = next;
    released = false;
    snapshot = { kind: 'tamber-snapshot', structure: next.shape, base: null, design: next.digest, tokens: next.tokens, scrub: frame };
    const tokens = next.tokens.filter((t, i) => !same(t, from.tokens[i]));
    if (tokens.length) {
      const batch = framed({ kind: 'tamber-batch', structure: next.shape, base: from.digest, design: next.digest, tokens, scrub: frame });
      push(batch);
      tamber?.channel.broadcast(batch);
    }
    return {};
  }
  /** Why a live proposal's texts would not apply in place on these pages
   * (dev-inline.mjs): the built reader only, so a proposal never waits on a
   * compile; with none yet, a rebuild is assumed. */
  const inlineWhy = (texts) => inlineWhyOf({ texts, file: relative(app.dir, designPath(app.dir)), ready: toolReady,
    read: (text) => scrubDesign(text, { readOnly: true }), pages: () => scrubbed ?? design, same });
  /** A scrub ended without its write: the pages back on the committed design. */
  function scrubRestore(frame) {
    released = false;
    if (!scrubbed || !design) return;
    const from = scrubbed;
    scrubbed = null;
    // The committed design's snapshot (a page fetching it applies nothing new).
    snapshot = { ...(aside ?? { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens }), scrub: frame };
    aside = null; const kept = design.digest; setImmediate(() => gallery.designKept(kept)); // an edit kept meanwhile reaches the gallery, after the restore's batch
    const tokens = design.tokens.filter((t, i) => !same(t, from.tokens[i]));
    if (!tokens.length) return;
    const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens, scrub: frame });
    push(batch);
    tamber?.channel.broadcast(batch);
  }
  // The install pages (LLP 1030.003 D6a) and, from a loopback page on a Mac,
  // the local iOS build (D6b): this server's, since only it sees the real
  // peer (a forwarded request's is loopback).
  const installer = localInstaller({ app: () => app, origins, port, gate, listener: { host, port } });
  // The design server's wire (LLP 1121 D13), when the app has a design.
  // `page`: this loop's own origin, which the design tool frames (LLP 1122 T2).
  // The source maps of the plans the resident loop serves the natives
  // (LLP 1122 T2c), by plan digest. The child says each plan it publishes,
  // and its map's card, on a pipe this loop gave it for that alone (fd 3:
  // nothing a request makes is written there); this loop fetches the bytes
  // from the child and keeps them only when they match that card. No
  // request path takes a map in, and a process that took the child's port
  // could not make bytes match a card it did not write. The plan said last
  // is the running one; a pull overtaken by a later card of its plan is
  // dropped; the 8 plans said last are kept.
  const residentMaps = new Map(), saidAt = new Map();
  let residentPlan = null, said = 0;
  const residentMap = async (internal, card) => {
    const n = ++said;
    residentPlan = card.plan;
    saidAt.delete(card.plan); saidAt.set(card.plan, n);
    for (const plan of [...saidAt.keys()].slice(0, Math.max(0, saidAt.size - 8))) { saidAt.delete(plan); residentMaps.delete(plan); }
    if (!card.map) { residentMaps.delete(card.plan); return; }
    // (A card can come before the child listens: asked again, a while, while it is still the latest.)
    for (let tries = 0; tries < 20 && saidAt.get(card.plan) === n; tries++) {
      const got = await pullMap(internal, { plan: card.plan, ...card.map }, `127.0.0.1:${port}`);
      if (got) { if (saidAt.get(card.plan) === n) residentMaps.set(got.digest, got.bytes); return; }
      await new Promise((ok) => setTimeout(ok, 250));
    }
  };
  tamber = designPath(app.dir) ? tamberServer({ appDir: app.dir, pairTtlMs, remote: () => doors?.list() ?? [], live: true, page: `http://127.0.0.1:${port}`, sourceMap: (digest) => (digest ? residentMaps.get(digest) ?? pageMaps.get(digest) ?? gallery.map(digest) : residentMaps.get(residentPlan)) ?? null, designServed: () => (scrubbed ? null : served), scrub: { patch: scrubPatch, verify: scrubVerify, preview: scrubPreview, restore: scrubRestore, released: (frame) => { released = scrubbed != null && (frame ?? true); }, inline: inlineWhy } }) : null;
  // A polled stream (Linux, the resident loop's relay) is told the design to run.
  tamber?.channel.greet(() => snapshot && notice(snapshot));
  const server = createServer(async (req, res) => {
    // Every HTML answer is framed by the design tool only (LLP 1122 T2).
    frameOnlyByTool(res);
    // A sandboxed page's (a Figma plugin's) wire request, with --allow-plugin-origin (stage 10b): the wire's, before the gate.
    if (remoteAsked.plugin && tamber && await pluginOnLoopback(tamber, req, res)) return;
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    const url = webRequestURL(req.url);
    if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    if (tamber && await tamber.handle(req, res, url, access)) return;
    if (await installer.handle(req, res, url, access)) return;
    // The build channel (LLP 1122 D4): the page's stream gives it a nonce;
    // reports come up by POST; a build with no stream here (the resident
    // loop's devices, Linux) opens a polled one.
    // The page's inspect table, for the plan dist serves now, or (beside a page's generation base) the plan of the
    // build that page booted (host/web/generations.mjs).
    const inspectAt = generationPath(url.pathname);
    if ((url.pathname === '/dev-inspect.json' || (inspectAt && inspectAt !== 'bad' && inspectAt.path === '/dev-inspect.json')) && req.method === 'GET') {
      let digest = null, plan = resolve(dist, 'app.plan');
      if (inspectAt) {
        // That build's plan, read whole from it (generationFile: its own rules, no link out), into a file of this
        // loop's for the table's compiler.
        const kept = await generationFile(dist, inspectAt.id, '/app.plan');
        if (typeof kept === 'string' || !kept) { res.writeHead(kept === 'gone' ? 410 : kept === 'busy' ? 503 : 404, { 'cache-control': 'no-store' }); return res.end(); }
        digest = createHash('sha256').update(kept.body).digest('hex');
        // (a name of its own, made anew, never through a link there; gone once its table is made)
        plan = resolve(app.target, `inspect-plan-${digest}-${process.pid}-${randomBytes(6).toString('hex')}.plan`);
        writeFileSync(plan, kept.body, { flag: 'wx' });
      } else try { digest = createHash('sha256').update(readFileSync(plan)).digest('hex'); } catch { /* no build */ }
      const bytes = digest && await inspectTable(digest, plan).finally(() => { if (inspectAt) rmSync(plan, { force: true }); });
      if (!bytes) { res.writeHead(503, { 'cache-control': 'no-store' }); return res.end(); }
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      return res.end(bytes);
    }
    // Inspect mode's half in the page (LLP 1122 T2b, the JS target): the
    // dev client loads it; it is in no build.
    if (url.pathname === '/__dev/inspect.js' && (req.method === 'GET' || req.method === 'HEAD')) {
      res.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8', 'cache-control': 'no-store' });
      return res.end(req.method === 'HEAD' ? undefined : readFileSync(resolve(root, 'host/web-js/inspect-dev.js')));
    }
    if (await gallery.handle(req, res, url)) return; // the variant gallery's page, files and stream (dev-gallery.mjs)
    if (url.pathname === '/__dev/report' && req.method === 'POST') {
      if (!tamber) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
      await serveReport(tamber.channel, req, res);
      return;
    }
    if ((url.pathname === '/__dev/hello' || url.pathname === '/__dev/commands') && req.method === 'GET') {
      if (!tamber) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
      serveCommands(tamber.channel, url, res);
      return;
    }
    if (url.pathname === '/__dev/page') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      const opened = tamber?.channel.open(frame => res.write(`data: ${JSON.stringify(frame)}\n\n`));
      // Ended once, by whichever close comes first: under Bun a request on a socket handed to the server
      // (emit('connection'): the TLS door's, an old [::1] door's) never fires req 'close'; its res 'close' does.
      let done = false;
      const end = () => { if (done) return; done = true; opened?.close(); clients.delete(res); };
      req.on('close', end);
      res.on('close', end);
      res.write(`data: ${JSON.stringify({ ...(error ? { error } : {}), reload: seq, revision: logicRevision, ...(opened ? { nonce: opened.nonce } : {}) })}\n\n`);
      // The design the pages run, after value-only edits since the build.
      if (snapshot) res.write(`data: ${JSON.stringify(notice(snapshot))}\n\n`);
      clients.add(res);
      console.log(`page connected (${clients.size})`);
      return;
    }
    if (url.pathname === '/__dev/design' && (req.method === 'GET' || req.method === 'HEAD')) {
      if (!snapshot) { res.writeHead(204, { 'cache-control': 'no-store' }); res.end(); return; }
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : JSON.stringify(snapshot));
      return;
    }
    if (url.pathname === '/__dev/checkpoint' && req.method === 'POST') {
      const id = url.searchParams.get('id') ?? '', n = Number(url.searchParams.get('seq')), revision = url.searchParams.get('revision') ?? '', chunks = [];
      if (!/^\d+(?:-\d+){3}$/.test(id) || !Number.isSafeInteger(n)) { res.writeHead(400); res.end(); return; }
      // Bounded: a checkpoint is at most the hosts' 16 MiB surface carry, and the
      // loop keeps the newest few (a page reloads one at a time).
      let size = 0, over = false;
      req.on('data', chunk => { size += chunk.length; if (size > CHECKPOINT_BYTES) over = true; else chunks.push(chunk); });
      req.on('end', () => {
        if (over) { res.writeHead(413); res.end(); return; }
        try {
          const text = Buffer.concat(chunks).toString('utf8'); JSON.parse(text);
          checkpoints.delete(id); checkpoints.set(id, { seq: n, revision, text });
          while (checkpoints.size > CHECKPOINTS) checkpoints.delete(checkpoints.keys().next().value);
          res.writeHead(204); res.end();
        } catch { res.writeHead(400); res.end(); }
      });
      return;
    }
    if (url.pathname === '/__dev/reloaded') {
      const n = Number(url.searchParams.get('seq')), at = pending.get(n);
      if (at) {
        const total = Number(url.searchParams.get('at')) - at;
        console.log(`  → page: runtime up ${url.searchParams.get('boot')} ms after its script; edit → first frame in the DOM ${total} ms (budget ${budget}: a rebuild and a reload)`);
        console.log(`reloaded seq=${n} total_ms=${total}`);
      }
      res.writeHead(204); res.end(); return;
    }
    if (url.pathname === '/__exact/trace' && req.method === 'POST') return saveTrace(req, res, { app, dist, root });
    if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
    // A native client opening this URL (`build.mjs --url`, `/__dev/open`,
    // `exact run`) reads the envelope, the dev generations and their event
    // stream (LLP 1023), which the resident loop's producers make: forwarded
    // to it (below), started at the first such request.
    const discovery = url.pathname === '/exact.json' || (url.pathname === '/' && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'));
    if (discovery || url.pathname === '/__dev' || url.pathname === '/__dev/open' || url.pathname.startsWith('/__dev/generation/')) { forward(req, res, discovery); return; }
    // From the build the page booted (host/web/generations.mjs): a document names its generation in its base, and
    // its scripts, chunks and plan are read from that build while the next one is put in place.
    const served = await servedFile(dist, url.pathname, req.headers.accept);
    if (served.status === 410 || served.status === 400 || served.status === 503) { res.writeHead(served.status, { 'cache-control': 'no-store', ...(served.status === 503 ? { 'retry-after': '1' } : {}) }); res.end(); return; }
    // A miss is a 404, with the build's page for one where it has one (serve.mjs `buildTreeFile`).
    const found = served.body ? served : null;
    // No build yet (the first failed): a blank page that shows the errors and reloads when one lands.
    if (!found && error && (url.pathname === '/' || appDocumentPath(url.pathname, acceptsHTML(req.headers.accept)))) { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); res.end(`<!doctype html><meta charset="utf-8"><body>${client(seq, logicRevision)}`); return; }
    if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    let body = installer.page(found.route, found.body, access);
    if (found.route.endsWith('.html')) {
      body = body.toString();
      const id = url.searchParams.get('__exactDev'), carried = id && checkpoints.get(id);
      if (carried) {
        checkpoints.delete(id);
        if (carried.revision !== String(logicRevision ?? '')) {
          const c = JSON.parse(carried.text);
          if (c.carryAnswers || c.answers?.length) { c.carryAnswers = false; c.answers = []; carried.text = JSON.stringify(c); }
        }
        const checkpoint = `<script type="application/vnd.exact.dev-checkpoint" data-seq="${carried.seq}">${carried.text.replaceAll('<', '\\u003c')}</script><script>const u=new URL(location.href);u.searchParams.delete('__exactDev');history.replaceState(history.state,'',u.pathname+(u.search?'?'+u.searchParams:'')+u.hash)</script>`;
        body = body.replace('<script type="module"', checkpoint + '<script type="module"');
      }
      body += client(seq, logicRevision);
    }
    sendStaticBody(req, res, body, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' }, found.status);
  });
  // The resident loop's producers (host/web/dev.mjs `--serve-as`: the
  // resident compiler, the TypeScript and Rust module producers, the
  // envelope and generations) on a loopback port, into their own dist; a
  // native client's requests are forwarded as they came (Host included, so
  // the pages it serves name this URL). Its wasm build is internal.
  const residentLoop = () => resident ??= new Promise((ok, fail) => {
    // A named port's next one (an assigned range holds both), else a free one.
    const probe = createServer();
    listenOn(probe, residentPort).then(() => {
      const internal = probe.address().port;
      probe.close(() => {
        console.log(`native client: starting the resident loop's producers (loopback :${internal})`);
        // (one per dist, as the stage: its parent made here, since the publication's lock sits beside it)
        const residentDist = resolve(app.target, 'web-dist-resident', createHash('sha256').update(resolve(dist)).digest('hex').slice(0, 16));
        mkdirSync(dirname(residentDist), { recursive: true });
        // (EXACT_DEV_RESIDENT_SCRIPT: a test's stand-in resident)
        residentChild = spawn(process.execPath, [process.env.EXACT_DEV_RESIDENT_SCRIPT || resolve(root, 'host/web/dev.mjs'), '--app', app.name, '--wasm', '--port', String(internal), '--serve-as', String(port), ...(lan ? ['--lan'] : []), ...allowHosts.flatMap((name) => ['--allow-host', name])],
          { cwd: root, env: { ...process.env, EXACT_WEB_DIST: residentDist, EXACT_LAUNCHER_PID: String(process.pid), EXACT_MAP_FD: '3' }, stdio: ['ignore', 'pipe', 'pipe', 'pipe'] });
        // Its errors as before (piped, so a slow terminal holds it back), the
        // last of them kept: what a start that fails says to the client.
        let errors = '';
        residentChild.stderr.pipe(process.stderr, { end: false });
        residentChild.stderr.on('data', (d) => { errors = (errors + d).slice(-8000); });
        // Its plans and their maps' cards, on the pipe given for that alone.
        let cards = '';
        residentChild.stdio[3].on('data', (d) => {
          cards += d; const lines = cards.split('\n'); cards = lines.pop();
          for (const l of lines) { const card = mapCard(l); if (card) void residentMap(internal, card); }
        });
        let buf = '';
        residentChild.stdout.on('data', (d) => {
          buf += d; const lines = buf.split('\n'); buf = lines.pop();
          for (const l of lines) {
            console.log(`  [resident] ${l}`);
            // A step it said failed before its first plan is the start's error;
            // the resident stays, and builds again at an edit.
            if (l.startsWith(FAILED)) {
              if (!starting.ready) { starting.failure = l.slice(FAILED.length); for (const w of starting.waiting) w(); starting.waiting.clear(); }
            }
            else if (l.startsWith(PHASE)) starting.phase = l.slice(PHASE.length);
            if (/^(?:plan ready|module generation ready|Rust generation \w+ ready)/.test(l)) { starting.ready = true; starting.failure = null; ok(internal); }
          }
        });
        // At `close`, its last lines read: a failed step it said is the error.
        residentChild.on('close', (code) => {
          const why = `the resident loop exited ${code}`;
          if (!starting.ready) starting.failure ??= errors.trim() ? `${why}: ${gist(errors)}` : why;
          resident = null; residentChild = null; starting.ready = false; starting.phase = null; fail(new Error(why));
        });
      });
    }).catch(error => { resident = null; starting.failure = `the resident loop could not start: ${error.message}`; fail(error); });
  });
  const forward = (req, res, discovery = false) => {
    // Discovery while the resident starts is answered now with what it is
    // doing (application/vnd.exact.dev-status+json, llp/1023.001): not held
    // for the minutes a first build takes; the rest waits for it, as before.
    // After a start failed every request has its error, and none starts it
    // again until an edit.
    if (starting.failure || (discovery && !starting.ready)) {
      if (!starting.failure) void residentLoop().catch(() => {});
      const failed = starting.failure != null;
      const message = failed ? starting.failure : starting.phase ?? 'starting the dev loop\'s native producers (the first run builds them: a few minutes)';
      res.writeHead(failed ? 500 : 503, { 'content-type': DEV_STATUS_TYPE, 'cache-control': 'no-store', ...(failed ? {} : { 'retry-after': '2' }) });
      res.end(JSON.stringify({ exact: 1, status: failed ? 'failed' : 'preparing', message }));
      return;
    }
    const failedNow = (res) => {
      if (res.headersSent || res.destroyed) return;
      res.writeHead(500, { 'content-type': DEV_STATUS_TYPE, 'cache-control': 'no-store' });
      res.end(JSON.stringify({ exact: 1, status: 'failed', message: starting.failure ?? 'the resident loop stopped' }));
    };
    // A device gone ends its forwarded stream too (the resident's relay
    // stops), and one gone while the resident loop starts opens none.
    let out = null, gone = false;
    // A step the resident says failed while this waits answers it (the resident stays).
    const fail = () => failedNow(res);
    starting.waiting.add(fail);
    res.on('close', () => { gone = true; starting.waiting.delete(fail); out?.destroy(); });
    residentLoop().then((internal) => {
      starting.waiting.delete(fail);
      if (gone || res.destroyed || res.headersSent) return;
      out = request({ host: '127.0.0.1', port: internal, method: req.method, path: req.url, headers: req.headers }, (answer) => { res.writeHead(answer.statusCode, answer.headers); answer.pipe(res); });
      out.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
      req.pipe(out);
    }, () => { starting.waiting.delete(fail); if (!gone) failedNow(res); });
  };
  const stop = async () => {
    stopping = true;
    for (const w of watchers) w.close(); stopBuild('SIGKILL'); stopGroup(tool.child); stopGroup(tool.reading); stopGroup(tool.cleaner); stopGroup(patcher.child); residentChild?.kill('SIGTERM'); server.close(); server.closeAllConnections?.();
    // The design server lets go of its lock before this exits.
    await Promise.all([tamber?.close(), doors?.close(), gallery.stop()]);
    // A local iOS build in flight is stopped and waited for, as the resident loop does.
    const install = installer.child;
    if (install && install.exitCode === null && install.signalCode === null) { const exit = new Promise((ok) => install.once('exit', ok)); install.kill('SIGTERM'); await exit; }
    process.exit(process.exitCode ?? 0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  process.on('SIGHUP', stop);
  watchLauncher(stop);
  await listenDelay();
  // (and on [::1], for a localhost URL: the design tool's canvas)
  const six = await alsoOnIPv6Loopback(server, port).catch(async (e) => { console.error(`cannot listen on [::1]:${port}: ${e.code} (another server there would answer a localhost URL; --port <n> picks another)`); stopping = true; stopGroup(tool.child); stopGroup(tool.cleaner); await tamber?.close(); process.exit(1); });
  await new Promise((ok, fail) => { server.on('error', fail); server.listen(port, host, ok); })
    .catch(async (e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}${e.code === 'EADDRINUSE' ? ' (another dev loop? --port <n> picks another)' : ''}`); six?.close(); stopping = true; stopGroup(tool.child); stopGroup(tool.cleaner); await tamber?.close(); process.exit(1); });
  doors = tamber ? await openRemoteCanvas(tamber, remoteAsked, `http://127.0.0.1:${port}`).catch(async (e) => { console.error(`cannot listen: ${e.message}`); process.exitCode = 1; await stop(); }) : null;
  const urls = origins.map((o) => `${o.origin}/`);
  console.log(urls.join('\n'));
  console.log(urls.map(url => `  Open in native: ${url}__dev/open`).join('\n'));
  if (allowHosts.length) console.log(`  also answering to ${allowHosts.join(', ')} (--allow-host)`);
  console.log(`  (dev loop on the JS target: ${app.dir.replace(root + '/', '')} and host/web-js rebuild and reload the page; a native client's requests go to the resident loop's producers, started at the first; ${lan ? 'LAN bind — any peer on this network can read the app and its compile errors' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
  // Said once the server answers, not at the first build: a test or an agent takes it as the cue to connect.
  console.log('plan ready');
  listening = true; for (const line of held.splice(0)) console.log(line);
  await new Promise(() => {});
}
