// One build of one spec in one arm: make the working directory, run the builder unattended, record wall clock and
// tokens, and keep a copy of the app it left installed.
//   bun bench/bakeoff/build.mjs --round <dir> --spec <id> --arm exact|swiftui|uikit --sim <udid> --pin <exact2 checkout>
// Writes <round>/records/<spec>-<arm>.json and <round>/logs/<spec>-<arm>.jsonl; the app is <round>/apps/<spec>-<arm>.
// Builders run gated, never unsandboxed: Codex in its workspace-write sandbox with escalations reviewed
// (`--approve-for-me`), Claude Code in its `auto` permission mode.
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, createWriteStream, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, loadavg } from 'node:os';
import { resolve } from 'node:path';
import { randomBytes } from 'node:crypto';
import net from 'node:net';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const args = Object.fromEntries(process.argv.slice(2).reduce((a, x, i, all) => (x.startsWith('--') ? [...a, [x.slice(2), all[i + 1]]] : a), []));
const { round, spec, arm, sim, pin } = args;
if (!round || !spec || !arm || !sim || !pin) { console.error('usage: build.mjs --round <dir> --spec <id> --arm exact|swiftui|uikit --sim <udid> --pin <exact2> [--cold 1] [--server-port n]'); process.exit(2); }
const LIMIT_MIN = Number(args.limit ?? 75);
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
// The plan fixes both arms to the same builder so the comparison measures the toolkit rather than model choice.
const model = 'gpt';
const name = `${spec}-${arm}`;
const dir = resolve(round, 'apps', name);
for (const d of ['records', 'logs', 'apps', 'built']) mkdirSync(resolve(round, d), { recursive: true });

