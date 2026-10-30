import { requireReleasePins } from '../game/inputs.mjs';
// `exact push` — the dogfood publisher: an edit to an app's Contract, TypeScript or Rust
// reaches its installed release binaries in seconds, as a signed update.
//
//   bun scripts/push.mjs <app> [--origin <dir|url>] [--channel <name>] [--keys <dir>] [--seq-above <n>] [--watch]
//
// `exact deploy` (deploy.mjs) is the publisher of record: it snapshots the
// source closure, bakes every platform's binary to learn its cohort, and
// classifies against it. Push starts where a deploy left off instead. Each
// stream on the channel already has a signed head, and the head carries its
// cohort's receipt (`cohort`: the compatibility inputs, the source shapes,
// the binary's digest). So push bakes only what an update carries — the
// plan, through the same resident Rust baker the dev loop runs, and the Rust
// module when Rust changed (a Contract edit reuses the compiled modules); for
// a TypeScript app, the plan and its Hermes module as the build script bakes
// them —
// checks the candidate against that receipt with deploy's own
// `classifyArtifacts`, and publishes it with deploy's `publishStream`: blobs
// read back, `seq` under the stream's lock, the head signed, an immutable
// release record. What a cohort cannot take (a new source shape, a grant, an
// executor) is refused with deploy's reasons; that change needs `deploy` and
// a new binary.
//
// A stream with no head yet starts from the receipt of the production build
// that made the installed binary (`build.mjs` with EXACT_UPDATE_TRUST=
// production writes it beside the bake), as deploy starts a new stream from
// its own build: the cohort is then exactly the binary in the person's hands.
//
// It publishes the working tree (the release record says `dirty`) and never
// the web root. `--watch` keeps the baker and the compiled modules resident
// and publishes after each save.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync, mkdtempSync, rmSync, watch } from 'node:fs';
import { homedir } from 'node:os';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Refusal, refuse, loadSigner } from './deploy-signing.mjs';
import { defaultRelease, inspectHead, publishStream } from './deploy.mjs';
import { classifyArtifacts, cohortReceipt, readBuilds, resolveApp } from './app.mjs';
import { buildRust, rustBundle, RustBaker } from './rust.mjs';
import { blobPath, channelOrigin, openOrigin, sha256, streamPath } from './origin.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const USAGE = 'usage: bun scripts/push.mjs <app> [--origin <dir|url>] [--channel <name>] [--keys <dir>] [--seq-above <n>] [--watch]';

function parseArgs(argv) {
  const opts = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--watch') opts.watch = true;
    else if (a === '--seq-above') opts.seqAbove = Number(argv[++i]);
    else if (a === '--help' || a === '-h') opts.help = true;
    else if (['--origin', '--channel', '--keys'].includes(a)) opts[a.slice(2)] = argv[++i];
    else if (a.startsWith('--')) refuse(`unknown flag ${a}\n${USAGE}`);
    else opts._.push(a);
  }
  return opts;
}

const seconds = (since) => `${((performance.now() - since) / 1000).toFixed(2)} s`;

/** The streams of `channel`: each admissible signed head with its cohort
 * receipt, and each production build of an updating binary with no head yet. */
