// The settings an app reads at build (LLP 1153 D4): `import.meta.env.NAME` in a top-level const of app.ts, each name
// declared in app.json's `env`, its value from the environment, then `.env.local`, then `.env` beside app.json, else
// the const's `??` default. The value is inlined into the captured copy, so every host runs the same module. The
// native bake writes this file into its stage beside `__exact_config.mjs`, which imports it
// (js/bake/src/resident.rs `ENV`); the web build and the scripts import it from here.
import { existsSync, readFileSync } from 'node:fs';
import { relative, resolve, sep } from 'node:path';
import { parseEnv } from 'node:util';

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

// Bun loads `.env` files from its working directory into the environment before a script's first line runs
// (https://bun.sh/docs/runtime/env): `.env.local` (except when NODE_ENV is `test`), then `.env.<NODE_ENV, else
// development>`, then `.env`, never over a variable already set. So `bun exact.mjs` run in the app carries the app's
// files in `process.env`, and every process it starts inherits them; `--no-env-file` on a child cannot remove them,
// and no script runs before the loading to read the environment without them. Instead the first exact2 script of a
// command notes the names Bun loaded while the files still hold what Bun read (`noteBunLoaded`, called by
// scripts/app.mjs and the app's launcher, into EXACT_BUN_LOADED, which every process below inherits): those names'
// inherited values are Bun's, however the files have changed since (a dev loop builds long after it started). Any
// value equal to what Bun's loading of the app's files gives now is taken as Bun's too, which covers a command no
// exact2 script started. Either way a variable set to that very value is said to come from the file: the source named
// differs, not the value.
const bunLoaded = (dir, env) => {
  const mode = env.NODE_ENV || 'development';
  return Object.assign({}, ...['.env', `.env.${mode}`, ...mode === 'test' ? [] : ['.env.local']].map(file => assignments(dir, file)));
};

/** Notes in `env` the names Bun loaded from `cwd`'s `.env` files, unless a process above this one has. */
export function noteBunLoaded(env = process.env, cwd = process.cwd()) {
  if (env.EXACT_BUN_LOADED !== undefined) return;
  const loaded = bunLoaded(cwd, env);
  env.EXACT_BUN_LOADED = Object.keys(loaded).filter(name => env[name] === loaded[name]).join(',');
}

/** Each of `names` with its value and source (`environment`, `.env.local` or `.env`), or null where none sets it. */
export function settingValues(app, names, env = process.env) {
  const files = Object.fromEntries(ENV_FILES.map(file => [file, assignments(app, file)])), bun = bunLoaded(app, env);
  const noted = new Set((env.EXACT_BUN_LOADED ?? '').split(','));
  return Object.fromEntries(names.map(name => {
    if (env[name] !== undefined && !noted.has(name) && env[name] !== bun[name]) return [name, { value: env[name], source: 'environment' }];
    const file = ENV_FILES.find(f => Object.hasOwn(files[f], name));
    return [name, file ? { value: files[file][name], source: file } : null];
  }));
}

/** The web build's input digest entries for the app `app` (its directory and manifest): each declared name, keyed
 * `import.meta.env.NAME`, with its value and source as JSON, or null when unset. */
export const settingInputs = (app, env = process.env) => Object.fromEntries(Object.entries(settingValues(app.dir, [...new Set(app.manifest?.env ?? [])], env))
  .map(([name, setting]) => [`import.meta.env.${name}`, setting && JSON.stringify(setting)]));

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
  if (!declared.length && !/\bimport\s*\.\s*meta\b/.test(code)) return { code, record: {} };
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

/** Each `import.meta.env` in the captured module `file` under `stage`, as `path:line:col: why`: once `inlineSettings`
 * has run, app.ts has none left, so any is a read where none is allowed. */
export function strayReads(stage, file, code, parse) {
  if (!/\bimport\s*\.\s*meta\b/.test(code) || /\.d\.[cm]?ts$/.test(file) || !/\.[cm]?[jt]sx?$/.test(file)) return [];
  const lang = /\.[cm]?tsx?$/.test(file) ? (file.endsWith('x') ? 'tsx' : 'ts') : 'js', label = relative(stage, file).split(sep).join('/');
  const at = lineColumn(code);
  return metaEnvs(parse(code, { lang })).map(node => `${label}:${at(node.start)}: ${READ}`);
}

/** The settings whose value differs from what the build in `dist` inlined (its `.exact-build.json`'s `env`), each as
 * `{ name, built, now }` (a setting, or null where none is set). With `app`, the app's directory beside the build,
 * each is read again from its sources; without it (a build copied away from them) only `env` can change one, and a
 * name it leaves unset keeps the build's value. */
export function changedSettings(dist, env = process.env, app = null) {
  let built = {};
  try { built = JSON.parse(readFileSync(resolve(dist, '.exact-build.json'), 'utf8')).env ?? {}; } catch { /* no record: none inlined */ }
  const read = app ? settingValues(app, Object.keys(built), env) : {};
  return Object.entries(built).flatMap(([name, was]) => {
    const now = app ? read[name] ?? (was?.source === 'default' ? was : null) : env[name] !== undefined ? { value: env[name], source: 'environment' } : was;
    return now?.value === was?.value ? [] : [{ name, built: was, now }];
  });
}

/** What a JS build in `out` inlined (`host/web-js/build.mjs` keeps `configure`'s record in its stage), as its
 * completion marker's `env`: nothing when it read no setting. */
export function builtSettings(out) {
  let record = {};
  try { record = JSON.parse(readFileSync(resolve(out, '.gen/typescript/__exact_env.json'), 'utf8')); } catch { /* no data module */ }
  return Object.keys(record).length ? { env: record } : {};
}

/** What the agent's `logs` and the build say of each setting a build inlined: `import.meta.env.NAME = "value" (source)`. */
export const settingLines = (record) => Object.entries(record ?? {}).map(([name, { value, source }]) => `import.meta.env.${name} = ${JSON.stringify(value)} (${source})`);
