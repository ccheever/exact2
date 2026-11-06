import { describe, expect, test } from 'bun:test';
import { composerBlockInputRule } from '../../src/composer/composer-input-rules';
const paragraph = { kind: 'paragraph' } satisfies Parameters<typeof composerBlockInputRule>[1];
describe('live composer block gestures', () => {
  test('dash waits for content so a task prefix can arrive whole', () => {
    for (const line of ['-', '- ', '-  ', '- [', '- [ ', '- [ ]', '- [x]', '- [a', '- [a text']) expect(composerBlockInputRule(line, paragraph)).toBeNull();
    expect(composerBlockInputRule('- a', paragraph)).toEqual({ kind: 'list', list: 'bullet', marker: '-', space: ' ', carried: 'a' });
    expect(composerBlockInputRule('-\t\tx', paragraph)).toEqual({ kind: 'list', list: 'bullet', marker: '-', space: '\t\t', carried: 'x' });
    expect(composerBlockInputRule('- [ ] ', paragraph)).toEqual({ kind: 'task', checked: false });
    expect(composerBlockInputRule('- [X] ', paragraph)).toEqual({ kind: 'task', checked: true });
  });
  test('star and plus open immediately; ordered source markers and spacing are retained', () => {
    expect(composerBlockInputRule('* ', paragraph)).toEqual({ kind: 'list', list: 'bullet', marker: '*', space: ' ', carried: '' });
    expect(composerBlockInputRule('+\t', paragraph)).toEqual({ kind: 'list', list: 'bullet', marker: '+', space: '\t', carried: '' });
    expect(composerBlockInputRule('003) ', paragraph)).toEqual({ kind: 'list', list: 'ordered', marker: '003)', space: ' ', carried: '' });
    expect(composerBlockInputRule('0. ', paragraph)?.kind).toBe('list');
  });
  test('third dash creates a rule while stars and underscores require their following space', () => {
    expect(composerBlockInputRule('---', paragraph)).toEqual({ kind: 'rule', source: '---' });
    for (const line of ['***', '___', '----', '* * *']) expect(composerBlockInputRule(line, paragraph)).toBeNull();
    for (const line of ['*** ', '___ ']) expect(composerBlockInputRule(line, paragraph)).toEqual({ kind: 'rule', source: line });
  });
  test('headings and quotes require the trigger whitespace and nested block gestures stay literal', () => {
    expect(composerBlockInputRule('## ', paragraph)).toEqual({ kind: 'heading', level: 2, space: ' ' });
    expect(composerBlockInputRule('>\t', paragraph)).toEqual({ kind: 'quote', prefix: '>\t' });
    for (const line of ['#1234', '####### ', '>', '## body']) expect(composerBlockInputRule(line, paragraph)).toBeNull();
    for (const line of ['# ', '> ', '---', '* ', '1. ']) {
      expect(composerBlockInputRule(line, { kind: 'list', marker: '-' })).toBeNull();
      expect(composerBlockInputRule(line, { kind: 'other' })).toBeNull();
    }
    expect(composerBlockInputRule('[x] ', { kind: 'list', marker: '*' })).toEqual({ kind: 'task', checked: true });
    expect(composerBlockInputRule('[x] ', { kind: 'list', marker: '1.' })).toBeNull();
  });
});