// Neutral identity: the grader must not learn the arm from the bundle id or the home-screen name.
const appId = `dev.bakeoff.${spec.replace(/[^a-z0-9]/g, '')}.b${randomBytes(3).toString('hex')}`;
const appName = readFileSync(resolve(HERE, 'specs', spec, 'SPEC.md'), 'utf8').match(/^# (.+)$/m)[1];

rmSync(dir, { recursive: true, force: true });
if (arm === 'exact') {
  const r = spawnSync('bun', [resolve(pin, 'scripts/exact.mjs'), 'new', dir], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`exact new failed: ${r.stderr}${r.stdout}`);
} else cpSync(resolve(HERE, 'native'), dir, { recursive: true });
if (existsSync(resolve(HERE, 'specs', spec, 'data'))) cpSync(resolve(HERE, 'specs', spec, 'data'), resolve(dir, 'data'), { recursive: true });
writeFileSync(resolve(dir, 'SPEC.md'), readFileSync(resolve(HERE, 'specs', spec, 'SPEC.md')));

// A clean simulator: uninstall earlier bake-off apps (never erase: first boots load the machine).
spawnSync('xcrun', ['simctl', 'boot', sim]);
const listed = spawnSync('xcrun', ['simctl', 'listapps', sim], { encoding: 'utf8' }).stdout ?? '';
for (const id of new Set(listed.match(/dev\.bakeoff\.[A-Za-z0-9.]+/g) ?? [])) spawnSync('xcrun', ['simctl', 'uninstall', sim, id]);

const serverPort = Number(args.serverPort ?? 43000);
const fill = (t) => t.replaceAll('{SIM}', sim).replaceAll('{APP_ID}', appId).replaceAll('{APP_NAME}', appName).replaceAll('{SERVER_PORT}', String(serverPort));
const prompt = fill(readFileSync(resolve(HERE, 'prompts/common.md'), 'utf8')
  .replace('{STACK}', readFileSync(resolve(HERE, 'prompts', `${arm}.md`), 'utf8'))
  .replace('{SPEC}', readFileSync(resolve(HERE, 'specs', spec, 'SPEC.md'), 'utf8')));
writeFileSync(resolve(round, 'logs', `${name}.prompt.md`), prompt);

const env = { ...process.env, EXACT_SIM: sim, SIM: sim };
for (const k of Object.keys(env)) if (k.startsWith('CLAUDE_CODE') || k === 'CLAUDECODE') delete env[k];
const [cmd, argv] = ['codex', ['exec', '--json', '--skip-git-repo-check', '--approve-for-me', '-c', 'sandbox_workspace_write.network_access=true', '-m', 'gpt-6.1-sol', '-c', 'model_reasoning_effort="high"', '-C', dir, '-']];
const log = createWriteStream(resolve(round, 'logs', `${name}.jsonl`));
let started;
const usage = { input: 0, cached: 0, output: 0, cost: null, calls: 0, toolCalls: 0 };
let timedOut = false, atCapacity = false, worked = false, thread = null, workMs = 0;

const serverFile = resolve(HERE, 'specs', spec, 'server/server.mjs');
const portOpen = () => new Promise((done) => {
  const socket = net.connect({ host: '127.0.0.1', port: serverPort });
  socket.once('connect', () => { socket.destroy(); done(true); });
  socket.once('error', () => { socket.destroy(); done(false); });
});
async function startServer() {
  if (!existsSync(serverFile)) return null;
  if (await portOpen()) throw new Error(`server port ${serverPort} already in use`);
  const child = spawn('bun', [serverFile, '--port', String(serverPort)], { cwd: dir, env, stdio: ['ignore', 'pipe', 'pipe'] });
  const output = [];
  child.stdout.on('data', (data) => output.push(String(data)));
  child.stderr.on('data', (data) => output.push(String(data)));
  for (const until = Date.now() + 15000; Date.now() < until;) {
    if (await portOpen()) return { child, owned: true };
    if (child.exitCode != null) break;
    await new Promise((r) => setTimeout(r, 100));
  }
  try { child.kill('SIGTERM'); } catch {}
  throw new Error(`server failed to listen on ${serverPort}: ${output.join('')}`);
}
async function stopServer(server) {
  if (!server?.owned || !server.child.pid) return;
  if (server.child.exitCode != null) return;
  try { server.child.kill('SIGTERM'); } catch {}
  await new Promise((done) => server.child.once('exit', done));
}
function enterColdCache() {
  if (args.cold !== '1') return null;
  const cache = resolve(homedir(), '.cache/exact');
  const backup = resolve(round, 'cold-cache', `${name}-${process.pid}`);
  mkdirSync(resolve(round, 'cold-cache'), { recursive: true });
  const hadCache = existsSync(cache);
  // The home cache and the round are on different volumes, so rename cannot move the cache here.
  // Copy it to the SSD before clearing home, then restore it after the isolated build.
  if (hadCache) { cpSync(cache, backup, { recursive: true }); rmSync(cache, { recursive: true, force: true }); }
  return { cache, backup, hadCache };
}
function leaveColdCache(state) {
  if (!state) return;
  rmSync(state.cache, { recursive: true, force: true });
  if (state.hadCache) { cpSync(state.backup, state.cache, { recursive: true }); rmSync(state.backup, { recursive: true, force: true }); }
}
// A model at capacity is the service's failure, not the build's. Refused before any work, the run starts
// again; cut off mid-run, a Codex session resumes where it stopped. Up to six tries, waiting longer each
// time; the wall clock is the tries' own time, not the waits between them.
const RESUME = 'The service interrupted your run (the model was at capacity). Continue from where you left off and finish the app as the original instructions say, including NOTES.md.';
let code;
const coldState = enterColdCache();
// Do not charge the SSD backup and restore for the builder's first-build time.
started = Date.now();
let server;
try {
  server = await startServer();
  for (let attempt = 0; ; attempt++) {
    atCapacity = false;
    const resume = worked && thread && model === 'gpt';
    const t0 = Date.now();
    code = await runOnce(resume ? [...argv.slice(0, -1), 'resume', thread, '-'] : argv, resume ? RESUME : prompt);
    workMs += Date.now() - t0;
    if (!atCapacity || attempt >= 5 || (worked && !thread)) break;
    log.write(JSON.stringify({ retry: attempt + 1, reason: 'model at capacity', resume: worked && !!thread }) + '\n');
    await new Promise((r) => setTimeout(r, 90000 * (attempt + 1)));
  }
} finally {
  await stopServer(server);
  leaveColdCache(coldState);
}

function runOnce(args, input) {
const child = spawn(cmd, args, { cwd: dir, env, stdio: ['pipe', 'pipe', 'pipe'] });
child.stdin.end(input);
let buf = '';
child.stdout.on('data', (d) => {
  log.write(d);
  if (/at capacity/.test(d)) atCapacity = true;
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i); buf = buf.slice(i + 1);
    let e; try { e = JSON.parse(line); } catch { continue; }
    if (e.type === 'thread.started' && e.thread_id) thread = e.thread_id;
    if (e.type === 'item.completed' || e.type === 'assistant') worked = true;
    if (e.type === 'turn.completed' && e.usage) { // Codex: one per turn
      usage.input += e.usage.input_tokens ?? 0; usage.cached += e.usage.cached_input_tokens ?? 0;
      usage.output += e.usage.output_tokens ?? 0;
    }
    if (e.type === 'item.completed' && /command|tool|patch|file/.test(e.item?.type ?? '')) usage.toolCalls++;
    if (e.type === 'item.completed' && e.item?.type === 'agent_message') usage.calls++;
    if (e.type === 'assistant') { usage.calls++; usage.toolCalls += (e.message?.content ?? []).filter((c) => c.type === 'tool_use').length; }
    if (e.type === 'result') { // Claude Code: every model's totals, subagents included
      usage.input = 0; usage.cached = 0; usage.output = 0;
      for (const m of Object.values(e.modelUsage ?? {})) {
        usage.input += (m.inputTokens ?? 0) + (m.cacheReadInputTokens ?? 0) + (m.cacheCreationInputTokens ?? 0);
        usage.cached += m.cacheReadInputTokens ?? 0; usage.output += m.outputTokens ?? 0;
      }
      usage.cost = e.total_cost_usd ?? null;
    }
  }
});
child.stderr.on('data', (d) => log.write(JSON.stringify({ stderr: String(d) }) + '\n'));
const timer = setTimeout(() => { timedOut = true; child.kill('SIGTERM'); setTimeout(() => child.kill('SIGKILL'), 10000); }, LIMIT_MIN * 60000);
return new Promise((r) => child.on('exit', (c) => { clearTimeout(timer); r(c); }));
}
const minutes = workMs / 60000;

