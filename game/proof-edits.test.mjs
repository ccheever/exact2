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

// Captured EXSIM v7 bytes from Editable in world/edit_tests.rs, before and
// after admission. The pending artifact replays its edit even after live restart.
const editSaves = {
  clean:Buffer.from('RVhTSU0ABwgBAARnYW1lBgpzZXQtb3JhY2xlAQAFd29ybGQM/wFFWEdBTUUABQgBAAVzdGF0ZQgBAAR0aWNrAgABAAJoegI8AQAEc2VlZAIAAQAFc2xvdHMMNgADCmdlbmVyYXRpb24FYWxpdmUEbmFtZQEMCAEAAgEBEAECCwYAAQAAAgAAAgECBnBsYXllcgEABGJ1c3kHAAABAANybmcIAQECstrppZjCu4QaAAEACmNvbXBvbmVudHMIAQAGVHVuaW5nDE0BAAEDBXNwZWVkBWxhYmVsCWRpcmVjdGlvbgEPCAEABAEBBgECBwMEBAQAAQAAAgAAgEAAAgAAAAAAAgAAAAAAAgAAAAACBnBsYXllcgABAAlyZXNvdXJjZXMIAAABAARhcmdzBhF7InJlc3RhcnQiOmZhbHNlfQEABWlucHV0CAEABGtleXMHAAEAB3ByZXNzZWQHAAEACHJlbGVhc2VkBwABAAdwb2ludGVyCgEACGNvbnRhY3RzBwABAAV3aGVlbAcCBAAAAAAEAAAAAAEACHZpZXdwb3J0BwIEAAAAAAQAAAAAAQAIbWVzc2FnZXMHAAABAAVxdWV1ZQcAAQAId29ybGRfdXMDAAEAB2pvdXJuYWwHAQgBAAVpbmRleAIAAQAEdGljawIAAQAHc2Vjb25kcwUAAAAAAAAAAAEABGxpbmUGE3Q9MCB0aWNrPTAgc3Bhd24gIzAAAQAMam91cm5hbF9uZXh0AgEBAA9vdmVyZmxvd19sb2dnZWQAAQAJcHVibGlzaGVkCAAA', 'base64'),
  pending:Buffer.from('RVhTSU0ABwgBAARnYW1lBgpzZXQtb3JhY2xlAQAFd29ybGQM/wFFWEdBTUUABQgBAAVzdGF0ZQgBAAR0aWNrAgABAAJoegI8AQAEc2VlZAIAAQAFc2xvdHMMNgADCmdlbmVyYXRpb24FYWxpdmUEbmFtZQEMCAEAAgEBEAECCwYAAQAAAgAAAgECBnBsYXllcgEABGJ1c3kHAAABAANybmcIAQECstrppZjCu4QaAAEACmNvbXBvbmVudHMIAQAGVHVuaW5nDE0BAAEDBXNwZWVkBWxhYmVsCWRpcmVjdGlvbgEPCAEABAEBBgECBwMEBAQAAQAAAgAAgEAAAgAAAAAAAgAAAAAAAgAAAAACBnBsYXllcgABAAlyZXNvdXJjZXMIAAABAARhcmdzBhF7InJlc3RhcnQiOmZhbHNlfQEABWlucHV0CAEABGtleXMHAAEAB3ByZXNzZWQHAAEACHJlbGVhc2VkBwABAAdwb2ludGVyCgEACGNvbnRhY3RzBwABAAV3aGVlbAcCBAAAAAAEAAAAAAEACHZpZXdwb3J0BwIEAAAAAAQAAAAAAQAIbWVzc2FnZXMHAAABAAVxdWV1ZQcBCAEAB2hvc3RfdXMDAAEACHdvcmxkX3VzCwMAAQAFZXZlbnQJBQAHTWVzc2FnZQgBAAR0ZXh0BhlzZXQgcGxheWVyIFR1bmluZy5zcGVlZCA4AQAFYXRfbXMFAAAAAAAAAAAAAAEPAwABAAdqb3VybmFsBwEIAQAFaW5kZXgCAAEABHRpY2sCAAEAB3NlY29uZHMFAAAAAAAAAAABAARsaW5lBhN0PTAgdGljaz0wIHNwYXduICMwAAEADGpvdXJuYWxfbmV4dAIBAQAPb3ZlcmZsb3dfbG9nZ2VkAAEACXB1Ymxpc2hlZAgAAA==', 'base64'),
  edited:Buffer.from('RVhTSU0ABwgBAARnYW1lBgpzZXQtb3JhY2xlAQAFd29ybGQMkwJFWEdBTUUABQgBAAVzdGF0ZQgBAAR0aWNrAgEBAAJoegI8AQAEc2VlZAIAAQAFc2xvdHMMNgADCmdlbmVyYXRpb24FYWxpdmUEbmFtZQEMCAEAAgEBEAECCwYAAQAAAgAAAgECBnBsYXllcgEABGJ1c3kHAAABAANybmcIAQECstrppZjCu4QaAAEAEGRldmVsb3BtZW50RWRpdHMBAQAKY29tcG9uZW50cwgBAAZUdW5pbmcMTQEAAQMFc3BlZWQFbGFiZWwJZGlyZWN0aW9uAQ8IAQAEAQEGAQIHAwQEBAABAAACAAAAQQACAAAAAAACAAAAAAACAAAAAAIGcGxheWVyAAEACXJlc291cmNlcwgAAAEABGFyZ3MGEXsicmVzdGFydCI6ZmFsc2V9AQAFaW5wdXQIAQAEa2V5cwcAAQAHcHJlc3NlZAcAAQAIcmVsZWFzZWQHAAEAB3BvaW50ZXIKAQAIY29udGFjdHMHAAEABXdoZWVsBwIEAAAAAAQAAAAAAQAIdmlld3BvcnQHAgQAAAAABAAAAAABAAhtZXNzYWdlcwcAAAEABXF1ZXVlBwABAAh3b3JsZF91cwPQiQIBAAdqb3VybmFsBwIIAQAFaW5kZXgCAAEABHRpY2sCAAEAB3NlY29uZHMFAAAAAAAAAAABAARsaW5lBhN0PTAgdGljaz0wIHNwYXduICMwAAgBEAIBARECAAESBQAAAAAAAAAAARMGJHQ9MCB0aWNrPTAgc2V0IHBsYXllciBUdW5pbmcuc3BlZWQgOAABAAxqb3VybmFsX25leHQCAgEAD292ZXJmbG93X2xvZ2dlZAABAAlwdWJsaXNoZWQIAAA=', 'base64'),
};

