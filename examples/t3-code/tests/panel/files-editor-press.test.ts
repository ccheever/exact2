// The Files editor begins under the press that mounts it (exact2 #951): the press on the file's body mounts the
// textarea in its own action, so the host focuses it with the caret under the click, and no native hook is needed.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';

const files = readFileSync(appPath('files-surface.contract'), 'utf8');
const preview = files.slice(files.indexOf('component FilePreview\n'), files.indexOf('\ncomponent ', files.indexOf('component FilePreview\n') + 1));

describe('Files editor press', () => {
  test('the press on the file body marks the edit in its own action, numbered, then asks the module', () => {
    expect(preview).toContain('press=beginEdit role="code"');
    const begin = preview.slice(preview.indexOf('  action beginEdit'), preview.indexOf('  action endEdit'));
    expect(begin).toContain('pressedEdit = files.path');
    expect(begin).toContain('local("surface-files-begin-edit", files.path, `${serial}`)');
    expect(preview).toContain('derive editing = files.editing or (files.editable and pressedEdit == files.path and files.editPress != `${pressedSerial}`)');
  });

  test('the editor mounts on that mark with the file text and carries no native hook', () => {
    const editor = preview.slice(preview.indexOf('textarea id="file-editor"'));
    expect(preview).toContain('if editing\n            textarea id="file-editor"');
    expect(editor).toContain('value=(files.editing ? files.editorText : files.text)');
    expect(editor).toContain('autofocus=true');
    expect(editor).toContain('autocorrect="off"');
    expect(editor.split('\n')[0]).not.toContain('hatch=');
    expect(preview.slice(preview.indexOf('  action endEdit'), preview.indexOf('  view\n'))).toContain('pressedEdit = ""');
  });
});
