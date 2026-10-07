// EXACT_APP_DIR=<probe> bun drive.mjs <host> <outdir> <label>
import { open } from '/Users/daehyeonmun/orca/workspaces/exact2/fix-138-mac-scroll-anchoring/scripts/agent.mjs';
const [host = 'web', out = '.', label = 'x', mode = 'fold'] = process.argv.slice(2);
const s = await open({ host, app: 'anchor-probe', size: [480, 640] });
const ids = async () => { const t = await s.tree(); const m = {}; for (const n of t.nodes) if (n.props?.testId) m[n.id] = n.props.testId; return [m, t]; };
const rect = async (want) => { const [m] = await ids(); const l = await s.layout(); return l.nodes.filter(n => m[n.id] === want).map(n => ({ t: m[n.id], y: n.y, h: n.height ?? n.h, ...(n.sy !== undefined ? { sy: n.sy } : {}) }))[0]; };
const scrollTop = async () => { const [m, t] = await ids(); const n = t.nodes.find(n => m[n.id] === 'plain'); return n?.scroll ?? n?.scrollTop ?? n?.props?.scrollTop ?? JSON.stringify(Object.keys(n ?? {})); };
const log = (k, v) => console.log(`${host} ${label}`, k, JSON.stringify(v));
try {
  await s.clock('data');
  if (mode !== 'top') { await s.tap('plain', { wheel: [0, 400] }); await s.clock('settle'); }
  for (const [step, btn] of { fold: [['fold', 'fold']], insert: [['insert', 'add']], top: [['top-insert', 'add']] }[mode]) {
    const before = await rect('p-20'); log(`${step}.p20.before`, before); log(`${step}.plain.before`, await rect('plain'));
    await s.screenshot(`${out}/${host}-${label}-${step}-before.png`);
    await s.tap(btn); await s.clock('settle');
    const after = await rect('p-20'); log(`${step}.p20.after`, after); log(`${step}.plain.after`, await rect('plain'));
    await s.screenshot(`${out}/${host}-${label}-${step}-after.png`);
    log(`${step}.shift`, after.y - before.y);
  }
} finally { await s.close(); }
