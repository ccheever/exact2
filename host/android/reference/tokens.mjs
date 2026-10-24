// The Material column's sources (LLP 1115.001.001 D2): Compose Material 3's sources jar at the pinned version, fetched
// from Google Maven and checked against the SHA-256 it publishes, read for its generated tokens and the constants
// `rows.json` names, and held against the schema's Material column.
//
//   bun host/android/reference/tokens.mjs                 check every Material row against the pinned source
//   bun host/android/reference/tokens.mjs --diff <old>    what changed from version <old> to the pin (D8)
//
// A `defaults` row is checked only where the reference has run (`InputsTest` writes `defaults.json`). The snapshot of
// every token, resolved, is written to build/tokens/<version>.json. Exits 1 when a row disagrees with its source.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

const here = dirname(new URL(import.meta.url).pathname);
const root = resolve(here, '../../..');
const build = join(here, 'build');
const pinned = readFileSync(join(root, 'host/android/gradle/libs.versions.toml'), 'utf8').match(/^material3 = "([^"]+)"/m)[1];
const args = process.argv.slice(2);
const old = args.includes('--diff') ? args[args.indexOf('--diff') + 1] : null;

const now = await read(pinned);
writeFileSync(join(mkdir(join(build, 'tokens')), `${pinned}.json`), JSON.stringify(now.tokens, null, 1));
if (old) {
  diff(await read(old), now, old);
} else {
  process.exit(check(now) ? 0 : 1);
}

/** A version's sources: its token objects resolved, and every file's text by its path under `commonMain/…/material3/`. */
async function read(version) {
  const jar = await fetchJar(version);
  const list = spawnSync('unzip', ['-Z1', jar], { encoding: 'utf8' }).stdout.split('\n');
  const prefix = 'commonMain/androidx/compose/material3/';
  const files = {};
  for (const name of list.filter((n) => n.startsWith(prefix) && n.endsWith('.kt'))) {
    files[name.slice(prefix.length)] = spawnSync('unzip', ['-p', jar, name], { encoding: 'utf8', maxBuffer: 1 << 26 }).stdout;
  }
  const objects = {};
  for (const [path, text] of Object.entries(files)) if (path.startsWith('tokens/')) Object.assign(objects, parseObjects(text));
  const tokens = {};
  for (const [object, members] of Object.entries(objects)) {
    tokens[object] = {};
    for (const name of Object.keys(members)) tokens[object][name] = resolveToken(objects, `${object}.${name}`);
  }
  return { version, files, objects, tokens };
}

async function fetchJar(version) {
  const dir = mkdir(join(build, 'sources'));
  const file = join(dir, `material3-android-${version}-sources.jar`);
  if (existsSync(file)) return file;
  const url = `https://dl.google.com/android/maven2/androidx/compose/material3/material3-android/${version}/material3-android-${version}-sources.jar`;
  const [jar, sum] = await Promise.all([fetch(url), fetch(`${url}.sha256`)]);
  if (!jar.ok || !sum.ok) throw new Error(`Google Maven has no material3-android ${version} sources (${jar.status}, ${sum.status})`);
  const bytes = Buffer.from(await jar.arrayBuffer());
  const want = (await sum.text()).trim().split(/\s/)[0];
  const got = createHash('sha256').update(bytes).digest('hex');
  if (got !== want) throw new Error(`material3-android ${version} sources: SHA-256 ${got}, Google Maven says ${want}`);
  writeFileSync(file, bytes);
  return file;
}

/** Each `object` (and `enum class`) of a generated tokens file: its members' value text, joined across lines. */
function parseObjects(text) {
  const out = {};
  for (const m of text.matchAll(/^internal (?:object|enum class) (\w+)\s*\{([\s\S]*?)^\}/gm)) {
    const members = {};
    const body = m[2];
    const decls = [...body.matchAll(/^ {4}(?:const )?val (\w+)\s*=\s*/gm)];
    decls.forEach((d, i) => {
      const end = i + 1 < decls.length ? decls[i + 1].index : body.length;
      members[d[1]] = body.slice(d.index + d[0].length, end).replace(/\s+/g, ' ').trim().replace(/,\s*\)$/, ')');
    });
    out[m[1]] = members;
  }
  return out;
}

