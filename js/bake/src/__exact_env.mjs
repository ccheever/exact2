// The settings an app reads at build (LLP 1153 D4): `import.meta.env.NAME` in a top-level const of app.ts, each name
// declared in app.json's `env`, its value from the environment, then `.env.local`, then `.env` beside app.json, else
// the const's `??` default. The value is inlined into the captured copy, so every host runs the same module. The
// native bake writes this file into its stage beside `__exact_config.mjs`, which imports it
// (js/bake/src/resident.rs `ENV`); the web build and the scripts import it from here.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { parseEnv } from 'node:util';
import { dlopen, FFIType } from 'bun:ffi';

/** The files a value is read from after the environment, in order. */
export const ENV_FILES = ['.env.local', '.env'];
const NAME = /^[A-Z][A-Z0-9_]*$/;

/** The names app.json's `env` declares in the app directory `app` (none without one). */
export function declaredSettings(app) {
  let manifest = {};
  try { manifest = JSON.parse(readFileSync(resolve(app, 'app.json'), 'utf8')); } catch { /* no manifest: none declared */ }
  return [...new Set(Array.isArray(manifest?.env) ? manifest.env : [])];
}

// A file's assignments in Node's dotenv grammar (`util.parseEnv`: `NAME=value`, `#` comments, quoted values, taken as
// written, with no `$` expansion; https://nodejs.org/api/util.html#utilparseenvcontent). An absent file assigns none.
const assignments = (app, file) => { const path = resolve(app, file); return existsSync(path) ? parseEnv(readFileSync(path, 'utf8')) : {}; };

// Bun loads `.env` files from its working directory into `process.env` before a script's first line runs
// (https://bun.sh/docs/runtime/env): `.env.local` (except when NODE_ENV is `test`), then `.env.<NODE_ENV, else
// development>`, then `.env`, never over a variable already set. Every process a script starts inherits them as its
// own environment, and `--no-env-file` on a child cannot take them out. The environment a process was started with is
// still there to read, untouched by that loading: the C library's (getenv(3); on Linux /proc/self/environ, proc(5)),
// and on Windows the process's environment block (GetEnvironmentVariableW, which a script's own assignment to
// `process.env` does reach, so only names in Bun's files are asked of it). So the first exact2 script of a command
// (scripts/app.mjs and the app's launcher call `noteBunLoaded`) notes in EXACT_BUN_LOADED each name Bun loaded, one set
// in `process.env` but not in the environment it was given, and every process below inherits the note: those names'
// values are Bun's, never the environment's, whatever the files say later (a dev loop builds long after it started).
// A variable the command was given is the environment's whether or not a file holds the same value. On any other
// system nothing is noted, and a value Bun loaded counts as the environment's.
const bunLoaded = (dir, env) => {
  const mode = env.NODE_ENV || 'development';
  return Object.assign({}, ...['.env', `.env.${mode}`, ...mode === 'test' ? [] : ['.env.local']].map(file => assignments(dir, file)));
};
// Whether the environment this process was started with holds a name; null where it cannot be read.
let startedWith;
const given = () => {
  if (startedWith !== undefined) return startedWith;
  startedWith = null;
  try {
    if (process.platform === 'linux') {
      const names = new Set(readFileSync('/proc/self/environ', 'latin1').split('\0').map(pair => pair.slice(0, pair.indexOf('='))));
      startedWith = name => names.has(name);
    } else if (process.platform === 'darwin') {
      const { symbols } = dlopen('/usr/lib/libSystem.B.dylib', { getenv: { args: [FFIType.cstring], returns: FFIType.cstring } });
      startedWith = name => symbols.getenv(Buffer.from(`${name}\0`)) !== null;
    } else if (process.platform === 'win32') {
      // With no buffer, the length a value needs: 0 for a name the block lacks.
      const { symbols } = dlopen('kernel32.dll', { GetEnvironmentVariableW: { args: [FFIType.ptr, FFIType.ptr, FFIType.u32], returns: FFIType.u32 } });
      startedWith = name => symbols.GetEnvironmentVariableW(Buffer.from(`${name}\0`, 'utf16le'), null, 0) > 0;
    }
  } catch { /* unreadable: nothing is noted */ }
  return startedWith;
};
// The names Bun loaded into this process from its working directory's `.env` files.
const loadedHere = () => {
  const candidates = Object.keys(bunLoaded(process.cwd(), process.env)).filter(name => process.env[name] !== undefined);
  const has = candidates.length ? given() : null;
  return has ? candidates.filter(name => !has(name)) : [];
};
// The names whose values in `env` are Bun's: those noted above, and, in `process.env`, those this process loaded.
const bunNames = env => new Set([...(env.EXACT_BUN_LOADED ?? '').split(',').filter(Boolean), ...env === process.env ? loadedHere() : []]);

