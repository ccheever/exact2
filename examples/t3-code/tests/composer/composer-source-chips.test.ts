import { describe, expect, test } from 'bun:test';
import { cancelFreshCitation, chipDecorations, composerChips, composerChipsInBlocks, composerTrigger, insertCitation, moveFreshCitation, pastedPathQueryLength, reconcileComposerChips } from '../../src/composer/composer-source-chips';
import { applyComposerEdit } from '../../src/composer/composer-markdown-edit';
import { composerMarkdown } from '../../src/composer/composer-markdown';

const citation = '[Assistant quote](t3-citation://v1/e/t/m?text=the+quote&start=0&end=9&prefix=&suffix=)';
describe('composer source chips and active triggers', () => {
  test('UTF-16 source ranges retain file, skill, context, image and citation records in source order', () => {
    const source = `😀 [x.ts](src/x.ts) $deploy [Thread](t3-context://v1/thread/thread_one) ![image](t3-context://v1/image/image_one) ${citation} `;
    const chips = composerChips(source);
    expect(chips.map(chip => chip.kind)).toEqual(['mention', 'skill', 'context', 'context', 'citation']);
    expect(chips.map(chip => source.slice(chip.start, chip.end))).toEqual(chips.map(chip => chip.source));
    expect(chips[0]?.start).toBe(3);
    expect(chips.at(-1)?.label).toBe('the quote');
    expect(chipDecorations(chips, { deploy: 'Deploy app' })[1]?.label).toBe('Deploy app');
  });
  test('malformed or incomplete citation metadata stays editable and valid percent encoding is preserved', () => {
    for (const href of [
      't3-citation://v1/env/thread/message?text=missing+metadata',
      't3-citation://v1/env/thread/message?text=quote&start=0&end=5&prefix=',
      't3-citation://v1/env/thread/message?text=quote&start=5&end=0&prefix=&suffix=',
      't3-citation://v1/env/thread/message?text=quote&start=0&end=5&prefix=&suffix=&extra=x',
    ]) expect(composerChips(`[Assistant quote](${href})`)).toEqual([]);
    const source = citation.replaceAll('+', '%20');
    const chips = composerChips(`前(${source}),${source}後`);
    expect(chips).toHaveLength(2);
    expect(chips.map(chip => chip.source)).toEqual([source, source]);
    expect(chips[0]?.label).toBe('the quote');
    expect(chips[0]?.kind === 'citation' ? chips[0].href : '').toContain('text=the%20quote');
  });
  test('links win over their embedded tokens and scoped npm packages stay literal', () => {
    const source = '[see $tool and @file ](t3-context://v1/thread/thread_one) @scope/package $100k @"a b.ts" ';
    expect(composerChips(source).map(chip => [chip.kind, chip.label])).toEqual([['context', 'see $tool and @file '], ['mention', 'a b.ts']]);
    expect(composerChips('[web](https://example.com) [a.ts](other.ts) ')).toEqual([]);
  });
  test('completed atoms survive adjacent edits and delimiter deletion while touched source is reparsed', () => {
    const source = '😀 $deploy $deploy ', original = composerChips(source), withoutSpace = source.slice(0, -1);
    const carried = reconcileComposerChips(source, withoutSpace, original);
    expect(carried).toEqual(original);
    const shifted = reconcileComposerChips(withoutSpace, 'x' + withoutSpace, carried);
    expect(shifted.map(chip => chip.start)).toEqual(original.map(chip => chip.start + 1));
    expect(shifted.map(chip => ('x' + withoutSpace).slice(chip.start, chip.end))).toEqual(['$deploy', '$deploy']);
    const last = shifted.at(-1);
    if (!last) throw new Error('second occurrence must retain its own source range');
    const changed = ('x' + withoutSpace).slice(0, last.start + 3) + '☃' + ('x' + withoutSpace).slice(last.start + 4);
    expect(reconcileComposerChips('x' + withoutSpace, changed, shifted).map(chip => chip.start)).toEqual([shifted[0]?.start]);
    expect(reconcileComposerChips(source, '$new ', original).map(chip => chip.label)).toEqual(['new']);
  });
  test('source content admits references without an atom table and retains repeated payloads', () => {
    const context = '[Thread](t3-context://v1/thread/thread_one)', image = '![image](t3-context://v1/image/image_one)';
    const value = `😀 > ${context}\n- [ ] ${context}\n${citation} ${citation}\n${image} [x.ts](src/x.ts) `;
    const parsed = composerMarkdown(value), candidates = composerChips(value);
    expect(parsed.decorations.atoms).toEqual([]);
    const accepted = composerChipsInBlocks(value, parsed.blocks, candidates);
    expect(accepted).toEqual(candidates);
    expect(accepted.map(chip => chip.source)).toEqual([context, context, citation, citation, image, '[x.ts](src/x.ts)']);
    expect(accepted[0]?.start).toBe(5);
    expect(accepted.filter(chip => chip.kind === 'citation').map(chip => chip.href)).toEqual([citation.slice('[Assistant quote]('.length, -1), citation.slice('[Assistant quote]('.length, -1)]);
    expect(accepted.find(chip => chip.kind === 'context' && chip.image)).toMatchObject({ contextId: 'image_one', image: true });
    expect(accepted.find(chip => chip.kind === 'mention')).toMatchObject({ path: 'src/x.ts' });
  });
  test('source admission excludes code, rules, hidden prefixes, stale slices and invalid ranges', () => {
    const value = '# $heading \n---\n```ts $info\n$body \n```\n$tail ';
    const parsed = composerMarkdown(value), candidates = composerChips(value), tail = candidates.at(-1);
    if (!tail) throw new Error('tail reference must exist');
    const invalid = [
      { ...tail, source: '$old' }, { ...tail, start: -1 }, { ...tail, end: value.length + 1 },
      { ...tail, start: tail.start + 0.5 }, { ...tail, end: tail.start },
      { ...tail, start: 0, end: 2, source: '# ' },
      { ...tail, start: value.indexOf('---'), end: value.indexOf('---') + 3, source: '---' },
      { ...tail, start: tail.start - 1, source: '\n' + tail.source },
    ];
    expect(composerChipsInBlocks(value, parsed.blocks, [...candidates, ...invalid]).map(chip => chip.source)).toEqual(['$heading', '$tail']);
    const completed = composerChips('$deploy '), carried = reconcileComposerChips('$deploy ', '$deploy', completed);
    expect(composerChips('$deploy')).toEqual([]);
    expect(composerChipsInBlocks('$deploy', composerMarkdown('$deploy').blocks, carried)).toEqual(completed);
  });
  test('commands open only at the line start; code and chip interiors suppress suggestions', () => {
    expect(composerTrigger('line\n/plan', 10)).toMatchObject({ kind: 'slash-command', query: 'plan', start: 5 });
    expect(composerTrigger('word /plan', 10)).toBeNull();
    expect(composerTrigger('see #1234', 9)).toMatchObject({ kind: 'pull-request', query: '1234' });
    expect(composerTrigger('$de', 3)).toMatchObject({ kind: 'skill', query: 'de' });
    const code = '```\n@src/x\n```';
    expect(composerTrigger(code, 10)).toBeNull();
    expect(composerTrigger('[x.ts](src/x.ts) ', 8)).toBeNull();
  });
  test('path paste protects only the leading literal prefix of an actual active typed query', () => {
    expect(pastedPathQueryLength('@src/', '__init__.py next **bold**', true)).toBe(11);
    expect(pastedPathQueryLength('@src/', '__init__.py', false)).toBe(0);
    expect(pastedPathQueryLength('[x.ts](src/x.ts) ', '__init__.py', true)).toBe(0);
    expect(pastedPathQueryLength('@src/', ' __init__.py', true)).toBe(0);
    const pasted = '__init__.py **bold**', length = pastedPathQueryLength('@src/', pasted, true);
    const projected = composerMarkdown(pasted, { literal: [{ start: 0, end: length }] });
    expect(projected.decorations.spans.map(span => pasted.slice(span.start, span.end))).toEqual(['bold']);
  });
});

