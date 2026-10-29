import { expect, test } from 'bun:test';
import { highlight } from './highlight';

test('a known language is coloured by token, adjacent tokens of a kind merged', () => {
  expect(highlight('const x = f("a", 42); // done', 'ts')).toEqual([
    { kind: 'keyword', text: 'const' }, { kind: 'plain', text: ' x = ' }, { kind: 'call', text: 'f' },
    { kind: 'plain', text: '(' }, { kind: 'string', text: '"a"' }, { kind: 'plain', text: ', ' },
    { kind: 'number', text: '42' }, { kind: 'plain', text: '); ' }, { kind: 'comment', text: '// done' },
  ]);
  expect(highlight('def go(): # hi\n  return None', 'python')!.filter(t => t.kind !== 'plain').map(t => [t.kind, t.text])).toEqual([
    ['keyword', 'def'], ['call', 'go'], ['comment', '# hi'], ['keyword', 'return'], ['keyword', 'None'],
  ]);
  expect(highlight('let v: Vec<u8> = x2;', 'rust')!.map(t => t.kind)).toEqual(['keyword', 'plain', 'type', 'plain']);
});

test('an unknown language, or none, is left plain', () => {
  expect(highlight('x = 1', '')).toBeNull();
  expect(highlight('x = 1', 'brainfuck')).toBeNull();
  expect(highlight('ab'.repeat(10001), 'js')).toBeNull();
});

test('an unclosed comment or string runs to the end of its text or line, in linear time', () => {
  expect(highlight('a /* b', 'js')).toEqual([{ kind: 'plain', text: 'a ' }, { kind: 'comment', text: '/* b' }]);
  expect(highlight('"a\nb', 'js')!.map(t => t.kind)).toEqual(['string', 'plain']);
  const nasty = '/* '.repeat(1300) + '" '.repeat(1300);
  const started = performance.now();
  for (let i = 0; i < 200; i++) highlight(nasty, 'js');
  expect(performance.now() - started).toBeLessThan(200);
});
