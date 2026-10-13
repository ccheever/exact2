/**
 * async-inputs — when the async lane may skip a check (scripts/async.mjs).
 *
 * A gated check's inputs are hashed into a fingerprint, and the lane keeps the
 * outcome it saw under each fingerprint. A check is skipped only when its
 * fingerprint's last recorded outcome is a pass: its inputs are then those of
 * a run that passed, so its outcome cannot differ. A failure under a
 * fingerprint replaces its pass; a run that did not finish, a check that could
 * not run (`unavailable`) and any error computing a fingerprint record
 * nothing, so the check runs.
 *
 * Inputs are what the check reads, observed on disk rather than in git (an
 * untracked `.shells/app.json` is read by the compiler; an empty `strings/`
 * is not an absent one):
 *   - Rust: the bytes of the binaries the check runs, built as it builds them.
 *     That covers every source, `include_str!` target, build-script input,
 *     registry crate, feature, profile and `.cargo/config` at once. An
 *     incremental build is not reproducible, so this costs extra runs, never
 *     a missed one.
 *   - files and directories it reads at run time: the Lean project, the
 *     programs and every file compiling them consults (`contract sources`),
 *     the JS runtime the differential driver bundles;
 *   - the tools the code resolves (lake, bun, Charon, Aeneas) and the check's
 *     environment, which the lane reduces to ENV_KEPT.
 *
 * The store is one file per fingerprint, `<dir>/<check>/<fingerprint>.json`,
 * written whole (temporary file, then rename), plus `<check>/last-pass.json`,
 * which `--explain` compares against to name what changed.
 */
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, readlinkSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, dirname, join, relative, resolve } from 'node:path';
import { HOST_WEB_GLUE } from '../host/web-js/glue.mjs';

/** Bumped when what a fingerprint covers changes: every check then runs once. */
export const SCHEMA = 1;

/** The environment a gated check runs under: everything else is dropped. */
export const ENV_KEPT = ['HOME', 'PATH', 'TMPDIR', 'LANG', 'LC_ALL', 'USER', 'LOGNAME', 'SHELL', 'CARGO_HOME', 'RUSTUP_HOME',
  'RUSTUP_TOOLCHAIN', 'BUN', 'CHARON', 'CHARON_TOOLCHAIN', 'AENEAS_DIR'];

const sha256 = data => createHash('sha256').update(data).digest('hex');

export function checkEnv(base, extra = {}) {
  const env = {};
  for (const k of ENV_KEPT) if (base[k] !== undefined) env[k] = base[k];
  return { ...env, HERMES_LEAN_SYS_OFFLINE: '1', ...extra };
}

/** A path's state: absent, a directory, a symlink and its target, or a file and its content's digest. */
export function entry(path) {
  let st;
  try { st = lstatSync(path); } catch (e) { if (e.code === 'ENOENT' || e.code === 'ENOTDIR') return 'absent'; throw e; }
  if (st.isSymbolicLink()) return `link ${readlinkSync(path)}`;
  if (st.isDirectory()) return 'dir';
  if (st.isFile()) return `file ${sha256(readFileSync(path))}`;
  return `other ${st.mode}`;
}

/** Every path under `rel` (itself included) with its entry, relative to `root`, sorted; `skip(rel, name)` prunes. */
export function walk(root, rel, skip = () => false) {
  const lines = [];
  const visit = r => {
    const e = entry(join(root, r));
    lines.push(`${r}\t${e}`);
    if (e !== 'dir') return;
    for (const name of readdirSync(join(root, r)).sort()) {
      if (name === '.DS_Store' || skip(r, name)) continue;
      visit(`${r}/${name}`);
    }
  };
  visit(rel);
  return lines;
}

const run = (command, args, opts) => {
  const r = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 1 << 28, ...opts });
  if (r.error || r.status !== 0) throw new Error(`${command} ${args.join(' ')}: ${r.error?.message ?? r.stderr?.trim() ?? `exit ${r.status}`}`);
  return r.stdout;
};

/** Build `pkg` exactly as the check's `cargo run -p` or `cargo build -p` does, and hash the binary. */
export function binary(root, env, pkg, bin) {
  run('cargo', ['build', '-q', '-p', pkg, ...(bin ? ['--bin', bin] : [])], { cwd: root, env });
  return [`${bin ?? pkg}\tfile ${sha256(readFileSync(resolve(root, 'target/debug', bin ?? pkg)))}`];
}

const onPath = (name, env) => (env.PATH ?? '').split(delimiter).map(d => join(d, name)).find(p => existsSync(p));

/** `lake` as `leanrun::lake` finds it, and the Lean it runs where `dir`'s `lean-toolchain` points. */
export function lake(root, env, dir) {
  const elan = join(env.HOME ?? homedir(), '.elan/bin/lake');
  const path = existsSync(elan) ? elan : onPath('lake', env);
  if (!path) return [`lake\tabsent`];
  return [`lake\t${path}\t${run(path, ['--version'], { cwd: resolve(root, dir), env }).trim()}`];
}

