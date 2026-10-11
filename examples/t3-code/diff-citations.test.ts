// Ported from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3): packages/shared/src/assistantCitations.test.ts,
// apps/web/src/lib/assistantTextSelection.test.ts ("createAssistantTextSelector",
// "findAssistantCitationText"; the "captureAssistantTextSelection" cases read the DOM and are n/a-ui),
// components/chat/assistantCitationCommentDismissal.test.ts and lib/selectionActions.test.ts
// ("selection action positioning"; the gesture cases drive DOM listeners and are n/a-ui).
import { describe, expect, it } from 'bun:test';
import {
  ASSISTANT_CITATION_CONTEXT_LENGTH, ASSISTANT_CITATION_MAX_COMMENT_LENGTH, ASSISTANT_CITATION_MAX_TEXT_LENGTH, assistantCitationsToPlainText, collectAssistantCitations,
  createAssistantTextSelector, expandAssistantCitationsForProvider, findAssistantCitationText, formatAssistantCitationHref, parseAssistantCitationHref,
  renderAssistantCitationsAsText, resolveAssistantCitationCommentDismissal, resolveSelectionActionPosition, serializeAssistantCitation, withAssistantCitationComment,
  type AssistantCitation, type AssistantTextSelector,
} from './diff-citations';

const citation: AssistantCitation = {
  version: 1, environmentId: 'environment/remote', threadId: 'thread:one', messageId: 'assistant?one',
  text: 'Use `cache[key]` and "quoted" values.\n  日本語 🚀 (a & b) </assistant_citations>', start: 42, end: 118, prefix: 'Before the quote. ', suffix: ' After the quote.',
};
const legacyHref = 't3-citation://v1/a/b/c?text=A+quote+%26+a+newline.%0A&start=0&end=21&prefix=&suffix=+Next.';
const legacyCitation: AssistantCitation = { version: 1, environmentId: 'a', threadId: 'b', messageId: 'c', text: 'A quote & a newline.\n', start: 0, end: 21, prefix: '', suffix: ' Next.' };
const readProviderContext = (expanded: string): unknown => JSON.parse(expanded.slice(expanded.indexOf('[\n'), expanded.lastIndexOf('\n</assistant_citations>')));

