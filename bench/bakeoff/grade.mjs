// One blind pairwise grade: both arms' apps installed on a grader simulator, A and B by coin flip, a Claude grader
// (auto permission mode) driving them through drive.mjs.
//   bun bench/bakeoff/grade.mjs --round <dir> --spec <id> --sim <udid>
// Writes <round>/grades/<spec>/{grade.json,key.json,shots/,log.jsonl}.
import { spawn, spawnSync } from 'node:child_process';
import { createWriteStream, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const args = Object.fromEntries(process.argv.slice(2).reduce((a, x, i, all) => (x.startsWith('--') ? [...a, [x.slice(2), all[i + 1]]] : a), []));
const { round, spec, sim } = args;
if (!round || !spec || !sim) { console.error('usage: grade.mjs --round <dir> --spec <id> --sim <udid>'); process.exit(2); }
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
const arms = ['exact', pairs[spec].stack];
const records = Object.fromEntries(arms.map((a) => [a, JSON.parse(readFileSync(resolve(round, 'records', `${spec}-${a}.json`), 'utf8'))]));
const out = resolve(round, 'grades', spec);
rmSync(out, { recursive: true, force: true });
mkdirSync(resolve(out, 'shots'), { recursive: true });

const flip = Math.random() < 0.5;
const key = { A: flip ? arms[0] : arms[1], B: flip ? arms[1] : arms[0] };
writeFileSync(resolve(out, 'key.json'), JSON.stringify(key, null, 2) + '\n');

spawnSync('xcrun', ['simctl', 'boot', sim]);
const listed = spawnSync('xcrun', ['simctl', 'listapps', sim], { encoding: 'utf8' }).stdout ?? '';
for (const id of new Set(listed.match(/dev\.bakeoff\.[A-Za-z0-9.]+/g) ?? [])) spawnSync('xcrun', ['simctl', 'uninstall', sim, id]);
spawnSync('xcrun', ['simctl', 'ui', sim, 'appearance', 'light']);
spawnSync('xcrun', ['simctl', 'ui', sim, 'content_size', 'large']);
const ids = {};
for (const side of ['A', 'B']) {
  const rec = records[key[side]];
  if (!rec.app || !existsSync(rec.app)) { ids[side] = null; continue; }
  const r = spawnSync('xcrun', ['simctl', 'install', sim, rec.app], { encoding: 'utf8' });
  ids[side] = r.status === 0 ? rec.appId : null;
}

// The sheet names the driver by its repository path; the grader runs it by its absolute one.
const drive = readFileSync(resolve(HERE, 'drive.md'), 'utf8').replaceAll('bun bench/bakeoff/drive.mjs', `bun ${resolve(HERE, 'drive.mjs')}`).replaceAll('<UDID>', sim);
const prompt = readFileSync(resolve(HERE, 'prompts/grader.md'), 'utf8')
  .replaceAll('{SIM}', sim).replace('{DRIVE}', drive)
  .replace('{A}', ids.A ?? '(not installed: the build left no app; grade it as missing)')
  .replace('{B}', ids.B ?? '(not installed: the build left no app; grade it as missing)')
  .replace('{SPEC}', readFileSync(resolve(HERE, 'specs', spec, 'SPEC.md'), 'utf8'));
writeFileSync(resolve(out, 'prompt.md'), prompt);

const env = { ...process.env };
for (const k of Object.keys(env)) if (k.startsWith('CLAUDE_CODE') || k === 'CLAUDECODE') delete env[k];
const started = Date.now();
const log = createWriteStream(resolve(out, 'log.jsonl'));
const child = spawn('claude', ['-p', '--model', 'claude-opus-5-5', '--permission-mode', 'auto', '--output-format', 'stream-json', '--verbose'], { cwd: out, env, stdio: ['pipe', 'pipe', 'pipe'] });
child.stdin.end(prompt);
child.stdout.pipe(log);
child.stderr.on('data', (d) => log.write(JSON.stringify({ stderr: String(d) }) + '\n'));
const timer = setTimeout(() => child.kill('SIGTERM'), Number(args.limit ?? 60) * 60000);
const code = await new Promise((r) => child.on('exit', r));
clearTimeout(timer);
spawnSync('xcrun', ['simctl', 'ui', sim, 'appearance', 'light']);
spawnSync('xcrun', ['simctl', 'ui', sim, 'content_size', 'large']);
let grade = null;
try { grade = JSON.parse(readFileSync(resolve(out, 'grade.json'), 'utf8')); } catch {}
const minutes = Math.round((Date.now() - started) / 600) / 100;
writeFileSync(resolve(out, 'meta.json'), JSON.stringify({ spec, key, exit: code, minutes, shots: readdirSync(resolve(out, 'shots')).length, graded: !!grade }, null, 2) + '\n');
const winner = grade ? (grade.better === 'tie' ? 'tie' : key[grade.better]) : 'ungraded';
console.log(`${spec}: ${winner} (${minutes} min)`);