describe('fresh citation cancellation space ownership', () => {
  test('removes exactly the spaces inserted with a fresh citation', () => {
    for (const [value, cursor] of [['word', 4], ['word ', 5], ['word tail', 4], ['', 0], [' ', 0]] satisfies [string, number][]) {
      const inserted = insertCitation(value, cursor, citation), next = applyComposerEdit(value, inserted.edit);
      expect(next).toBe(inserted.citation.value);
      const removal = cancelFreshCitation(inserted.citation, next);
      if (!removal) throw new Error('fresh citation must retain cancellation ownership');
      expect(applyComposerEdit(next, removal)).toBe(value);
    }
  });
  test('existing trailing space is re-emitted without becoming owned', () => {
    const inserted = insertCitation('left right', 4, citation);
    expect(inserted.citation.spaces).toEqual({ before: true, after: false });
    expect(inserted.edit.end).toBe(5);
    const removal = cancelFreshCitation(inserted.citation, inserted.citation.value);
    if (!removal) throw new Error('missing cancellation');
    expect(applyComposerEdit(inserted.citation.value, removal)).toBe('left right');
  });
  test('surrounding edits move the right occurrence; touching source relinquishes cancellation', () => {
    const inserted = insertCitation('x', 1, citation), current = 'prefix ' + inserted.citation.value + 'tail';
    // Inputs arrive separately, so each change rebases against the preceding source.
    const moved = moveFreshCitation(inserted.citation, 'prefix ' + inserted.citation.value);
    if (!moved) throw new Error('surrounding edit must preserve citation');
    const removal = cancelFreshCitation(moved, current);
    if (!removal) throw new Error('missing rebased citation');
    expect(applyComposerEdit(current, removal)).toBe('prefix xtail');
    expect(cancelFreshCitation(inserted.citation, inserted.citation.value.replace('the+quote', 'other'))).toBeNull();
  });
});
