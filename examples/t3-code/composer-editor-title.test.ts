import { describe, expect, test } from 'bun:test';
import { launchTitle } from './composer-editor-title';

const quote = '[Assistant quote](t3-citation://v1/env/thread/msg?text=Hello%20there&start=0&end=11&prefix=&suffix=)';
describe('new-thread title seed (ChatView onSend)', () => {
  test('context chips are left out of the title', () => {
    expect(launchTitle('fixture complete see [notes.md](t3-context://v1/file/file_6ec02855-c221-4fba-8e6e-691c4815d544) ')).toBe('fixture complete see');
    expect(launchTitle('![shot.png](t3-context://v1/image/image_a) explain')).toBe('explain');
  });
  test('an assistant quote reads as its text', () => {
    expect(launchTitle(`about ${quote}`)).toBe('about Hello there');
  });
  test('with no prose the first image, then the first file, names it', () => {
    expect(launchTitle('[notes.md](t3-context://v1/file/file_x)', 'photo.png', 'notes.md')).toBe('Image: photo.png');
    expect(launchTitle('[notes.md](t3-context://v1/file/file_x)', '', 'notes.md')).toBe('File: notes.md');
    expect(launchTitle('  ')).toBe('New thread');
  });
  test('truncate keeps 50 characters', () => {
    expect(launchTitle('x'.repeat(60))).toBe(`${'x'.repeat(50)}...`);
    expect(launchTitle('x'.repeat(50))).toBe('x'.repeat(50));
  });
});