/** A token's value: dp, sp, a number, a shape's four corners (start, end, bottom end, bottom start), or what it names. */
function resolveToken(objects, ref, depth = 0) {
  const [object, name] = ref.split('.');
  const text = objects[object]?.[name];
  if (text == null) {
    // A key token (`ShapeKeyTokens.CornerSmall`) stands for the scale's member of its name.
    if (object === 'ShapeKeyTokens' && objects.ShapeTokens?.[name]) return resolveToken(objects, `ShapeTokens.${name}`, depth + 1);
    return { ref };
  }
  return value(objects, text, depth);
}

function value(objects, text, depth) {
  let m;
  if ((m = text.match(/^(-?[\d.]+)\.dp$/))) return { dp: +m[1] };
  if ((m = text.match(/^(-?[\d.]+)\.sp$/))) return { sp: +m[1] };
  if ((m = text.match(/^(-?[\d.]+)f$/)) || (m = text.match(/^(-?\d+(?:\.\d+)?)$/))) return { number: +m[1] };
  if (text === 'CircleShape') return { corners: 'full' };
  if (text === 'RectangleShape') return { corners: [0, 0, 0, 0] };
  if ((m = text.match(/^RoundedCornerShape\(([\d.]+)\.dp\)$/))) return { corners: [+m[1], +m[1], +m[1], +m[1]] };
  if ((m = text.match(/^RoundedCornerShape\((.*)\)$/))) {
    const named = Object.fromEntries([...m[1].matchAll(/(\w+) = ([\d.]+)\.dp/g)].map((x) => [x[1], +x[2]]));
    if (['topStart', 'topEnd', 'bottomEnd', 'bottomStart'].every((k) => k in named)) {
      return { corners: [named.topStart, named.topEnd, named.bottomEnd, named.bottomStart] };
    }
  }
  if ((m = text.match(/^(\w+Tokens)\.(\w+)$/)) && depth < 8) {
    const target = resolveToken(objects, `${m[1]}.${m[2]}`, depth + 1);
    return m[1] === 'ColorSchemeKeyTokens' || m[1] === 'TypographyKeyTokens' ? { ref: text } : target;
  }
  return { expr: text };
}

/** Each Material row of the schema against its source. True when all agree. */
function check({ files, objects }) {
  const schema = JSON.parse(readFileSync(join(root, 'kernel/tables/schema.json'), 'utf8'));
  const material = Object.fromEntries(schema.metrics.filter((r) => r.length === 7 && r[6] !== 'unset').map((r) => [r[0], r[6]]));
  const { rows } = JSON.parse(readFileSync(join(here, 'rows.json'), 'utf8'));
  const defaultsFile = join(build, 'oracle/defaults.json');
  const defaults = existsSync(defaultsFile) ? JSON.parse(readFileSync(defaultsFile, 'utf8')) : null;
  let ok = true;
  for (const name of Object.keys(material)) if (!rows[name]) { console.log(`  ${name}: no source in rows.json`); ok = false; }
  for (const [name, src] of Object.entries(rows)) {
    const want = material[name];
    let got, from;
    if (src.token) {
      const v = resolveToken(objects, src.token);
      got = src.corner ? v.corners?.[['topStart', 'topEnd', 'bottomEnd', 'bottomStart'].indexOf(src.corner)] : v.dp ?? v.number;
      from = src.token;
    } else if (src.source) {
      got = src.expr ? evaluate(files[src.source] ?? '', src.expr) : constant(files[src.source] ?? '', src.name);
      from = `${src.source} ${src.expr ?? src.name}`;
    } else if (src.defaults) {
      got = defaults?.[src.defaults];
      from = src.defaults + (defaults ? '' : ' (not read: run the reference, `oracle.mjs`, first)');
      if (!defaults) { console.log(`  ${name}: ${want}   ${from}`); continue; }
    } else if (src.computed) {
      const expr = src.computed.replace(/\b(\w+Tokens)\.(\w+)\b/g, (_, o, n) => {
        const v = resolveToken(objects, `${o}.${n}`);
        return String(v.dp ?? v.number ?? NaN);
      });
      got = /^[\d.\s()+\-*/]+$/.test(expr) ? Function(`return (${expr})`)() : undefined;
      from = `${src.computed} (${src.in})`;
    }
    const same = got !== undefined && Math.abs(got - want) < 1e-6;
    ok &&= same;
    console.log(`  ${same ? 'ok' : 'DIFFERS'} ${name}: schema ${want}, source ${got ?? 'not found'}   ${from}`);
  }
  console.log(ok ? `every Material row is Compose Material 3 ${pinned}'s` : `rows differ from Compose Material 3 ${pinned}`);
  return ok;
}

