// The share target's Android activity (LLP 1133.024 D5): the derived filters and the member the reader reads.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { androidShareActivity } from './share-target.mjs';

const fieldnotes = {
  action: '/share', method: 'POST', enctype: 'multipart/form-data',
  params: { title: 'title', text: 'text', files: [{ name: 'files', accept: ['text/plain', '.md', 'image/*'] }] },
};

test('text and files: SEND takes text and every accepted type, SEND_MULTIPLE the files', () => {
  const xml = androidShareActivity(fieldnotes).join('\n');
  assert.match(xml, /android:name="dev\.exact\.app\.MainActivity" android:launchMode="singleTask" tools:replace="android:launchMode"/);
  const [send, multiple] = xml.split('</intent-filter>');
  assert.match(send, /action\.SEND"/);
  assert.deepEqual([...send.matchAll(/mimeType="([^"]+)"/g)].map((m) => m[1]), ['text/plain', 'text/markdown', 'image/*']);
  assert.match(multiple, /action\.SEND_MULTIPLE"/);
  assert.deepEqual([...multiple.matchAll(/mimeType="([^"]+)"/g)].map((m) => m[1]), ['text/plain', 'text/markdown', 'image/*']);
});

test('the meta-data carries action and params only, escaped for an attribute', () => {
  const value = androidShareActivity(fieldnotes).join('\n').match(/share_target" android:value="([^"]*)"/)[1];
  assert.deepEqual(JSON.parse(value.replaceAll('&quot;', '"').replaceAll('&lt;', '<').replaceAll('&amp;', '&')), { action: '/share', params: fieldnotes.params });
});

test('text only has no SEND_MULTIPLE; no target, no activity', () => {
  const xml = androidShareActivity({ action: '/s', params: { text: 't' } }).join('\n');
  assert.doesNotMatch(xml, /SEND_MULTIPLE/);
  assert.equal(androidShareActivity(undefined), null);
});