/** Adds to EXACT_BUN_LOADED, which every process this one starts inherits, the names Bun loaded into this process from
 * `.env` files. Each exact2 script calls it before it starts a process (scripts/app.mjs on import), so the first
 * one's note reaches every process below it. */
export function noteBunLoaded() {
  process.env.EXACT_BUN_LOADED = [...bunNames(process.env)].sort().join(',');
}

/** Each of `names` with its value and source (`environment`, `.env.local` or `.env`), or null where none sets it. */
export function settingValues(app, names, env = process.env) {
  const files = Object.fromEntries(ENV_FILES.map(file => [file, assignments(app, file)])), bun = bunNames(env);
  return Object.fromEntries(names.map(name => {
    if (env[name] !== undefined && !bun.has(name)) return [name, { value: env[name], source: 'environment' }];
    const file = ENV_FILES.find(f => Object.hasOwn(files[f], name));
    return [name, file ? { value: files[file][name], source: file } : null];
  }));
}

/** The web build's input digest entries for the app `app` (its directory and manifest): each declared name, keyed
 * `import.meta.env.NAME`, with what the bake inlines for it (its value and source, a default included) as JSON, or
 * null where it would refuse. `parse` (Rolldown's) reads app.ts's defaults; without it, a default is not seen. */
export function settingInputs(app, env = process.env, parse = null) {
  const names = [...new Set(app.manifest?.env ?? [])], values = settingValues(app.dir, names, env);
  let record = {};
  try { if (parse && names.length) record = inlineSettings(readFileSync(resolve(app.dir, 'app.ts'), 'utf8'), app.dir, parse, env).record; } catch { /* refused, or no app.ts: what the sources give */ }
  return Object.fromEntries(names.map(name => { const setting = record[name] ?? values[name]; return [`import.meta.env.${name}`, setting ? JSON.stringify(setting) : null]; }));
}

const WRAPPERS = ['TSNonNullExpression', 'ParenthesizedExpression', 'ChainExpression', 'TSAsExpression', 'TSSatisfiesExpression', 'TSTypeAssertion'];
const bare = node => { while (node && WRAPPERS.includes(node.type)) node = node.expression; return node; };
const key = node => { const p = bare(node.property); return !node.computed ? p?.name ?? null : p?.type === 'Literal' && typeof p.value === 'string' ? p.value : null; };
const metaEnv = node => node?.type === 'MemberExpression' && key(node) === 'env' && bare(node.object)?.type === 'MetaProperty' && bare(node.object).meta?.name === 'import';
const literal = node => node?.type === 'Literal' && typeof node.value === 'string' ? node.value
  : node?.type === 'TemplateLiteral' && !node.expressions.length ? node.quasis[0].value.cooked : null;
// Every `import.meta.env` in `ast`, and a position's `line:col` in `code`.
const metaEnvs = (ast) => {
  const found = [];
  const walk = node => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) { node.forEach(walk); return; }
    if (metaEnv(node)) found.push(node);
    for (const k in node) if (k !== 'parent') walk(node[k]);
  };
  walk(ast);
  return found;
};
const lineColumn = code => offset => { const lines = code.slice(0, offset).split(/\r\n?|\n|\u2028|\u2029/); return `${lines.length}:${lines.at(-1).length + 1}`; };
const READ = 'import.meta.env is read only in a top-level const of app.ts: const API = import.meta.env.API_BASE_URL, with ?? \'a default\' or without, then API where it is used';

/** `code` (app.ts) with each top-level `const X = import.meta.env.NAME`, or `… ?? 'default'`, holding its value, and
 * `record`, each name's value and source (`default` where the default was taken). Every refusal is said at once: a
 * read anywhere else, a name app.json's `env` does not declare, one it declares that app.ts never reads, one with no
 * value and no default. `parse` is Rolldown's. */
