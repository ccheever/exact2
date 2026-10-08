#!/usr/bin/env bun
// The garden bakeoff (LLP 1046.010): the same brief and art, built once per engine by
// the same model, measured the same way.
//
//   bun game/bakeoff/garden/run.mjs prepare [--run <id>] [--lanes exact2,three,godot,unity]
//   bun game/bakeoff/garden/run.mjs build --run <id> [--cap-min 240]
//   bun game/bakeoff/garden/run.mjs status --run <id>
//   bun game/bakeoff/garden/run.mjs selfscore --run <id>
//   bun game/bakeoff/garden/run.mjs report --run <id>
//
// Fidelity is judged between selfscore and report by the orchestrating agent, one
// judge per lane with JUDGE.md (LLP 1046.010 §5); report reads <lane>/FIDELITY.json.
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync, appendFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { extname, join, relative, resolve } from 'node:path';

const HERE = import.meta.dir;
const EXACT2 = resolve(HERE, '../../..');
const ROOT = process.env.BAKEOFF_ROOT ?? join(homedir(), 'Library/Caches/exact2-game/bakeoff/garden');
const GROK = process.env.BAKEOFF_GROK ?? join(homedir(), '.grok/bin/grok');
const MODEL = process.env.BAKEOFF_MODEL ?? 'grok-4.7';
const EFFORT = process.env.BAKEOFF_EFFORT ?? 'high';
const LANES = JSON.parse(readFileSync(join(HERE, 'lanes.json'), 'utf8'));
const REFERENCE = ['arrival.png', 'walk.png', 'grown.png', 'market.png', 'planted.png', 'ripe.png', 'watering.png'];
// A headless session ends when the model ends its turn — including when it has just
// started a slow command in the background expecting to be woken. Resumes are how a
// lane survives that; the cap on wall clock, not this count, is what bounds a lane.
const MAX_RESUMES = Number(process.env.BAKEOFF_MAX_RESUMES ?? 8);
const HEADLESS = ' This session is headless: it ends the moment you end your turn, and nothing wakes you when a background command finishes. Run builds and proofs in the foreground and wait for them.';

const args = process.argv.slice(2);
const verb = args[0];
const flag = (name, fallback) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback; };
const stamp = () => new Date().toISOString();
const runDir = id => join(ROOT, id);
const readJson = (path, fallback = null) => { try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return fallback; } };
const writeJson = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + '\n');
const fill = (text, vars) => text.replace(/\{(\w+)\}/g, (m, k) => vars[k] ?? m);

function unityLicensed() {
  const dirs = ['/Library/Application Support/Unity', join(homedir(), 'Library/Unity/licenses')];
  return dirs.some(d => existsSync(d) && readdirSync(d).some(f => /\.(ulf|xml)$/.test(f)));
}

