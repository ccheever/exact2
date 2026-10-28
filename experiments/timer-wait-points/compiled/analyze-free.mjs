#!/usr/bin/env bun
// Reads the traces `fling-free.mjs` saved: per fling, the frames presented in
// the 1.5 s after the touch (late, missed, phases), the frames that applied
// a batch and their commit sequence numbers, and the work by plan site
// between the trace before the fling and the one after it.
//   bun experiments/timer-wait-points/compiled/analyze-free.mjs <dir> [--json]
import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { readTrace, perfDelta } from '../../../scripts/agent-inspect.mjs';
import { traceLocators } from '../../../scripts/agent-launch.mjs';

const [dir, ...rest] = process.argv.slice(2);
const json = rest.includes('--json');
const { log, flings } = JSON.parse(readFileSync(resolve(dir, 'fling-free-log.json'), 'utf8'));
const files = readdirSync(dir).filter((f) => /^trace-\d+-.*\.json$/.test(f)).sort((a, b) => Number(a.split('-')[1]) - Number(b.split('-')[1]));
const traces = [];
for (const f of files) traces.push(await readTrace(resolve(dir, f), traceLocators));

// Media-clock ms (the trace's `t`) to wall ms: bootWall is the wall at the media clock's zero.
const wallOf = (trace, t) => trace.identity.bootWall + t;
const records = new Map(), late = new Map();
for (const t of traces) {
  for (const r of t.frames.records ?? []) records.set(r.t, { ...r, wall: wallOf(t, r.t) });
  for (const r of t.frames.late ?? []) late.set(r.t, { ...r, wall: wallOf(t, r.t) });
}
const all = [...records.values()].sort((a, b) => a.t - b.t);
const where = (x) => x.source?.status === 'compatible' ? `${x.source.file.split('/').pop()}:${x.source.line} (${x.source.component})` : `site ${x.site}`;
const r2 = (x) => Math.round(x * 100) / 100;

const out = [];
for (let i = 0; i < flings; i++) {
  const start = log.find((e) => e.what === 'fling' && e.i === i).t;
  const end = start + 1500;
  const window = all.filter((r) => r.wall >= start && r.wall < end);
  const lateIn = [...late.values()].filter((r) => r.wall >= start && r.wall < end);
  const withBatches = window.filter((r) => r.batches > 0);
  const commits = withBatches.reduce((n, r) => n + (r.seq ? r.seq[1] - r.seq[0] + 1 : 0), 0);
  const phases = new Map();
  for (const l of lateIn) for (const p of l.phases ?? []) phases.set(p.phase, (phases.get(p.phase) ?? 0) + p.ms);
  const delta = perfDelta(traces[i].perf, traces[i + 1].perf);
  const sites = delta.sites.filter((s) => s.evaluated > 0 || s.created > 0)
    .sort((a, b) => b.evaluated - a.evaluated).slice(0, 12)
    .map((s) => ({ at: where(s), evaluated: s.evaluated, unchanged: s.unchanged, created: s.created, rowConstructed: s.rowConstructed, rowRebound: s.rowRebound }));
  out.push({
    fling: i + 1,
    window: { frames: window.length, period: window[0]?.period ?? null, late: lateIn.length, missed: lateIn.reduce((n, r) => n + r.missed, 0),
      overrunMs: r2(lateIn.reduce((n, r) => n + (r.overrun ?? 0), 0)), phases: [...phases].map(([phase, ms]) => ({ phase, ms: r2(ms) })).sort((a, b) => b.ms - a.ms),
      unattributed: r2(lateIn.reduce((n, r) => n + (r.unattributed ?? 0), 0)) },
    batchFrames: withBatches.map((r) => ({ at: Math.round(r.wall - start), seq: r.seq, batches: r.batches, apply: r2(r.apply), interval: r2(r.interval), missed: r.missed })),
    commits,
    lateFrames: lateIn.map((r) => ({ at: Math.round(r.wall - start), interval: r2(r.interval), missed: r.missed, overrun: r2(r.overrun ?? 0), seq: r.seq, batches: r.batches, phases: r.phases, unattributed: r.unattributed })),
    perf: { seq: [delta.from.seq, delta.seq], clock: [r2(delta.from.clock), r2(delta.clock)], sites, handlers: delta.handlers.map((h) => ({ at: where(h), event: h.event, dispatched: h.dispatched, committed: h.committed })) },
  });
}
if (json) console.log(JSON.stringify(out, null, 2));
else for (const f of out) {
  console.log(`fling ${f.fling}: ${f.window.frames} frames sampled in 1.5 s (period ${f.window.period} ms) · ${f.window.late} late (${f.window.missed} missed) · ${f.batchFrames.length} frames applied a batch (${f.commits} commits) · runner seq ${f.perf.seq.join('..')} over clock ${f.perf.clock.join('..')} ms`);
  console.log(`  batch frames (ms after the touch): ${f.batchFrames.map((b) => `${b.at}${b.seq ? ` [${b.seq.join('..')}]` : ''}`).join(', ')}`);
  for (const l of f.lateFrames) console.log(`  late at ${l.at} ms: interval ${l.interval} ms, ${l.missed} missed, overrun ${l.overrun} ms, seq ${l.seq?.join('..') ?? '—'}, batches ${l.batches}, ${(l.phases ?? []).map((p) => `${p.phase} ${p.ms}`).join(' ')} unattributed ${l.unattributed}`);
  for (const s of f.perf.sites) console.log(`  ${s.at}: evaluated ${s.evaluated} (unchanged ${s.unchanged}), created ${s.created}`);
  for (const h of f.perf.handlers) console.log(`  ${h.event} at ${h.at}: ${h.dispatched} dispatched, ${h.committed} changed the tree`);
}