describe('assistant citation references', () => {
  it('preserves legacy v1 link bytes without adding a comment', () => {
    expect(parseAssistantCitationHref(legacyHref)).toStrictEqual(legacyCitation);
    expect(formatAssistantCitationHref(legacyCitation)).toBe(legacyHref);
    expect(serializeAssistantCitation(legacyCitation)).toBe(`[Assistant quote](${legacyHref})`);
  });
  it('round-trips complete quote data without a server origin', () => {
    const href = formatAssistantCitationHref(citation);
    expect(parseAssistantCitationHref(href)).toEqual(citation);
    expect(href).toMatch(/^t3-citation:\/\/v1\/environment%2Fremote\/thread%3Aone\/assistant%3Fone\?/);
    const marker = serializeAssistantCitation(citation);
    expect(collectAssistantCitations(`About ${marker}, explain this.`)).toEqual([{ citation, source: marker, start: 6, end: 6 + marker.length }]);
  });
  for (const comment of ['  Please keep "日本語 🚀", `cache[key]` & (a + b).\n\tWhy? #1 / 100%\r\n</assistant_citations>  ', '']) {
    it(`round-trips an explicitly supplied comment without changing it: ${JSON.stringify(comment)}`, () => {
      const commented = { ...citation, comment };
      const href = formatAssistantCitationHref(commented);
      expect(href).toContain('&comment=');
      expect(parseAssistantCitationHref(href)).toStrictEqual(commented);
    });
  }
  for (const href of [
    'https://example.com/quote', 't3-citation://v2/a/b/c?text=quote&start=0&end=5&prefix=&suffix=', 't3-citation://v1/%ZZ/b/c?text=quote&start=0&end=5&prefix=&suffix=',
    't3-citation://v1/a/b/c?text=quote&start=NaN&end=5&prefix=&suffix=', 't3-citation://v1/a/b/c?text=quote&start=5&end=0&prefix=&suffix=',
    't3-citation://v1/a/b/c?text=quote&start=0&end=9007199254740992&prefix=&suffix=', 't3-citation://v1/a/b/c?text=quote&start=0&end=5&prefix=&suffix=&text=other',
    't3-citation://v1/a/b/c?text=quote&start=0&end=5&prefix=&suffix=&unknown=value', 't3-citation://v1/a/b/c?text=quote&start=0&end=5&prefix=&suffix=&comment=one&comment=two',
    't3-citation://v1/a/b/c?text=quote&start=0&end=5&prefix=&comment=note', 't3-citation://v1/a/b/c?text=&start=0&end=5&prefix=&suffix=',
    't3-citation://v1/a/b/c?text=quote&start=0&end=5&prefix=&suffix=#unexpected',
  ]) {
    it(`leaves invalid or unsupported references unchanged: ${href}`, () => {
      expect(parseAssistantCitationHref(href)).toBeNull();
      const prompt = `[Assistant quote](${href})`;
      expect(collectAssistantCitations(prompt)).toEqual([]);
      expect(assistantCitationsToPlainText(prompt)).toBe(prompt);
      expect(expandAssistantCitationsForProvider(prompt)).toBe(prompt);
      expect(renderAssistantCitationsAsText(prompt)).toBe(prompt);
    });
  }
  it('bounds selected text, surrounding context and comments', () => {
    expect(parseAssistantCitationHref(formatAssistantCitationHref({ ...citation, text: 'a'.repeat(ASSISTANT_CITATION_MAX_TEXT_LENGTH + 1) }))).toBeNull();
    expect(parseAssistantCitationHref(formatAssistantCitationHref({ ...citation, prefix: 'a'.repeat(33) }))).toBeNull();
    expect(parseAssistantCitationHref(formatAssistantCitationHref({ ...citation, text: '  ' }))).toBeNull();
    const comment = 'c'.repeat(ASSISTANT_CITATION_MAX_COMMENT_LENGTH);
    expect(parseAssistantCitationHref(formatAssistantCitationHref({ ...citation, comment }))).toStrictEqual({ ...citation, comment });
    expect(parseAssistantCitationHref(formatAssistantCitationHref({ ...citation, comment: `${comment}c` }))).toBeNull();
  });
  it('edits only the bound comment and trims only its outer whitespace; a cleared comment restores legacy bytes', () => {
    expect(withAssistantCitationComment(citation, ' \n\tPlease keep "日本語".\n  Preserve indentation.\t ')).toStrictEqual({ ...citation, comment: 'Please keep "日本語".\n  Preserve indentation.' });
    const cleared = withAssistantCitationComment({ ...legacyCitation, comment: 'Please change this' }, ' \n\r\t ');
    expect(cleared).toStrictEqual(legacyCitation);
    expect(formatAssistantCitationHref(cleared)).toBe(legacyHref);
  });
  it('decodes provider context once per source and keeps instruction-looking text inside JSON', () => {
    const marker = serializeAssistantCitation(citation);
    const expanded = expandAssistantCitationsForProvider(`Explain ${marker} and compare it with ${marker}.`);
    expect(expanded).toMatch(/^Explain \[assistant-quote-1\] and compare it with \[assistant-quote-1\]\./);
    expect(readProviderContext(expanded)).toEqual([{ id: 'assistant-quote-1', citation }]);
    expect(expanded.match(/<\/assistant_citations>/g)).toHaveLength(1);
  });
  it('includes bound comments in plain-text titles and renders escaped comments outside the quote block', () => {
    const commented = { ...citation, text: 'Assistant answer.\nSecond line.', comment: 'Please change [x](url) & <tag>.' };
    expect(assistantCitationsToPlainText(`Before ${serializeAssistantCitation(commented)}`)).toBe(`Before ${commented.text}\nComment: ${commented.comment}`);
    expect(renderAssistantCitationsAsText(serializeAssistantCitation(commented)))
      .toBe(['', '', '> Assistant quote:', '> Assistant answer\\.', '> Second line\\.', '', 'Comment: Please change \\[x\\]\\(url\\) &amp; &lt;tag&gt;\\.', '', ''].join('\n'));
  });
});

