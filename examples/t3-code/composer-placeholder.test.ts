// The empty prompt's placeholder (ComposerPromptEditorTiptap.tsx: `pointer-events-none absolute inset-0`).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const composer = readFileSync(join(import.meta.dir, 'composer.contract'), 'utf8');

describe('composer placeholder', () => {
  test('takes no press, so a press on its line reaches the field behind it', () => {
    // Positioned, it paints above the static field; a press on it took the focus from Message with no caret.
    const line = composer.split('\n').find((row) => row.includes('testId="composer-placeholder"'));
    expect(line).toBeDefined();
    expect(line).toContain('position="absolute"');
    expect(line).toContain('pointer-events="none"');
    expect(line).toContain('aria-hidden=true');
  });
});
