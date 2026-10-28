// assets/file-preview/: the document viewers' page (file-preview-host/src/, an LLP 1020.001 bundled page the
// attachment body frames, src/panel/viewer-frame.contract), built from the pinned packages the reference locks:
// pdfjs-dist (Apache-2.0), @betteroffice docx, xlsx, pptx (Apache-2.0) and fonts (Apache-2.0 and OFL-1.1), sql.js,
// jszip, fast-xml-parser (MIT), saxes (ISC), entities (BSD-2-Clause), dompurify (MPL-2.0 or Apache-2.0), effect (MIT)
// and lucide (ISC). The page and its workers are ES modules in js/; each package's wasm, fonts and data sit where its code
// resolves them from `import.meta.url`, and the licence of every package the bundle reads is copied into vendor/. The
// wasm, fonts and CMaps are written gzipped, as `<file>.gz` (the page inflates them, shared/packed.ts): an app's bake
// captures at most 64 MiB of sources, its assets included, and 16 MiB a file, and they are 56 MiB as the packages ship.
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const host = join(example, 'file-preview-host');
const out = join(example, 'assets', 'file-preview');
const module = name => join(here, 'node_modules', name);

/** The page's entries: the page itself and every worker it starts, and the Word engine's own worker. */
const ENTRIES = [
  join(host, 'src', 'entry.ts'),
  join(host, 'src', 'text', 'large-text.worker.ts'),
  join(host, 'src', 'office', 'office-preview.worker.ts'),
  join(host, 'src', 'office', 'document.worker.ts'),
  join(host, 'src', 'office', 'presentation.worker.ts'),
  join(host, 'src', 'office', 'spreadsheet.worker.ts'),
  join(host, 'src', 'binary', 'binary-preview.worker.ts'),
  join(host, 'src', 'office', 'resident-engine.worker.ts'),
];
/** What ships gzipped (shared/packed.ts PACKED). */
const PACKED = /\.(?:wasm|ttf|otf|bcmap|pfb)$/i;
/** The app sources the page shares (the classifiers), beside its own. */
const APP_SOURCES = [join(example, 'src', 'panel', 'file-preview-kind.ts'), join(example, 'src', 'composer', 'composer-video.ts')];
const PACKAGES = /^(?:@betteroffice\/(?:docx|fonts|pptx|xlsx)|pdfjs-dist|sql\.js|jszip|saxes|fast-xml-parser|entities|dompurify|effect|lucide)(?:\/|$)/;
const FORBIDDEN = ['@t3tools', 'react', 'react-dom', 'tailwind-merge', 'zustand'];

function walk(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(path));
    else files.push(path);
  }
  return files.sort();
}

/** What the page is built from (generate.mjs `inputs`): its sources and the app sources it imports. */
export function inputs() {
  return [...walk(host).filter(file => !file.endsWith('.test.ts')), ...APP_SOURCES];
}