/** `bun` as `js::bun` finds it (`BUN`, the installer's place, `PATH`). */
export function bun(env) {
  const installed = join(env.HOME ?? homedir(), '.bun/bin/bun');
  const path = env.BUN ?? (existsSync(installed) ? installed : onPath('bun', env));
  if (!path) return ['bun\tabsent'];
  return [`bun\t${path}\t${entry(path)}\t${run(path, ['--revision'], { env }).trim()}`];
}

/** Charon and Aeneas as semantics/vm-extract/check.sh finds them. */
export function extractionTools(env) {
  const home = env.HOME ?? homedir();
  const charon = env.CHARON ?? join(home, 'tools/charon/bin/charon');
  const aeneas = env.AENEAS_DIR ?? join(home, 'tools/aeneas');
  const lines = [`charon\t${charon}\t${entry(charon)}`, `aeneas\t${aeneas}/bin/aeneas\t${entry(join(aeneas, 'bin/aeneas'))}`];
  const pin = join(dirname(dirname(charon)), 'rust-toolchain');
  const toolchain = env.CHARON_TOOLCHAIN ?? (existsSync(pin) ? /^channel = "(.*)"/m.exec(readFileSync(pin, 'utf8'))?.[1] : undefined);
  if (toolchain) {
    const r = spawnSync('rustup', ['run', toolchain, 'rustc', '-vV'], { encoding: 'utf8', env });
    lines.push(`charon toolchain\t${toolchain}\t${r.status === 0 ? r.stdout.trim() : 'absent'}`);
  }
  if (existsSync(join(aeneas, '.git'))) {
    // Its Lean library and the packages (mathlib) it pins are what the proofs build against.
    lines.push(`aeneas rev\t${run('git', ['rev-parse', 'HEAD'], { cwd: aeneas, env }).trim()}`);
    lines.push(`aeneas changes\t${sha256(run('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: aeneas, env }))}`);
  }
  return lines;
}

const LEAN_LIBRARY = ['semantics/Contract.lean', 'semantics/Contract', 'semantics/lakefile.toml', 'semantics/lake-manifest.json', 'semantics/lean-toolchain'];
const paths = (root, rels, skip) => rels.flatMap(r => walk(root, r, skip));

/** The programs difftest runs (`corpus`, `explore`, `types`, `expansion`, `apps`), and what compiling each reads. */
export async function programs(root, env, sources = sourcesOf(root, env)) {
  const apps = readdirSync(resolve(root, 'apps')).sort().filter(a => existsSync(resolve(root, 'apps', a, 'app.contract')));
  // The web build writes `app.contract.d.ts` beside an app for its editor (the lane's conformance run builds every
  // app first); nothing these checks run reads it.
  const lines = [`apps\t${apps.join(' ')}`, ...paths(root, ['semantics/corpus', 'contract/corpus', ...apps.map(a => `apps/${a}`)],
    (r, name) => name === 'app.contract.d.ts' && /^apps\/[^/]+$/.test(r))];
  const files = [...lines.flatMap(l => /^((?:semantics|contract)\/corpus\/.*\.contract)\t/.exec(l)?.[1] ?? []), ...apps.map(a => `apps/${a}/app.contract`)];
  for (const [file, graph] of await sources(files)) {
    for (const s of graph.sources) lines.push(`${file} source\t${rel(root, s.path)}\t${entry(s.path)}`);
    for (const c of graph.consulted) lines.push(`${file} consulted\t${rel(root, c)}\t${entry(c)}`);
  }
  return lines;
}

const rel = (root, p) => { const r = relative(root, p); return r.startsWith('..') ? p : r; };

/** `contract sources` for each file, eight at a time; a reply that is not its JSON graph throws. */
export function sourcesOf(root, env) {
  return async files => {
    binary(root, env, 'contract', 'contract');
    const compiler = resolve(root, 'target/debug/contract');
    const ask = file => new Promise((done, fail) => {
      const child = spawn(compiler, ['sources', file], { cwd: root, env });
      let out = '';
      child.stdout.on('data', d => { out += d; });
      child.on('error', fail);
      child.on('close', () => { try { done([file, JSON.parse(out)]); } catch { fail(new Error(`contract sources ${file}: no graph`)); } });
    });
    const answers = [];
    for (let i = 0; i < files.length; i += 8) answers.push(...await Promise.all(files.slice(i, i + 8).map(ask)));
    return answers;
  };
}

/** The JS runtime `semantics/difftest/js/drive.mjs` stages and bundles. */
export function jsRuntime(root) {
  const webJs = readdirSync(resolve(root, 'host/web-js')).filter(f => f.endsWith('.js') || f === 'glue.mjs').sort().map(f => `host/web-js/${f}`);
  const web = [...HOST_WEB_GLUE, 'navigation.js', 'grant-admission.js'].map(f => `host/web/${f}`);
  return [...paths(root, [...webJs, ...web]), ...walk(root, 'semantics/difftest/js')];
}

