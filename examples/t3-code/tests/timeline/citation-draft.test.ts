import { expect, test } from 'bun:test';
import { composerEditorView } from '../../src/composer/composer-editor';
import { T3Client } from '../../src/client/client';
import { requestPresentation } from '../../src/composer/requests';
import { value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

import { clearCitationDraft, noteCitationDraft, preserveCitationDraft, registerCitationDraft } from '../../src/timeline/citation-draft';
import { parseAssistantCitationHref, serializeAssistantCitation } from '../../src/timeline/diff-citations';

const link = serializeAssistantCitation({ version: 1, environmentId: 'env', threadId: 't', messageId: 'a', text: 'quoted text', start: 0, end: 11, prefix: '', suffix: '' });
function fixture(comment: string) {
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't', projectId: 'p' });
  client.local.drafts[client.draftKey] = `Review ${link} please`;
  client.thread = { projection: { runtimeRequests: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  registerCitationDraft(client, { open: true, kind: 'citation', surface: 'composer', seq: 7, owner: client.snapshotOwner, link });
  noteCitationDraft(client, 7, comment);
  return client;
}

/** A pending question, which takes the composer as its answer. */
function ask(client: T3Client) {
  client.projection.runtimeRequests = [{ id: 'request', status: 'pending', kind: 'user_input' }];
  client.projection.turnItems = [{ type: 'user_input_request', requestId: 'request', questions: [{ id: 'q', question: 'Next?', options: [] }] }];
}

test('an unsaved comment survives a question replacing the composer', async () => {
  const client = fixture('Check the edge cases');
  expect(preserveCitationDraft(client)).toBe(false);
  ask(client);
  await composerEditorView(client, null);
  const href = /\[Assistant quote\]\(([^)]+)\)/.exec(client.draft)![1]!;
  expect(parseAssistantCitationHref(href)?.comment).toBe('Check the edge cases');
  expect(client.draft.startsWith('Review ')).toBe(true);
  expect(client.draft.endsWith(' please')).toBe(true);
  expect(preserveCitationDraft(client)).toBe(false);
});

test('a pending approval leaves the composer and its unsaved comment alone (a93b03c8a5)', async () => {
  const client = fixture('Check the edge cases');
  client.projection.runtimeRequests = [{ id: 'request', status: 'pending', kind: 'approval' }];
  await composerEditorView(client, null);
  expect(client.draft).toBe(`Review ${link} please`);
  expect(preserveCitationDraft(client)).toBe(false);
});

test('a later edit, another thread, cancellation and over-length comments cannot overwrite the prompt', () => {
  const edited = fixture('comment');
  edited.local.drafts[edited.draftKey] = 'newer prompt';
  ask(edited);
  expect(preserveCitationDraft(edited)).toBe(false);
  expect(edited.draft).toBe('newer prompt');
  const other = fixture('comment');
  other.threadId = 'another'; ask(other);
  expect(preserveCitationDraft(other)).toBe(false);
  expect(other.draft).toBe('');
  for (const comment of ['cancelled', 'x'.repeat(8001)]) {
    const client = fixture(comment);
    if (comment === 'cancelled') clearCitationDraft(client, 7);
    ask(client);
    expect(preserveCitationDraft(client)).toBe(false);
    expect(client.draft).toBe(`Review ${link} please`);
  }
});

test('identical quote chips keep separate comments through a request replacement', async () => {
  const client = fixture('first draft');
  client.local.drafts[client.draftKey] = `${link} and ${link}`;
  registerCitationDraft(client, { open: true, kind: 'citation', surface: 'composer', seq: 8, owner: client.snapshotOwner, link, start: link.length + 5 });
  noteCitationDraft(client, 8, 'second chip');
  ask(client);
  await composerEditorView(client, null);
  const comments = [...client.draft.matchAll(/\[Assistant quote\]\(([^)]+)\)/g)].map(match => parseAssistantCitationHref(match[1]!)?.comment);
  expect(comments).toEqual([undefined, 'second chip']);
});

test('the last input still commits if a request removes the native editor before its input operation reaches the data module', async () => {
  const client = fixture('earlier input');
  ask(client);
  noteCitationDraft(client, 7, 'last inpu');
  noteCitationDraft(client, 7, 'last input');
  const href = /\[Assistant quote\]\(([^)]+)\)/.exec(client.draft)![1]!;
  expect(parseAssistantCitationHref(href)?.comment).toBe('last input');
});

test('background authentication requests do not replace the composer or save its comment', () => {
  const client = fixture('comment');
  client.projection.runtimeRequests = [{ id: 'auth', status: 'pending', kind: 'auth_refresh' }];
  expect(preserveCitationDraft(client)).toBe(false);
  expect(client.draft).toBe(`Review ${link} please`);
});

test('the window retires its typed copy after a citation rewrite through a question', async () => {
  const client = fixture('Retain this unsaved review note.');
  const draft = client.draft, composerOwner = 'current-thread';
  const draftOwner = composerOwner + requestPresentation(client).requestKey;
  ask(client);
  await composerEditorView(client, null);
  client.projection.runtimeRequests = [];
  const data = { requestKey: requestPresentation(client).requestKey, draft: client.draft };
  const source = await appText('app.contract');
  const expression = /^  derive composerText = (.+)$/m.exec(source)?.[1];
  if (!expression) throw Error('composerText derive missing');
  const shown = value(expression, { draft, draftOwner, composerOwner, data });
  expect(shown).toBe(client.draft);
  expect(shown).not.toBe(draft);
  expect(parseAssistantCitationHref(/\[Assistant quote\]\(([^)]+)\)/.exec(String(shown))![1]!)?.comment).toBe('Retain this unsaved review note.');
});