/**
 * A file's constant in dp or a plain number: a literal, or a sum or difference of literals and the file's other
 * constants (`TopAppBarTitleInset = 16.dp - TopAppBarHorizontalPadding`).
 */
function constant(text, name, depth = 0) {
  const m = text.match(new RegExp(`^(?:internal |private )?(?:const )?val ${name}\\s*(?::\\s*\\w+\\s*)?=\\s*(.+?)\\s*$`, 'm'));
  return m && depth <= 8 ? evaluate(text, m[1], depth) : undefined;
}

/** `expr`, a sum, difference, product or quotient of dp literals and `text`'s constants, in dp. */
function evaluate(text, expr, depth = 0) {
  const numbers = expr.replace(/\b(\d+(?:\.\d+)?)(?:\.dp|f)?(?!\w)/g, '$1')
    .replace(/\b([A-Za-z_]\w*)\b/g, (_, n) => String(constant(text, n, depth + 1) ?? NaN));
  if (!/^[\d.\s()+\-*/]+$/.test(numbers)) return undefined;
  return Function(`return (${numbers})`)();
}

/** D8's report: every token whose value changed, appeared or went, and every other source file that changed. */
function diff(a, b, from) {
  const changed = [];
  const objects = new Set([...Object.keys(a.tokens), ...Object.keys(b.tokens)]);
  for (const o of [...objects].sort()) {
    const names = new Set([...Object.keys(a.tokens[o] ?? {}), ...Object.keys(b.tokens[o] ?? {})]);
    for (const n of [...names].sort()) {
      const x = JSON.stringify(a.tokens[o]?.[n]), y = JSON.stringify(b.tokens[o]?.[n]);
      if (x !== y) changed.push(`${o}.${n}: ${x ?? 'none'} → ${y ?? 'none'}`);
    }
  }
  const files = new Set([...Object.keys(a.files), ...Object.keys(b.files)]);
  const sources = [...files].filter((f) => !f.startsWith('tokens/') && a.files[f] !== b.files[f]).sort();
  const report = [`Compose Material 3 ${from} → ${pinned}`, '', `Tokens (${changed.length}):`, ...changed.map((c) => `  ${c}`), '',
    `Other sources that changed (${sources.length}):`, ...sources.map((f) => `  ${f}${a.files[f] == null ? ' (new)' : b.files[f] == null ? ' (gone)' : ''}`)].join('\n');
  const file = join(mkdir(join(build, 'tokens')), `diff-${from}-${pinned}.txt`);
  writeFileSync(file, report + '\n');
  console.log(report);
  console.log(`\n${file}`);
}

function mkdir(d) {
  mkdirSync(d, { recursive: true });
  return d;
}
