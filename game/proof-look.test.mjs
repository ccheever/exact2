// The look result (LLP 1127.000 D5): what makes it PASS, FAIL or UNVERIFIED in
// one proof and across a prove's hosts, apart from the simulation's.
import {test, expect} from 'bun:test';
import {equal, frameTells, lookAcrossRuns, lookOnlyFailure, lookResult, tellOf, worldJournal} from './proof.mjs';
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
});

test('a measurement not taken leaves the look unverified, never passed', () => {
  const gaps = ['canvas 4: particles: not measured (the counting pass\'s counts were not read back)'];
  const r = lookResult({worlds:[seen(1)], tells:[], gaps}, none);
  expect(r.status).toBe('UNVERIFIED');
  expect(r.missing).toEqual(gaps);
});

test('a tell fails the look alone, counted once a frame, and across hosts a failure or a gap wins over a pass', () => {
  const line = 'look: 120 particles live, 0 pixels covered (particle dots · additive)';
  expect(tellOf(`tick=30 ${line}`)).toEqual({name:'particles-uncovered', line});
  const tells = [tellOf(line), tellOf(line)];
  const r = lookResult({worlds:[seen(1)], tells}, none);
  expect([r.status, r.detail, r.loud[0].times]).toEqual(['FAIL', 'particles-uncovered', 2]);
  expect(lookResult({worlds:[seen(1)], tells}, new Set(['particles-uncovered'])).status).toBe('PASS');
  // The run's simulation passed: its failure is the look's.
  expect(lookOnlyFailure({simulation:'PASS', look:{status:'FAIL'}})).toBe(true);
  expect(lookOnlyFailure({simulation:'FAIL', look:{status:'FAIL'}})).toBe(false);
  expect(lookOnlyFailure({simulation:'PASS', look:{status:'UNVERIFIED'}})).toBe(false);
  expect(lookAcrossRuns([{status:'PASS'}, {status:'UNVERIFIED'}])).toBe('UNVERIFIED');
  expect(lookAcrossRuns([{status:'PASS'}, undefined])).toBe('UNVERIFIED');
  expect(lookAcrossRuns([{status:'PASS'}, {status:'FAIL'}])).toBe('FAIL');
  expect(lookAcrossRuns([{status:'PASS'}, {status:'PASS'}])).toBe('PASS');
  expect(lookAcrossRuns([])).toBe('UNVERIFIED');
});

test('only a whole frame from the game\'s camera is judged', () => {
  const tell = {name:'near-black', line:'look: mean luminance 0.004, near black'};
  const look = {tells:[tell], gaps:['sun casters: not measured (x)']};
  expect(frameTells({look}).tells).toEqual([tell]);
  expect(frameTells({look}).gaps).toEqual(['sun casters: not measured (x)']);
  expect(frameTells({look:{...look, reduced:true}}).tells).toEqual([]);
  expect(frameTells({look, view:{top:40}}).tells).toEqual([]);
  // A settle's or a screenshot's reply: lines, with gaps and failed look frames beside them.
  const settled = frameTells({look:[tell.line], lookGaps:['canvas 3: particles: not measured (y)'], lookFailed:['canvas 5: lost']});
  expect(settled.tells).toEqual([tell]);
  expect(settled.gaps).toEqual(['canvas 3: particles: not measured (y)', 'canvas 5: lost']);
  // A film's frames together.
  expect(lookAcross([{look:['a']}, {lookGaps:['g']}, {look:['b']}])).toEqual({look:['a', 'b'], lookGaps:['g']});
  // A film of look frames keeps each frame's own `look`: nothing is merged.
  expect(lookAcross([{look:{tells:[]}}])).toEqual({});
});

test('a tell line in a world\'s logs does not fail Save against Off', () => {
  const off = [{from:0, next:2, lines:['t=0 tick=0 spawned', 'tick=30 look: mean luminance 0.004, near black', 't=500 tick=30 rested'], tick:30}];
  const save = [{from:0, next:2, lines:['t=0 tick=0 spawned', 't=500 tick=30 rested'], tick:30}];
  expect(equal(off, save)).toBe(false);
  // Tell lines join a `logs` reply, never the journal ring: its cursors are the same in both.
  expect(equal(worldJournal(off), worldJournal(save))).toBe(true);
  expect(worldJournal(off)[0].lines).toEqual(['t=0 tick=0 spawned', 't=500 tick=30 rested']);
});
