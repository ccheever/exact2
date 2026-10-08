import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const source = readFileSync(new URL('./scripts/agent.mjs', `file://${process.cwd()}/`), 'utf8');
const start = source.indexOf("    async clock(spec = 'settle') {");
const end = source.indexOf('\n    /** The window', start);
for (const host of ['macos', 'ios', 'host', 'host-ios', 'web', 'linux']) {
  const sent = [], wall = [18.329417, 18.329417, 18.829417, 20.329417, 20.329417];
  let reported = 250;
  const s = { now: host === 'web' || host === 'linux' ? reported : 0, async op(req) {
    if (req.take) return { clock: reported };
    sent.push(req.to);
    assert.ok(req.to >= reported, `clock moved backwards (${reported} -> ${req.to})`);
    reported = req.to;
    return { clock: reported };
  } };
  const clock = new Function('s', 'carrier', 'performance', 'setTimeout', 'REAL_STEP_MS', `return ({${source.slice(start, end)}}).clock`)(s, { host }, { now: () => wall.shift() }, done => done(), 16);
  try {
    const reply = await clock('+1 real');
    assert.equal(reply.clock, 251);
    assert.equal(sent[0], 250);
    console.log(JSON.stringify({ host, status: 'PASS', sent, final: reply.clock }));
  } catch (error) { console.log(JSON.stringify({ host, status: 'FAIL', sent, error: error.message })); process.exitCode = 1; }
}
