// A textarea's resize at T3 Code 7c3b3b3605ef: Tailwind's preflight gives every textarea `resize: vertical`, and the
// reference's own (ui/textarea.tsx, ThemeImportDialog's `resize-y`) keep it. AssistantCitationCommentEditor says
// `resize-none`, the browser page's key catcher is `sr-only`, and the composer, the prompt preview and the file editor
// are not textareas there. Here a textarea's default is `none` (docs/contract-for-agents.md), so each says its own.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appFiles, appPath } from '../support/app-files';

const NONE = ['id="composer"', 'id="file-editor"', 'id=`server-browser-keys-', 'id="citation-comment-text"', 'id="prompt-font-preview"'];

test('every textarea the reference draws with a grip says resize="vertical", and the others none', () => {
  const found: Array<[string, string]> = [];
  for (const name of appFiles().filter(file => file.endsWith('.contract') && !file.includes('.test.'))) {
    for (const line of readFileSync(appPath(name), 'utf8').split('\n')) {
      if (!/^\s*textarea\s/.test(line)) continue;
      const none = NONE.find(id => line.includes(id));
      found.push([none ?? name, line.includes('resize="vertical"') ? 'vertical' : line.includes('resize=') ? 'other' : 'none']);
    }
  }
  expect(found.filter(([key, resize]) => NONE.includes(key) !== (resize === 'none'))).toEqual([]);
  expect(found.filter(([, resize]) => resize === 'vertical').length).toBe(14);
});
