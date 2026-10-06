// Lane r12-sidebar: the sidebar rows' keyboard context menus (T3 Code, MIT, see
// LICENSE-T3: Sidebar.tsx SidebarDraftRow handleKeyDown and the thread rows'
// browser-default contextmenu), as the f870c419fc reference behaves in Chrome on
// macOS (lanes/r12-sidebar/tools/refkbd.mjs):
// - a draft row's own handler opens its menu for ContextMenu or Shift+F10 at the
//   row's bottom-left corner ({ x: rect.left, y: rect.bottom });
// - a thread row has no key handler: ContextMenu reaches Chromium's keyboard
//   contextmenu, which opens the row's menu at the focused row's centre; Shift+F10
//   does nothing on macOS.
// Shift+F10 is the draft row's window shortcut while it holds focus
// (sidebar-row.contract DraftCard: `draft-menu` with the value `key`); the row
// key handler carries the key name only (no modifiers), so it answers ContextMenu.
// The native menu (T3Sidebar.swift `sidebarMenu` `anchor`) is placed from the
// window's first responder, the focused row button, instead of the pointer.
import type { T3Client } from './client';

export type MenuAnchor = 'center' | 'bottom-left';

/** The context-menu key by the host's names: the agent's web name, AppKit's NSMenuFunctionKey, Carbon's function-key code. */
const MENU_KEYS = new Set(['ContextMenu', '', '\u0010']);
export const isMenuKey = (name: string): boolean => MENU_KEYS.has(name);

/** A focused row's key: the menu op it opens (a `draft:<project>` id is a draft row), or null. */
export function rowKeyMenu(id: string, name: string): { op: 'menu' | 'draft-menu'; id: string; value: string; anchor: MenuAnchor } | null {
  if (!isMenuKey(name) || !id) return null;
  if (id.startsWith('draft:')) return id.length > 6 ? { op: 'draft-menu', id: id.slice(6), value: 'key', anchor: 'bottom-left' } : null;
  return { op: 'menu', id, value: 'row', anchor: 'center' };
}

const anchors = new WeakMap<object, MenuAnchor>();
/** Run a menu op with its native menu anchored to the focused row; the anchor never outlives the op. */
export async function withMenuAnchor<T>(client: T3Client, anchor: MenuAnchor, run: () => Promise<T>): Promise<T> {
  anchors.set(client, anchor);
  try { return await run(); } finally { anchors.delete(client); }
}
/** The `sidebarMenu` request's anchor field: empty for a pointer menu (right click). */
export function menuAnchor(client: T3Client): { anchor?: MenuAnchor } {
  const anchor = anchors.get(client);
  return anchor ? { anchor } : {};
}