export function inlineSettings(code, app, parse, env = process.env) {
  const declared = declaredSettings(app);
  if (!declared.length && !/\bimport\b/.test(code)) return { code, record: {} };
  const parsed = parse('app.ts', code);
  // The type check says what does not parse.
  if (parsed.errors?.length) return { code, record: {} };
  const at = lineColumn(code);
  const refusals = declared.filter(name => typeof name !== 'string' || !NAME.test(name)).map(name => `app.json's "env": ${JSON.stringify(name)} is not a setting's name (import.meta.env names are [A-Z][A-Z0-9_]*)`);
  const reads = [], allowed = new Set();
  for (const statement of parsed.program.body) {
    const s = statement.type === 'ExportNamedDeclaration' ? statement.declaration : statement;
    if (s?.type !== 'VariableDeclaration' || s.kind !== 'const') continue;
    for (const d of s.declarations) {
      const init = bare(d.init), fallback = init?.type === 'LogicalExpression' && init.operator === '??' ? bare(init.right) : null;
      const read = fallback ? bare(init.left) : init;
      if (d.id.type !== 'Identifier' || read?.type !== 'MemberExpression' || !metaEnv(bare(read.object)) || key(read) === null) continue;
      allowed.add(bare(read.object).start);
      reads.push({ name: key(read), node: d.init, fallback, at: at(read.start) });
    }
  }
  for (const node of metaEnvs(parsed.program)) if (!allowed.has(node.start)) refusals.push(`app.ts:${at(node.start)}: ${READ}`);
  // Read twice, a name takes the one default: else the record would say one value and the module hold two.
  const said = (fallback) => (fallback ? JSON.stringify(literal(fallback)) : 'no default'), first = new Map();
  for (const read of reads) {
    const seen = first.get(read.name);
    if (!seen) first.set(read.name, read);
    else if (said(seen.fallback) !== said(read.fallback)) refusals.push(`app.ts:${read.at}: import.meta.env.${read.name} is read again with ${said(read.fallback)}, and at app.ts:${seen.at} with ${said(seen.fallback)}: read it once, or give every read the same default`);
  }
  for (const { name, at } of reads) if (!declared.includes(name)) refusals.push(`app.ts:${at}: import.meta.env.${name} is not declared: add "${name}" to app.json's "env" (every declared value is public: it ships in the module on every host)`);
  for (const name of declared) if (NAME.test(name) && !reads.some(r => r.name === name)) refusals.push(`import.meta.env.${name}: app.json's "env" declares it, and app.ts reads it nowhere: read it as const X = import.meta.env.${name}, or remove it from "env"`);
  const values = settingValues(app, declared.filter(name => reads.some(r => r.name === name)), env), record = {}, edits = [];
  for (const { name, node, fallback, at } of reads) {
    if (!declared.includes(name)) continue;
    const given = fallback ? literal(fallback) : null;
    if (fallback && given === null) { refusals.push(`app.ts:${at}: import.meta.env.${name}'s default after ?? must be a string literal`); continue; }
    const setting = values[name] ?? (fallback ? { value: given, source: 'default' } : null);
    if (!setting) { if (!refusals.some(r => r.startsWith(`import.meta.env.${name} has no value`))) refusals.push(`import.meta.env.${name} has no value: set it in the environment, in .env.local or in .env beside app.json (read in that order), or write a default: import.meta.env.${name} ?? '…'`); continue; }
    record[name] ??= setting;
    edits.push([node.start, node.end, JSON.stringify(setting.value)]);
  }
  if (refusals.length) throw new Error(refusals.join('\n'));
  for (const [start, end, value] of edits.sort((a, b) => b[0] - a[0])) code = code.slice(0, start) + value + code.slice(end);
  return { code, record };
}

/** Each `import.meta.env` in the captured module `file`, the app's or a package's: once `inlineSettings` has run,
 * app.ts has none left, so any is a read where none is allowed. `where(':line:col')` names the place, as the capture
 * names it (`path:line:col`, or `name@version (path:line:col)` in a package). */
export function strayReads(file, code, parse, where, inPackage = false) {
  if (!/\bimport\b/.test(code) || /\.d\.[cm]?ts$/.test(file) || !/\.[cm]?[jt]sx?$/.test(file)) return [];
  const lang = /\.[cm]?tsx?$/.test(file) ? (file.endsWith('x') ? 'tsx' : 'ts') : 'js', at = lineColumn(code);
  return metaEnvs(parse(code, { lang })).map(node => inPackage
    ? `${where(`:${at(node.start)}`)} reads import.meta.env, which only app.ts may: app.ts reads the setting (const API = import.meta.env.API_BASE_URL) and passes it in`
    : `${where(`:${at(node.start)}`)}: ${READ}`);
}

