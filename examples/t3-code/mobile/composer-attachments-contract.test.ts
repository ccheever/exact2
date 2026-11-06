import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = readFileSync(new URL('./composer-attachments.contract', import.meta.url), 'utf8');

test('composer attachment strip follows the pinned native tile geometry', () => {
  expect(source).toContain('scrollbar-width="none" touch-action="pan-x"');
  expect(source).toContain('row gap=10 padding-left=14 padding-right=14 padding-bottom=8.75');
  expect(source).toContain('width=72 height=72 flex-shrink=0 border-radius=16');
  expect(source).toContain('overflow="hidden" background-color=colors.subtle');
  expect(source).not.toContain('max-width=180');
  expect(source).not.toContain('width=24 height=24');
});

test('attachment actions expose source-specific labels, roles and stable test targets', () => {
  expect(source).toContain('role=(item.kind == "image" ? "imagebutton" : "button")');
  expect(source).toContain('aria-label=(item.kind == "image" ? `Preview ${item.name}` : `Open ${item.name}`)');
  expect(source).toContain('testId=`composer-attachment-open-${item.id}`');
  expect(source).toContain('testId=`composer-attachment-remove-${item.id}`');
  expect(source).toContain('disabled=item.disabled');
  expect(source).toContain('image item.preview width=72 height=72 object-fit="cover" aria-hidden=true');
});

test('attachment tiles expose truthful upload progress states and retry', () => {
  expect(source).toContain('if item.uploadState == "uploading"');
  expect(source).toContain('Uploading…');
  expect(source).toContain('press=remove("retry-upload", item.id, data.contentOwner)');
  expect(source).toContain('Retry upload for ${item.name}');
});
