// Scroll a bounded heavy list through its window shifts with the agent's
// wheel (the host's own input path) and record the message at the list's
// top edge after each step: it must only move forward.
// Run from the checkout with EXACT_APP_DIR naming the bounded app (and the app built and running):
//   EXACT_APP_DIR=$PWD/bench/dioxus/exact-bounded bun bench/dioxus/native/shiftcheck.mjs <macos|ios|web> [steps=160] [dy=600]
import { open } from '../../../scripts/agent.mjs';

const [host = 'macos', steps = '160', dy = '600'] = process.argv.slice(2);
const s = await open({ host });
const top = async () => {
  const st = await s.state();
  const lay = await s.layout('messages');
  const list = (lay.nodes ?? []).find((n) => n.testId === 'messages') ?? lay.node;
  const sy = list?.sy ?? lay.node?.scroll?.[0]?.sy ?? 0;
  const c = st.collections.find((c) => c.axis === 'y');
  const feed = st.resources.feed;
  const row = c.rows.find((r) => sy >= r.start - 0.5 && sy < r.start + r.size) ?? null;
  return { sy: Math.round(sy), cursor: st.slots.cursor, first: feed.rows[0].id, top: row ? feed.rows[row.index].id : null, mounted: c.rows.length, extent: Math.round(c.totalExtent) };
};
const n = (id) => (id && id.startsWith('m') ? Number(id.slice(1)) : null);
const out = [];
try {
  await s.clock('settle');
  out.push(await top());
  for (let i = 0; i < Number(steps); i++) {
    await s.tap('messages', { wheel: [0, Number(dy)] });
    await s.clock('settle');
    out.push(await top());
  }
} finally {
  await s.close?.();
}
let back = 0, worst = 0, best = -1, shifts = 0, nulls = 0;
for (let i = 0; i < out.length; i++) {
  const v = n(out[i].top);
  if (i && out[i].first !== out[i - 1].first) {
    shifts++;
    console.log('shift', JSON.stringify(out[i - 1]), '->', JSON.stringify(out[i]));
  }
  if (v === null) { nulls++; continue; }
  if (v < best) { back++; worst = Math.max(worst, best - v); }
  best = Math.max(best, v);
}
console.log(JSON.stringify({ host, steps: out.length - 1, dy: Number(dy), lastTop: out.at(-1).top, lastSy: out.at(-1).sy, shifts, backward: back, worstBackRows: worst, noTopRow: nulls }));
