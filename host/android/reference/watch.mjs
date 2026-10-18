// The Material oracle's hourly job (LLP 1115.001.001 D8, D9), run by cron on the Linux build box from a worktree of
// its own, which it moves to origin/main each run.
//
// 1. A release: when Google Maven has a stable Compose Material 3 newer than the pin, it moves the pin on a branch
//    `material-bump/<version>` (the BOM too, when a stable BOM resolves to it yet), writes the token diff, runs the
//    oracle there, pushes the branch and opens one GitHub issue with the work list. So the work list exists within the
//    hour of a release (§5 Q3).
// 2. A regression: when origin/main moved and touched the Android look, the reference or the pins, it runs the oracle
//    and opens an issue when a specimen that passed before fails now. It blocks nothing (§5 Q4).
//
//   bun host/android/reference/watch.mjs [--dry-run] [--here]
//
// It needs what `oracle.mjs` needs (JAVA_HOME a JDK 21, ANDROID_HOME, Cargo, Bun 1.4.2 on PATH), `gh` signed in, and
// a push to origin. Its state is `$EXACT_MATERIAL_STATE` (default ~/.cache/exact/material-watch.json). `--dry-run`
// files nothing and pushes nothing; `--here` leaves this checkout where it is, to try the job on a pin of one's own.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const here = dirname(new URL(import.meta.url).pathname);
const root = resolve(here, '../../..');
const dry = process.argv.includes('--dry-run');
const here_ = process.argv.includes('--here');
const stateFile = process.env.EXACT_MATERIAL_STATE ?? join(homedir(), '.cache/exact/material-watch.json');
const state = existsSync(stateFile) ? JSON.parse(readFileSync(stateFile, 'utf8')) : { filed: [], checked: null, passing: null };
const save = () => { mkdirSync(dirname(stateFile), { recursive: true }); writeFileSync(stateFile, JSON.stringify(state, null, 2)); };
/** What a change must touch for the oracle to run again. */
const WATCHED = ['host/linux/src/paint/', 'host/linux/src/presenter/', 'host/android/', 'apps/material-specimens/',
  'kernel/tables/schema.json'];
const MAVEN = 'https://dl.google.com/android/maven2/androidx/compose';

const git = (args, cwd = root) => {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr.trim()}`);
  return r.stdout.trim();
};
const run = (cmd, args, cwd) => spawnSync(cmd, args, { cwd, encoding: 'utf8', maxBuffer: 1 << 28 });
const log = (...a) => console.log(new Date().toISOString(), ...a);

git(['fetch', '-q', 'origin']);
const tip = here_ ? git(['rev-parse', 'HEAD']) : git(['rev-parse', 'origin/main']);
if (!here_) git(['checkout', '-q', '--detach', tip]);
const pin = (toml) => ({ material3: toml.match(/^material3 = "([^"]+)"/m)[1], bom: toml.match(/^composeBom = "([^"]+)"/m)[1] });
const pinned = pin(readFileSync(join(root, 'host/android/gradle/libs.versions.toml'), 'utf8'));
await release();
regression();
save();

async function release() {
  const stable = (xml) => [...xml.matchAll(/<version>([^<]+)<\/version>/g)].map((m) => m[1]).filter((v) => /^\d+(\.\d+)+$/.test(v));
  const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0); return false; };
  const versions = stable(await (await fetch(`${MAVEN}/material3/material3/maven-metadata.xml`)).text());
  const latest = versions.reduce((a, b) => (newer(b, a) ? b : a), pinned.material3);
  if (latest === pinned.material3) return log(`Material 3 ${pinned.material3} is the newest stable release`);
  if (state.filed.includes(latest)) return log(`Material 3 ${latest} is filed already`);
  log(`Material 3 ${latest} is out; the pin is ${pinned.material3}`);
  // The newest stable BOM that resolves to it, if one has shipped.
  const boms = stable(await (await fetch(`${MAVEN}/compose-bom/maven-metadata.xml`)).text()).sort((a, b) => (newer(a, b) ? -1 : 1));
  let bom = null;
  for (const b of boms.slice(0, 6)) {
    const pom = await (await fetch(`${MAVEN}/compose-bom/${b}/compose-bom-${b}.pom`)).text();
    if (new RegExp(`<artifactId>material3</artifactId>\\s*<version>${latest.replace(/\./g, '\\.')}</version>`).test(pom)) { bom = b; break; }
  }
  const branch = `material-bump/${latest}`;
  const dir = join(dirname(root), `${root.split('/').pop()}-bump`);
  if (!existsSync(dir)) git(['worktree', 'add', '-q', '--detach', dir, tip]);
  git(['checkout', '-q', '-B', branch, tip], dir);
  const tomlPath = join(dir, 'host/android/gradle/libs.versions.toml');
  let toml = readFileSync(tomlPath, 'utf8').replace(/^material3 = "[^"]+"/m, `material3 = "${latest}"`);
  // Without a BOM that has it, `material3` is named by its own version, which outranks the BOM's.
  toml = bom ? toml.replace(/^composeBom = "[^"]+"/m, `composeBom = "${bom}"`)
    : toml.replace(/^compose-material3 = \{ module = "androidx.compose.material3:material3" \}/m,
      'compose-material3 = { module = "androidx.compose.material3:material3", version.ref = "material3" }');
  writeFileSync(tomlPath, toml);
  git(['commit', '-q', '-am', `Android: Compose Material 3 ${latest}${bom ? `, BOM ${bom}` : ''} (LLP 1115.001.001 D8, the hourly job)`], dir);
  const diff = run('bun', ['host/android/reference/tokens.mjs', '--diff', pinned.material3], dir);
  const check = run('bun', ['host/android/reference/tokens.mjs'], dir);
  const oracle = run('bun', ['host/android/reference/oracle.mjs'], dir);
  const report = readReport(dir);
  const tokens = diff.stdout.split('\n').filter((l) => l.includes('→'));
  const body = [`Compose Material 3 **${latest}** is on Google Maven; the pin is ${pinned.material3}${bom ? ` (BOM ${bom} has it)` : ' (no stable BOM has it yet: the branch names `material3` by version)'}.`,
    `Branch \`${branch}\` moves the pin (LLP 1115.001.001 D8). Until its fixes land, or the matches it breaks are revoked, it does not land.`, '',
    `**The Material rows** (\`tokens.mjs\`): ${check.status === 0 ? 'every row agrees with the new source' : 'some rows differ:'}`, '```', check.stdout.trim().split('\n').filter((l) => !l.includes(' ok ')).join('\n'), '```', '',
    `**Tokens changed:** ${tokens.length}. The oracle's specimens, against Exact: ${report ? `${report.failing.length} of ${report.total} differ` : `the oracle did not finish (exit ${oracle.status})`}.`,
    ...(report?.failing ?? []).map((f) => `- ${f}`), '',
    `Detection test: ${report?.detection ?? 'not run'}.`, '', '<details><summary>Token diff</summary>', '', '```', tokens.slice(0, 400).join('\n'), '```', '</details>'].join('\n');
  if (dry) return log(body);
  git(['push', '-q', '-f', 'origin', `HEAD:refs/heads/${branch}`], dir);
  const issue = run('gh', ['issue', 'create', '--repo', 'ccheever/exact2', '--title', `Compose Material 3 ${latest} is out: the Material oracle's work list`, '--body', body], dir);
  if (issue.status !== 0) throw new Error(`gh issue create: ${issue.stderr}`);
  log(`filed ${issue.stdout.trim()}`);
  state.filed.push(latest);
}

