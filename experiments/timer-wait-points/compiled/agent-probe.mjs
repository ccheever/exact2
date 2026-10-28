#!/usr/bin/env bun
// Under the agent (platform timing, real touches): records the aims a
// non-agent run replays (the inbox's demo thread, the Add panel's "Add 50",
// the transcript), and reads `state` before and after one real fling of the
// transcript, to show whether the `network` task's timer advanced on its own.
//   EXACT_SIM=<udid> bun experiments/timer-wait-points/compiled/agent-probe.mjs <out.json> [--fifty N]
import { writeFileSync } from 'node:fs';
import { open } from '../../../scripts/agent.mjs';

const [out, ...rest] = process.argv.slice(2);
const fifty = rest.includes('--fifty') ? Number(rest[rest.indexOf('--fifty') + 1]) : 3;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const s = await open({ host: 'ios', app: 'messages', timing: 'platform', touch: 'platform' });
const record = { aims: {}, states: [] };
const idOf = async (testId) => (await s.layout()).nodes.find((n) => n.testId === testId);
const aim = async (testId, how) => {
  const node = await idOf(testId);
  if (!node) throw new Error(`${testId} has no box`);
  const r = await s.op({ op: 'tap', id: node.id, aim: how });
  record.aims[testId] = { box: { x: node.x, y: node.y, w: node.w, h: node.h }, aim: r.aim ?? r };
  return node;
};
const snap = async (label) => {
  const st = await s.op({ op: 'state' });
  const pick = { label, wall: Date.now(), tasks: st.tasks, time: st.state?.time ?? st.time ?? null, clock: st.clock ?? null, keys: Object.keys(st) };
  record.states.push(pick);
  return pick;
};
try {
  await sleep(1500);
  await snap('inbox');
  await aim('conversation-demo', { press: true });
  await s.tap('conversation-demo');
  await sleep(1500);
  for (let i = 0; i < fifty; i++) {
    await aim('add', { press: true });
    await s.tap('add');
    await sleep(800);
    await aim('add-fifty', { press: true });
    await s.tap('add-fifty');
    await sleep(1200);
  }
  const box = await idOf('transcript');
  for (const [name, f] of [['transcript-top', 0.15], ['transcript-bottom', 0.85]]) {
    const r = await s.op({ op: 'tap', id: box.id, aim: { x: box.x + box.w / 2, y: box.y + box.h * f } });
    record.aims[name] = { box: { x: box.x, y: box.y, w: box.w, h: box.h }, aim: r.aim ?? r };
  }
  await snap('before fling');
  await sleep(2000);
  await snap('after 2 s at rest');
  await s.tap('transcript', { drag: { dx: 0, dy: Math.min(600, box.h * 0.7), from: [box.w / 2, box.h * 0.15], over: 60 } });
  await sleep(2000);
  await snap('after one fling and 2 s');
} finally {
  writeFileSync(out, JSON.stringify(record, null, 2));
  await s.close();
}
console.log(JSON.stringify(record.states, null, 2));
