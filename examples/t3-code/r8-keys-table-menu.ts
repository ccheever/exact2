// Lane r8-keys: a Markdown table's Copy menu as a window-level popup.
//
// ChatMarkdown.tsx MarkdownTable (MIT, see LICENSE-T3) opens <MenuPopup
// align="end"> from the footer's Copy button: Base UI portals it above every
// layer (z-[130], over the provider banner), positions it below the trigger
// with a 4px side offset, flips it above when the window has no room below,
// and, as a modal menu, closes it on Escape or a press outside. The window's
// chatLocal reports the trigger's frame and the window size with
// `table-menu` (x|y|width|height|windowWidth|windowHeight|markdown, a tab, csv).
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { num, obj } from './domain';

export interface TableMenuView {
  id: string; x: number; y: number; triggerX: number; triggerY: number; markdown: string; csv: string;
  /** fix-keyboard-focus: the item the menu focuses as it mounts, when ↓ (1, the first) or ↑ (-1, the last) opened it; 0 for none. */
  keyed: number;
}
type TableMenu = TableMenuView & { threadId: string };

/** The popup's box: 160 wide (min-w-40), two 28pt rows in 4pt padding and a 1pt border. */
export const TABLE_MENU_WIDTH = 160;
export const TABLE_MENU_HEIGHT = 66;
const SIDE_OFFSET = 4, COLLISION_PADDING = 5;

const menus = new WeakMap<T3Client, TableMenu>();

/** Where the popup goes for a trigger frame inside a window of the given size. */
export function placeTableMenu(trigger: { x: number; y: number; width: number; height: number }, windowWidth: number, windowHeight: number): { x: number; y: number } {
  const maxX = Math.max(COLLISION_PADDING, windowWidth - TABLE_MENU_WIDTH - COLLISION_PADDING);
  const x = Math.min(maxX, Math.max(COLLISION_PADDING, trigger.x + trigger.width - TABLE_MENU_WIDTH));
  const below = trigger.y + trigger.height + SIDE_OFFSET;
  const above = trigger.y - SIDE_OFFSET - TABLE_MENU_HEIGHT;
  const fitsBelow = below + TABLE_MENU_HEIGHT <= windowHeight - COLLISION_PADDING;
  return { x, y: fitsBelow || above < COLLISION_PADDING ? below : above };
}

/** The trigger as drawn now (R8KeysMeasure.swift): Contract's frame() is the layout's, which a scrolled transcript leaves behind. */
async function drawnFrame(native: Native | undefined, id: string): Promise<number[] | null> {
  if (!native?.available) return null;
  try {
    const reply = obj(await native.later({ op: 'r8MeasureFrame', name: `table-copy-${id}`, generation: 0 }));
    const box = obj(reply.value);
    return reply.ok === true && num(box.width) > 0 ? [num(box.x), num(box.y), num(box.width), num(box.height), num(box.windowWidth), num(box.windowHeight)] : null;
  } catch { return null; }
}

/** `chatlocal:table-menu` toggles the table's popup; `table-menu-close` dismisses it. */
export async function tableMenuAction(client: T3Client, op: string, id: string, value: string, native?: Native): Promise<string> {
  if (op === 'table-menu-close') { menus.delete(client); return ''; }
  const open = menus.get(client);
  if (open && open.id === id && open.threadId === client.threadId) { menus.delete(client); return ''; }
  const parts = value.split('|');
  const [x, y, width, height, windowWidth, windowHeight] = (await drawnFrame(native, id)) ?? parts.slice(0, 6).map(Number);
  // fix-keyboard-focus: ↓ or ↑ on the Copy button marks its request `keys:first` or `keys:last` and a tab before the
  // table (MenuTrigger's arrow keys open the menu at its first or last item; a table's Markdown starts with a pipe).
  const rest = parts.slice(6).join('|');
  const end = /^keys:(first|last)\t/.exec(rest);
  // The Copy button sends `${markdown}\t${csv}`: Contract's `\t` is a real tab, and neither text holds one
  // (r4_timeline_tables.rs collapses each cell's whitespace and joins rows with newlines), so the first tab
  // divides them. This split once looked for `\u0000`, which Contract no longer writes, so Copy as CSV copied
  // nothing and Copy as Markdown copied both (fix-keyboard-focus).
  const body = end ? rest.slice(end[0].length) : rest;
  const cut = body.indexOf('\t');
  const markdown = cut < 0 ? body : body.slice(0, cut);
  const csv = cut < 0 ? '' : body.slice(cut + 1);
  if (!id || !markdown || ![x, y, width, height, windowWidth, windowHeight].every(Number.isFinite) || width! <= 0 || height! <= 0) throw new ClientError('That table is unavailable.');
  const place = placeTableMenu({ x: x!, y: y!, width: width!, height: height! }, windowWidth!, windowHeight!);
  menus.set(client, { id, threadId: client.threadId, x: place.x, y: place.y, triggerX: x!, triggerY: y!, markdown, csv, keyed: end ? (end[1] === 'last' ? -1 : 1) : 0 });
  return '';
}

/** A copy from the popup closes it (Base UI closes a menu when an item is clicked). */
export function closeTableMenu(client: T3Client): void { menus.delete(client); }

/** The snapshot's open popup (none, or the one on this thread). */
export function tableMenuSnapshot(client: T3Client): { tableMenu: TableMenuView[] } {
  const open = menus.get(client);
  if (!open || open.threadId !== client.threadId) return { tableMenu: [] };
  const { threadId: _thread, ...view } = open;
  return { tableMenu: [view] };
}