async function streams(origin, app, channel, env) {
  const out = [];
  for (const compatibilityId of (await origin.list(`.exact/${channel}`)) ?? []) {
    const stream = { channel, compatibilityId };
    const found = await origin.head(stream);
    if (!found) continue;
    // Apps share a channel's directory on an origin; another app's stream is not ours.
    try { if (JSON.parse(found.bytes).app?.id !== app.id) continue; } catch { /* inspectHead says why */ }
    const admission = inspectHead(found, app, stream);
    if (!admission.usable || !admission.authenticated) refuse(`${streamPath(stream)}: the head is not admissible (${admission.problem ?? 'unauthenticated'}); run deploy`);
    const cohort = admission.head.cohort;
    if (cohort?.version !== 1 || cohort.compat?.id !== compatibilityId) refuse(`${streamPath(stream)}: the head carries no cohort receipt; run deploy`);
    out.push({ stream, head: admission.head, cohort, platform: cohort.compat.inputs.platform });
  }
  for (const build of readBuilds(app, env)) {
    const inputs = build.compat.inputs;
    if (build.trust !== 'production' || inputs.platform === 'web' || inputs.store?.L === '0') continue;
    if (out.some((s) => s.stream.compatibilityId === build.compat.id)) continue;
    out.push({ stream: { channel, compatibilityId: build.compat.id }, head: null, cohort: cohortReceipt(build), platform: inputs.platform, build });
  }
  if (!out.length) refuse(`no stream on the channel ${channel} at ${origin.describe()} has a head; a first deploy publishes them (bun scripts/deploy.mjs ${app.name} --channel ${channel} --origin ${origin.describe()} --yes)`);
  return out;
}

/** `exact-bake graph` and, for a TypeScript app, `exact-js-bake`, built once
 * beside the Rust baker's tools. */
function pushTools(app, env, typescript) {
  const target = resolve(app.target, 'push-tools');
  const packages = ['exact-bake', ...(typescript ? ['exact-js-bake'] : [])];
  const r = spawnSync('cargo', ['build', '-q', '--release', ...packages.flatMap((p) => ['-p', p])], { cwd: ROOT, env: { ...env, CARGO_TARGET_DIR: target }, stdio: 'inherit' });
  if (r.status !== 0) refuse(`could not build ${packages.join(' and ')}`);
  return { graph: resolve(target, 'release/exact-bake'), typescript: resolve(target, 'release/exact-js-bake') };
}

/** A TypeScript app's plan and module pair, baked as its build script bakes
 * them (`exact_js_bake::bake`, production mode). */
