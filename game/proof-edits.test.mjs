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
