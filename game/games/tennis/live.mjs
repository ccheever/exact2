#!/usr/bin/env bun
// Opt-in: play real points against real Jev, in real time, on the Linux host.
// Runs only when AI_GATEWAY_API_KEY is in the environment (the native data
// source reads it at request time). The world clock is paced to the wall clock,
// so "late" means what it means in play. Reports latency and the late rate.
import {readdirSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import { proof } from '../../proof.mjs';
import { bot, lines } from './proof.mjs';

const key = process.env.AI_GATEWAY_API_KEY?.trim();
if (!key) {
  console.log('SKIP live Jev check: AI_GATEWAY_API_KEY is not set');
  process.exit(0);
}
const seconds = Number(process.env.TENNIS_LIVE_SECONDS ?? 90);
const quantile = (xs, q) => xs.length ? [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(q * xs.length))] : NaN;

await proof(import.meta, async ({open, check, out, host}) => {
  check('the live check runs on the Linux host', host === 'linux');
  const s = await open();
  await s.tap('play');
  const raw = s.world('world');
  const started = performance.now();
  let worldMs = 0;
  // Never let the world run ahead of the wall clock.
  const world = {...raw, run: async ms => {
    worldMs += ms;
    const ahead = worldMs - (performance.now() - started);
    if (ahead > 0) await Bun.sleep(ahead);
    return raw.run(ms);
  }};
  const me = bot(s, world);
  const text = (tree, id) => tree.nodes.find(n => n.props?.testId === id)?.props?.text;
  while (performance.now() - started < seconds * 1000) {
    const tree = await s.tree();
    if (tree.nodes.some(n => n.props?.testId === 'match-over')) break;
    const prompt = text(tree, 'prompt') ?? '';
    if (prompt.startsWith('Your serve') || prompt.startsWith('Second serve')) await me.serve();
    else if (!(await me.play())) await world.run(25);
  }
  // Let the last question settle before counting.
  for (let i = 0; i < 100 && (text(await s.tree(), 'jev-line') ?? '') === 'Jev thinking…'; i++) await world.run(25);
  const wall = performance.now() - started;
  const journal = await lines(s);
  const asked = journal.filter(l => /jev ask \d+/.test(l)).length;
  const onTime = journal.map(l => /jev answer \d+ in (\d+) ms/.exec(l)).filter(Boolean).map(m => +m[1]);
  const lateArrivals = journal.map(l => /jev answer \d+ arrived (\d+) ms after asking/.exec(l)).filter(Boolean).map(m => +m[1]);
  const failed = journal.filter(l => /jev answer \d+ failed/.test(l));
  const committedLate = journal.filter(l => /jev decision \d+ late/.test(l)).length;
  const executed = journal.filter(l => /far (Forehand|Backhand) \w+ → \w+ \(Jev\)/.test(l)).length;
  const points = journal.filter(l => /^t=.* point /.test(l) || / point (You|Jev):/.test(l)).length;
  const latency = [...onTime, ...lateArrivals];
  console.log(`LIVE ${points} points, ${(wall / 1000).toFixed(1)} s wall, world/wall ${(worldMs / wall).toFixed(3)}`);
  console.log(`LIVE asked ${asked}, answered ${latency.length} (${onTime.length} before the deadline, ${lateArrivals.length} after), failed ${failed.length}, fallback for lateness ${committedLate}, Jev intents executed ${executed}`);
  console.log(`LIVE latency ms (world clock): min ${Math.min(...latency)} p50 ${quantile(latency, 0.5)} p90 ${quantile(latency, 0.9)} max ${Math.max(...latency)}`);
  console.log(`LIVE late rate ${(committedLate / Math.max(1, onTime.length + committedLate) * 100).toFixed(1)}% of committed decisions`);
  for (const line of failed.slice(0, 3)) console.log(`LIVE failure: ${line}`);
  check('Jev answered', latency.length > 0);
  check('every answer parsed', failed.length === 0, failed.slice(0, 3));
  check('most plans beat their deadline', onTime.length > committedLate, {onTime: onTime.length, committedLate});
  check('Jev’s intents drove the far player', executed > 0);
  await s.close();
  // The key must not reach any artifact this run wrote.
  const leaked = readdirSync(out).filter(f => readFileSync(resolve(out, f)).includes(key));
  check('no artifact contains the key', leaked.length === 0, leaked);
});
