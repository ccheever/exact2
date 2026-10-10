// right-panel-escape: T3 Code 1e2ecbd975 binds no Escape to the right panel (DEFAULT_KEYBINDINGS: rightPanel.toggle is
// mod+alt+b, rightPanel.close mod+w). Only its sheet (RightPanelSheet, a Base UI Dialog at a window 980 wide or less)
// closes on Escape, from any focus whose Escape reaches the document. Checked on the live reference over CDP
// (2026-10-10): inline, Escape kept Diff, Files (tree, search, editor), Browser (page, URL field) and the launcher open,
// from the page and from the focused toggle; in a sheet it closed Diff, Browser, the launcher, the URL field's and the
// Files search's sheet, and the file editor's after its first Escape blurred the editor. The clone's toggle declared
// Escape inline too, and its launcher handed Escape to the panel, which hid it: both closed the inline panel.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { panelState, panelView, surfaceLocal } from './r4-surfaces-panel';
import { filesLocal } from './r4-surfaces-files';
import { decodeClientPrefs } from './settings-core';

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

const holders = ['panel.deviceSetup', 'panel.files.editorsOpen', 'panel.files.editing', 'searchFocused', 'panel.browser.capture.pickActive'];
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

  test('the Files search\'s own Escape closes the search and blurs the field (FileSearchField onKeyDown), and closes a sheet', () => {
    const explorer = block(source('r4-surfaces-files.contract'), 'component R4Explorer', '//');
    expect(explorer).toContain('  action searchKey(k: string)\n    if k == "Escape"\n      local("surface-files-search-key", sheet ? "sheet" : "", k)\n      preventDefault()\n      blur()');
    expect(line(explorer, 'testId="files-search"')).toContain('key=searchKey focus=searchFocus(true) blur=searchFocus(false)');
    // The focus is the window's (no send: a second `localChanged` send in one turn forgets the first, here the Escape's).
    const panel = block(source('shell-panels.contract'), 'component SurfacePanel', '//');
    expect(panel).toContain('  action filesSearchFocus(on: bool)\n    filesSearchAt = on ? shell.panel.active : (filesSearchAt == shell.panel.active ? "" : filesSearchAt)');
    expect(line(panel, 'R4PanelHeader(')).toContain('searchFocused=(filesSearchAt != "" and filesSearchAt == shell.panel.active and shell.panel.files.showExplorer))');
    expect(line(panel, 'R4SurfaceBody(')).toContain('searchFocus=filesSearchFocus)');
    expect(line(source('diff.contract'), 'R4PanelHeader(')).toContain('searchFocused=false)');
    // The sheet's Escape closes the dialog after closeSearch: the window's half of the close (rightPanelStore.close).
    const chatLocal = source('app.contract').split('\n').find(row => row.includes('op == "surface-browser-float"')) ?? '';
    expect(chatLocal).toContain('(op == "surface-files-search-key" and id == "sheet" and value == "Escape")');
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

const native = { available: true } as unknown as Native;
function filesClient() {
  const calls: string[] = [];
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, revision: 0, generation: 1, diffOpen: false, diffText: '', diffError: '', diffLoading: false, origin: 'http://127.0.0.1:9',
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    config: {}, presentation: {}, local: { clientSettings: { ...decodeClientPrefs({}), wordWrap: true } },
    shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', branch: 'main' }] },
    rpc: async () => ({}),
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>) => {
        calls.push(`${method} ${String(payload.query ?? payload.relativePath ?? '')}`.trim());
        if (method === 'projects.listEntries') return { entries: [{ path: 'src', kind: 'directory' }, { path: 'README.md', kind: 'file' }] };
        if (method === 'projects.searchEntries') return { entries: [{ path: 'README.md', kind: 'file' }], truncated: false };
        return {};
      },
      call: async () => ({}),
    }),
  } as unknown as T3Client;
  return { client, calls };
}

describe('right-panel-escape: the Files search\'s Escape (FileSearchField closeSearch)', () => {
  test('it clears the search, and inline the panel stays', async () => {
    const { client } = filesClient();
    await surfaceLocal(client, native, 'open', '', 'files');
    await filesLocal(client, native, 'search', '', 'read');
    expect((await panelView(client, native, 1)).files.query).toBe('read');
    await surfaceLocal(client, native, 'files-search-key', '', 'Escape');
    const view = await panelView(client, native, 1);
    expect(view.files.query).toBe('');
    expect(view.open).toBe(true);
  });

  test('a sheet\'s Escape clears it and closes the panel (the dialog after closeSearch); it reopens with no query', async () => {
    const { client } = filesClient();
    await surfaceLocal(client, native, 'open', '', 'files');
    await filesLocal(client, native, 'search', '', 'read');
    await surfaceLocal(client, native, 'files-search-key', 'sheet', 'Escape');
    expect(panelState(client).visible).toBe(false);
    await surfaceLocal(client, native, 'show', '', '');
    const view = await panelView(client, native, 1, true);
    expect(view.open).toBe(true);
    expect(view.files.query).toBe('');
  });

  test('other keys leave the search alone', async () => {
    const { client } = filesClient();
    await surfaceLocal(client, native, 'open', '', 'files');
    await filesLocal(client, native, 'search', '', 'read');
    await surfaceLocal(client, native, 'files-search-key', 'sheet', 'Enter');
    expect(panelState(client).visible).toBe(true);
    expect((await panelView(client, native, 1)).files.query).toBe('read');
  });
});
