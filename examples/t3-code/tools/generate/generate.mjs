// The clone's generated sources. Generated files are built, not committed (rules/RULES.md), so each
// of these is made here from the pinned packages beside this file (package.json and bun.lock, at the
// integrity values the reference T3 Code locks; installed on first use) and from committed inputs,
// and examples/t3-code/.gitignore keeps it out of git:
//
//   gen-highlights.mjs data/highlights.wasm and its licence: @pierre/highlights' lexers; src/highlight/
//                      highlights-tables.ts and syntax-ink.contract: its languages and its colour classes
//   gen-icons.mjs      src/settings/settings-b-lucide.ts, src/shared/pages-icons.contract (Lucide);
//                      src/shared/timeline-files.contract and timeline-files-tables.ts, data/pierre-icons.tsv,
//                      the icon tables of icon-tables.mjs and
//                      icon-marks.mjs (src/shared/icons.contract and nine more in their features' folders)
//   gen-morph.mjs      src/shell/shell-morph.contract: morphicons' plan for each icon pair
//   gen-vectors.mjs    tests/terminal/vectors.json: the buffer cases and their committed expectations
//   gen-shell-parser.mjs src/highlight/shell-parser/: unbash parser runtime and declarations
//   gen-mermaid.mjs    assets/mermaid/vendor/ and assets/math/vendor/: pele and DOMPurify for the Mermaid page,
//                      KaTeX for the math page
//   gen-codex-directives.mjs src/highlight/codex-directives.js and .d.ts: the pinned inline directive parser
//   gen-minimap-text.mjs src/timeline/minimap-text.js and .d.ts: the minimap preview's Markdown as plain text
//   gen-page-scripts.mjs src/browser/page-scripts.ts: the page scripts a Browser tab's frame runs (Annotate's overlay,
//                      Playwright's injected script), from the committed assets
//
// The app build runs it before it bakes (data/build.rs). Run it by hand before anything else reads
// them: `bun test examples/t3-code`, the strict tsc, `contract build`, the AppKit tests.
//
//   bun examples/t3-code/tools/generate/generate.mjs          write what differs
//   bun examples/t3-code/tools/generate/generate.mjs --check  exit 1 when an output differs or is missing
//
// A file is written only when its bytes change, dated as its newest input, and a new file leaves its
// directory's time as it was. Cargo runs a build script again when a file it watches is newer than
// that script's last run started, and data/build.rs runs this and watches every input and output (the
// bake, after it, watches each source it reads and the whole of assets/): an output it writes must not
// look newer than the run that wrote it.

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, unlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const GENERATORS = ['gen-highlights.mjs', 'gen-icons.mjs', 'gen-morph.mjs', 'gen-vectors.mjs', 'gen-mermaid.mjs', 'gen-codex-directives.mjs', 'gen-minimap-text.mjs', 'gen-shell-parser.mjs', 'gen-page-scripts.mjs'];

/** Every input: this directory's scripts, package.json and bun.lock, and the assets gen-page-scripts.mjs copies (no
 *  generator reads another app source). */
export function inputs() {
  return [...readdirSync(here).filter(name => /\.mjs$/.test(name) || name === 'package.json' || name === 'bun.lock').map(name => join(here, name)),
    ...['browser-annotate.js', 'browser-annotate.css', 'vendor/playwright/injected-script.js'].map(asset => join(example, 'assets', asset))];
}

/** The pinned packages: a frozen install with nothing to do takes milliseconds; a missing package is fetched once.
 *  The directories an install makes are dated as bun.lock, and this directory keeps its time: a Cargo
 *  build watches the whole app directory (the bake's missing `deck/` root falls back to it), so a
 *  first install during a build would otherwise bake once more. */
function install() {
  const kept = statSync(here), lock = statSync(join(here, 'bun.lock')).mtime;
  const installed = spawnSync(process.execPath, ['install', '--frozen-lockfile'], { cwd: here, encoding: 'utf8' });
  if (installed.status !== 0) throw new Error(`bun install --frozen-lockfile in ${here}:\n${installed.stdout}${installed.stderr}`);
  const pending = [join(here, 'node_modules')];
  while (pending.length) {
    const directory = pending.pop();
    for (const entry of readdirSync(directory, { withFileTypes: true })) if (entry.isDirectory()) pending.push(join(directory, entry.name));
    if (statSync(directory).mtime > lock) utimesSync(directory, lock, lock);
  }
  utimesSync(here, kept.atime, kept.mtime);
}

/** Every generated file's path and text, and the files a generator no longer writes (the packages installed). */
export async function outputs() {
  const files = [], stale = [];
  for (const name of GENERATORS) {
    const generator = await import(join(here, name));
    const out = await generator.generate();
    files.push(...out);
    if (generator.stale) stale.push(...generator.stale(out));
  }
  return { files, stale };
}

if (import.meta.main && !process.argv.includes('--installed')) {
  // Bun resolves a package by what it found when it started, so the generators run in a process
  // started after the install.
  install();
  const run = spawnSync(process.execPath, [fileURLToPath(import.meta.url), '--installed', ...process.argv.slice(2)], { stdio: 'inherit' });
  process.exit(run.status ?? 1);
} else if (import.meta.main) {
  const check = process.argv.includes('--check');
  const { files, stale } = await outputs();
  const time = new Date(Math.max(...inputs().map(file => statSync(file).mtimeMs)));
  const name = file => relative(example, file);
  // Under Cargo (data/build.rs) every input and output is watched: a changed input or a missing
  // output generates again.
  if (process.env.OUT_DIR && process.env.CARGO_MANIFEST_DIR) for (const file of [...inputs(), ...files.map(({ file }) => file)]) console.log(`cargo:rerun-if-changed=${file}`);
  let differs = 0;
  for (const { file, text } of files) {
    // A font is bytes; every other output is text.
    const bytes = typeof text === 'string' ? Buffer.from(text) : text;
    const current = existsSync(file) ? readFileSync(file) : null;
    if (current?.equals(bytes)) continue;
    differs++;
    if (check) { console.error(`${current === null ? 'missing' : 'differs'}: ${name(file)}`); continue; }
    const directory = dirname(file);
    mkdirSync(directory, { recursive: true });
    const before = current === null ? statSync(directory) : null;
    writeFileSync(file, bytes);
    utimesSync(file, time, time);
    if (before) utimesSync(directory, before.atime, before.mtime);
    console.error(`generate: wrote ${name(file)} (${bytes.length} bytes)`);
  }
  for (const file of stale) {
    differs++;
    if (check) console.error(`stale: ${name(file)}`); else { unlinkSync(file); console.error(`generate: removed ${name(file)}`); }
  }
  if (check && differs) process.exit(1);
  if (check) console.error(`generate: ${files.length} generated files match their inputs`);
}