const CSP = [
  "default-src 'none'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "worker-src 'self'",
  // The file comes from the environment's server, whose origin is known only at run time.
  "connect-src 'self' http: https: blob: data:",
  "img-src 'self' blob: data:",
  "font-src 'self' blob: data:",
  "style-src 'self' 'unsafe-inline'",
  "frame-src 'self' about: blob:",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

function page() {
  return '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">' +
    `<meta http-equiv="Content-Security-Policy" content="${CSP}">` +
    '<title>File preview</title><link rel="stylesheet" href="viewer.css"></head>' +
    '<body><div id="viewer"></div><script type="module" src="js/entry.js"></script></body></html>\n';
}

/** A package's directory for a file the bundle read from it, and its name. */
function packageOf(input) {
  const parts = input.split(sep);
  const at = parts.lastIndexOf('node_modules');
  if (at < 0) return null;
  const name = parts[at + 1]?.startsWith('@') ? `${parts[at + 1]}/${parts[at + 2]}` : parts[at + 1];
  return { name, dir: parts.slice(0, at + 1).join(sep) + sep + name };
}

function licence(dir) {
  return readdirSync(dir, { withFileTypes: true }).filter(entry => entry.isFile() && /^(licen[cs]e|copying|notice)/i.test(entry.name)).map(entry => entry.name).sort();
}

/** A package import as a browser build resolves it: the `browser` and `import` export conditions, then the package's
 *  `browser` field, `module` and `main`. Bun's own resolver does so inside node_modules; the page's sources live
 *  outside it, so their imports come here. */
function browserEntry(specifier) {
  const [, name, rest] = /^(@[^/]+\/[^/]+|[^/]+)(\/.*)?$/.exec(specifier);
  const dir = module(name), subpath = rest ? `.${rest}` : '.';
  const manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
  const pick = target => {
    if (typeof target === 'string') return target;
    if (!target || typeof target !== 'object') return null;
    for (const condition of ['browser', 'import', 'module', 'default']) {
      const found = condition in target ? pick(target[condition]) : null;
      if (found) return found;
    }
    return null;
  };
  const file = path => {
    for (const candidate of [path, `${path}.js`, `${path}.mjs`, join(path, 'index.js')]) {
      try { if (statSync(candidate).isFile()) return candidate; } catch { /* the next candidate */ }
    }
    throw new Error(`file-preview-host: ${specifier} resolves to no file (${path})`);
  };
  if (manifest.exports) {
    const exports = typeof manifest.exports === 'string' || !Object.keys(manifest.exports).some(key => key.startsWith('.')) ? { '.': manifest.exports } : manifest.exports;
    let target = exports[subpath];
    for (const [key, value] of Object.entries(exports)) {
      if (target !== undefined) break;
      if (key.endsWith('*') && subpath.startsWith(key.slice(0, -1))) target = JSON.parse(JSON.stringify(value).replaceAll('*', subpath.slice(key.length - 1)));
    }
    const found = pick(target);
    if (found) return file(join(dir, found));
  }
  if (subpath !== '.') return file(join(dir, subpath));
  const main = manifest.main ?? 'index.js';
  const browser = typeof manifest.browser === 'string' ? manifest.browser
    : manifest.browser?.[main] ?? manifest.browser?.[`./${main.replace(/^\.\//, '')}`];
  return file(join(dir, browser || manifest.module || main));
}

async function bundle() {
  const resolver = {
    name: 'file-preview-packages',
    setup(build) {
      // The CJK add-on of the fonts package is not shipped (office/fonts-cjk.ts), as the reference aliases it.
      build.onResolve({ filter: /^@betteroffice\/fonts-cjk$/ }, () => ({ path: join(host, 'src', 'office', 'fonts-cjk.ts') }));
      // The page's sources are outside this directory, so their package imports resolve against its node_modules.
      build.onResolve({ filter: PACKAGES }, args => (args.importer.includes(`${sep}node_modules${sep}`) ? undefined : { path: browserEntry(args.path) }));
    },
  };
  const entrypoints = ENTRIES;
  // Bun writes a split build to its outdir, so it builds into a scratch one and generate.mjs writes what differs. Its
  // chunk names hash paths relative to the working directory, so it builds from the app's, whoever runs it (the app
  // build, the tests, a person at the repository root). Identifiers stay unminified: minified, the names chunks import
  // from each other change from one build to the next, and so would every chunk's name.
  const scratch = mkdtempSync(join(tmpdir(), 'file-preview-'));
  const cwd = process.cwd();
  let result;
  try {
    process.chdir(example);
    result = await Bun.build({
      entrypoints, target: 'browser', format: 'esm', splitting: true, minify: { whitespace: true, syntax: true, identifiers: false }, metafile: true, plugins: [resolver],
      naming: { entry: '[name].[ext]', chunk: 'chunk-[hash].[ext]', asset: '[name]-[hash].[ext]' },
      root: example, outdir: scratch, throw: false,
    });
    if (!result.success) throw new Error(`file-preview-host: ${result.logs.map(String).join('\n')}`);
    for (const output of result.outputs) output.bytes = Buffer.from(await output.arrayBuffer());
  } finally {
    process.chdir(cwd);
    rmSync(scratch, { recursive: true, force: true });
  }
  const read = Object.keys(result.metafile?.inputs ?? {}).map(input => resolve(example, input));
  const leaked = read.filter(input => FORBIDDEN.some(name => input.includes(`${sep}node_modules${sep}${name}${sep}`)));
  const declared = new Set(inputs());
  const undeclared = read.filter(input => !input.includes(`${sep}node_modules${sep}`) && !declared.has(input));
  if (leaked.length || undeclared.length) throw new Error(`file-preview-host: the bundle reads ${[...leaked, ...undeclared].map(input => relative(example, input)).join(', ')}`);
  const files = [];
  for (const output of result.outputs) {
    // The Word engine starts its own worker at `./residentEngineWorker.mjs`.
    const name = relative(scratch, output.path).split(sep).pop().replace(/^resident-engine\.worker\.js$/, 'residentEngineWorker.mjs');
    files.push({ file: join(out, 'js', name), text: output.bytes });
  }
  return { files, read };
}

/** Each copied file: its source in a package and its place under assets/file-preview/. */
function copies(read) {
  const pairs = [];
  const tree = (from, to, keep = () => true) => {
    for (const file of walk(from).filter(keep)) pairs.push([file, join(to, relative(from, file))]);
  };
  const pdfjs = module('pdfjs-dist');
  pairs.push([join(pdfjs, 'build', 'pdf.worker.min.mjs'), join('pdfjs', 'pdf.worker.min.mjs')]);
  tree(join(pdfjs, 'standard_fonts'), join('pdfjs', 'standard_fonts'));
  tree(join(pdfjs, 'cmaps'), join('pdfjs', 'cmaps'), file => file.endsWith('.bcmap') || /LICENSE/.test(file));
  tree(join(pdfjs, 'wasm'), join('pdfjs', 'wasm'));
  // The engines' wasm, beside the modules that ask for `./generated/…`, and the faces the fonts package asks for at
  // `../assets/…`.
  tree(join(module('@betteroffice/docx'), 'dist', 'generated'), join('js', 'generated'), file => file.endsWith('.wasm'));
  tree(join(module('@betteroffice/xlsx'), 'dist', 'generated'), join('js', 'generated'), file => file.endsWith('.wasm'));
  tree(join(module('@betteroffice/pptx'), 'dist', 'generated'), join('js', 'generated'), file => file.endsWith('.wasm'));
  tree(join(module('@betteroffice/fonts'), 'assets'), 'assets');
  tree(join(module('@betteroffice/fonts'), 'LICENSES'), join('vendor', 'fonts'));
  pairs.push([join(module('sql.js'), 'dist', 'sql-wasm.wasm'), join('js', 'sql-wasm.wasm')]);
  pairs.push([join(host, 'src', 'office', 'LICENSE.opencode'), join('vendor', 'LICENSE.opencode')]);
  const packages = new Map();
  for (const input of read) {
    const owner = packageOf(input);
    if (owner) packages.set(owner.name, owner.dir);
  }
  for (const name of ['pdfjs-dist', '@betteroffice/fonts', 'sql.js']) packages.set(name, module(name));
  for (const [name, dir] of [...packages].sort(([a], [b]) => a.localeCompare(b))) {
    const found = licence(dir);
    if (!found.length) {
      // A package that names its licence only in its manifest (saxes, ISC; @nodable/entities, MIT) keeps that.
      const manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
      if (!manifest.license) throw new Error(`file-preview-host: ${name} has no licence`);
      pairs.push([join(dir, 'package.json'), join('vendor', `LICENSE-${name.replace(/^@/, '').replace('/', '-')}.package.json`)]);
      continue;
    }
    for (const file of found) pairs.push([join(dir, file), join('vendor', `${file.replace(/\.(md|markdown|txt)$/i, '')}-${name.replace(/^@/, '').replace('/', '-')}${/\.(md|markdown|txt)$/i.exec(file)?.[0] ?? ''}`)]);
  }
  return pairs.map(([from, to]) => (PACKED.test(to) ? { file: join(out, `${to}.gz`), text: gzipSync(readFileSync(from), { level: 9 }) } : { file: join(out, to), text: readFileSync(from) }));
}

export async function generate() {
  const { files, read } = await bundle();
  const style = ['viewer.css', join('pdf', 'pdf-viewer.css')].map(name => readFileSync(join(host, 'src', name), 'utf8')).join('\n');
  return [...files, ...copies(read), { file: join(out, 'index.html'), text: page() }, { file: join(out, 'viewer.css'), text: style }];
}

/** A file under assets/file-preview/ this build does not write: an older chunk or a package file it dropped. */
export function stale(outputs) {
  const written = new Set(outputs.map(({ file }) => file));
  let present = [];
  try { if (statSync(out).isDirectory()) present = walk(out); } catch { return []; }
  return present.filter(file => file.startsWith(out + sep) && !written.has(file));
}
