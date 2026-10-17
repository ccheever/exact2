// T3 Code 1e2ecbd975 binds no Escape to the right panel (DEFAULT_KEYBINDINGS: rightPanel.toggle is
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

// The thread details popover (`detailsOpen`, the window's above a sheet) holds it as the topmost layer.
const holders = ['detailsOpen', 'panel.deviceSetup', 'panel.files.editing', 'searchFocused', 'panel.browser.capture.pickActive'];
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

  test('the launcher\'s bar (no surface yet) declares Escape only as a sheet with no details popover over it', () => {
    const launcher = keysOf(line(source('shell-panels.contract'), 'testId="panel-toggle-right"'));
    expect(launcher({ sheet: false, detailsOpen: false })).toBe('');
    expect(launcher({ sheet: true, detailsOpen: false })).toBe('Escape');
    expect(launcher({ sheet: true, detailsOpen: true })).toBe('');
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
