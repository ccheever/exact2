import {test, expect} from 'bun:test';
import {proofVerdict} from './proof.mjs';

test('every proof path has its verdict, exit code and reason', () => {
  const pins = {ticks:{1:'hash'}, saves:{a:'bytes'}};
  const complete = {failures:[], expected:pins, pins};
  expect(proofVerdict(complete)).toEqual({status:'PASS', code:0, reason:'complete baseline checked'});
  expect(proofVerdict({...complete, failures:['broken']}, ['--build-only']).code).toBe(1);
  for (const [args, reason] of [[['--build-only'],'build only'], [['--screenshot-only'],'pixels only'], [['--capture40'],'pixels only']])
    expect(proofVerdict(complete,args)).toEqual({status:'UNVERIFIED', code:3, reason});
  expect(proofVerdict({...complete, collecting:true}).code).toBe(3);
  expect(proofVerdict({...complete, expected:{}}).reason).toBe('no baseline yet (expected before the first prove)');
  expect(proofVerdict({...complete, pins:{}}).code).toBe(3);
});

test('coverage names missing modes and release without upgrading native-only legacy pins', async () => {
  const {pinCoverageGaps} = await import('./proof.mjs');
  expect(pinCoverageGaps({hosts:['linux','web']})).toEqual([]);
  expect(pinCoverageGaps({hosts:['linux']})).toEqual(['web not pinned']);
  expect(pinCoverageGaps({hosts:['linux','web'],modes:['0'],profiles:['gpu-dev']}))
    .toEqual(['mode 1 not pinned','mode fresh-game not pinned','release not pinned']);
});

test('delivery refuses development pins and accepts release or legacy coverage', async () => {
  const {requireReleasePins} = await import('./inputs.mjs');
  const {mkdtempSync,writeFileSync,rmSync} = await import('node:fs');
  const {tmpdir} = await import('node:os');
  const dir = mkdtempSync(tmpdir()+'/release-pins-');
  const app = {dir,manifest:{game:{}}};
  try {
    expect(() => requireReleasePins(app)).toThrow('release not pinned');
    const pins = {hosts:['linux','web'],ticks:{0:'hash'},saves:{a:'bytes'}};
    writeFileSync(dir+'/pins.json',JSON.stringify({...pins,profiles:['gpu-dev']}));
    expect(() => requireReleasePins(app)).toThrow('prove --release');
    writeFileSync(dir+'/pins.json',JSON.stringify({...pins,profiles:['gpu-dev','release']}));
    expect(() => requireReleasePins(app)).not.toThrow();
    for (const coverage of [{hosts:['linux']},{hosts:['web']},{modes:['0']},{profiles:['release']}]) {
      writeFileSync(dir+'/pins.json',JSON.stringify({...pins,...coverage}));
      expect(() => requireReleasePins(app)).toThrow('game delivery refused');
    }
    writeFileSync(dir+'/pins.json',JSON.stringify(pins));
    expect(() => requireReleasePins(app)).not.toThrow();
  } finally { rmSync(dir,{recursive:true,force:true}); }
});


test('deploy refuses native-only release pins at its command entrypoint', async () => {
  const {createGame} = await import('./new.mjs');
  const {mkdtempSync,writeFileSync,readFileSync,rmSync} = await import('node:fs');
  const {spawnSync} = await import('node:child_process');
  const {resolve} = await import('node:path');
  const {tmpdir} = await import('node:os');
  const parent = mkdtempSync(resolve(tmpdir(),'delivery-coverage-')), dir = resolve(parent,'native-only');
  try {
    createGame(dir);
    const path = resolve(dir,'pins.json');
    const pins = {hosts:['linux'],modes:['0','1','fresh-game'],profiles:['gpu-dev','release'],ticks:{0:'0x'+'1'.repeat(16)},saves:{a:'a'.repeat(64)}};
    writeFileSync(path,JSON.stringify(pins));
    const result = spawnSync(process.execPath,[resolve(import.meta.dir,'../scripts/deploy.mjs'),'native-only'],{
      env:{...process.env,EXACT_APP_DIR:dir},encoding:'utf8'});
    expect(result.status).toBe(1);
    expect(result.stdout+result.stderr).toContain('game delivery refused: web not pinned');
    expect(JSON.parse(readFileSync(path))).toEqual(pins);
  } finally { rmSync(parent,{recursive:true,force:true}); }
}, 30000);

test('executed modes reject missing, wrong or vacuous reconstruction evidence for every world', async () => {
  const {executedModes} = await import('./proof.mjs');
  const state = (mode, tick = 0, checked = 0, canvas = 1, owed = false) => ({world:[{canvas,tick,hash:'0x1',restarted:0,paranoid:{mode,checked,skipped:0,owed}}]});
  for (const mode of ['0','1','fresh-game']) {
    const evidence = executedModes(mode);
    expect(() => evidence.assert()).toThrow('no worlds observed');
    expect(() => evidence.observe({world:[{canvas:1,tick:0,hash:'0x1'}]},1)).toThrow('executed mode missing');
    evidence.observe(state(mode),1);
    if (mode !== '0') {
      expect(() => evidence.observe(state('0'),2)).toThrow(`differs from requested ${mode}`);
      expect(() => evidence.observe(state(mode === '1' ? 'fresh-game' : '1'),2)).toThrow(`differs from requested ${mode}`);
      expect(() => evidence.assert()).toThrow('no completed reconstruction samples');
      evidence.observe(state(mode,2,1),1);
      evidence.observe(state(mode,0,0,2),1);
      evidence.observe(state(mode,2,0,2),1);
      expect(() => evidence.assert()).toThrow('advanced world has no reconstruction samples');
      evidence.observe(state(mode,2,1,2,true),1);
      expect(() => evidence.assert()).toThrow('sample still owed');
      evidence.observe(state(mode,2,1,2),1);
    }
    expect(() => evidence.assert()).not.toThrow();
  }
});
