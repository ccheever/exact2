// The look result (LLP 1127.000 D5): what makes it PASS, FAIL or UNVERIFIED in
// one proof and across a prove's hosts, apart from the simulation's; and the
// browser's negative control, a starter whose sparks burn under the ground.
import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {equal, frameTells, lookAcrossRuns, lookOnlyFailure, lookResult, tellOf, worldJournal} from './proof.mjs';
import {createGame} from './new.mjs';
import {lookAcross} from '../scripts/agent-inspect.mjs';

const none = new Set();
const seen = (session, canvas = 'world') => ({session, canvas, picture:`/tmp/look-${session}.png`});

test('a two-world proof with one world unobserved is unverified, naming it', () => {
  const worlds = [seen(1), {session:2, canvas:'world', unavailable:'no picture on this host: no adapter'}];
  const r = lookResult({worlds, tells:[]}, none);
  expect(r.status).toBe('UNVERIFIED');
  expect(r.detail).toBe('session 2 world: no picture on this host: no adapter');
  const none2 = [2, 3].map(session => ({session, canvas:'world', unavailable:'no picture on this host: no adapter'}));
  expect(lookResult({worlds:none2, tells:[]}, none).missing).toEqual(['sessions 2, 3 world: no picture on this host: no adapter']);
  expect(lookResult({worlds:[seen(1), seen(2)], tells:[]}, none).status).toBe('PASS');
  expect(lookResult({worlds:[], tells:[]}, none).detail).toBe('no world was shown');
  const failed = lookResult({worlds:[seen(1), {session:2, canvas:'world', error:'look frame failed: lost'}], tells:[]}, none);
  expect(failed.status).toBe('UNVERIFIED');
  // A world a settle showed that was gone by the close (proof.mjs's accounting) is a gap, not a pass.
  const vanished = lookResult({worlds:[seen(1), {session:1, id:9, canvas:9, error:'shown, and never looked at'}], tells:[]}, none);
  expect([vanished.status, vanished.detail]).toEqual(['UNVERIFIED', 'session 1 9: shown, and never looked at']);
});

test('a measurement not taken leaves the look unverified, never passed', () => {
  const gaps = ['canvas 4: particles: not measured (the counts were not read back)'];
  const r = lookResult({worlds:[seen(1)], tells:[], gaps}, none);
  expect(r.status).toBe('UNVERIFIED');
  expect(r.missing).toEqual(gaps);
});

test('a tell fails the look alone, counted once a frame, and across runs a failure or a gap wins over a pass', () => {
  const line = 'look: 120 particles live, 0 pixels covered (particle dots · additive)';
  expect(tellOf(`tick=30 ${line}`)).toEqual({name:'particles-uncovered', line});
  const other = 'look: 30 particles live, 0 pixels covered (particles smoke.tex · alpha)';
  // Two batches in one frame and the same line in a second frame: two frames, not three.
  const tells = [{...tellOf(line), frame:1}, {...tellOf(other), frame:1}, {...tellOf(line), frame:2}];
  const r = lookResult({worlds:[seen(1)], tells}, none);
  expect([r.status, r.detail, r.loud[0].times]).toEqual(['FAIL', 'particles-uncovered', 2]);
  expect(lookResult({worlds:[seen(1)], tells}, new Set(['particles-uncovered'])).status).toBe('PASS');
  // No simulation assertion failed: its failure is the look's, whether the simulation passed or collected pins.
  expect(lookOnlyFailure({simulation:'PASS', failures:[], look:{status:'FAIL'}})).toBe(true);
  expect(lookOnlyFailure({simulation:'UNVERIFIED', failures:[], look:{status:'FAIL'}})).toBe(true);
  expect(lookOnlyFailure({simulation:'UNVERIFIED', failures:['pin 60 differs'], look:{status:'FAIL'}})).toBe(false);
  expect(lookOnlyFailure({simulation:'FAIL', failures:['x'], look:{status:'FAIL'}})).toBe(false);
  expect(lookOnlyFailure({simulation:'PASS', failures:[], look:{status:'UNVERIFIED'}})).toBe(false);
  const row = (host, status, repeat = 1) => ({host, repeat, look:{status}});
  expect(lookAcrossRuns([row('linux', 'PASS'), row('web', 'UNVERIFIED')]).status).toBe('UNVERIFIED');
  expect(lookAcrossRuns([row('linux', 'PASS'), row('web', 'FAIL')]).status).toBe('FAIL');
  expect(lookAcrossRuns([row('linux', 'PASS'), row('web', 'PASS')]).status).toBe('PASS');
  expect(lookAcrossRuns([]).status).toBe('UNVERIFIED');
  // A requested host whose run left no summary is a gap: linux's PASS does not stand for it.
  const hosts = [{host:'linux', repeat:1}, {host:'web', repeat:1}];
  expect(lookAcrossRuns([row('linux', 'PASS')], hosts)).toEqual({status:'UNVERIFIED', gaps:['web run 1: no look result']});
});

