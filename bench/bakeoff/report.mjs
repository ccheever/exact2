// A round's tables: scrolling first (the probe's numbers per app, Exact beside the other), then per pair wall clock,
// tokens, the grader's scores and verdict; Exact over the other as geometric means. Writes <round>/summary.md.
//   bun bench/bakeoff/report.mjs <round dir>
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const ROUND = resolve(process.argv[2]);
const nativeArg = process.argv.indexOf('--native');
const NATIVE = nativeArg >= 0 ? resolve(ROUND, '..', process.argv[nativeArg + 1]) : null;
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
const json = (p) => { try { return JSON.parse(readFileSync(p, 'utf8')); } catch { return null; } };
const geo = (xs) => (xs.length ? Math.exp(xs.reduce((a, x) => a + Math.log(x), 0) / xs.length) : NaN);
const mean = (xs) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);
const f = (x, d = 1) => (x == null || Number.isNaN(x) ? '–' : Number(x).toFixed(d));
const source = (dir, rel) => {
  const current = resolve(dir, rel);
  if (existsSync(current)) return current;
  if (NATIVE) {
    const baseline = resolve(NATIVE, rel);
    if (existsSync(baseline)) return baseline;
  }
  return current;
};
const record = (spec, arm) => json(source(ROUND, `records/${spec}-${arm}.json`));
const CLASS = [
  ['start-here', /start-here\.md/], ['card', /card\.md/], ['recipes', /docs\/recipes\//], ['patterns', /pattern|docs\/patterns/],
  ['long guides', /contract-for-agents|contract-grammar|reference\.md|agent-pitfalls/], ['vocab/docs', /exact\.mjs (contract vocab|docs)/],
  ['exact source', /exact2[^ ]*\/(scripts|host|contract|kernel|runner|apps)\//], ['verify', /exact\.mjs verify/], ['test', /exact\.mjs test/],
  ['agent drive', /exact\.mjs agent|agent\.mjs/], ['ios build', /exact\.mjs ios|build\.sh|swiftc|xcodebuild/], ['contract build', /contract build|exact\.mjs check/],
  ['simctl', /simctl/], ['edit', /^(edit|Edit|Write|MultiEdit)/], ['read own', /^Read |cat |sed -n|head |nl /], ['search', /rg |grep |Grep|Glob|find /],
];
const anatomy = (spec, arm) => {
  const path = source(ROUND, `logs/${spec}-${arm}.jsonl`);
  if (!existsSync(path)) return null;
  const by = {}, lines = readFileSync(path, 'utf8').split('\n');
  let calls = 0, lookups = 0;
  for (const line of lines) {
    let e; try { e = JSON.parse(line); } catch { continue; }
    let cmd = '', bytes = 0;
    if (e.type === 'item.completed' && e.item?.type === 'command_execution') { cmd = e.item.command ?? ''; bytes = String(e.item.aggregated_output ?? '').length; }
    else if (e.type === 'item.completed' && e.item?.type === 'file_change') { cmd = 'edit'; }
    else continue;
    calls++;
    if (/contract vocab|exact\.mjs (docs|help)|pattern /.test(cmd)) lookups++;
    const key = CLASS.find(([, re]) => re.test(cmd))?.[0] ?? 'other';
    by[key] = (by[key] ?? 0) + bytes / 4;
  }
  const classes = Object.entries(by).sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k}:${Math.round(v / 1000)}k`).join(', ');
  return { calls, lookups, classes };
};

// The probe's verdict counts for one app: how many metrics read "tell" or "bad" (host/apple/feel/analyze.mjs keeps each
// group's verdicts in its own `verdict` object), and which.
const feelOf = (spec, arm) => {
  const j = json(resolve(ROUND, 'feel', `${spec}-${arm}`, 'feel.json'));
  if (!j) return null;
  if (j.verdict !== 'probed') return { text: j.verdict ?? '?' };
  const found = [];
  const walk = (o, at) => {
    for (const [k, v] of Object.entries(o ?? {})) {
      if (k === 'verdict' && v && typeof v === 'object') for (const [m, x] of Object.entries(v)) { if (x === 'tell' || x === 'bad') found.push(`${at}.${m} ${x}`); }
      else if (v && typeof v === 'object' && !Array.isArray(v)) walk(v, k);
    }
  };
  walk(j, '');
  return { found, text: found.length ? found.join(', ') : 'ok' };
};

// The numbers a person feels, from feel.json: hitch ratio (ms late per s of motion; Apple calls < 5 good), the
// longest frame, offset jumps while decelerating, flings that drop a frame at the lift, touch-to-content lag; the
// scroll indicator dragged end to end (`scrub`: its hitch ratio and longest frame); and a horizontal list in the list
// (`carousel`: its hitch ratio, and whether the two diagonal drags went where UIKit sends them).
const scrollOf = (spec, arm) => {
  const j = json(resolve(ROUND, 'feel', `${spec}-${arm}`, 'feel.json'));
  if (!j || j.verdict !== 'probed') return null;
  const h = j.horizontal;
  return { hitch: j.smooth?.hitchRatio, longest: j.smooth?.longestFrameMs, jumps: j.momentum?.jumps, lift: j.momentum?.lifts ? `${j.momentum.liftDrops}/${j.momentum.lifts}` : '–', lag: j.tracking?.lagFrames,
    scrub: j.scrub?.smooth ? `${f(j.scrub.smooth.hitchRatio)} / ${f(j.scrub.smooth.longestFrameMs, 0)}` : j.scrub ? 'not taken' : '–', scrubHitch: j.scrub?.smooth?.hitchRatio,
    carousel: h ? `${f(h.smooth?.hitchRatio)}${h.verdict.across === 'ok' && h.verdict.along === 'ok' ? '' : ` (drags ${h.verdict.across}, ${h.verdict.along})`}` : '–' };
};
const scrollRows = [], hitches = { exact: [], other: [] }, scrubs = { exact: [], other: [] };
const smooth = { exact: 0, other: 0, probed: 0 };
for (const [spec, { stack }] of Object.entries(pairs)) {
  const e = scrollOf(spec, 'exact'), o = scrollOf(spec, stack);
  if (!e && !o) continue;
  const cell = (x) => (x ? `${f(x.hitch)} | ${f(x.longest, 0)} | ${x.jumps ?? '–'} | ${x.lift} | ${x.lag ?? '–'} | ${x.scrub} | ${x.carousel}` : '– | – | – | – | – | – | –');
  scrollRows.push(`| ${spec} | ${stack} | ${cell(e)} | ${cell(o)} |`);
  if (e && o) {
    smooth.probed++; hitches.exact.push(e.hitch); hitches.other.push(o.hitch);
    if (e.scrubHitch != null && o.scrubHitch != null) { scrubs.exact.push(e.scrubHitch); scrubs.other.push(o.scrubHitch); }
    if (e.hitch < 5 && !e.jumps) smooth.exact++;
    if (o.hitch < 5 && !o.jumps) smooth.other++;
  }
}

const rows = [], ratios = { min: [], tok: [] }, tally = { exact: 0, other: 0, tie: 0 }, sums = { exact: { o: [], p: [], t: [], b: [] }, other: { o: [], p: [], t: [], b: [] } };
for (const [spec, { stack, model }] of Object.entries(pairs)) {
  const ex = record(spec, 'exact'), ot = record(spec, stack);
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
  const ae = anatomy(spec, 'exact'), ao = anatomy(spec, stack);
  const an = (a) => a ? `${a.calls}/${a.lookups}; ${a.classes || '–'}` : '–';
  rows.push(`| ${spec} | ${model} | ${stack} | ${f(ex?.minutes)} | ${f(ot?.minutes)} | ${f(ex && ex.tokens.total / 1e6, 2)} | ${f(ot && ot.tokens.total / 1e6, 2)} | ${ge?.scores?.overall ?? '–'} | ${go?.scores?.overall ?? '–'} | ${ge?.scores?.platform ?? '–'} | ${go?.scores?.platform ?? '–'} | ${ge ? ge.tells?.length : '–'} / ${go ? go.tells?.length : '–'} | ${ge ? ge.bugs?.length : '–'} / ${go ? go.bugs?.length : '–'} | ${fe?.text ?? '–'} | ${fo?.text ?? '–'} | ${better} | ${an(ae)} | ${an(ao)} |`);
}
const cold = Object.keys(pairs).map((spec) => record(spec, 'exact')).filter((r) => r?.cold).map((r) => `${r.spec} ${f(r.minutes)} min`);
const md = [
  `# Bake-off ${basename(ROUND)}`,
  '',
  `exact2 ${(() => { try { return readFileSync(resolve(ROUND, 'pin.txt'), 'utf8').split('\n')[0]; } catch { return '?'; } })()}. Exact over the other, per pair: minutes **${f(geo(ratios.min), 2)}×**, tokens **${f(geo(ratios.tok), 2)}×** (geometric means, timeouts excluded). Preferred: Exact ${tally.exact}, other ${tally.other}, tie ${tally.tie}.`,
  `Cold first builds: ${cold.length ? cold.join(', ') : 'none recorded'}.`,
  '',
  `## Scrolling`,
  '',
  `Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **${smooth.exact} of ${smooth.probed}**, the other ${smooth.other} of ${smooth.probed}. Mean hitch ratio: Exact ${f(mean(hitches.exact))} ms/s, the other ${f(mean(hitches.other))}. Scrubbing (the scroll indicator dragged end to end and back, ${scrubs.exact.length} pairs): Exact ${f(mean(scrubs.exact))} ms/s, the other ${f(mean(scrubs.other))}.`,
  '',
  'Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags\' verdicts when either is not UIKit\'s.',
  '',
  '| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |',
  '|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|',
  ...scrollRows,
  '',
  '## The pairs',
  '',
  `Means, Exact vs other: overall ${f(mean(sums.exact.o))} vs ${f(mean(sums.other.o))}; platform ${f(mean(sums.exact.p))} vs ${f(mean(sums.other.p))}; tells an app ${f(mean(sums.exact.t))} vs ${f(mean(sums.other.t))}; bugs an app ${f(mean(sums.exact.b))} vs ${f(mean(sums.other.b))}.`,
  '',
  '| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better | Exact anatomy (calls/lookups; tokens by class) | Other anatomy (calls/lookups; tokens by class) |',
  '|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|',
  ...rows,
  '',
].join('\n');
writeFileSync(resolve(ROUND, 'summary.md'), md);
console.log(md);
