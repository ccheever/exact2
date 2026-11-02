// An attachment-only message's prompt stays out of what the person reads or shares (T3 Code 2359643cff
// timelineMinimapItems.ts, copyPromptLink.ts; MIT, see LICENSE-T3).
import { expect, test } from 'bun:test';
import { ATTACHMENT_ONLY_PROMPT, isAttachmentOnlyPrompt } from '../../src/composer/attachment-only-prompt';
import { minimapItems } from '../../src/timeline/timeline-minimap';
import { promptLinkFor } from '../../src/prompt-links/copy-prompt-link';

test('the minimap labels an attachment-only message as a user message, and its prompt shares as no link', () => {
  expect([ATTACHMENT_ONLY_PROMPT, `Ultrathink:\n${ATTACHMENT_ONLY_PROMPT}`, `${ATTACHMENT_ONLY_PROMPT} more`].map(isAttachmentOnlyPrompt)).toEqual([true, true, false]);
  const items = minimapItems([{ id: 'u1', kind: 'user', body: ATTACHMENT_ONLY_PROMPT }, { id: 'u2', kind: 'user', body: 'Fix the build' }]);
  expect(items.map(item => item.label)).toEqual(['User message', 'Fix the build']);
  expect(promptLinkFor(ATTACHMENT_ONLY_PROMPT)).toBeNull();
  expect(promptLinkFor(`Ultrathink:\n${ATTACHMENT_ONLY_PROMPT}`)).toBeNull();
  expect(promptLinkFor('Fix the build')).not.toBeNull();
});
