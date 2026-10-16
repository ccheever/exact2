import {expect,test} from 'bun:test';
import {readFileSync} from 'node:fs';
import {checkReadmeVersion} from './check-snapback-readme.mjs';
test('qualification refuses every stale or missing README test claim and a different wave pin',()=>{
  expect(checkReadmeVersion('tested as written against 0.4.29\ntested as written against\n0.4.29','0.4.29')).toEqual({version:'0.4.29',claims:2});
  expect(()=>checkReadmeVersion('tested as written against 0.4.28','0.4.29')).toThrow(/Re-run both/);
  expect(()=>checkReadmeVersion('tested as written against 0.4.29\ntested as written against 0.4.28','0.4.29')).toThrow(/0.4.28/);
  expect(()=>checkReadmeVersion('no claim','0.4.29')).toThrow(/no tested-against/);
  expect(()=>checkReadmeVersion('tested as written against 0.4.29','0.4.29','0.4.30')).toThrow(/differs/);
});
test('the committed README claims match the pinned Snapback package',()=>{
  const pin=JSON.parse(readFileSync(new URL('../package.json',import.meta.url),'utf8')).devDependencies.snapback4;
  expect(checkReadmeVersion(readFileSync(new URL('../snapback4/README.md',import.meta.url),'utf8'),pin).version).toBe(pin);
});
