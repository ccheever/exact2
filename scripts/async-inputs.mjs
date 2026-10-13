/**
 * async-inputs — when the async lane may skip a check (scripts/async.mjs).
 *
 * A gated check's inputs are hashed into a fingerprint, and the lane keeps the
 * outcome it saw under each fingerprint. A check is skipped only when its
 * fingerprint's last recorded outcome is a pass: its inputs are then those of
 * a run that passed, so its outcome cannot differ. A failure under a
 * fingerprint replaces its pass; a run that did not finish, a check that could
 * not run (exit UNAVAILABLE) and any error computing a fingerprint record
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
import { availableParallelism, homedir } from 'node:os';
import { delimiter, dirname, join, relative, resolve } from 'node:path';
import { driverRuntime } from '../host/web-js/glue.mjs';

/** Bumped when what a fingerprint covers changes: every check then runs once. */
export const SCHEMA = 1;

/** The exit status of a check that could not run here (a tool is missing): neither a pass nor a failure. */
export const UNAVAILABLE = 77;

/** The environment a gated check runs under: everything else is dropped. */
const ENV_KEPT = ['HOME', 'PATH', 'TMPDIR', 'LANG', 'LC_ALL', 'USER', 'LOGNAME', 'SHELL', 'CARGO_HOME', 'RUSTUP_HOME',
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

const run = (command, args, opts) => {
  const r = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 1 << 28, ...opts });
  if (r.error || r.status !== 0) throw new Error(`${command} ${args.join(' ')}: ${r.error?.message ?? r.stderr?.trim() ?? `exit ${r.status}`}`);
  return r.stdout;
};

const onPath = (name, env) => (env.PATH ?? '').split(delimiter).map(d => join(d, name)).find(p => existsSync(p));
const rel = (root, p) => { const r = relative(root, p); return r.startsWith('..') ? p : r; };
const once = (map, key, make) => map.has(key) ? map.get(key) : (map.set(key, make()), map.get(key));

const LEAN_LIBRARY = ['semantics/Contract.lean', 'semantics/Contract', 'semantics/lakefile.toml', 'semantics/lake-manifest.json', 'semantics/lean-toolchain'];

/**
 * What the gates read at one checkout, each read once: the checks share
 * binaries, the Lean library and the programs. Make one per commit.
 */
export class Inputs {
  #entries = new Map();
  #made = new Map();
  constructor(root, env) { this.root = root; this.env = env; }

  /** `rel`'s entry, read once. */
  entry(r) { return once(this.#entries, r, () => entry(resolve(this.root, r))); }

  /** Every path under `rel` (itself included) and its entry, sorted; `skip(rel, name)` prunes. */
  walk(r, skip = () => false) {
    const pairs = [];
    const visit = p => {
      const e = this.entry(p);
      pairs.push([p, e]);
      if (e !== 'dir') return;
      for (const name of readdirSync(resolve(this.root, p)).sort()) if (name !== '.DS_Store' && !skip(p, name)) visit(`${p}/${name}`);
    };
    visit(r);
    return pairs;
  }

  /** `walk` over each of `rels`, as lines. */
  lines(rels, skip) { return rels.flatMap(r => this.walk(r, skip)).map(([p, e]) => `${p}\t${e}`); }

  /** Build `pkg` as the check's `cargo run -p` or `cargo build -p` does; the binary's line. */
  binary(pkg, bin = pkg) {
    return once(this.#made, `binary ${pkg} ${bin}`, () => {
      run('cargo', ['build', '-q', '-p', pkg, '--bin', bin], { cwd: this.root, env: this.env });
      return `${bin}\tfile ${sha256(readFileSync(resolve(this.root, 'target/debug', bin)))}`;
    });
  }

  /** `lake` as `leanrun::lake` finds it, and the Lean it runs where `dir`'s `lean-toolchain` points. */
  lake(dir) {
    return once(this.#made, `lake ${dir}`, () => {
      const elan = join(this.env.HOME ?? homedir(), '.elan/bin/lake');
      const path = existsSync(elan) ? elan : onPath('lake', this.env);
      return path ? `lake\t${path}\t${run(path, ['--version'], { cwd: resolve(this.root, dir), env: this.env }).trim()}` : 'lake\tabsent';
    });
  }

