// Staged capabilities (LLP 1047.000 §9): the production web artifact is split
// into a core and one module per staged capability, which the glue loads
// before the capability's first use.
//
// A stage holds what its seam entries reach and the core does not, on
// binaryen's call graph. The core's roots are its exports and address-taken
// functions, less the seam entries, so a helper the core shares stays in the
// core whatever its name. A direct call from the core into a stage is a build
// failure; the core reaches a stage only through its seam, an address-taken
// function the glue gates. The modules share the core's memory and tables
// (`wasm-split --multi-split --no-placeholders`), so a stage's functions work
// on the runner's own state, and a call into a stage not yet loaded traps: an
// assertion that a gate was bypassed, never a fallback.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

/** The staged capabilities and their seam entries, demangled. */
export const STAGES = [
  // Inspection (LLP 1012): the agent API's reads. Host::agent calls the hook
  // `HostLinks::of` installs; the glue's `ask` is its only caller.
  { name: 'inspection', seam: /^exact_runner::agent::handle::</ },
];
/** The binaryen that splits. A stage imports the core's generated export
 * names, so the pair comes from one run of one version; another version
 * builds unsplit rather than guess. */
export const BINARYEN = 'version 132';
/** Where the pinned binaryen comes from on a machine without a package
 * manager (LLP 1054 O2): the release tarball, unpacked, its `bin/` on PATH. */
export const BINARYEN_DOWNLOAD = `https://github.com/WebAssembly/binaryen/releases/tag/${BINARYEN.replace(' ', '_')}`;
const FEATURES = ['--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--enable-reference-types'];

const run = (command, args, options = {}) => spawnSync(command, args, { encoding: 'utf8', maxBuffer: 1 << 28, ...options });

/** Why this machine can't split, or null when it can. */
export function unsplitReason() {
  const version = run('wasm-split', ['--version']);
  if (version.error) return `wasm-split is not on PATH (binaryen ${BINARYEN}: brew install binaryen, or ${BINARYEN_DOWNLOAD})`;
  if (!version.stdout.includes(BINARYEN)) return `wasm-split is ${version.stdout.trim()}, not the pinned ${BINARYEN} (${BINARYEN_DOWNLOAD})`;
  if (run('c++filt', ['--version']).error) return 'c++filt is not on PATH';
  return null;
}

/** Each stage's functions (raw names) in `named`, a wasm that keeps its name
 * section. Throws when a core function calls into a stage directly. */
export function planStages(named, stages = STAGES) {
  const graph = run('wasm-opt', [named, '--print-call-graph', ...FEATURES, '-o', '/dev/null']);
  if (graph.status !== 0) throw new Error(`wasm-opt --print-call-graph failed: ${graph.stderr}`);
  const nodes = new Map(), calls = new Map(), callers = new Map();
  for (const line of graph.stdout.split('\n')) {
    const edge = /^\s*"(.+?)" -> "(.+?)";?\s*(\/\/.*)?$/.exec(line);
    if (edge) {
      if (!calls.has(edge[1])) calls.set(edge[1], new Set());
      if (!callers.has(edge[2])) callers.set(edge[2], new Set());
      calls.get(edge[1]).add(edge[2]); callers.get(edge[2]).add(edge[1]);
      continue;
    }
    const node = /^\s*"(.+?)" \[(.*)\];?\s*$/.exec(line);
    // The graph's key names four example nodes; a function is never named so.
    if (node && !['Import', 'Export', 'Indirect Target', 'A', 'B'].includes(node[1])) {
      nodes.set(node[1], { root: /fillcolor="gray"/.test(node[2]) || /rounded/.test(node[2]) });
    }
  }
  for (const name of [...calls.keys(), ...callers.keys()]) if (!nodes.has(name)) nodes.set(name, { root: false });
  const raw = [...nodes.keys()];
  const plain = run('c++filt', [], { input: raw.join('\n') }).stdout.split('\n');
  const demangled = new Map(raw.map((name, i) => [name, (plain[i] ?? name).replace(/\[[0-9a-f]{16}\]/g, '')]));
  const entries = new Map(stages.map(stage => [stage.name, raw.filter(name => stage.seam.test(demangled.get(name)))]));
  const seams = new Set([...entries.values()].flat());
  const reach = (starts, blocked) => {
    const seen = new Set(), stack = [...starts];
    while (stack.length) {
      const f = stack.pop();
      if (seen.has(f) || blocked.has(f)) continue;
      seen.add(f);
      for (const g of calls.get(f) ?? []) stack.push(g);
    }
    return seen;
  };
  const core = reach(raw.filter(name => nodes.get(name).root && !seams.has(name)), seams);
  const planned = [];
  for (const stage of stages) {
    const seam = entries.get(stage.name);
    if (!seam.length) throw new Error(`stage ${stage.name}: no function matches its seam ${stage.seam}`);
    for (const entry of seam) {
      if (!nodes.get(entry).root) throw new Error(`stage ${stage.name}: ${demangled.get(entry)} is not address-taken, so it is no seam`);
      const direct = [...callers.get(entry) ?? []].filter(caller => core.has(caller));
      if (direct.length) throw new Error(`stage ${stage.name}: the core calls its seam ${demangled.get(entry)} directly from ${direct.slice(0, 3).map(c => demangled.get(c)).join(', ')}`);
    }
    planned.push({ name: stage.name, functions: [...reach(seam, new Set())].filter(f => !core.has(f)) });
  }
  return planned;
}

