// Lane r8-keys: a Markdown table's Copy menu as a window-level popup.
//
// ChatMarkdown.tsx MarkdownTable (MIT, see LICENSE-T3) opens <MenuPopup
// align="end"> from the footer's Copy button: Base UI portals it above every
// layer (z-[130], over the provider banner), positions it below the trigger
// with a 4px side offset, flips it above when the window has no room below,
// and, as a modal menu, closes it on Escape or a press outside. The window's
// chatLocal reports the trigger's frame and the window size with
// `table-menu` (x|y|width|height|windowWidth|windowHeight|markdown`\u0000`csv).
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { num, obj } from './domain';

export interface TableMenuView {
  id: string; x: number; y: number; triggerX: number; triggerY: number; markdown: string; csv: string;
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
  // A Contract template keeps `\u0000` as its six characters (templates are raw).
  const [markdown = '', csv = ''] = parts.slice(6).join('|').split(/\\u0000|\u0000/);
  if (!id || !markdown || ![x, y, width, height, windowWidth, windowHeight].every(Number.isFinite) || width! <= 0 || height! <= 0) throw new ClientError('That table is unavailable.');
  const place = placeTableMenu({ x: x!, y: y!, width: width!, height: height! }, windowWidth!, windowHeight!);
  menus.set(client, { id, threadId: client.threadId, x: place.x, y: place.y, triggerX: x!, triggerY: y!, markdown, csv });
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
