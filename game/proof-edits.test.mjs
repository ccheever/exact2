import {test, expect} from 'bun:test';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {editProvenance, pinRecorder} from './proof.mjs';

test('admitted edits refuse capture, match and collection across restart, restore and close', () => {
  for (const collecting of [false, true]) {
    const provenance = editProvenance();
    const clean = {tick:0, hash:'0x' + '0'.repeat(15) + '1'};
    const edited = {...clean, developmentEdits:true};
    const recorder = pinRecorder({ticks:{0:clean.hash}}, 'fixture', () => {}, collecting, provenance);
    provenance.observe([edited]);
    expect(() => recorder.pin(0, edited)).toThrow('set development edit');
    provenance.observe([clean]); // setup-argument restart
    provenance.observe([clean]); // clean restore
    expect(() => recorder.pin(0, clean)).toThrow('set development edit');
    const dir = mkdtempSync(join(tmpdir(), 'proof-edits-'));
    try {
      const save = join(dir, 'checkpoint.world'); writeFileSync(save, 'clean save');
      expect(() => recorder.pinSave('checkpoint', save)).toThrow('set development edit');
    } finally { rmSync(dir, {recursive:true, force:true}); }
    provenance.observe([]); // world removed before close
    expect(() => provenance.assert()).toThrow('set development edit');
    expect(recorder.pins).toEqual({ticks:{}, saves:{}});
  }
  const refused = editProvenance();
  refused.observe([{}], [{lines:['t=17 tick=1 set refused: production bake', 't=17 tick=1 set difficulty hard', 't=17 tick=1 set player Missing.speed 8']}]);
  expect(() => refused.assert()).not.toThrow();
});

test('the ordinary world snapshot keeps admitted-edit provenance', async () => {
  const {worldView} = await import('../scripts/agent-inspect.mjs');
  const snapshot = await worldView({state:async () => ({tick:1, hash:'0x' + '0'.repeat(15) + '1', developmentEdits:true, entities:[]})}, 'world').snapshot();
  expect(snapshot.developmentEdits).toBe(true);
});

test('queued sets refuse pin and pinSave until the queue drains', () => {
  for (const collecting of [false, true]) {
    const provenance = editProvenance();
    const state = {tick:0, hash:'0x' + '0'.repeat(15) + '1', queuedSets:1};
    const recorder = pinRecorder({ticks:{0:state.hash}}, 'fixture', () => {}, collecting, provenance);
    provenance.observe({world:[state]});
    expect(() => recorder.pinSave('queued', '/unused')).toThrow('queued set development edit');
    expect(() => recorder.pin(0, state)).toThrow('queued set development edit');
    provenance.observe({world:[{...state, queuedSets:0}]});
    expect(() => provenance.assert()).not.toThrow();
  }
});

test('host session evidence refuses pins when no edited world was observed', () => {
  for (const collecting of [false, true]) {
    const provenance = editProvenance();
    const clean = {tick:0, hash:'0x' + '0'.repeat(15) + '1'};
    const recorder = pinRecorder({ticks:{0:clean.hash}}, 'fixture', () => {}, collecting, provenance);
    provenance.observe({world:[], sessionDevelopmentEdits:true});
    provenance.observe({world:[clean], sessionDevelopmentEdits:true});
    expect(() => recorder.pin(0, clean)).toThrow('set development edit');
    expect(() => recorder.pinSave('removed', '/unused')).toThrow('set development edit');
    expect(() => provenance.assert()).toThrow('set development edit');
    expect(recorder.pins).toEqual({ticks:{}, saves:{}});
  }
});

