import { describe, expect, test } from 'bun:test';
import { launchTitle, truncate } from '../../src/composer/composer-editor-title';
import { contextLabel } from '../../src/composer/composer-editor-menu';

const quote = '[Assistant quote](t3-citation://v1/env/thread/msg?text=Hello%20there&start=0&end=11&prefix=&suffix=)';
describe('new-thread title seed (ChatView onSend, deriveThreadTitleSeed)', () => {
  test('a context chip reads as its label; an image chip is left out (#19822)', () => {
    expect(launchTitle('fixture complete see [notes.md](t3-context://v1/file/file_6ec02855-c221-4fba-8e6e-691c4815d544) ')).toBe('fixture complete see notes.md');
    expect(launchTitle('![shot.png](t3-context://v1/image/image_a) explain')).toBe('explain');
    expect(launchTitle('fix\n\n  this   now')).toBe('fix this now');
  });
  test('an assistant quote reads as its text', () => {
    expect(launchTitle(`about ${quote}`)).toBe('about Hello there');
  });
  test('image chips alone read "Attached image" or "Attached images", by distinct image', () => {
    expect(launchTitle('![shot.png](t3-context://v1/image/image_a)')).toBe('Attached image');
    expect(launchTitle('![a.png](t3-context://v1/image/image_a) ![a.png](t3-context://v1/image/image_a)')).toBe('Attached image');
    expect(launchTitle('![a.png](t3-context://v1/image/image_a) ![b.png](t3-context://v1/image/image_b)')).toBe('Attached images');
  });
  test('with no prose, image attachments, then the first attachment, name it', () => {
    expect(launchTitle('', [{ type: 'image', name: 'photo.png' }])).toBe('Attached image');
    expect(launchTitle('', [{ type: 'image', name: 'a.png' }, { type: 'image', name: 'b.png' }])).toBe('Attached images');
    expect(launchTitle('', [{ type: 'file', name: 'notes.md' }])).toBe('File: notes.md');
    expect(launchTitle('  ')).toBe('New thread');
  });
  test('truncate keeps 50 code points, so an emoji at the cut stays whole', () => {
    expect(launchTitle('x'.repeat(60))).toBe(`${'x'.repeat(50)}...`);
    expect(launchTitle('x'.repeat(50))).toBe('x'.repeat(50));
    expect(launchTitle(`${'x'.repeat(49)}🙂🙂`)).toBe(`${'x'.repeat(49)}🙂...`);
    expect(launchTitle(`${'x'.repeat(49)}🙂`)).toBe(`${'x'.repeat(49)}🙂`);
    expect(truncate('Implement ' + 'p'.repeat(60))).toBe(`Implement ${'p'.repeat(40)}...`);
  });
  test('a context chip label cut at 200 units keeps an emoji whole', () => {
    expect(contextLabel(`${'x'.repeat(199)}🙂tail`, 'thread')).toBe('x'.repeat(199));
    expect(contextLabel(`${'x'.repeat(198)}🙂tail`, 'thread')).toBe(`${'x'.repeat(198)}🙂`);
    expect(contextLabel('  ', 'thread')).toBe('thread');
  });
});