function prepare() {
  const id = flag('run', new Date().toISOString().slice(0, 16).replace(/[:T]/g, '-'));
  const lanes = flag('lanes', 'exact2,three,godot,unity').split(',');
  const dir = runDir(id);
  for (const lane of lanes) if (!LANES[lane]) throw new Error(`unknown lane ${lane}`);
  const inputs = {};
  function fingerprint(dir) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === '.DS_Store') continue;
      const path = join(dir, entry.name);
      if (entry.isDirectory()) fingerprint(path);
      else inputs[relative(HERE, path)] = createHash('sha256').update(readFileSync(path)).digest('hex');
    }
  }
  fingerprint(join(HERE, 'art'));
  for (const file of ['BRIEF.md', 'ART.md', 'JUDGE.md', 'SELFSCORE.md', 'lanes.json', ...REFERENCE.map(f => `reference/${f}`)]) {
    inputs[file] = createHash('sha256').update(readFileSync(join(HERE, file))).digest('hex');
  }
  if (existsSync(dir)) throw new Error(`run ${id} exists: ${dir}`);
  const head = spawnSync('git', ['-C', EXACT2, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
  const run = { id, created: stamp(), exact2: head, model: MODEL, effort: EFFORT, inputs, lanes: {} };
  for (const lane of lanes) {
    if (!LANES[lane]) throw new Error(`unknown lane ${lane}`);
    const at = join(dir, lane);
    mkdirSync(join(at, '.bakeoff'), { recursive: true });
    cpSync(join(HERE, 'BRIEF.md'), join(at, 'BRIEF.md'));
    cpSync(join(HERE, 'ART.md'), join(at, 'ART.md'));
    cpSync(join(HERE, 'art'), join(at, 'art'), { recursive: true, filter: s => !s.endsWith('.DS_Store') });
    mkdirSync(join(at, 'reference'));
    for (const file of REFERENCE) cpSync(join(HERE, 'reference', file), join(at, 'reference', file));
    const vars = { EXACT2, LANE: at };
    writeFileSync(join(at, 'LANE.md'), `# Your lane: ${LANES[lane].engine}\n\nYour lane directory is \`${at}\`. Build everything inside it.\n\n${fill(LANES[lane].note, vars)}\n`);
    const blocked = lane === 'unity' && !unityLicensed() ? 'not run: no Unity license on this Mac' : null;
    run.lanes[lane] = { dir: at, blocked };
  }
  writeJson(join(dir, 'run.json'), run);
  console.log(`prepared ${dir}`);
  for (const [lane, l] of Object.entries(run.lanes)) console.log(`  ${lane.padEnd(7)} ${l.blocked ?? l.dir}`);
}

function grokArgs({ session, resume, fork, cwd, prompt }) {
  const base = ['-m', MODEL, '--effort', EFFORT, '--sandbox', 'off', '--always-approve', '--no-subagents',
    '--max-turns', '5000', '--cwd', cwd, '--output-format', 'streaming-json'];
  if (resume) base.push('--resume', resume);
  if (fork) base.push('--fork-session');
  if (session) base.push('--session-id', session);
  return [...base, '-p', prompt];
}

function runGrok(opts, log) {
  return new Promise((done, reject) => {
    const started = Date.now();
    const child = spawn(GROK, grokArgs(opts), { cwd: opts.cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    appendFileSync(log, JSON.stringify({ bakeoff: 'spawn', pid: child.pid, at: stamp() }) + '\n');
    child.stdout.on('data', d => appendFileSync(log, d));
    child.stderr.on('data', d => appendFileSync(log + '.stderr', d));
    const timer = opts.deadline ? setTimeout(() => { appendFileSync(log, JSON.stringify({ bakeoff: 'cap', at: stamp() }) + '\n'); child.kill('SIGTERM'); }, Math.max(1000, opts.deadline - Date.now())) : null;
    child.on('error', error => { if (timer) clearTimeout(timer); reject(error); });
    child.on('exit', code => { if (timer) clearTimeout(timer); done({ code, ms: Date.now() - started }); });
  });
}

const BUILD_PROMPT = lane => `Read BRIEF.md, then LANE.md, ART.md and the reference screenshots in this directory (${lane}). Build the game the brief describes in the engine LANE.md names, keeping DIARY.md as the brief says, until the proof passes or you have done all you can. Work autonomously: there is no one to answer questions. Finish by writing DONE.json as BRIEF.md §7 says.${HEADLESS}`;
const RESUME_PROMPT = 'You stopped before writing DONE.json. Continue the build from where you left off (DIARY.md says where you were), keep the diary, and finish by writing DONE.json as BRIEF.md §7 says.' + HEADLESS;

async function buildLane(id, lane, capMs) {
  const run = readJson(join(runDir(id), 'run.json'));
  const l = run.lanes[lane];
  const meta = join(l.dir, '.bakeoff');
  const log = join(meta, 'build.ndjson');
  const state = readJson(join(meta, 'build.json'), { session: crypto.randomUUID(), segments: [] });
  const deadline = Date.now() + capMs - state.segments.reduce((s, x) => s + x.ms, 0);
  while (!existsSync(join(l.dir, 'DONE.json')) && state.segments.length <= MAX_RESUMES && Date.now() < deadline) {
    const first = state.segments.length === 0;
    const segment = { started: stamp(), resume: !first };
    const result = await runGrok(first
      ? { session: state.session, cwd: l.dir, prompt: BUILD_PROMPT(l.dir), deadline }
      : { resume: state.session, cwd: l.dir, prompt: RESUME_PROMPT, deadline }, log);
    state.segments.push({ ...segment, ...result, ended: stamp() });
    writeJson(join(meta, 'build.json'), state);
    console.log(`[${lane}] segment ${state.segments.length} exit ${result.code} after ${(result.ms / 60000).toFixed(1)} min`);
  }
  state.done = existsSync(join(l.dir, 'DONE.json'));
  state.capped = Date.now() >= deadline && !state.done;
  state.wall_ms = state.segments.reduce((s, x) => s + x.ms, 0);
  writeJson(join(meta, 'build.json'), state);
  collectUsage(l.dir, state.session, 'build');
  console.log(`[${lane}] ${state.done ? 'DONE' : state.capped ? 'CAPPED' : 'STOPPED'} in ${(state.wall_ms / 60000).toFixed(1)} min`);
}

function collectUsage(dir, session, kind) {
  const usage = spawnSync(GROK, ['usage', session], { encoding: 'utf8' });
  writeFileSync(join(dir, '.bakeoff', `${kind}-usage.json`), usage.stdout || usage.stderr);
  const exported = spawnSync(GROK, ['export', session], { encoding: 'utf8', maxBuffer: 1 << 28 });
  writeFileSync(join(dir, '.bakeoff', `${kind}-transcript.md`), exported.stdout || exported.stderr);
}

async function build() {
  const id = flag('run');
  const capMs = Number(flag('cap-min', '240')) * 60000;
  const run = readJson(join(runDir(id), 'run.json'));
  const lanes = Object.entries(run.lanes).filter(([, l]) => !l.blocked).map(([k]) => k);
  console.log(`building ${lanes.join(', ')} in parallel (cap ${capMs / 60000} min each)`);
  await Promise.all(lanes.map(lane => buildLane(id, lane, capMs)));
}

function status() {
  const id = flag('run');
  const run = readJson(join(runDir(id), 'run.json'));
  for (const [lane, l] of Object.entries(run.lanes)) {
    if (l.blocked) { console.log(`${lane.padEnd(7)} ${l.blocked}`); continue; }
    const st = readJson(join(l.dir, '.bakeoff/build.json'), { segments: [] });
    const diary = existsSync(join(l.dir, 'DIARY.md')) ? readFileSync(join(l.dir, 'DIARY.md'), 'utf8').trim().split('\n').filter(Boolean).slice(-1)[0] : '(no diary yet)';
    const logPath = join(l.dir, '.bakeoff/build.ndjson');
    const age = existsSync(logPath) ? Math.round((Date.now() - statSync(logPath).mtimeMs) / 1000) : null;
    console.log(`${lane.padEnd(7)} segments ${st.segments.length} done ${existsSync(join(l.dir, 'DONE.json'))} log-age ${age}s\n        ${diary.slice(0, 160)}`);
  }
}

async function selfscore() {
  const id = flag('run');
  const run = readJson(join(runDir(id), 'run.json'));
  const text = readFileSync(join(HERE, 'SELFSCORE.md'), 'utf8');
  await Promise.all(Object.entries(run.lanes).filter(([, l]) => !l.blocked).map(async ([lane, l]) => {
    const st = readJson(join(l.dir, '.bakeoff/build.json'));
    const session = crypto.randomUUID();
    const prompt = fill(text, { ENGINE: LANES[lane].engine, LANE: l.dir });
    const result = await runGrok({ resume: st.session, fork: true, session, cwd: l.dir, prompt }, join(l.dir, '.bakeoff/selfscore.ndjson'));
    writeJson(join(l.dir, '.bakeoff/selfscore.json'), { session, ...result });
    collectUsage(l.dir, session, 'selfscore');
    console.log(`[${lane}] selfscore ${existsSync(join(l.dir, 'SELFSCORE.json')) ? 'written' : 'MISSING'}`);
  }));
}

const CODE = new Set(['.rs', '.contract', '.gd', '.tscn', '.tres', '.js', '.mjs', '.ts', '.html', '.css', '.cs', '.py', '.sh', '.godot', '.toml', '.json']);
const SKIP = new Set(['node_modules', '.godot', 'target', 'dist', 'dist.previous', '.shells', 'artifacts', 'art', 'reference', 'judge', '.bakeoff', 'Library', 'Temp', 'Logs', 'obj', '.git', 'assets', 'test-results', 'playwright-report']);
const SKIP_FILES = /^(DONE|SELFSCORE|FIDELITY|package-lock|bun\.lock|Cargo\.lock|pins|app\.contract\.d)\b|\.import$|\.uid$|\.meta$/;
function countLines(dir, into = { files: 0, lines: 0, byExt: {} }) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) { if (!SKIP.has(e.name)) countLines(join(dir, e.name), into); continue; }
    const ext = extname(e.name);
    if (!CODE.has(ext) || SKIP_FILES.test(e.name)) continue;
    const n = readFileSync(join(dir, e.name), 'utf8').split('\n').filter(s => s.trim()).length;
    into.files++; into.lines += n; into.byExt[ext] = (into.byExt[ext] ?? 0) + n;
  }
  return into;
}