// Builds a generated game, then drives the host ABI and Contract lifecycle. E10
// keeps these native/browser builds separate from the recorder's unit tests.
test('E10 real admission survives canvas removal and remount inside one clock operation', async () => {
  const {createGame} = await import('./new.mjs');
  const {mkdirSync, readFileSync, existsSync} = await import('node:fs');
  const {resolve} = await import('node:path');
  const root = resolve(import.meta.dir, '..');
  mkdirSync(resolve(root, 'target'), {recursive:true});
  const temporary = mkdtempSync(resolve(root, 'target/edit-lifetime-'));
  const app = process.env.EXACT_EDIT_PROBE ?? resolve(temporary, 'edit-lifetime');
  const originals = new Map();
  const write = (name, text) => {
    const path = resolve(app, name);
    if (process.env.EXACT_EDIT_PROBE) originals.set(path, readFileSync(path));
    writeFileSync(path, text);
  };
  try {
    if (!process.env.EXACT_EDIT_PROBE) {
      createGame(app);
      write('logic/src/lib.rs', `use exact_game::*;
#[derive(Default, Component)] struct Tuning { speed: f32 }
pub struct EditLifetime;
impl Game for EditLifetime {
    const ID: &'static str = "edit-lifetime";
    type Args = ();
    fn setup(w: &mut World, _: &()) { w.spawn_named("player", Tuning { speed: 4. }); }
    fn tick(w: &mut World, input: &Input, _: &()) {
        if input.messages().iter().any(|m| m == "replace") { w.emit("replace"); }
    }
}
`);
      write('app.contract', `component EditLifetime
  state alternate = false
  action replace(message: string)
    if message == "replace"
      alternate = not alternate
  action cycle
    postMessage("set player Tuning.speed 8", "world")
    postMessage("replace", "world")
  view
    column
      button press=cycle testId="cycle"
        text "Cycle"
      if alternate
        canvas surface=world() message=replace testId="world" width=64 height=64
      else
        canvas surface=world() message=replace testId="world" width=64 height=64
`);
    }
    write('pins.json', JSON.stringify({ticks:{}, saves:{}}));
    write('proof.mjs', `import {proof} from ${JSON.stringify(resolve(root, 'game/proof.mjs'))};
await proof(import.meta, async ({open, check, pin, pinSave}) => {
  const s = await open();
  if ((await s.tree()).nodes.some(n => n.props?.testId === 'play')) await s.tap('play');
  const refused = (label, run, text) => {
    try { run(); check(label, false); } catch (e) { check(label, e.message.includes(text), e.message); }
  };
  await s.tap('cycle');
  const queued = await s.world('world').snapshot();
  check('queued set count', queued.queuedSets === 1 && !queued.developmentEdits);
  refused('queued pin', () => pin(queued.tick, queued), 'queued set');
  refused('queued pinSave', () => pinSave('queued', '/unused'), 'queued set');
  await s.clock('+34');
  const clean = await s.world('world').snapshot();
  check('clock remounted a clean world', clean.tick === 0 && !clean.developmentEdits);
  check('host session retained admission', clean.sessionDevelopmentEdits === true);
  refused('remounted pin', () => pin(clean.tick, clean), 'set development edit');
  refused('remounted pinSave', () => pinSave('clean', '/unused'), 'set development edit');
  await s.close();
});
`);
    for (const host of ['linux', 'web', 'macos']) for (const collecting of [false, true]) {
      const out = resolve(temporary, `${host}-${collecting}`); mkdirSync(out);
      const child = Bun.spawn([process.execPath, resolve(app, 'proof.mjs'), host], {
        cwd:root, env:{...process.env, EXACT_PROOF_REPIN: collecting ? '1' : '0', EXACT_PROOF_OUT:out},
        stdout:Bun.file(resolve(out, 'run.log')), stderr:Bun.file(resolve(out, 'build.log')),
      });
      expect(await child.exited).toBe(1);
      const summary = JSON.parse(readFileSync(resolve(out, 'summary.json'), 'utf8'));
      expect(summary.failures).toEqual(['pin refused: run contains a set development edit']);
      expect(summary.pins).toEqual({ticks:{}, saves:{}});
      console.log(`PASS real ${host} collecting=${collecting}: admission, remount, capture, match, pinSave and close`);
    }
  } finally {
    for (const [path, bytes] of originals) writeFileSync(path, bytes);
    if (existsSync(temporary)) rmSync(temporary, {recursive:true, force:true});
  }
}, 600000);
