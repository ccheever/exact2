import assert from 'node:assert/strict';
const { open } = await import(`file://${process.cwd()}/scripts/agent.mjs`);
const host = process.argv[2] ?? 'macos';
const s = await open({ host, app: 'caltrain', plan: new URL('./clock.plan', import.meta.url).pathname });
let calls = 0, backwards = 0, seeks = 0, last = s.now;
const op = s.op.bind(s);
s.op = async req => {
  if (req.op === 'clock' && typeof req.to === 'number') {
    seeks++;
    if (req.to < last) backwards++;
  }
  const reply = await op(req);
  if (typeof reply.clock === 'number') last = reply.clock;
  return reply;
};
const begin = performance.now();
try {
  for (; calls < 40000; calls++) {
    const from = (await s.op({op:'clock',take:true})).clock;
    const r = await s.clock('+1 real');
    assert.equal(r.clock, from + 1);
    if ((calls + 1) % 10000 === 0) console.log(JSON.stringify({host, progress: calls + 1, seeks, backwards, clock:r.clock}));
  }
  assert.equal(backwards, 0);
  console.log(JSON.stringify({state: await s.state()}));
  console.log(JSON.stringify({host,status:'PASS',calls,seeks,backwards,clock:last,elapsedMs:Math.round(performance.now()-begin)}));
} catch(error) {
  console.log(JSON.stringify({host,status:'FAIL',calls,seeks,backwards,clock:last,error:error.message,elapsedMs:Math.round(performance.now()-begin)}));
  process.exitCode = 1;
} finally {
  try { console.log(JSON.stringify({ screenshot: await s.screenshot(new URL(`${process.argv[3] ?? 'run'}-${host}.png`, import.meta.url).pathname) })); }
  finally { await s.close?.(); }
}