function bakeTypescript(tool, app, env) {
  const dir = mkdtempSync(resolve(app.target, 'push-ts-'));
  try {
    rmSync(dir, { recursive: true });
    const r = spawnSync(tool, [app.dir, '--out', dir], { cwd: ROOT, env, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
    if (r.status !== 0) refuse(r.stderr.trim().split('\n').slice(-6).join('\n'));
    return { plan: readFileSync(resolve(dir, 'app.plan')), receipt: readFileSync(resolve(dir, 'app.module.json')), bytecode: readFileSync(resolve(dir, 'app.hbc')) };
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

/** The module pair as an update carries it (`typescript/`): the plan no
 * longer needs the cohort's source shapes, since the module answers them;
 * the pair needs the prelude runtime the bake found it uses (its receipt's
 * `typescriptRuntime`, `bake/src/compat.rs` `typescript_runtime_of`), which
 * the classifier compares with the cohort's binary, so a module that uses
 * nothing newer still goes to an older binary (LLP 1016.000 D10); and it
 * needs exactly the cohort's grants (the host's data source refuses any
 * other, `Paired::decode`). */
function typescriptBundle(bundle, build, baked, cohort) {
  const meta = JSON.parse(baked.receipt), runtime = meta.typescriptRuntime;
  const inputs = cohort.compat.inputs;
  if (meta.appId !== inputs.app) refuse(`the module names app ${meta.appId}, not ${inputs.app}`);
  if ((meta.grants ?? '').trim() !== (inputs.grantCeiling ?? '').trim()) refuse('the TypeScript module declares different grants than the installed binary; that needs a new binary');
  const assets = [
    { name: 'typescript/app.module.json', bytes: baked.receipt },
    { name: 'typescript/app.hbc', bytes: baked.bytecode },
  ].map((a) => ({ ...a, sha256: sha256(a.bytes) }));
  const requires = { grantCeiling: inputs.grantCeiling, ...(runtime !== undefined ? { typescriptRuntime: runtime } : {}) };
  const artifacts = build.graph.artifacts.map((a) => {
    if (a.name !== 'app.plan') return a;
    const { sources, ...rest } = a.requires;
    return { ...a, requires: { ...rest, ...requires } };
  });
  artifacts.push(...assets.map((a) => ({ name: a.name, sha256: a.sha256, bytes: a.bytes.length, kind: 'bundle', requires })));
  return { bundle: { ...bundle, assets: [...bundle.assets, ...assets] }, build: { ...build, graph: { ...build.graph, artifacts } } };
}

function planGraph(tool, app, plan, cohort, env) {
  const dir = mkdtempSync(resolve(app.target, 'push-graph-'));
  try {
    writeFileSync(resolve(dir, 'app.plan'), plan);
    writeFileSync(resolve(dir, 'compat.json'), JSON.stringify(cohort.compat));
    const r = spawnSync(tool, ['graph', resolve(dir, 'app.plan'), resolve(dir, 'compat.json'), cohort.compat.target], { env, encoding: 'utf8' });
    if (r.status !== 0) refuse(r.stderr.trim());
    return JSON.parse(r.stdout);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

/** A new stream's assets: the bake's bundle rows, read from the app's files
 * and held to the digests the binary was built with. */
function bakedAssets(app, build) {
  return build.graph.artifacts.filter((a) => a.name !== 'app.plan').map((a) => {
    const path = resolve(app.dir, a.name.startsWith('shaders/') ? `gpu/${a.name}` : a.name);
    const bytes = existsSync(path) ? readFileSync(path) : null;
    if (!bytes || sha256(bytes) !== a.sha256) refuse(`${a.name}: ${path} is not what the installed binary was built with; push to an existing stream, or rebuild the binary`);
    return { name: a.name, bytes, sha256: a.sha256 };
  });
}

/** The head's assets other than its module pair: an `assets/` or `deck/`
 * file as it is now in the app, anything else (shaders) as the head has it. */
async function carriedAssets(origin, app, head) {
  const out = [];
  for (const card of head.assets ?? []) {
    if (/^(rust|typescript)\//.test(card.name)) continue;
    const local = resolve(app.dir, card.name);
    const own = /^(assets|deck)\//.test(card.name) && existsSync(local);
    const bytes = own ? readFileSync(local) : await origin.get(blobPath(card.sha256));
    if (!bytes || (!own && sha256(bytes) !== card.sha256)) refuse(`${card.name}: the head's blob is missing from the origin`);
    out.push({ name: card.name, bytes, sha256: sha256(bytes) });
  }
  return out;
}

function gitHead() {
  const r = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : '0'.repeat(40);
}

export async function pusher(opts) {
  const app = resolveApp(opts._[0]);
  requireReleasePins(app);
  const channel = opts.channel ?? app.manifest.deploy?.channel ?? 'prod';
  const spec = opts.origin ?? channelOrigin(app.manifest, channel);
  if (!spec) refuse(`no origin for the channel ${channel}: pass --origin <dir>, or name deploy.channels.${channel} ("appdrop" needs no server)`);
  const origin = openOrigin(spec);
  if (!origin.writable) refuse(`${origin.describe()} is read-only: pass --origin <the directory it serves>`);
  const signer = loadSigner(app, opts.keys ?? process.env.EXACT_SIGNING_KEY_DIR ?? resolve(homedir(), '.config/exact/keys'));
  origin.useSigner?.(signer);
  const signingKey = { id: signer.keyId, public: app.manifest.deploy?.signing?.keys?.[signer.keyId] };
  // Release bytes: what `deploy` builds, signed as it signs a macOS module.
  const env = { ...process.env, EXACT_UPDATE_TRUST: 'production', EXACT_GAME_PROOF_PROFILE: 'release' };
  const typescript = existsSync(resolve(app.dir, 'app.ts'));
  const tools = pushTools(app, env, typescript);
  const baker = new RustBaker(app, env);

  async function push() {
    requireReleasePins(app);
    const started = performance.now();
    const found = await streams(origin, app, channel, env);
    const native = found.find((s) => ['native', 'tiered'].includes(s.cohort.compat.inputs.rustMode));
    const built = typescript ? bakeTypescript(tools.typescript, app, env)
      : await buildRust(app, { compat: found[0].cohort.compat, env, nativeTarget: native?.cohort.compat.target ?? null, profile: 'release', baker });
    if (!built) refuse(`${app.name} has neither a TypeScript module (app.ts) nor a rust.module.package; push carries a plan baked with its module`);
    const bakedIn = seconds(started);
    const release = defaultRelease(gitHead()).replace(/^r-/, 'p-');
    const snapshot = { id: gitHead(), commit: gitHead(), dirty: true };
    const results = [];
    for (const { stream, head, cohort, platform, build: binary } of found) {
      const graph = planGraph(tools.graph, app, built.plan, cohort, env);
      const inputs = cohort.compat.inputs;
      const assets = head ? await carriedAssets(origin, app, head) : bakedAssets(app, binary);
      let bundle = { plan: { bytes: built.plan, sha256: sha256(built.plan), formatVersion: inputs.formatVersion, kernelSchema: inputs.kernelSchema }, assets };
      let build = { version: 1, trust: 'production', compat: cohort.compat, binary: { sha256: cohort.binary },
        graph: { ...graph, artifacts: [graph.artifacts[0], ...assets.map((a) => ({ name: a.name, sha256: a.sha256, bytes: a.bytes.length, kind: 'bundle', requires: {} }))] } };
      ({ bundle, build } = typescript ? typescriptBundle(bundle, build, built, cohort) : rustBundle(app, bundle, build, built));
      const checked = classifyArtifacts(build, cohort, signingKey);
      if (!checked.bundle) refuse(`${platform} ${stream.compatibilityId.slice(0, 8)} cannot take this as an update — ${checked.missing.join('; ')}. It needs a new binary: bun scripts/deploy.mjs ${app.name}`);
      const row = { kind: 'stream', platform, channel, compatibilityId: stream.compatibilityId, action: 'bundle', receipt: cohort };
      results.push(await publishStream({ origin, row, bundle, compat: cohort.compat, app, signer, release, snapshot, opts: { seqAbove: opts.seqAbove }, log: () => {}, build }));
    }
    for (const r of results) {
      const what = r.action === 'current' ? 'already current' : `seq ${r.seq}: ${r.changes.map((c) => c.name).join(', ')}`;
      console.log(`  ${r.platform} ${r.compatibilityId.slice(0, 8)} — ${what}`);
    }
    console.log(`pushed ${release} in ${seconds(started)} (baked in ${bakedIn})`);
  }

  try {
    await push();
    if (!opts.watch) return 0;
    console.log(`watching ${app.dir} — save to push`);
    let timer = null, running = null, again = false;
    const kick = () => {
      if (running) { again = true; return; }
      running = push().catch((e) => console.error(`push refused: ${e.message}`)).finally(() => {
        running = null;
        if (again) { again = false; kick(); }
      });
    };
    watch(app.dir, { recursive: true }, (_, file) => {
      if (!file || /(^|\/)(target|node_modules|\.git)(\/|$)/.test(file) || !/\.(contract|rs|toml|json|png|jpe?g|svg|mjs|ts|css|html)$/.test(file)) return;
      clearTimeout(timer);
      timer = setTimeout(() => { console.log(`edit: ${relative(app.dir, resolve(app.dir, file))}`); kick(); }, 60);
    });
    await new Promise(() => {});
  } finally { baker.close(); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const run = async () => {
    const opts = parseArgs(process.argv.slice(2));
    if (opts.help || !opts._.length) { console.log(USAGE); return opts.help ? 0 : 2; }
    return pusher(opts);
  };
  run().then((code) => { process.exitCode = code; }, (e) => {
    console.error(e instanceof Refusal ? `exact push: ${e.message}` : e);
    process.exitCode = 1;
  });
}
