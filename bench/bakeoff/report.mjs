// A round's table: per pair, wall clock, tokens, the grader's scores and verdict, and the scroll probe's verdicts;
// then Exact over the other as geometric means. Writes <round>/summary.md and prints it.
//   bun bench/bakeoff/report.mjs <round dir>
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const ROUND = resolve(process.argv[2]);
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
const json = (p) => { try { return JSON.parse(readFileSync(p, 'utf8')); } catch { return null; } };
const geo = (xs) => (xs.length ? Math.exp(xs.reduce((a, x) => a + Math.log(x), 0) / xs.length) : NaN);
const f = (x, d = 1) => (x == null || Number.isNaN(x) ? '–' : Number(x).toFixed(d));

// The probe's verdict counts for one app: how many metrics read "tell" or "bad".
const feelOf = (spec, arm) => {
  const j = json(resolve(ROUND, 'feel', `${spec}-${arm}`, 'feel.json'));
  if (!j) return null;
  if (j.status && j.status !== 'ok') return { text: j.status };
  const verdicts = Object.values(j.verdicts ?? {}).map((v) => (typeof v === 'string' ? v : v?.verdict));
  const bad = verdicts.filter((v) => v === 'bad').length, tell = verdicts.filter((v) => v === 'tell').length;
  return { bad, tell, text: bad + tell ? `${bad} bad, ${tell} tell` : 'ok' };
};

const rows = [], ratios = { min: [], tok: [] }, tally = { exact: 0, other: 0, tie: 0 }, sums = { exact: { o: [], p: [], t: [], b: [] }, other: { o: [], p: [], t: [], b: [] } };
for (const [spec, { stack, model }] of Object.entries(pairs)) {
  const ex = json(resolve(ROUND, 'records', `${spec}-exact.json`)), ot = json(resolve(ROUND, 'records', `${spec}-${stack}.json`));
  if (!ex && !ot) continue;
  const g = json(resolve(ROUND, 'grades', spec, 'grade.json')), key = json(resolve(ROUND, 'grades', spec, 'key.json'));
  const side = (arm) => (key ? (key.A === arm ? 'A' : 'B') : null);
  const gs = (arm) => (g && key ? g[side(arm)] : null);
  const ge = gs('exact'), go = gs(stack);
  const better = g && key ? (g.better === 'tie' ? 'tie' : key[g.better] === 'exact' ? 'Exact' : stack) : '–';
  if (g) tally[better === 'Exact' ? 'exact' : better === 'tie' ? 'tie' : 'other']++;
  for (const [k, x] of [['exact', ge], ['other', go]]) if (x?.scores) { sums[k].o.push(x.scores.overall); sums[k].p.push(x.scores.platform); sums[k].t.push(x.tells?.length ?? 0); sums[k].b.push(x.bugs?.length ?? 0); }
  if (ex && ot && !ex.timedOut && !ot.timedOut) { ratios.min.push(ex.minutes / ot.minutes); if (ex.tokens.total && ot.tokens.total) ratios.tok.push(ex.tokens.total / ot.tokens.total); }
  const fe = feelOf(spec, 'exact'), fo = feelOf(spec, stack);
  rows.push(`| ${spec} | ${model} | ${stack} | ${f(ex?.minutes)} | ${f(ot?.minutes)} | ${f(ex && ex.tokens.total / 1e6, 2)} | ${f(ot && ot.tokens.total / 1e6, 2)} | ${ge?.scores?.overall ?? '–'} | ${go?.scores?.overall ?? '–'} | ${ge?.scores?.platform ?? '–'} | ${go?.scores?.platform ?? '–'} | ${ge ? ge.tells?.length : '–'} / ${go ? go.tells?.length : '–'} | ${ge ? ge.bugs?.length : '–'} / ${go ? go.bugs?.length : '–'} | ${fe?.text ?? '–'} | ${fo?.text ?? '–'} | ${better} |`);
}
const mean = (xs) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);
const md = [
  `# Bake-off ${basename(ROUND)}`,
  '',
  `exact2 ${(() => { try { return readFileSync(resolve(ROUND, 'pin.txt'), 'utf8').split('\n')[0]; } catch { return '?'; } })()}. Exact over the other, per pair: minutes **${f(geo(ratios.min), 2)}×**, tokens **${f(geo(ratios.tok), 2)}×** (geometric means, timeouts excluded). Preferred: Exact ${tally.exact}, other ${tally.other}, tie ${tally.tie}.`,
  `Means, Exact vs other: overall ${f(mean(sums.exact.o))} vs ${f(mean(sums.other.o))}; platform ${f(mean(sums.exact.p))} vs ${f(mean(sums.other.p))}; tells an app ${f(mean(sums.exact.t))} vs ${f(mean(sums.other.t))}; bugs an app ${f(mean(sums.exact.b))} vs ${f(mean(sums.other.b))}.`,
  '',
  '| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |',
  '|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|',
  ...rows,
  '',
].join('\n');
writeFileSync(resolve(ROUND, 'summary.md'), md);
console.log(md);