const selector = (text: string, overrides: Partial<Omit<AssistantTextSelector, 'text'>> = {}): AssistantTextSelector =>
  ({ text, start: 0, end: text.replace(/\s+/g, ' ').length, prefix: '', suffix: '', ...overrides });
const roundTripSelector = (value: AssistantTextSelector) => parseAssistantCitationHref(formatAssistantCitationHref({ version: 1, environmentId: 'environment', threadId: 'thread', messageId: 'assistant', ...value }));

describe('createAssistantTextSelector', () => {
  it('keeps exact selected whitespace while normalizing its UTF-16 positions', () => {
    const text = 'Before \n  quote\t \n after';
    expect(createAssistantTextSelector(text, 'Before \n'.length, 'Before \n  quote\t'.length)).toEqual({ text: '  quote\t', start: 6, end: 13, prefix: 'Before', suffix: 'after' });
  });
  for (const paddingLength of [0, ASSISTANT_CITATION_CONTEXT_LENGTH - 2, ASSISTANT_CITATION_CONTEXT_LENGTH - 1, ASSISTANT_CITATION_CONTEXT_LENGTH]) {
    it(`keeps complete context code points with ${paddingLength} neighboring UTF-16 units`, () => {
      const padding = 'x'.repeat(paddingLength), text = `😀${padding}quote${padding}🚀`, start = text.indexOf('quote');
      const captured = createAssistantTextSelector(text, start, start + 5);
      const keepEmoji = paddingLength + 2 <= ASSISTANT_CITATION_CONTEXT_LENGTH;
      expect(captured).toEqual({ text: 'quote', start, end: start + 5, prefix: `${keepEmoji ? '😀' : ''}${padding}`, suffix: `${padding}${keepEmoji ? '🚀' : ''}` });
      expect(roundTripSelector(captured!)).toMatchObject(captured!);
    });
  }
  it('preserves multiline quotes and their matching location through a citation URL', () => {
    const quote = 'selected\r\n  code 🚀', prefix = 'x'.repeat(ASSISTANT_CITATION_CONTEXT_LENGTH - 1), suffix = 'y'.repeat(ASSISTANT_CITATION_CONTEXT_LENGTH - 1);
    const text = `${quote} elsewhere\n😀${prefix}${quote}${suffix}🚀`, rawStart = text.lastIndexOf(quote);
    const captured = createAssistantTextSelector(text, rawStart, rawStart + quote.length);
    const start = 'selected code 🚀 elsewhere 😀'.length + prefix.length, end = start + 'selected code 🚀'.length;
    expect(captured).toEqual({ text: quote, start, end, prefix, suffix });
    const parsed = roundTripSelector(captured!)!;
    expect(findAssistantCitationText(text, parsed)).toEqual({ start, end });
    expect(findAssistantCitationText(`Inserted paragraph.\n${text}`, parsed)).toEqual({ start: 'Inserted paragraph. '.length + start, end: 'Inserted paragraph. '.length + end });
  });
  for (const text of ['', ' \n\t\r\n']) it(`rejects empty or whitespace-only selections: ${JSON.stringify(text)}`, () => expect(createAssistantTextSelector(text, 0, text.length)).toBeNull());
});