  /** `bun` as `js::bun` finds it (`BUN`, the installer's place, `PATH`). */
  bun() {
    const installed = join(this.env.HOME ?? homedir(), '.bun/bin/bun');
    const path = this.env.BUN ?? (existsSync(installed) ? installed : onPath('bun', this.env));
    return path ? `bun\t${path}\t${entry(path)}\t${run(path, ['--revision'], { env: this.env }).trim()}` : 'bun\tabsent';
  }

  /** Charon and Aeneas as semantics/vm-extract/check.sh finds them; `absent` names a missing one. */
  extractionTools() {
    const home = this.env.HOME ?? homedir();
    const charon = this.env.CHARON ?? join(home, 'tools/charon/bin/charon');
    const aeneas = this.env.AENEAS_DIR ?? join(home, 'tools/aeneas');
    const lines = [`charon\t${charon}\t${entry(charon)}`, `aeneas\t${aeneas}/bin/aeneas\t${entry(join(aeneas, 'bin/aeneas'))}`];
    const pin = join(dirname(dirname(charon)), 'rust-toolchain');
    const toolchain = this.env.CHARON_TOOLCHAIN ?? (existsSync(pin) ? /^channel = "(.*)"/m.exec(readFileSync(pin, 'utf8'))?.[1] : undefined);
    if (toolchain) {
      const r = spawnSync('rustup', ['run', toolchain, 'rustc', '-vV'], { encoding: 'utf8', env: this.env });
      lines.push(`charon toolchain\t${toolchain}\t${r.status === 0 ? r.stdout.trim() : 'absent'}`);
    }
    if (existsSync(join(aeneas, '.git'))) {
      // Its Lean library and the packages (mathlib) it pins are what the proofs build against.
      lines.push(`aeneas rev\t${run('git', ['rev-parse', 'HEAD'], { cwd: aeneas, env: this.env }).trim()}`);
      lines.push(`aeneas changes\t${sha256(run('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: aeneas, env: this.env }))}`);
    }
    return lines;
  }

  /** The programs difftest runs (`corpus`, `explore`, `types`, `expansion`, `apps`), and what compiling each reads. */
  programs() {
    return once(this.#made, 'programs', async () => {
      const apps = readdirSync(resolve(this.root, 'apps')).sort().filter(a => existsSync(resolve(this.root, 'apps', a, 'app.contract')));
      // The web build writes `app.contract.d.ts` beside an app for its editor (the lane's conformance run builds every
      // app first); nothing these checks run reads it.
      const walked = ['semantics/corpus', 'contract/corpus', ...apps.map(a => `apps/${a}`)]
        .flatMap(r => this.walk(r, (p, name) => name === 'app.contract.d.ts' && /^apps\/[^/]+$/.test(p)));
      const files = [...walked.filter(([p, e]) => p.endsWith('.contract') && e.startsWith('file ') && !p.startsWith('apps/')).map(([p]) => p),
        ...apps.map(a => `apps/${a}/app.contract`)];
      const lines = [`apps\t${apps.join(' ')}`, ...walked.map(([p, e]) => `${p}\t${e}`)];
      for (const [file, graph] of await this.#sources(files)) {
        for (const s of graph.sources) lines.push(`${file} source\t${rel(this.root, s.path)}\t${this.entry(rel(this.root, s.path))}`);
        for (const c of graph.consulted) lines.push(`${file} consulted\t${rel(this.root, c)}\t${this.entry(rel(this.root, c))}`);
      }
      return lines;
    });
  }

  /** `contract sources` for each file, as many at once as there are cores; a reply that is not its graph throws. */
  async #sources(files) {
    run('cargo', ['build', '-q', '-p', 'contract', '--bin', 'contract'], { cwd: this.root, env: this.env });
    const compiler = resolve(this.root, 'target/debug/contract');
    const ask = file => new Promise((done, fail) => {
      const child = spawn(compiler, ['sources', file], { cwd: this.root, env: this.env });
      let out = '';
      child.stdout.on('data', d => { out += d; });
      child.on('error', fail);
      child.on('close', () => { try { done([file, JSON.parse(out)]); } catch { fail(new Error(`contract sources ${file}: no graph`)); } });
    });
    const answers = new Array(files.length);
    let next = 0;
    const worker = async () => { while (next < files.length) { const i = next++; answers[i] = await ask(files[i]); } };
    await Promise.all(Array.from({ length: Math.min(availableParallelism(), files.length) }, worker));
    return answers;
  }
}