/** A wasm custom section: id 0, its size, then its name and payload. */
function customSection(name, payload) {
  const leb = (n) => { const out = []; do { let b = n & 0x7f; n >>>= 7; if (n) b |= 0x80; out.push(b); } while (n); return Buffer.from(out); };
  const body = Buffer.concat([leb(Buffer.byteLength(name)), Buffer.from(name), payload]);
  return Buffer.concat([Buffer.from([0]), leb(body.length), body]);
}

/** Split `named` (optimized, names kept) into a core and its stages. Returns
 * the core's bytes and each stage's `stages/<name>.<digest>.wasm`; the core's
 * `exact.stages` custom section names them, so a core never loads another
 * build's stage (the digest is the stage's content). */
export function splitStages(named, stages = STAGES) {
  const planned = planStages(named, stages).filter(stage => stage.functions.length);
  const work = mkdtempSync(resolve(tmpdir(), 'exact-stages-'));
  try {
    const manifest = resolve(work, 'manifest.txt');
    writeFileSync(manifest, planned.map(stage => [stage.name, ...stage.functions].join('\n')).join('\n\n') + '\n');
    const split = run('wasm-split', ['--multi-split', named, '--manifest', manifest, '--out-prefix', resolve(work, 'stage-'), '-o', resolve(work, 'core.wasm'), '--no-placeholders', ...FEATURES]);
    if (split.status !== 0) throw new Error(`wasm-split failed: ${split.stderr}`);
    const strip = (from) => {
      const to = from.replace(/\.wasm$/, '.stripped.wasm');
      const opt = run('wasm-opt', [from, '--strip-debug', '--strip-producers', ...FEATURES, '-o', to]);
      if (opt.status !== 0) throw new Error(`wasm-opt --strip-debug failed: ${opt.stderr}`);
      return readFileSync(to);
    };
    const files = new Map(), table = {};
    for (const stage of planned) {
      const bytes = strip(resolve(work, `stage-${stage.name}.wasm`));
      const path = `stages/${stage.name}.${createHash('sha256').update(bytes).digest('hex').slice(0, 16)}.wasm`;
      files.set(path, bytes);
      table[stage.name] = path;
    }
    const core = Buffer.concat([strip(resolve(work, 'core.wasm')), customSection('exact.stages', Buffer.from(JSON.stringify({ binaryen: BINARYEN, stages: table })))]);
    return { core, files, report: planned.map(stage => `${stage.name} ${stage.functions.length} functions, ${(files.get(table[stage.name]).length / 1024).toFixed(0)} KiB`) };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}
