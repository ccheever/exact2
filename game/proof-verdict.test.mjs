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
