import {expect,test} from 'bun:test';
import {readFileSync} from 'node:fs';
import {pick as online} from '../online/projection.ts';
import {pick as device} from './projection.ts';
test('checked projections emit only the Contract keys and preserve nullable values',()=>{
  for(const pick of [online,device]) {
    const note=pick<{id:string;parent:string|null}>();
    const row={id:'n',parent:null,extra:'server-only'};
    expect(note(row,'id','parent')).toEqual({id:'n',parent:null});
    expect(row.extra).toBe('server-only');
  }
  expect(readFileSync(new URL('../online/projection.ts',import.meta.url),'utf8')).toBe(readFileSync(new URL('./projection.ts',import.meta.url),'utf8'));
});