// The id the builder actually set (it may have kept another).
let finalId = appId;
try {
  if (arm === 'exact') finalId = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).app?.id ?? appId;
  else finalId = readFileSync(resolve(dir, 'app.env'), 'utf8').match(/APP_ID="?([^"\n]+)/)?.[1] ?? appId;
} catch {}
// The app as the builder left it installed; a native build's own copy if the simulator has none.
let app = null;
const installed = spawnSync('xcrun', ['simctl', 'get_app_container', sim, finalId, 'app'], { encoding: 'utf8' });
if (installed.status === 0) app = installed.stdout.trim();
else if (arm !== 'exact' && existsSync(resolve(dir, 'build/App.app'))) app = resolve(dir, 'build/App.app');
else if (arm === 'exact') { // the newest simulator build under its id in the app's build tree
  const found = spawnSync('find', [resolve(dir, 'target'), '-maxdepth', '9', '-type', 'd', '-name', '*.app', '-path', `*${finalId}*ios-simulator*`], { encoding: 'utf8' }).stdout.split('\n').filter(Boolean);
  app = found.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0] ?? null;
}
const kept = resolve(round, 'built', `${name}.app`);
rmSync(kept, { recursive: true, force: true });
if (app) cpSync(app, kept, { recursive: true });

const record = { spec, arm, model, cold: args.cold === '1', load: Math.round(loadavg()[0] * 100) / 100, appId: finalId, assignedId: appId, appName, sim,
  pin: spawnSync('git', ['-C', pin, 'rev-parse', '--short=10', 'HEAD'], { encoding: 'utf8' }).stdout.trim(),
  minutes: Math.round(minutes * 100) / 100, exit: code, timedOut,
  tokens: { total: usage.input + usage.output, fresh: usage.input - usage.cached, ...usage },
  app: app ? kept : null, notes: existsSync(resolve(dir, 'NOTES.md')), finished: new Date().toISOString() };
writeFileSync(resolve(round, 'records', `${name}.json`), JSON.stringify(record, null, 2) + '\n');
console.log(`${name}: ${record.minutes} min, ${(record.tokens.total / 1e6).toFixed(2)} Mtok, exit ${code}${timedOut ? ' (timed out)' : ''}, app ${app ? 'kept' : 'MISSING'}`);