test('only a whole frame from the game\'s camera is judged, and a camera film judges each frame', () => {
  const tell = {name:'near-black', line:'look: mean luminance 0.004, near black'};
  const look = {tells:[tell], gaps:['sun casters: not measured (x)']};
  expect(frameTells({look}).frames).toEqual([[tell]]);
  expect(frameTells({look}).gaps).toEqual(['sun casters: not measured (x)']);
  expect(frameTells({look:{...look, reduced:true}}).frames).toEqual([]);
  expect(frameTells({look, view:{top:40}}).frames).toEqual([]);
  // A settle's or a screenshot's reply: lines, with gaps and failed look frames beside them.
  const settled = frameTells({look:[tell.line], lookGaps:['canvas 3: particles: not measured (y)'], lookFailed:['canvas 5: lost']});
  expect(settled.frames).toEqual([[tell]]);
  expect(settled.gaps).toEqual(['canvas 3: particles: not measured (y)', 'canvas 5: lost']);
  // A full-resolution camera film whose first frame fails and last is healthy keeps the first frame's tell.
  const film = lookAcross([{look}, {look:{tells:[], gaps:[]}}, {look:{tells:[tell], gaps:[]}, view:{top:10}}]);
  expect(film).toEqual({look:[tell.line], lookFrames:[[tell.line], []], lookGaps:['sun casters: not measured (x)']});
  expect(frameTells(film).frames).toEqual([[tell], []]);
  expect(lookAcross([{look:{...look, reduced:true}}])).toEqual({});
  expect(lookAcross([{look:['a']}, {lookGaps:['g'], lookSeen:[4]}, {look:['b'], lookSeen:[4]}]))
    .toEqual({look:['a', 'b'], lookFrames:[['a'], ['b']], lookGaps:['g'], lookSeen:[4]});
});

test('a tell line in a world\'s logs does not fail Save against Off', () => {
  const off = [{from:0, next:2, lines:['t=0 tick=0 spawned', 'tick=30 look: mean luminance 0.004, near black', 't=500 tick=30 rested'], tick:30}];
  const save = [{from:0, next:2, lines:['t=0 tick=0 spawned', 't=500 tick=30 rested'], tick:30}];
  expect(equal(off, save)).toBe(false);
  // Tell lines join a `logs` reply, never the journal ring: its cursors are the same in both.
  expect(equal(worldJournal(off), worldJournal(save))).toBe(true);
  expect(worldJournal(off)[0].lines).toEqual(['t=0 tick=0 spawned', 't=500 tick=30 rested']);
});

/** A fake game whose proof writes `summary(host, mode, profile)` and exits as the real one would. */
function fakeGame(name, summary) {
  const app = resolve(import.meta.dir, 'games', name);
  mkdirSync(app);
  const pins = {ticks:{1:'0x' + '1'.repeat(16)}, saves:{continuation:'a'.repeat(64)}};
  writeFileSync(resolve(app, 'pins.json'), JSON.stringify(pins));
  writeFileSync(resolve(app, 'proof.mjs'), `
    import {mkdirSync, writeFileSync} from 'node:fs';
    if (import.meta.main) {
    const out = process.env.EXACT_PROOF_OUT, host = process.argv[2], mode = process.env.EXACT_GAME_PARANOID;
    mkdirSync(out, {recursive:true});
    if (process.argv.includes('--build-only')) process.exit(0);
    const collecting = process.env.EXACT_PROOF_REPIN === '1';
    const row = (${summary})(host, mode, process.env.EXACT_GAME_PROOF_PROFILE, collecting);
    if (row) writeFileSync(out + '/summary.json', JSON.stringify({name:${JSON.stringify(name)}, inputs:'c'.repeat(64),
      sourceInputs:'d'.repeat(64), host, mode, pins:${JSON.stringify(pins)}, facilities:[], seconds:0,
      worlds:[{session:1, tick:1, hash:'same'}], saves:[{name:'a', sha256:'same'}], ...row}));
    // As proof.mjs exits: a failure, a look that failed, or an unverified run that was not collecting.
    process.exit(!row || row.failures.length || row.look?.status === 'FAIL' || (row.status !== 'PASS' && !collecting) ? 1 : 0);
    }
  `);
  return app;
}
const prove = async (name, args) => {
  const p = Bun.spawn([process.execPath, resolve(import.meta.dir, 'prove.mjs'), name, ...args], {stdout:'pipe', stderr:'pipe'});
  const [code, out, err] = await Promise.all([p.exited, new Response(p.stdout).text(), new Response(p.stderr).text()]);
  return {code, text:out + err};
};

