// Lane r12-sidebar: the sidebar rows' keyboard context menus (T3 Code, MIT, see
// LICENSE-T3: Sidebar.tsx SidebarDraftRow handleKeyDown and the thread rows'
// browser-default contextmenu), as the f870c419fc reference behaves in Chrome on
// macOS (lanes/r12-sidebar/tools/refkbd.mjs):
// - a draft row's own handler opens its menu for ContextMenu or Shift+F10 at the
//   row's bottom-left corner ({ x: rect.left, y: rect.bottom });
// - a thread row has no key handler: ContextMenu reaches Chromium's keyboard
//   contextmenu, which opens the row's menu at the focused row's centre; Shift+F10
//   does nothing on macOS. Since exact2 #314 the host does the same: an unprevented
//   ContextMenu runs the focused row's `contextmenu` and opens its context popover
//   (ThreadMenu) at the row's centre, so a thread row's key opens no menu here
//   (adopt-main-fixes-r7).
// The draft row's key handler (sidebar-row.contract DraftCard `rowKey`) reads the
// KeyboardEvent's modifiers (exact2 8a0afbeab), names Shift+F10 itself and prevents
// both keys' default, as SidebarDraftRow's does; the legacy sidebar's rows prevent
// ContextMenu's too, so the host's default never opens a second menu.
// The native menu (T3Sidebar.swift `sidebarMenu` `anchor`) is placed from the
// window's first responder, the focused row button, instead of the pointer.
import type { T3Client } from './client';

export type MenuAnchor = 'center' | 'bottom-left';

/** The context-menu key: the host names the physical key `ContextMenu` on every input path since exact2 #314. */
export const isMenuKey = (name: string): boolean => name === 'ContextMenu';

/** A focused draft row's key: the menu op it opens (a `draft:<project>` id), or null. */
export function rowKeyMenu(id: string, name: string): { op: 'draft-menu'; id: string; value: string; anchor: MenuAnchor } | null {
  if (!id) return null;
  if (id.startsWith('draft:')) return (isMenuKey(name) || name === 'Shift+F10') && id.length > 6 ? { op: 'draft-menu', id: id.slice(6), value: 'key', anchor: 'bottom-left' } : null;
  return null; // a thread row: the host's ContextMenu default opens its context popover (exact2 #314)
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
