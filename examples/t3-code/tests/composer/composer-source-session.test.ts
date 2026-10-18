import { describe, expect, test } from 'bun:test';
import { ComposerSourceSession, type ComposerSessionResult } from '../../src/composer/composer-source-session';
import { applyComposerEdit } from '../../src/composer/composer-markdown-edit';

function apply(session: ComposerSourceSession, owner: string, source: string, result: ComposerSessionResult): { source: string; start: number; end: number } {
  if (result.kind !== 'edit') throw new Error(`expected edit, got ${result.kind}`);
  const value = applyComposerEdit(source, result.edit);
  session.observe(owner, value);
  return { source: value, ...result.edit.selection };
}
const enter = { name: 'Enter', shift: false, command: false };
describe('composer live source identity', () => {
  test('lengthened serialization retains the original closing gesture for that occurrence', () => {
    const session = new ComposerSourceSession(), source = '```ts\nx\n```';
    session.observe('one', source);
    const protectedSource = apply(session, 'one', source, session.input('one', source, { start: 7, end: 7 }, '\n```'));
    expect(protectedSource.source).toBe('````ts\nx\n```\n````');
    const exited = apply(session, 'one', protectedSource.source, session.key('one', protectedSource.source, protectedSource, enter));
    expect(exited).toEqual({ source: '```ts\nx\n```\n', start: 12, end: 12 });
  });
  test('closing a middle body line retains following source as paragraphs', () => {
    const session = new ComposerSourceSession(), source = '```\nx\n```\nafter';
    session.observe('one', source);
    const inserted = apply(session, 'one', source, session.input('one', source, { start: 5, end: 5 }, '\n```\ny'));
    const closingCaret = inserted.source.indexOf('\n```\n') + 4;
    const result = apply(session, 'one', inserted.source, session.key('one', inserted.source, { start: closingCaret, end: closingCaret }, enter));
    expect(result.source).toBe('```\nx\n```\n\ny\nafter');
    expect(result.start).toBe(10);
  });
  test('native undo/redo keeps grammar metadata and stale owners or source cannot edit another draft', () => {
    const session = new ComposerSourceSession(), source = '```\nx\n```';
    session.observe('one', source);
    const protectedSource = apply(session, 'one', source, session.input('one', source, { start: 5, end: 5 }, '\n```'));
    session.observe('one', source, 'historyUndo');
    session.observe('one', protectedSource.source, 'historyRedo');
    expect(apply(session, 'one', protectedSource.source, session.key('one', protectedSource.source, protectedSource, enter)).source).toBe('```\nx\n```\n');
    session.observe('two', protectedSource.source);
    expect(session.key('one', protectedSource.source, protectedSource, enter)).toEqual({ kind: 'stale' });
    expect(session.input('two', source, { start: 5, end: 5 }, 'x')).toEqual({ kind: 'stale' });
    // A restored source has its actual longer fence, without the previous owner's editing metadata.
    const ordinary = session.key('two', protectedSource.source, protectedSource, enter);
    expect(ordinary.kind).toBe('edit');
    if (ordinary.kind === 'edit') expect(applyComposerEdit(protectedSource.source, ordinary.edit)).toBe('````\nx\n```\n\n````');
  });
});
