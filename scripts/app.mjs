// Where an app lives. Inside this repo an app is `apps/<name>` — its crates
// are workspace members and build into `target/`. Outside it (weird-castle:
// its own repo, its own cargo workspace, depending on this repo's crates by
// path so the two iterate together), `EXACT_APP_DIR` names the directory and
// everything else follows from it: the workspace cargo runs in, the target
// directory the artifacts land in, `app.contract`, `assets/`, `gpu/`. Every
// script that builds, serves, or drives an app resolves it here, so nothing
// else knows the difference.
//
//   node host/web/build.mjs weird-castle-web          (EXACT_APP_DIR set)
//   node host/apple/build.mjs --ios weird-castle-apple --run
//   node host/web/dev.mjs --app weird-castle
//   node scripts/agent.mjs --app weird-castle macos tree
//
// The app manifest (LLP 1030 D2; 1030.000 D7): `app.json` beside
// `app.contract` — the W3C Web App Manifest's own keys, which the web host
// copies out as `manifest.json`, plus `app` (identity: the id every platform
// derives its bundle id from, the name, the origin), `host.<platform>` (what
// bake generates each platform's host files from), and `deploy` (policy,
// never identity). Validated here against `scripts/app.schema.json` with a
// validator small enough to live beside the reader; an app without one gets
// the derived defaults it had before the manifest existed.
import { existsSync, readFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The app `nameOrCrate` names (`caltrain`, `caltrain-web`, …; `EXACT_APP_DIR`'s basename when unset): its directory, cargo workspace, target directory, crate names, and manifest. */
export function resolveApp(nameOrCrate) {
  const outside = process.env.EXACT_APP_DIR ? resolve(process.env.EXACT_APP_DIR) : null;
  const name = nameOrCrate ? String(nameOrCrate).replace(/-(web|apple|linux|gpu)$/, '') : outside ? basename(outside) : 'caltrain';
  const dir = outside ?? resolve(ROOT, 'apps', name);
  if (!existsSync(resolve(dir, 'app.contract'))) throw new Error(`no app at ${dir} (no app.contract)${outside ? '' : '; set EXACT_APP_DIR for an app outside this repo'}`);
  const workspace = outside ? dir : ROOT;
  const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(workspace, 'target');
  const manifest = readManifest(dir, name);
  return {
    name, dir, workspace, target, crate: (kind) => `${name}-${kind}`,
    /** The manifest, validated; the derived defaults when the app has none. */
    manifest,
    /** The app identity, reverse-DNS: the bundle id on every platform (`build.mjs:48–49` derived it from the crate name before the manifest). */
    id: manifest.app.id,
    /** The name people see. */
    displayName: manifest.app.name,
    /** The production origin (1023 D1's URL), or null in an app that has not declared one. */
    origin: manifest.app.origin ?? null,
    /** Whether the app declares its own manifest (false: the defaults above stand in). */
    declared: existsSync(resolve(dir, 'app.json')),
  };
}

/** `app.json` from `dir`, validated — or the defaults an app had before the manifest: `com.exact.<name>`, the name capitalized. */
export function readManifest(dir, name) {
  const path = resolve(dir, 'app.json');
  const fallback = { name: name[0].toUpperCase() + name.slice(1), app: { id: `com.exact.${name}`, name: name[0].toUpperCase() + name.slice(1) }, host: {}, deploy: {} };
  if (!existsSync(path)) return fallback;
  let parsed;
  try { parsed = JSON.parse(readFileSync(path, 'utf8')); } catch (e) { throw new Error(`${path}: ${e.message}`); }
  const problems = validate(parsed, schema(), '', schema());
  if (problems.length) throw new Error(`${path} does not conform to scripts/app.schema.json:\n  ${problems.join('\n  ')}`);
  return { host: {}, deploy: {}, ...parsed };
}

let cachedSchema = null;
function schema() {
  cachedSchema ??= JSON.parse(readFileSync(resolve(ROOT, 'scripts/app.schema.json'), 'utf8'));
  return cachedSchema;
}

/** The subset of JSON Schema the manifest's schema uses — type, required, properties, additionalProperties, items, enum, pattern, minLength, oneOf, $ref into $defs — checked by hand so the reader needs no dependency. Every problem in one pass. */
export function validate(value, node, at, root) {
  const problems = [];
  const where = at || '(root)';
  if (node.$ref) {
    const target = node.$ref.replace(/^#\//, '').split('/').reduce((o, k) => o?.[k], root);
    if (!target) return [`${where}: schema reference ${node.$ref} does not resolve`];
    return validate(value, target, at, root);
  }
  if (node.oneOf) {
    const fits = node.oneOf.filter((alt) => validate(value, alt, at, root).length === 0);
    if (fits.length !== 1) problems.push(`${where}: ${JSON.stringify(value)} matches ${fits.length} of the allowed forms (needs exactly one)`);
    return problems;
  }
  const types = node.type ? [].concat(node.type) : null;
  const actual = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (types && !types.includes(actual)) { problems.push(`${where}: expected ${types.join(' or ')}, got ${actual}`); return problems; }
  if (node.enum && !node.enum.includes(value)) problems.push(`${where}: ${JSON.stringify(value)} is not one of ${node.enum.map((e) => JSON.stringify(e)).join(', ')}`);
  if (typeof value === 'string') {
    if (node.minLength != null && value.length < node.minLength) problems.push(`${where}: shorter than ${node.minLength}`);
    if (node.pattern && !new RegExp(node.pattern).test(value)) problems.push(`${where}: ${JSON.stringify(value)} does not match ${node.pattern}`);
  }
  if (actual === 'array' && node.items) value.forEach((v, i) => problems.push(...validate(v, node.items, `${at}[${i}]`, root)));
  if (actual === 'object') {
    for (const key of node.required ?? []) if (!(key in value)) problems.push(`${where}: missing required ${JSON.stringify(key)}`);
    for (const [key, v] of Object.entries(value)) {
      const sub = node.properties?.[key];
      const path = at ? `${at}.${key}` : key;
      if (sub) problems.push(...validate(v, sub, path, root));
      else if (node.additionalProperties && typeof node.additionalProperties === 'object') problems.push(...validate(v, node.additionalProperties, path, root));
      else if (node.additionalProperties === false) problems.push(`${path}: not a known key`);
    }
  }
  return problems;
}

/** Developer entrypoints explicitly bake unsigned-update permission. Direct Cargo/contract bakes default to production; release callers can select it here too. */
export function developmentBuildEnv() {
  return { ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' };
}
