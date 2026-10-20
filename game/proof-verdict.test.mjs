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
    writeFileSync(dir+'/pins.json',JSON.stringify(pins));
    expect(() => requireReleasePins(app)).not.toThrow();
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