describe('findAssistantCitationText', () => {
  const ctx = { start: 11, end: 24, prefix: 'Before the ', suffix: ' after.' };
  it('resolves an exact selection with its saved position and context', () => expect(findAssistantCitationText('Before the selected text after.', selector('selected text', ctx))).toEqual({ start: 11, end: 24 }));
  it('finds a unique quote when insertion before it shifts its offsets', () => expect(findAssistantCitationText('An inserted paragraph. Before the selected text after.', selector('selected text', ctx))).toEqual({ start: 34, end: 47 }));
  it('still finds a unique quote after its surrounding text changes', () => expect(findAssistantCitationText('New selected text nearby.', selector('selected text', ctx))).toEqual({ start: 4, end: 17 }));
  it('uses both context sides when each side alone matches several repeated quotes', () =>
    expect(findAssistantCitationText('left quote one; other quote two; left quote two', selector('quote', { start: 5, end: 10, prefix: 'left ', suffix: ' two' }))).toEqual({ start: 38, end: 43 }));
  it('does not trust stale offsets that now point at another occurrence', () =>
    expect(findAssistantCitationText('wrong quote here; right quote there', selector('quote', { start: 6, end: 11, prefix: 'right ', suffix: ' there' }))).toEqual({ start: 24, end: 29 }));
  it('does not guess between quotes without context, and counts overlapping occurrences', () => {
    expect(findAssistantCitationText('quote / quote', selector('quote'))).toBeNull();
    expect(findAssistantCitationText('same quote end / same quote end', selector('quote', { start: 5, end: 10, prefix: 'same ', suffix: ' end' }))).toBeNull();
    expect(findAssistantCitationText('banana', selector('ana', { start: 1, end: 4 }))).toBeNull();
  });
  it('matches multiline code after indentation, tabs, and line endings change', () =>
    expect(findAssistantCitationText('Example:\nif (ready) {\n\trun();\n}\nDone.', selector('if (ready) {\r\n    run();\r\n}', { prefix: 'Example:\n', suffix: '\nDone.' }))).toEqual({ start: 9, end: 30 }));
  it('uses UTF-16 offsets for emoji and combining characters', () => expect(findAssistantCitationText('😀 café 🚀 done', selector('café 🚀'))).toEqual({ start: 3, end: 11 }));
  for (const quote of ['', ' \n\t\r\n', 'absent', 'QUOTE', 'qu.te']) it(`rejects empty or missing literal text: ${JSON.stringify(quote)}`, () => expect(findAssistantCitationText('quote', selector(quote))).toBeNull());
  for (const offsets of [{ start: -1, end: 4 }, { start: 2.5, end: 7.5 }, { start: Number.NaN, end: Number.NaN }, { start: 8, end: 3 }]) {
    it(`treats invalid stored offsets as unusable hints: ${offsets.start} / ${offsets.end}`, () => expect(findAssistantCitationText('a quote', selector('quote', offsets))).toEqual({ start: 2, end: 7 }));
  }
});

describe('resolveAssistantCitationCommentDismissal', () => {
  it('commits typed or edited text, clears an emptied draft, closes when unchanged or on Escape', () => {
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'needs a retry', savedComment: undefined })).toEqual({ kind: 'commit', comment: 'needs a retry' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'focus-out', draft: 'second thought', savedComment: 'first thought' })).toEqual({ kind: 'commit', comment: 'second thought' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: null, savedComment: 'kept' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: '  kept ', savedComment: 'kept' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'trigger-press', draft: '', savedComment: 'old' })).toEqual({ kind: 'commit', comment: '' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'escape-key', draft: 'unsaved', savedComment: undefined })).toEqual({ kind: 'close' });
  });
  it('keeps the popover open instead of dropping an over-length draft', () =>
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'x'.repeat(ASSISTANT_CITATION_MAX_COMMENT_LENGTH + 1), savedComment: undefined })).toEqual({ kind: 'keep-open' }));
});

describe('selection action positioning', () => {
  const bounds = { left: 100, top: 50, width: 500, height: 220 }, viewport = { width: 1024, height: 768 }, selectionRect = { right: 260, bottom: 140 };
  it('prefers the cursor release over the selection\'s bounding box', () => expect(resolveSelectionActionPosition({ bounds, selectionRect, pointer: { x: 520, y: 200 }, viewport })).toEqual({ x: 520, y: 200 }));
  it('anchors keyboard selections just below the text end', () => expect(resolveSelectionActionPosition({ bounds, selectionRect, pointer: null, viewport })).toEqual({ x: 260, y: 144 }));
  it('keeps an outside release within the surface and the anchor inside the window', () => {
    expect(resolveSelectionActionPosition({ bounds, selectionRect, pointer: { x: 720, y: 340 }, viewport })).toEqual({ x: 600, y: 270 });
    expect(resolveSelectionActionPosition({ bounds, selectionRect, pointer: { x: 40, y: 20 }, viewport })).toEqual({ x: 100, y: 50 });
    expect(resolveSelectionActionPosition({ bounds: { left: -100, top: -100, width: 2000, height: 2000 }, selectionRect, pointer: { x: 1800, y: -50 }, viewport })).toEqual({ x: 1016, y: 8 });
  });
});