test('pinSave inspects captured saves after argument restart cancels the live queue', () => {
  const dir = mkdtempSync(join(tmpdir(), 'proof-saved-edits-'));
  try {
    const paths = Object.fromEntries(Object.entries(editSaves).map(([name, bytes]) => {
      const path = join(dir, `${name}.world`); writeFileSync(path, bytes); return [name, path];
    }));
    for (const collecting of [false, true]) {
      const provenance = editProvenance();
      const state = {tick:0, hash:'0x' + '0'.repeat(15) + '1', queuedSets:1};
      const recorder = pinRecorder({}, 'fixture', () => {}, collecting, provenance);
      provenance.observe({world:[state]}, [], 'A');
      expect(() => recorder.pinSave('pending', paths.pending)).toThrow('queued set development edit');
      expect(() => recorder.pin(0, state)).toThrow('queued set development edit');
      // Argument restart clears the live queue before its admitting tick.
      provenance.observe({world:[{...state, queuedSets:0}]}, [], 'A');
      expect(() => recorder.pin(0, {...state, queuedSets:0})).not.toThrow();
      expect(() => recorder.pinSave('pending', paths.pending)).toThrow('save contains a queued set development edit');
      expect(() => recorder.pinSave('edited', paths.edited)).toThrow('save contains a set development edit');
      expect(() => recorder.pinSave('clean', paths.clean)).not.toThrow();
      expect(Object.keys(recorder.pins.saves)).toEqual(['clean']);
      // Replace only the captured Message's length-prefixed text; keep its queue envelope.
      const command = Buffer.from('set player Tuning.speed 8'), at = editSaves.pending.indexOf(command);
      expect(at).toBeGreaterThan(0);
      for (const [text, refused] of [
        ['  set\u0085player Tuning.speed 8  ', true],
        ['set player Tuning.speed.extra 8', false],
        ['set difficulty hard', false],
        ['set player Tuning.speed   ', false],
      ]) {
        const path = join(dir, 'message.world'), bytes = Buffer.from(text);
        writeFileSync(path, Buffer.concat([editSaves.pending.subarray(0, at - 1),
          Buffer.from([bytes.length]), bytes, editSaves.pending.subarray(at + command.length)]));
        const run = () => recorder.pinSave('message', path);
        if (refused) expect(run).toThrow('save contains a queued set development edit');
        else expect(run).not.toThrow();
      }
      const malformed = join(dir, 'malformed.world');
      for (const bytes of [Buffer.from('not a save'), editSaves.pending.subarray(0, -1)]) {
        writeFileSync(malformed, bytes);
        expect(() => recorder.pinSave('malformed', malformed)).toThrow('pinSave refused: invalid');
      }
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('a clean session observation cannot clear another session pending edit', () => {
  const provenance = editProvenance();
  provenance.observe({world:[{queuedSets:1}]}, [], 'A');
  provenance.observe({world:[{queuedSets:0}]}, [], 'B');
  expect(() => provenance.assert()).toThrow('queued set development edit');
  provenance.observe({world:[]}, [], 'B');
  expect(() => provenance.assert()).toThrow('queued set development edit');
  provenance.observe({world:[{queuedSets:0}]}, [], 'A');
  expect(() => provenance.assert()).not.toThrow();
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
#[derive(Default, Args)] pub struct Options { #[restart] pub restart: bool }
pub struct EditLifetime;
impl Game for EditLifetime {
    const ID: &'static str = "edit-lifetime";
    type Args = Options;
    fn setup(w: &mut World, _: &Options) { w.spawn_named("player", Tuning { speed: 4. }); }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        if input.messages().iter().any(|m| m == "replace") { w.emit("replace"); }
    }
}
`);
      write('app.contract', `component EditLifetime
  state alternate = false
  state again = false
  action restart
    again = not again
  action queue
    postMessage("set player Tuning.speed 8", "world")
  action replace(message: string)
    if message == "replace"
      alternate = not alternate
  action cycle
    postMessage("set player Tuning.speed 8", "world")
    postMessage("replace", "world")
  view
    column
      button press=restart testId="restart"
        text "Restart"
      button press=queue testId="queue"
        text "Queue"
      button press=cycle testId="cycle"
        text "Cycle"
      if alternate
        canvas surface=world(restart=again) message=replace testId="world" width=64 height=64
      else
        canvas surface=world(restart=again) message=replace testId="world" width=64 height=64
`);
    }
    write('pins.json', JSON.stringify({ticks:{}, saves:{}}));
    write('proof.mjs', `import {proof} from ${JSON.stringify(resolve(root, 'game/proof.mjs'))};
await proof(import.meta, async ({open, check, pin, pinSave, out}) => {
  const s = await open();
  if ((await s.tree()).nodes.some(n => n.props?.testId === 'play')) await s.tap('play');
  const refused = (label, run, text) => {
    try { run(); check(label, false); } catch (e) { check(label, e.message.includes(text), e.message); }
  };
  await s.tap('queue');
  const pendingFile = out + '/pending.world';
  await s.world('world').save(pendingFile);
  await s.tap('restart');
  const restarted = await s.world('world').snapshot();
  check('restart before admission clears only the live queue', restarted.queuedSets === 0 && !restarted.developmentEdits);
  refused('captured pending save after restart', () => pinSave('pending', pendingFile), 'save contains a queued set development edit');
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
}, 1200000);