/** Each setting of `record` (name to the value and source a build inlined) whose value its sources give otherwise
 * now, as `{ name, built, now }` (a setting, or null where none is set). With `app`, the app's directory beside the
 * build, each is read again from `env` and the app's files; without it (a build copied away from them) only `env` can
 * change one, and a name it leaves unset keeps the build's value. */
export function settingChanges(record, env = process.env, app = null) {
  const read = app ? settingValues(app, Object.keys(record), env) : {}, bun = bunNames(env);
  return Object.entries(record).flatMap(([name, was]) => {
    const now = app ? read[name] ?? (was?.source === 'default' ? was : null)
      : env[name] !== undefined && !bun.has(name) ? { value: env[name], source: 'environment' } : was;
    return now?.value === was?.value ? [] : [{ name, built: was, now }];
  });
}

/** `settingChanges` for the build in `dist`, from what its `.exact-build.json` says it inlined (`env`). */
export function changedSettings(dist, env = process.env, app = null) {
  let built = {};
  try { built = JSON.parse(readFileSync(resolve(dist, '.exact-build.json'), 'utf8')).env ?? {}; } catch { /* no record: none inlined */ }
  return settingChanges(built, env, app);
}

/** What the bake in `baked` inlined (its receipt's `env`), as a native build's record keeps it beside the app's
 * directory `app`, for `pendingBuildInputs`: nothing when it read no setting. */
export function bakedSettings(baked, app) {
  let env = null;
  try { env = JSON.parse(readFileSync(resolve(baked, 'app.module.json'), 'utf8')).env ?? null; } catch { /* no TypeScript module */ }
  return env ? { settings: { dir: app, env } } : {};
}

// A native build's settings (a Linux or Android executable, the Canvas reader's APK) are bound to the executable a
// successful build made: `<executable>.settings.json` holds its digest beside what its bake inlined. The bake's own
// receipt cannot stand in, as an interrupted rebuild writes a new one and leaves the old executable.
const digest = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');

/** After `executable` of the app `app` (its directory and manifest) built: what its bake inlined of each setting,
 * bound to it by its digest. `baked` is the bake's output directory, where the build knows it; else the one the
 * executable's dep-info names (`<executable>.d`), whose bytecode it embeds. An app that declares no setting has none. */
export function recordBuiltSettings(executable, app, baked = null) {
  if (!app.manifest?.env?.length) return;
  if (!baked) {
    let line = '';
    try { line = readFileSync(`${executable}.d`, 'utf8').split('\n')[0]; } catch { /* no dep-info */ }
    const bytecode = line.slice(line.indexOf(': ') + 2).split(/(?<!\\) /).map((path) => path.replace(/\\ /g, ' ')).find((path) => /\/build\/[^/]+\/out\/app\.hbc$/.test(path));
    baked = bytecode ? dirname(bytecode) : null;
  }
  const env = baked ? bakedSettings(baked, app.dir).settings?.env ?? {} : {};
  writeFileSync(`${executable}.settings.json`, JSON.stringify({ sha256: digest(executable), env }));
}

/** What the build `executable` inlined of each setting (`recordBuiltSettings`), or null where no record is bound to
 * it: none written, or one written for another build of it. */
export function boundSettings(executable) {
  try {
    const record = JSON.parse(readFileSync(`${executable}.settings.json`, 'utf8'));
    return record.sha256 === digest(executable) ? record.env : null;
  } catch { return null; }
}

/** What a web build in `out` inlined, as its completion marker's `env`: on the JS target, `configure`'s record in its
 * stage (`host/web-js/build.mjs`); on the wasm target, the receipt of the module paired with it. Nothing when it read
 * no setting. */
export function builtSettings(out) {
  let record = {};
  try { record = JSON.parse(readFileSync(resolve(out, '.gen/typescript/__exact_env.json'), 'utf8')); } catch {
    try { record = JSON.parse(readFileSync(resolve(out, 'app.module.json'), 'utf8')).env ?? {}; } catch { /* no data module */ }
  }
  return Object.keys(record).length ? { env: record } : {};
}

/** What the agent's `logs` and the build say of each setting a build inlined: `import.meta.env.NAME = "value" (source)`. */
export const settingLines = (record) => Object.entries(record ?? {}).map(([name, { value, source }]) => `import.meta.env.${name} = ${JSON.stringify(value)} (${source})`);