test('a collecting repin run whose look fails records its pins; a missing host leaves the look unverified', async () => {
  const name = `look-repin-${process.pid}`;
  const app = fakeGame(name, `(host, mode, profile, collecting) => mode === '0'
    ? {status:'FAIL', simulation:collecting ? 'UNVERIFIED' : 'PASS', failures:[], look:{status:'FAIL', detail:'near-black'}}
    : {status:collecting ? 'UNVERIFIED' : 'PASS', simulation:collecting ? 'UNVERIFIED' : 'PASS', failures:[]}`);
  try {
    const repin = await prove(name, ['--repin', '--hosts', 'linux', '--reason', 'look-only failure']);
    expect(repin.code, repin.text).toBe(0);
    expect(JSON.parse(readFileSync(resolve(app, 'pins.json'), 'utf8')).reason).toBe('look-only failure');
  } finally { rmSync(app, {recursive:true, force:true}); }
  const missing = `look-host-${process.pid}`;
  const app2 = fakeGame(missing, `host => host === 'web' ? null
    : {status:'PASS', simulation:'PASS', failures:[], look:{status:'PASS', detail:'0 tells'}}`);
  try {
    const run = await prove(missing, ['--hosts', 'linux,web']);
    expect(run.code).toBe(1);
    expect(run.text).toContain('look: unverified, web run 1: no look result');
    expect(run.text).toMatch(/simulation: FAIL · look: UNVERIFIED/);
  } finally { rmSync(app2, {recursive:true, force:true}); }
});

// The browser negative control (LLP 1127.000 D6): a starter whose sparks burn 2 m under the ground fails its web
// proof on particles-uncovered, and the same starter with the sparks above the ground passes. Long: it builds a game.
test('in the browser, buried sparks fail the look and sparks above the ground pass it', async () => {
  const name = `look-sparks-${process.pid}`, app = resolve(import.meta.dir, 'games', name);
  createGame(name);
  const out = mkdtempSync(resolve(import.meta.dir, '../target/look-sparks-'));
  const lib = resolve(app, 'logic/src/lib.rs'), template = readFileSync(lib, 'utf8');
  const sparks = y => template
    .replace('        w.publish("total"', `        w.spawn_named("sparks", (Transform::at(1.5, ${y}, 0.0), Emitter { size: [0.4, 0.2], ..Emitter::sparks().rate(400.).lifetime(2.) }));\n        w.publish("total"`)
    .replace('    fn tick(w: &mut World, input: &Input, _: &Options) {\n', '    fn tick(w: &mut World, input: &Input, _: &Options) {\n        emitter::step(w);\n');
  const proof = async label => {
    for (let attempt = 0; attempt < 2; attempt++) {
      const dir = resolve(out, `${label}-${attempt}`);
      const p = Bun.spawn([process.execPath, resolve(app, 'proof.mjs'), 'web'], {cwd:resolve(import.meta.dir, '..'),
        env:{...process.env, EXACT_PROOF_OUT:dir, CARGO_TARGET_DIR:resolve(import.meta.dir, '../target/look-sparks')},
        stdout:'pipe', stderr:'pipe'});
      const text = await new Response(p.stdout).text() + await new Response(p.stderr).text();
      await p.exited;
      // A fresh game's first proof stops at the directory guard; the rerun is the proof.
      if (attempt === 0 && text.includes('changed during build; rerun the proof')) continue;
      return {text, summary:JSON.parse(readFileSync(resolve(dir, 'summary.json'), 'utf8'))};
    }
    throw new Error(`${label}: the proof never ran past its build`);
  };
  try {
    writeFileSync(lib, sparks('-2.0'));
    const buried = await proof('buried');
    expect(buried.summary.look.status, buried.text.slice(-3000)).toBe('FAIL');
    expect(buried.summary.look.tells.map(t => t.name)).toContain('particles-uncovered');
    expect(buried.summary.failures).toEqual([]);
    writeFileSync(lib, sparks('1.0'));
    const above = await proof('above');
    expect(above.summary.look.status, above.text.slice(-3000)).toBe('PASS');
  } finally {
    rmSync(app, {recursive:true, force:true});
    rmSync(out, {recursive:true, force:true});
  }
}, 2400000);
