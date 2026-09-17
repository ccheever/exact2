import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
const defer = source.match(/function deferFulfill\(\.\.\.args\) \{[\s\S]*?\n\}/)[0];
const branch = source.match(/case "refuse": \{[^\n]*\}/)[0];
test('admission refusal delivers Refused only after the enclosing batch, with its incarnation', async () => {
  const delivered = [];
  const apply = Function('safelyFulfill', 'enc', `${defer}; return (op,incarnation)=>{switch(op.op){${branch}}};`)(
    (...args) => delivered.push(args), new TextEncoder());
  apply({ op: 'refuse', ticket: 17, message: 'only HTTP may opt into independent transport' }, 4);
  expect(delivered.length).toBe(0);
  await Promise.resolve();
  expect(delivered.length).toBe(1);
  expect(delivered[0].slice(0, 5)).toEqual([4, 17, 2, 0, '']);
  expect(new TextDecoder().decode(delivered[0][5])).toBe('only HTTP may opt into independent transport');
});
