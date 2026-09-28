// Answers that keep coming, driven (LLP 1016.000; LLP 1069.004 slice 2) —
// `smoke.mjs` runs this for Exact Live: `bun scripts/smoke.mjs
// <web|macos|linux> --app exact-live`. The smoke starts the job fixture on
// loopback (4349 data, 4350 control) and builds the web app against it; a
// native build must be made with the same origins:
//   EXACT_LIVE_JOB_ORIGINS=$'DATA=http://127.0.0.1:4349\nCONTROL=http://127.0.0.1:4350'
// Every assertion is through the agent's operations: a wave's progress
// arrives as server-sent events while 128 held requests wait; `clock settle`
// returns settled with the stream open; a dropped stream is reopened from
// its cursor and the log stays whole; a newer wave forgets the older stream
// and the fixture sees its connection close. It prints what coalesced.
import { createFixture } from '../apps/completion-storm/fixture.mjs';

export const STREAM_ORIGINS = 'DATA=http://127.0.0.1:4349\nCONTROL=http://127.0.0.1:4350';
export const startStreamFixture = () => createFixture({ port: 4349, controlPort: 4350, dist: null, maxWaves: 8, maxHeld: 512, holdMs: 30_000 });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function streamSmoke({ host, open, check, fixture }) {
  const t0 = Date.now();
  const s = await open({ host });
  const log = async () => (await s.state()).resources?.jobLog ?? {};
  const until = async (what, test, ms = 15000) => {
    const end = Date.now() + ms;
    let last;
    while (Date.now() < end) { last = await log(); if (test(last)) return last; await sleep(40); }
    check(false, `${host} stream: ${what}: ${JSON.stringify(last)}`);
    return last;
  };
  const post = (path) => fetch(`${fixture.control}${path}`, { method: 'POST' }).then((r) => r.json());
  try {
    // The job controls are at the foot of the workload drawer.
    await s.tap('toggle-load');
    await s.clock('settle');
    await s.tap('workload-controls', { wheel: [0, 2000] });
    await s.clock('settle');
    await s.tap('attach-fixture');
    await s.tap('start-jobs');
    // Held lanes arrive as the transports admit them (a browser opens six
    // at a time): progress streams in while they wait.
    let st = await until('the stream opens and held lanes arrive', (l) => l.status === 'live' && l.held > 0);
    const wave = st.wave;
    const state = await s.state();
    const streams = state.streams ?? [];
    check(streams.some((x) => x.name === 'jobLog' && x.messages > 0), `${host} stream: state.streams lists the open stream: ${JSON.stringify(streams)}`);
    check(!(state.pending ?? []).some((p) => p.name === 'jobLog'), `${host} stream: an open stream is not pending after its first message`);
    const released = Date.now();
    await post(`/api/release?wave=${wave}`);
    st = await until('every lane finishes and the log is whole', (l) => l.finished === 128 && l.entries === fixture.log(wave) && l.entries === l.seq);
    const whole = Date.now() - released;
    const settleAt = Date.now();
    const settled = await s.clock('settle');
    check(settled.settled === true, `${host} stream: clock settle with the stream open: ${JSON.stringify(settled)}`);
    const settleMs = Date.now() - settleAt;
    check(fixture.readers(wave) === 1, `${host} stream: one open reader for wave ${wave}, found ${fixture.readers(wave)}`);
    const first = { ...st };
    // A forced disconnect: the stream ends, and Reconnect re-asks from the cursor.
    const { dropped } = await post('/api/drop-events');
    check(dropped >= 1, `${host} stream: the fixture dropped the stream`);
    st = await until('the dropped stream ends', (l) => l.status !== 'live');
    check(!((await s.state()).streams ?? []).some((x) => x.name === 'jobLog'), `${host} stream: an ended stream is no longer open`);
    const resumedBefore = fixture.events.resumed;
    const reconnected = Date.now();
    await s.tap('reconnect-events');
    st = await until('the reconnect resumes live and whole', (l) => l.status === 'live' && l.entries === fixture.log(wave));
    const resumeMs = Date.now() - reconnected;
    check(fixture.events.resumed === resumedBefore + 1, `${host} stream: the reconnect sent Last-Event-ID (${fixture.events.resumed - resumedBefore} resumed)`);
    check(st.entries === first.entries, `${host} stream: the resumed log has no duplicate (${st.entries} vs ${first.entries})`);
    // A newer wave forgets the older stream: the host aborts it.
    await s.tap('start-jobs');
    await until('a newer wave opens its own stream', (l) => l.wave > wave && l.status === 'live');
    const end = Date.now() + 5000;
    while (fixture.readers(wave) > 0 && Date.now() < end) await sleep(40);
    check(fixture.readers(wave) === 0, `${host} stream: forgetting the old stream closed its connection (${fixture.readers(wave)} open)`);
    const next = (await log()).wave;
    await post(`/api/release?wave=${next}`);
    await until('the newer wave finishes', (l) => l.finished === 128);
    const again = await s.clock('settle');
    check(again.settled === true, `${host} stream: settle after a forget: ${JSON.stringify(again)}`);
    console.log(`${host} stream: wave ${wave}: ${fixture.log(wave)} entries, ${first.messages} messages (${first.coalesced} coalesced, ${first.gaps} gaps, ${first.reasks} cursor re-asks); whole ${whole} ms after release; settle ${settleMs} ms with the stream open; reconnect whole in ${resumeMs} ms; server sent ${fixture.events.sent} events, ${fixture.events.opened} opens, ${fixture.events.resumed} resumed (${((Date.now() - t0) / 1000).toFixed(1)} s)`);
  } finally {
    await s.close();
  }
}