function regression() {
  if (state.checked) {
    const changed = git(['diff', '--name-only', `${state.checked}..${tip}`]).split('\n');
    if (state.checked === tip || !changed.some((f) => WATCHED.some((w) => f.startsWith(w)))) {
      state.checked = tip;
      return log('nothing the oracle watches changed');
    }
  }
  const oracle = run('bun', ['host/android/reference/oracle.mjs'], root);
  const report = readReport(root);
  if (!report) return log(`the oracle did not finish (exit ${oracle.status}):\n${oracle.stderr.slice(-2000)}`);
  const lost = (state.passing ?? []).filter((n) => report.failing.some((f) => f.startsWith(`${n}: `)));
  const detectionBroke = oracle.status !== 0;
  if ((lost.length || detectionBroke) && state.checked) {
    const range = `${state.checked.slice(0, 9)}..${tip.slice(0, 9)}`;
    const body = [`The Material oracle (LLP 1115.001.001) on origin/main ${range}:`, '',
      ...(detectionBroke ? [`**The detection test failed:** ${report.detection}. The oracle is not to be trusted until it passes.`, ''] : []),
      ...(lost.length ? ['**Specimens that passed and now differ:**', ...lost.map((n) => `- ${report.failing.find((f) => f.startsWith(n))}`)] : []),
      '', 'Thresholds are provisional until calibration (O2): a difference here may be the threshold, not the painter.'].join('\n');
    if (dry) log(body);
    else {
      const issue = run('gh', ['issue', 'create', '--repo', 'ccheever/exact2', '--title', `Material oracle: ${lost.length ? `${lost.length} specimen(s) regressed` : 'detection test failed'} at ${tip.slice(0, 9)}`, '--body', body], root);
      log(issue.status === 0 ? `filed ${issue.stdout.trim()}` : `gh issue create: ${issue.stderr}`);
    }
  } else log(`oracle: ${report.failing.length} of ${report.total} differ; nothing regressed`);
  state.passing = report.passing;
  state.checked = tip;
}

/** What the oracle's report says, by specimen: which pass, which differ (with why), and the detection test. */
function readReport(dir) {
  const file = join(dir, 'host/android/reference/build/oracle/report.json');
  if (!existsSync(file)) return null;
  const r = JSON.parse(readFileSync(file, 'utf8'));
  const bad = (c) => ['geometry', 'probes', 'text'].some((k) => !c.layers[k].ok);
  const why = (c) => ['geometry', 'probes', 'text'].filter((k) => !c.layers[k].ok)
    .map((k) => `${k}: ${c.layers[k].items.filter((i) => !i.ok).map((i) => i.what).slice(0, 4).join(', ')}`).join('; ');
  const ok = r.detection.filter((d) => d.ok).length;
  return { total: r.comparisons.length, passing: r.comparisons.filter((c) => !bad(c)).map((c) => c.name),
    failing: r.comparisons.filter(bad).map((c) => `${c.name}: ${why(c)}`), detection: `${ok} of ${r.detection.length} as expected` };
}
