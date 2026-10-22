// The window-space boxes the layouts read (the composer's drawer and overlay, its menus' rows, the chat canvas and the
// thread details card, the device tools), by name: [x, top, width, height] in points. app.contract measures them with
// `frame` whenever one of them changes size (`layoutFrames`) and hands the list to the sources that read it.
import type { Obj } from '../client/domain';

const frames = new WeakMap<object, { source: string; value: Obj }>();

/** Takes `name:x,y,width,height;…` (layout-frames.contract layoutFrame), keeping the parsed value while it is unchanged. */
export function noteLayoutFrames(client: object, source: unknown): void {
  const text = typeof source === 'string' ? source : '';
  if (frames.get(client)?.source === text) return;
  const value: Obj = {};
  for (const row of text.split(';')) {
    const at = row.indexOf(':');
    if (at <= 0) continue;
    const numbers = row.slice(at + 1).split(',').map(Number);
    if (numbers.length === 4 && numbers.every(Number.isFinite)) value[row.slice(0, at)] = numbers;
  }
  frames.set(client, { source: text, value });
}

/** The boxes last measured, by name; a box not laid out is absent. */
export function layoutFrames(client: object): Obj {
  return frames.get(client)?.value ?? {};
}
