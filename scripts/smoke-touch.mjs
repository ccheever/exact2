#!/usr/bin/env bun
// Real touches on an iOS simulator (LLP 1080.000 §5–§6): Caltrain driven
// through the touch runner (`--touch platform`), in both timing modes. Each
// `tap` must reply `delivery: platform` with the touch the app's window
// dispatched; what the tap did is read from `tree`, never from the reply.
// A held contact is one real touch played at its lift (§14); a whole real drag
// (`tap … drag`, §11) scrolls the stations, with a read while the finger is
// down, and a still press opens them as a tap does. Prints the runner's
// start and per-tap times (G2) as observations, never a gate. A runner that
// does not start fails: this simulator is the supported destination. Then a
// grouped list (LLP 1084, `contract/corpus/grouped-touch.contract`, in
// Caltrain's bundle): a real tap on a row's switch, detail button or row
// lands on the cell or accessory UIKit draws, not the hidden authored node.
// The shared decorations fixture checks a native marker tap emits exactly one
// toggle and an atom tap selects the entire source range before deletion.
//   bun scripts/smoke-touch.mjs [--build] [--sim <udid|name>] [--decorations]
import { spawnSync } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open } from './agent.mjs';
import { HOST_DEV } from './app.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
if (argv.includes('--build')) {
  const built = spawnSync('bun', ['host/apple/build.mjs', '--ios', 'caltrain-apple'], { cwd: ROOT, stdio: 'inherit' });
  if (built.status !== 0) { console.error('FAIL ios-touch build: host/apple/build.mjs --ios caltrain-apple'); process.exit(1); }
}
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const t0 = Date.now();

for (const timing of (argv.includes('--decorations') ? [] : ['agent', 'platform'])) {
  const opened = Date.now();
  let s;
  try { s = await open({ host: 'ios', app: 'caltrain', touch: 'platform', timing }); }
  catch (error) { check(false, `${timing}: the session did not open: ${error.message}`); continue; }
  try {
    console.log(`${timing}: opened in ${Date.now() - opened} ms (the runner's start, then the app's)`);
    const taps = [];
    const tap = async (target) => {
      const at = Date.now();
      const r = await s.tap(target);
      taps.push(Date.now() - at);
      check(r.delivery === 'platform' && r.landed?.session === 'main' && r.touch?.type === 'direct', `${timing}: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
      return r;
    };
    // A still press (§11): the finger down 300 ms, a read inside that hold, then up, presses as a tap does.
    const still = await s.tap('change-station', { drag: { dx: 0, dy: 0, hold: 300, during: [async () => (await s.tree()).nodes.length] } });
    check(still.delivery === 'platform' && still.touch?.moved === 0 && still.during?.[0] > 0, `${timing}: a still press was not one real touch with a read while down: ${JSON.stringify(still)}`);
    if (timing === 'platform') await s.clock('settle');
    let tree = await s.tree();
    check(byTestId(tree, 'station-search'), `${timing}: a still real press on change-station did not open the stations (landed on ${JSON.stringify(still.landed)})`);
    // A whole real drag up the stations scrolls them: the row moves up the screen.
    const row = async () => (await s.layout()).nodes.find((n) => n.props?.testId === 'station-paloalto' || n.testId === 'station-paloalto')?.y;
    const y0 = await row();
    const dragged = await s.tap('stations-screen', { drag: { dx: 0, dy: -250, over: 400, hold: 100 } });
    await s.clock('settle');
    const y1 = await row();
    check(dragged.delivery === 'platform' && dragged.touch?.moved > 0 && y0 - y1 > 150, `${timing}: a real drag of 250 pt moved the stations ${y0 - y1} pt: ${JSON.stringify(dragged)}`);
    console.log(`${timing}: drag moved the stations ${y0 - y1} pt in ${dragged.touch?.moved} moves`);
    await s.type('station-search', 'Palo');
    await tap('station-paloalto');
    if (timing === 'platform') await s.clock('settle');
    tree = await s.tree();
    check(byTestId(tree, 'station-name')?.props.text === 'Palo Alto', `${timing}: after a real tap on station-paloalto the station is ${byTestId(tree, 'station-name')?.props.text}`);
    // A held contact is one real touch, played whole at its lift (LLP 1080.000 §14): the runner holds none across requests (P3).
    const down = await s.tap('station-name', { down: true });
    await s.pointer('hold', { ms: 600 });
    const up = await s.pointer('up');
    check(down.delivery === 'platform' && up.delivery === 'platform' && up.touch?.type === 'direct' && up.touch.ended - up.touch.began > 0.5, `${timing}: a held contact was not one real touch held 600 ms: ${JSON.stringify({ down, up })}`);
    console.log(`${timing}: taps ${taps.join(', ')} ms`);
  } catch (error) {
    check(false, `${timing}: ${error.message}`);
  } finally { await s.close(); }
}

// A grouped list's rows under real touches: each tap aims at what UIKit
// draws (the switch, the detail button, the cell) and does what a finger does.
if (!argv.includes('--decorations')) {
  const plan = resolve(mkdtempSync(resolve(tmpdir(), 'exact-touch-')), 'grouped-touch.plan');
  const c = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/grouped-touch.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  let s;
  if (check(c.status === 0, `grouped: the fixture did not compile: ${c.stderr}`)) {
    try {
      s = await open({ host: 'ios', app: 'caltrain', plan, touch: 'platform' });
      const facts = async () => byTestId(await s.tree(), 'facts')?.props.text;
      for (const [target, want] of [['toggle', 'true 0 0'], ['toggle', 'false 0 0'], ['info', 'false 1 0'], ['open', 'false 1 1']]) {
        const r = await s.tap(target);
        check(r.delivery === 'platform' && r.touch?.type === 'direct', `grouped: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
        await s.clock('settle');
        const seen = await facts();
        check(seen === want, `grouped: after a real tap on ${target} the facts read ${seen}, not ${want} (landed ${JSON.stringify(r.landed)})`);
      }
      console.log('grouped: a real switch, detail button and row tap each landed on UIKit\'s cell');
    } catch (error) {
      check(false, `grouped: ${error.message}`);
    } finally { await s?.close(); }
  }
}