/** Each gated check's inputs, by group: `{group: lines}`. */
export const INPUTS = {
  lean: async ctx => ({
    lean: ctx.lines([...LEAN_LIBRARY, 'semantics/Apps'], (_, name) => name === '.lake'),
    lake: [ctx.lake('semantics')],
  }),
  'vm-extract': async ctx => {
    const own = 'semantics/vm-extract';
    const metadata = JSON.parse(run('cargo', ['metadata', '--format-version', '1', '--offline', '--manifest-path', `${own}/Cargo.toml`], { cwd: ctx.root, env: ctx.env }));
    const crates = metadata.packages.filter(p => p.source === null).map(p => rel(ctx.root, dirname(p.manifest_path))).filter(d => d !== own).sort();
    // What check.sh writes (the extraction, Lake's build) is not an input; the runner's machine is, by `#[path]`.
    const generated = new Set([`${own}/lean/VmExtract/Types.lean`, `${own}/lean/VmExtract/Funs.lean`]);
    const tools = ctx.extractionTools();
    // Without Charon or Aeneas the check is unavailable whatever the rest is: don't make elan fetch its Lean to name it.
    if (!tools.some(t => t.endsWith('\tabsent'))) tools.push(ctx.lake(`${own}/lean`));
    return {
      extraction: ctx.lines([own], (p, name) => name === '.lake' || name === 'target' || name.endsWith('.md') || generated.has(`${p}/${name}`)),
      machine: ctx.lines(['runner/src/machine.rs']),
      crates: ctx.lines(crates, (_, name) => name === 'target'),
      contract: ctx.lines(LEAN_LIBRARY),
      tools,
    };
  },
  difftest: async ctx => ({
    binary: [ctx.binary('contract-difftest', 'difftest')],
    // The library its generated modules import, and the app embeddings `difftest apps` compares (not their proofs).
    lean: ctx.lines([...LEAN_LIBRARY, ...readdirSync(resolve(ctx.root, 'semantics/Apps')).filter(f => f.endsWith('.lean')).sort().map(f => `semantics/Apps/${f}`)]),
    programs: await ctx.programs(),
    lake: [ctx.lake('semantics')],
  }),
  'difftest-js': async ctx => ({
    binary: [ctx.binary('contract-difftest', 'difftest'), ctx.binary('exact-web-js')],
    runtime: ctx.lines([...driverRuntime(ctx.root), 'host/web-js/glue.mjs', 'semantics/difftest/js']),
    programs: await ctx.programs(),
    bun: [ctx.bun()],
  }),
};

/** The fingerprint of a check: its command, environment and input groups. */
export function fingerprint(command, args, env, groups) {
  const digests = Object.fromEntries(Object.entries(groups).map(([g, lines]) => [g, sha256(lines.join('\n'))]));
  const digest = sha256(JSON.stringify([SCHEMA, command, args, Object.entries(env).sort(), Object.entries(digests).sort()]));
  return { digest, groups: digests };
}

/** The outcomes the lane saw, by check and fingerprint. */
export class Store {
  constructor(dir, keep = 500) { this.dir = dir; this.keep = keep; }
  read(check, name) {
    try { return JSON.parse(readFileSync(join(this.dir, check, name), 'utf8')); } catch { return null; }
  }
  #write(check, name, value) {
    mkdirSync(join(this.dir, check), { recursive: true });
    const file = join(this.dir, check, name), tmp = `${file}.${process.pid}.tmp`;
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
    const files = readdirSync(dir).filter(f => /^[0-9a-f]{64}\.json$/.test(f));
    if (files.length <= this.keep) return;
    const newest = files.map(f => [f, statSync(join(dir, f)).mtimeMs]).sort((a, b) => b[1] - a[1]);
    for (const [f] of newest.slice(this.keep)) rmSync(join(dir, f), { force: true });
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

/**
 * Whether a gated check runs at the checkout `ctx` reads: `{fp, passed}` (a
 * pass under these inputs: skip), `{fp, changed}` (run), or `{error}` (the
 * gate failed: run).
 */
export async function decide(store, ctx, name, command, args, opts) {
  try {
    const fp = fingerprint(command, args, opts.env, await opts.inputs(ctx));
    const passed = store.passed(name, fp);
    return passed ? { fp, passed } : { fp, changed: changedSince(store, name, fp) };
  } catch (e) {
    return { error: String(e.message ?? e).slice(0, 2000) };
  }
}

/** The outcome a gated check's run records: a pass, a failure, or nothing (unavailable, or it did not finish). */
export function outcomeOf(status, hung) {
  if (hung) return 'fail';
  return status === 0 ? 'pass' : status === UNAVAILABLE ? 'unavailable' : 'fail';
}
