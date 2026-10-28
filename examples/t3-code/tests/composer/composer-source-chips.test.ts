import { describe, expect, test } from 'bun:test';
import { cancelFreshCitation, composerCharacterDeletion, composerChips, composerChipsInBlocks, composerTrigger, insertCitation, moveFreshCitation, outsideComposerChips } from '../../src/composer/composer-source-chips';
import { ComposerSourceSession } from '../../src/composer/composer-source-session';
import { citationsToPlainText, expandCitations } from '../../src/composer/composer-editor-menu';
import { citationOfLink } from '../../src/timeline/citation-comment';
import { stashSnippet } from '../../src/composer/composer-editor-stash';
import { messageChips, shouldCollapseUserMessage } from '../../src/timeline/message-chips';
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
  test('a reference is its source: an edit that takes its delimiter or lengthens it leaves text, as the editor\'s atom does', () => {
    const session = new ComposerSourceSession(), value = 'Look at @README.md later';
    session.observe('o', value);
    expect(session.chips.map(chip => [chip.kind, chip.start, chip.end])).toEqual([['mention', 8, 18]]);
    // The space before `later` deleted: the editor shows `@README.mdlater` as text, so no chip holds the caret or a paste.
    const joined = 'Look at @README.mdlater';
    session.observe('o', joined, 'deleteContentBackward');
    expect(session.chips).toEqual([]);
    const paste = session.input('o', joined, { start: 12, end: 12 }, 'XYZ', true);
    expect(paste.kind === 'edit' && paste.edit).toMatchObject({ start: 12, end: 12, text: 'XYZ' });
    // A character typed right after the mention makes a new source, text until something names it.
    session.observe('o', 'Look at @README.mdx later', 'insertText');
    expect(session.chips.map(chip => chip.source)).toEqual(['@README.mdx']);
    session.observe('o', '😀 $deploy $deploy', 'deleteContentBackward');
    expect(session.chips.map(chip => chip.source)).toEqual(['$deploy']);
  });
  test('source content admits references and retains repeated payloads', () => {
    const context = '[Thread](t3-context://v1/thread/thread_one)', image = '![image](t3-context://v1/image/image_one)';
    const value = `😀 > ${context}\n- [ ] ${context}\n${citation} ${citation}\n${image} [x.ts](src/x.ts) `;
    const parsed = composerMarkdown(value), candidates = composerChips(value);
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
    expect(composerChips('$deploy')).toEqual([]);
  });
  test('commands open only at the line start; code and chip interiors suppress suggestions', () => {
    expect(composerTrigger('line\n/plan', 10)).toMatchObject({ kind: 'slash-command', query: 'plan', start: 5 });
    expect(composerTrigger('word /plan', 10)).toBeNull();
    expect(composerTrigger('see #1234', 9)).toMatchObject({ kind: 'pull-request', query: '1234' });
    expect(composerTrigger('$de', 3)).toMatchObject({ kind: 'skill', query: 'de' });
    const code = '```\n@src/x\n```';
    // isCurrencyAmountQuery (composerTrigger.test.ts): an amount is money, not a skill.
    for (const text of ['It costs $5', 'budget $9.99', 'only $.99', 'about €20', 'raise $10k', '$1,000', '£2e6', 'raise $10k.', 'is it $5?', '(about $9.99)'])
      expect(composerTrigger(text, text.length)).toBeNull();
    for (const text of ['$', '$1password', '$2fa']) expect(composerTrigger(text, text.length)).toMatchObject({ kind: 'skill', query: text.slice(1) });
    expect(composerTrigger(code, 10)).toBeNull();
    expect(composerTrigger('[x.ts](src/x.ts) ', 8)).toBeNull();
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

describe('a composer reference is one unit (the reference\'s atom chips)', () => {
  const href = citation.slice('[Assistant quote]('.length, -1);
  test('a citation is its t3-citation destination, whatever its label reads', () => {
    for (const label of ['Assistant quo', '', 'x']) {
      const source = `[${label}](${href})`;
      expect(composerChips(`a ${source} b`)).toMatchObject([{ kind: 'citation', start: 2, end: 2 + source.length, href, label: 'the quote' }]);
      expect(citationOfLink(source)?.text).toBe('the quote');
      expect(citationsToPlainText(`a ${source}`)).toBe('a the quote');
      expect(expandCitations(`a ${source}`)).toStartWith('a [assistant-quote-1]');
    }
    expect(composerChips(`[Assistant quote](https://example.com) `)).toEqual([]);
    expect(messageChips({ text: `a [x](${href})` }, '', []).map(chip => [chip.kind, chip.label])).toEqual([['citation', 'the quote']]);
    expect(shouldCollapseUserMessage(`[x](${href})`)).toBe(false);
  });
  test('an unclosed [ before a citation stays text everywhere a citation is read', () => {
    const prompt = `use arr[i for ${citation} please`, at = prompt.indexOf('[Assistant');
    expect(composerChips(prompt)).toMatchObject([{ kind: 'citation', start: at, end: at + citation.length }]);
    expect(expandCitations(prompt)).toStartWith('use arr[i for [assistant-quote-1] please');
    expect(citationsToPlainText(prompt)).toBe('use arr[i for the quote please');
    expect(stashSnippet(prompt)).toBe('use arr[i for the quote please');
    expect(citationOfLink(`[i ${citation}`)).toBeNull();
    expect(messageChips({ text: prompt }, '', []).map(chip => chip.kind)).toEqual(['citation']);
  });
  test('a deletion the grammar leaves takes the selection or one grapheme beside the caret', () => {
    expect(composerCharacterDeletion('a👍🏽', { start: 5, end: 5 }, true)).toMatchObject({ start: 1, end: 5 });
    expect(composerCharacterDeletion('ab', { start: 0, end: 0 }, true)).toBeNull();
    expect(composerCharacterDeletion('abc', { start: 1, end: 2 }, false)).toMatchObject({ start: 1, end: 2 });
  });
  test('an app edit inside a reference goes after it; a range is widened over the ones it cuts', () => {
    const value = `a ${citation} [x.ts](src/x.ts) b`, chips = composerChips(value);
    const [cite, file] = chips as [typeof chips[0], typeof chips[0]];
    expect(outsideComposerChips({ start: cite.start + 4, end: cite.start + 4 }, chips)).toEqual({ start: cite.end, end: cite.end });
    expect(outsideComposerChips({ start: cite.start + 4, end: file.start + 2 }, chips)).toEqual({ start: cite.start, end: file.end });
  });
  test('the session forgets a deleted citation and Undo brings it back as a citation', () => {
    const session = new ComposerSourceSession(), value = `see ${citation} `;
    session.observe('one', value);
    session.observe('one', 'see  ', 'deleteContentBackward');
    expect(session.chips).toEqual([]);
    session.observe('one', value, 'historyUndo');
    expect(session.chips).toMatchObject([{ kind: 'citation', start: 4, end: 4 + citation.length }]);
    const typed = session.input('one', value, { start: 9, end: 9 }, 'x');
    expect(typed.kind === 'edit' && typed.edit).toMatchObject({ start: 4 + citation.length, end: 4 + citation.length, text: 'x' });
  });
});