// Coordinates come from action-time TextKit ranges, in viewport points. The
// fixture authors a 24 pt marker gutter and a 16 pt font, so its marker center
// is 16 pt left of the task range. No screenshot or device size is assumed.
{
  const plan = resolve(mkdtempSync(resolve(tmpdir(), 'exact-touch-')), 'decorations.plan');
  const c = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', resolve(ROOT, 'host/web-js/conformance/decorations.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  let s;
  if (check(c.status === 0, `decorations: the fixture did not compile: ${c.stderr}`)) {
    try {
      s = await open({ host: 'ios', app: 'caltrain', plan, touch: 'platform' });
      await s.clock('settle');
      const initialFailures = failures.length;
      const original = (await s.state()).slots.draft;
      await s.tap('probe');
      await s.clock('settle');
      const geometry = (await s.state()).slots.geometry.split(',').map(Number);
      if (geometry.length !== 7 || !geometry.every(Number.isFinite) || [geometry[2], geometry[5], geometry[6]].some(n => n <= 0)) throw new Error(`invalid range geometry: ${geometry}`);
      const [taskX, taskY, taskHeight, atomX, atomY, atomWidth, atomHeight] = geometry;
      const frame = (await s.layout('composer')).node.frame;
      const tap = async (x, y) => {
        const reply = await s.tap('composer', { at: [x - frame.x, y - frame.y] });
        check(reply.delivery === 'platform' && reply.touch?.type === 'direct', `decorations: expected a real touch: ${JSON.stringify(reply)}`);
        await s.clock('settle');
      };
      await tap(taskX - 24 + 8, taskY + taskHeight / 2);
      let state = await s.state();
      check(state.slots.draft === original.replace('[ ]', '[x]') && state.slots.toggleCount === 1,
        `decorations: one marker tap must write one checked source slot: ${JSON.stringify(state.slots)}`);
      check(state.decorations[0].applied.record.blocks.at(-1).marker === 'checkbox:on' && !state.decorations[0].applied.provisional,
        'decorations: checked metadata must settle after its same-length source echo');
      await tap(taskX - 24 + 8, taskY + taskHeight / 2);
      state = await s.state();
      check(state.slots.draft === original && state.slots.toggleCount === 2,
        `decorations: a second marker tap must toggle once: ${JSON.stringify(state.slots)}`);
      await tap(atomX + atomWidth / 2, atomY + atomHeight / 2);
      const selected = (await s.state()).slots;
      check(selected.draft === original && selected.lastSelection === '27:33',
        `decorations: an atom tap must select its complete unchanged source range: ${JSON.stringify(selected)}`);
      const deletion = await s.type('composer', { key: 'Backspace' });
      console.log(`decorations: atomic deletion ${JSON.stringify({ selected, deletion })}`);
      await s.clock('settle');
      state = await s.state();
      check(state.slots.draft === original.replace('@alice', '') && state.decorations[0].applied.record.atoms.length === 0,
        `decorations: a native atom tap and Backspace must delete the complete source atom: ${JSON.stringify(state.slots)}`);
      check(state.slots.toggleCount === 2, 'decorations: an atom tap must not emit a checkbox toggle');
      check(state.slots.inputCount === selected.inputCount + 1, 'decorations: Backspace must publish exactly one source edit');
      if (failures.length === initialFailures) console.log('decorations: real marker taps toggle once each; atom tap and Backspace delete its complete source');
    } catch (error) {
      check(false, `decorations: ${error.message}`);
    } finally { await s?.close(); }
  }
}

console.log(`touch smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
for (const f of failures) console.error(`FAIL ios-touch ${f}`);
process.exit(failures.length ? 1 : 0);