/** Each gated check's inputs, by group: `{group: lines}`. */
export const INPUTS = {
  lean: async (root, env) => ({
    lean: paths(root, [...LEAN_LIBRARY, 'semantics/Apps'], (_, name) => name === '.lake'),
    lake: lake(root, env, 'semantics'),
  }),
  'vm-extract': async (root, env) => {
    const own = 'semantics/vm-extract';
    const metadata = JSON.parse(run('cargo', ['metadata', '--format-version', '1', '--offline', '--manifest-path', `${own}/Cargo.toml`], { cwd: root, env }));
    const crates = metadata.packages.filter(p => p.source === null).map(p => rel(root, dirname(p.manifest_path))).filter(d => d !== own).sort();
    // What check.sh writes (the extraction, Lake's build) is not an input; the runner's machine is, by `#[path]`.
    const generated = new Set([`${own}/lean/VmExtract/Types.lean`, `${own}/lean/VmExtract/Funs.lean`]);
    return {
      extraction: walk(root, own, (r, name) => name === '.lake' || name === 'target' || name.endsWith('.md') || generated.has(`${r}/${name}`)),
      machine: walk(root, 'runner/src/machine.rs'),
      crates: paths(root, crates, (_, name) => name === 'target'),
      contract: paths(root, LEAN_LIBRARY),
      // Without Charon or Aeneas the check is unavailable whatever the rest is,
      // so don't make elan fetch the extraction's own Lean just to name it.
      tools: (tools => tools.some(t => t.endsWith('\tabsent')) ? tools : [...tools, ...lake(root, env, `${own}/lean`)])(extractionTools(env)),
    };
  },
  difftest: async (root, env, sources) => ({
    binary: binary(root, env, 'contract-difftest', 'difftest'),
    // The library its generated modules import, and the app embeddings `difftest apps` compares (not their proofs).
    lean: [...paths(root, LEAN_LIBRARY), ...readdirSync(resolve(root, 'semantics/Apps')).filter(f => f.endsWith('.lean')).sort().flatMap(f => walk(root, `semantics/Apps/${f}`))],
    programs: await programs(root, env, sources),
    lake: lake(root, env, 'semantics'),
  }),
  'difftest-js': async (root, env, sources) => ({
    binary: [...binary(root, env, 'contract-difftest', 'difftest'), ...binary(root, env, 'exact-web-js')],
    runtime: jsRuntime(root),
    programs: await programs(root, env, sources),
    bun: bun(env),
  }),
};

/** The fingerprint of a check: its command, environment and input groups. */
export function fingerprint(name, command, args, env, groups) {
  const digests = Object.fromEntries(Object.entries(groups).map(([g, lines]) => [g, sha256(lines.join('\n'))]));
  const digest = sha256(JSON.stringify([SCHEMA, name, command, args, Object.entries(env).sort(), Object.entries(digests).sort()]));
  return { digest, groups: digests };
}

/** The outcomes the lane saw, by check and fingerprint. */
export class Store {
  constructor(dir, keep = 500) { this.dir = dir; this.keep = keep; }
  #file(check, name) { return join(this.dir, check, name); }
  read(check, name) {
    try { return JSON.parse(readFileSync(this.#file(check, name), 'utf8')); } catch { return null; }
  }
  #write(check, name, value) {
    mkdirSync(join(this.dir, check), { recursive: true });
    const file = this.#file(check, name), tmp = `${file}.${process.pid}.tmp`;
    writeFileSync(tmp, JSON.stringify(value, null, 2) + '\n');
    renameSync(tmp, file);
  }
  /** The record whose last outcome under `fp` was a pass, if any. */
  passed(check, fp) {
    const r = this.read(check, `${fp.digest}.json`);
    return r?.schema === SCHEMA && r.outcome === 'pass' ? r : null;
  }
  /** Keep `outcome` (`pass` or `fail`) as the last seen under `fp`. */
  record(check, fp, outcome, info = {}) {
    if (outcome !== 'pass' && outcome !== 'fail') return;
    const value = { schema: SCHEMA, check, outcome, fingerprint: fp.digest, groups: fp.groups, at: new Date().toISOString(), ...info };
    this.#write(check, `${fp.digest}.json`, value);
    if (outcome === 'pass') this.#write(check, 'last-pass.json', value);
    this.#prune(check);
  }
  #prune(check) {
    const dir = join(this.dir, check);
    const files = readdirSync(dir).filter(f => /^[0-9a-f]{64}\.json$/.test(f))
      .map(f => [f, statSync(join(dir, f)).mtimeMs]).sort((a, b) => b[1] - a[1]);
    for (const [f] of files.slice(this.keep)) rmSync(join(dir, f), { force: true });
  }
}

/** The groups of `fp` that differ from the check's last pass. */
export function changedSince(store, check, fp) {
  const last = store.read(check, 'last-pass.json');
  if (!last || last.schema !== SCHEMA) return ['(no pass recorded)'];
  const names = new Set([...Object.keys(fp.groups), ...Object.keys(last.groups ?? {})]);
  const changed = [...names].filter(g => fp.groups[g] !== last.groups?.[g]);
  return changed.length ? changed : ['(command or environment)'];
}
