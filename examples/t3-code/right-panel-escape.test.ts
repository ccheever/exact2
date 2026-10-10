// right-panel-escape: T3 Code 1e2ecbd975 binds no Escape to the right panel (DEFAULT_KEYBINDINGS: rightPanel.toggle is
// mod+alt+b, rightPanel.close mod+w). Only its sheet (RightPanelSheet, a Base UI Dialog at a window 980 wide or less)
// closes on Escape, from any focus whose Escape reaches the document. Checked on the live reference over CDP
// (2026-10-10): inline, Escape kept Diff, Files (tree, search, editor), Browser (page, URL field) and the launcher open,
// from the page and from the focused toggle; in a sheet it closed Diff, Browser, the launcher, the URL field's and the
// Files search's sheet, and the file editor's after its first Escape blurred the editor. The clone's toggle declared
// Escape inline too, and its launcher handed Escape to the panel, which hid it: both closed the inline panel.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';
const block = (text: string, start: string, end: string) => {
  const all = text.split('\n'), at = all.findIndex(candidate => candidate.includes(start));
  return at < 0 ? '' : all.slice(at, all.findIndex((candidate, index) => index > at && candidate.startsWith(end))).join('\n');
};

// The toggle's `aria-keyshortcuts=(…)` as a function of the facts it reads (Contract's and/or/not and `?:` as JavaScript).
function keysOf(row: string): (facts: Record<string, boolean>) => string {
  const at = row.indexOf('aria-keyshortcuts=(');
  expect(at).toBeGreaterThan(-1);
  let depth = 0, end = at + 'aria-keyshortcuts='.length;
  for (; end < row.length; end++) {
    if (row[end] === '(') depth++;
    if (row[end] === ')' && --depth === 0) break;
  }
  const expr = row.slice(at + 'aria-keyshortcuts='.length, end + 1);
  const names = [...new Set(expr.replace(/"[^"]*"/g, '').match(/[A-Za-z_][\w.]*/g) ?? [])].filter(name => !['and', 'or', 'not'].includes(name));
  const js = expr.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
  return facts => {
    const scope: Record<string, unknown> = {};
    for (const name of names) {
      if (!(name in facts)) throw new Error(`no fact for ${name} in ${expr}`);
      const [head, ...rest] = name.split('.');
      let into = scope as Record<string, unknown>;
      if (rest.length === 0) { into[head] = facts[name]; continue; }
      into = (into[head] ??= {}) as Record<string, unknown>;
      for (const part of rest.slice(0, -1)) into = (into[part] ??= {}) as Record<string, unknown>;
      into[rest[rest.length - 1]] = facts[name];
    }
    const heads = Object.keys(scope);
    return new Function(...heads, `return ${js};`)(...heads.map(head => scope[head])) as string;
  };
}

const holders = ['panel.deviceSetup', 'panel.files.editorsOpen', 'panel.files.editing', 'panel.browser.capture.pickActive'];
const none = Object.fromEntries(holders.map(name => [name, false]));

describe('right-panel-escape: the right panel takes Escape only as a sheet', () => {
  const header = keysOf(line(source('r4-surfaces.contract'), 'testId="panel-toggle-right"'));

  test('inline, the tab bar\'s toggle declares no Escape, whatever holds the focus', () => {
    expect(header({ sheet: false, ...none })).toBe('');
    for (const holder of holders) expect(header({ sheet: false, ...none, [holder]: true })).toBe('');
  });

  test('as a sheet it declares Escape, except while a nested dialog or menu, the file editor or the page\'s pick keeps it', () => {
    expect(header({ sheet: true, ...none })).toBe('Escape');
    for (const holder of holders) expect(header({ sheet: true, ...none, [holder]: true })).toBe('');
  });

  test('the URL field no longer holds the toggle\'s Escape: inline nothing closes, and the reference\'s sheet closes from it', () => {
    const contract = source('r4-surfaces.contract');
    expect(block(contract, 'component R4HeaderBar', '//')).not.toContain('urlFocused');
    expect(block(contract, 'component R4PanelHeader', '//')).not.toContain('urlFocused');
    expect(line(source('shell-panels.contract'), 'R4PanelHeader(')).not.toContain('urlFocused=');
    expect(line(source('diff.contract'), 'R4PanelHeader(')).not.toContain('urlFocused=');
  });

  test('the launcher\'s bar (no surface yet) declares Escape only as a sheet', () => {
    const launcher = keysOf(line(source('shell-panels.contract'), 'testId="panel-toggle-right"'));
    expect(launcher({ sheet: false })).toBe('');
    expect(launcher({ sheet: true })).toBe('Escape');
  });

  test('the Files editor\'s own Escape leaves it (installFileEditorDismissal), and its blur ends the editing', () => {
    const preview = block(source('r4-surfaces-files.contract'), 'component R4FilePreview', '//');
    expect(preview).toContain('  action editorKey(k: string)\n    if k == "Escape"\n      preventDefault()\n      blur()');
    const editor = line(preview, 'textarea id="file-editor"');
    expect(editor).toContain('key=editorKey blur=local("surface-files-end-edit", files.path, "")');
  });
});

describe('right-panel-escape: the launcher\'s Escape hides nothing (RightPanelEmptyState binds none)', () => {
  test('the launcher still hands its other keys to the panel, and the panel\'s "key" closes nothing', () => {
    const launcher = block(source('shell-panels.contract'), 'component SurfaceLauncher', 'component SurfaceRow');
    expect(launcher).toContain('    else\n      ui("key", k)');
    for (const [file, action] of [['app.contract', 'action panelUi(what: string, id: string) //'], ['app-window.contract', '  action panelUi(what: string, id: string)\n']] as const) {
      const text = source(file), at = text.indexOf(action);
      expect(at).toBeGreaterThan(-1);
      const body = text.slice(at, text.indexOf('\n  action ', at + action.length));
      expect(body).not.toContain('id == "Escape"');
      expect(body).toContain('what == "toggle" or what == "close" or what == "tab-close"');
    }
  });
});
