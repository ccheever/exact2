import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { placeLinkedPrompt, replaceLinkedPrompt } from '../../src/prompt-links/linked-prompt';
import { sanitizeLinkedPrompt } from '../../src/prompt-links/prompt-links';
import { toasts } from '../../src/shell/toast';

test('a link fills the targeted draft and keeps a persistent warning for hidden instructions', () => {
  const client = new T3Client();
  client.local.drafts.other = 'Keep this draft';
  placeLinkedPrompt(client, 'env:new:project', sanitizeLinkedPrompt('Read\u200b this\u202E first'));
  expect(client.local.drafts).toMatchObject({ other: 'Keep this draft', 'env:new:project': 'Read this first' });
  expect(toasts(client)).toMatchObject([
    { kind: 'warning', title: 'This prompt had hidden characters, it may be malicious', timeoutMs: 0,
      description: 'T3 Code removed 2 hidden characters. Read the prompt before you send it.' },
    { kind: 'info', title: 'Prompt added from a link', description: 'Read it before you send.' },
  ]);
});

test('an existing draft survives until Replace draft, even after the reader moves to another thread', () => {
  const client = new T3Client();
  client.environmentId = 'env'; client.projectId = 'first';
  const key = client.draftKey;
  client.local.drafts[key] = 'My unfinished prompt';
  placeLinkedPrompt(client, key, { text: 'Linked prompt', hiddenCharacters: 0 }, ['Using your default model.']);
  const warning = toasts(client).find(toast => toast.title === 'Linked prompt ready')!;
  expect(client.local.drafts[key]).toBe('My unfinished prompt');
  expect(warning.action).toEqual({ label: 'Replace draft', op: 'shelllocal:prompt-link-replace', id: String(warning.id) });
  client.projectId = 'second';
  client.local.drafts[client.draftKey] = 'Another unfinished prompt';
  replaceLinkedPrompt(client, warning.id);
  expect(client.local.drafts[key]).toBe('Linked prompt');
  expect(client.draft).toBe('Another unfinished prompt');
  expect(toasts(client).at(-1)?.description).toBe('Read it before you send. Using your default model.');
  expect(toasts(client).some(toast => toast.id === warning.id)).toBe(false);
});

test('a link with an identical prompt needs no replacement confirmation', () => {
  const client = new T3Client();
  client.local.drafts.target = 'Same prompt';
  placeLinkedPrompt(client, 'target', { text: 'Same prompt', hiddenCharacters: 0 });
  expect(toasts(client).map(toast => toast.title)).toEqual(['Prompt added from a link']);
});