function usageOf(dir, kind) {
  const u = readJson(join(dir, '.bakeoff', `${kind}-usage.json`));
  const s = u?.session ?? {};
  return { input: s.inputTokens ?? null, uncached_input: s.inputTokens != null ? s.inputTokens - (s.cachedReadTokens ?? 0) : null,
    output: s.outputTokens ?? null, cached: s.cachedReadTokens ?? null,
    reasoning: s.reasoningTokens ?? null, total: s.totalTokens ?? null, calls: s.modelCalls ?? null,
    cost_usd: s.costUsdTicks != null ? s.costUsdTicks / 1e10 : null };
}

function report() {
  const id = flag('run');
  const run = readJson(join(runDir(id), 'run.json'));
  const out = join(HERE, 'results', id);
  mkdirSync(out, { recursive: true });
  const rows = {};
  for (const [lane, l] of Object.entries(run.lanes)) {
    if (l.blocked) { rows[lane] = { blocked: l.blocked }; continue; }
    const st = readJson(join(l.dir, '.bakeoff/build.json'), {});
    const row = {
      engine: LANES[lane].engine, done: !!st.done, capped: !!st.capped, segments: st.segments?.length ?? 0,
      wall_min: st.wall_ms != null ? +(st.wall_ms / 60000).toFixed(1) : null,
      tokens: usageOf(l.dir, 'build'), code: countLines(l.dir),
      // A forked session's usage includes its parent's: the self-score is the difference.
      selfscore_tokens: (usageOf(l.dir, 'selfscore').total ?? 0) - (usageOf(l.dir, 'build').total ?? 0),
      done_json: readJson(join(l.dir, 'DONE.json')), selfscore: readJson(join(l.dir, 'SELFSCORE.json')),
      fidelity: readJson(join(l.dir, 'FIDELITY.json')),
    };
    rows[lane] = row;
    mkdirSync(join(out, lane), { recursive: true });
    for (const f of ['DIARY.md', 'DONE.json', 'SELFSCORE.json', 'FIDELITY.json', 'README.md']) if (existsSync(join(l.dir, f))) cpSync(join(l.dir, f), join(out, lane, f));
  }
  writeJson(join(out, 'metrics.json'), { run: { id, exact2: run.exact2, model: run.model, effort: run.effort, created: run.created, ...(run.inputs ? { inputs: run.inputs } : {}) }, lanes: rows });
  const live = Object.entries(rows).filter(([, r]) => !r.blocked);
  const num = v => v == null ? '—' : typeof v === 'number' ? (Number.isInteger(v) ? v.toLocaleString('en-US') : v.toFixed(1)) : String(v);
  const line = (label, f) => `| ${label} | ${live.map(([, r]) => num(f(r))).join(' | ')} |`;
  const subs = ['setup', 'docs', 'loop', 'verification', 'debugging', 'assets', 'ui', 'api', 'reliability', 'visuals'];
  const md = [
    `# Garden bakeoff — run ${id}`, '',
    `Builder ${run.model} (${run.effort}), exact2 at ${run.exact2.slice(0, 10)}. ` + Object.entries(rows).filter(([, r]) => r.blocked).map(([k, r]) => `${k}: ${r.blocked}.`).join(' '), '',
    `| | ${live.map(([k]) => k).join(' | ')} |`, `|---|${live.map(() => '---:').join('|')}|`,
    line('finished (DONE.json)', r => r.done ? 'yes' : r.capped ? 'capped' : 'no'),
    line('proof passed (judge rerun)', r => r.fidelity?.proof_rerun ? (r.fidelity.proof_rerun.passed ? 'yes' : 'no') : null),
    line('wall clock, min', r => r.wall_min),
    line('tokens, total', r => r.tokens.total), line('… uncached input', r => r.tokens.uncached_input),
    line('… output (incl. reasoning)', r => r.tokens.output),
    line('model calls', r => r.tokens.calls), line('cost, USD', r => r.tokens.cost_usd),
    line('lines of game code', r => r.code.lines), line('files', r => r.code.files),
    line('**fidelity** (0–100)', r => r.fidelity?.fidelity),
    line('… world / rules / ui / proof %', r => r.fidelity ? `${r.fidelity.world}/${r.fidelity.rules}/${r.fidelity.ui}/${r.fidelity.proof}` : null),
    line('… look (0–10)', r => r.fidelity?.look?.[0]),
    line('**ergonomics, overall** (0–10)', r => r.selfscore?.overall),
    ...subs.map(s => line(`… ${s}`, r => r.selfscore?.subscores?.[s])),
    '',
  ].join('\n');
  writeFileSync(join(out, 'RESULTS.md'), md);
  console.log(md);
  console.log(`wrote ${relative(EXACT2, out)}`);
}

const verbs = { prepare, build, status, selfscore, report };
if (!verbs[verb]) { console.error('usage: run.mjs prepare|build|status|selfscore|report --run <id>'); process.exit(2); }
await verbs[verb]();
