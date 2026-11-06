// The Changes panel's and a pull request Code tab's one file tree width (T3 Code 5d3d78ab93, MIT, see LICENSE-T3:
// components/files/FileExplorerPane.tsx and hooks/useResizableWidth.ts, local storage "t3code:diff-file-tree-width"),
// kept in the client's preference file as `diffFileTreeWidth`. Until a width is chosen each list uses its responsive
// default (files-surface.contract treePaneWidth); the view clamps a saved width to the room it has.
import type { Obj } from '../client/domain';

type Holder = { local: object };
type Stored = { diffFileTreeWidth?: number };

/** The saved width, or -1 while none was chosen. */
export const diffTreeWidth = (client: Holder): number => (client.local as Stored).diffFileTreeWidth ?? -1;
/** Saves a width a drag ended on or a key chose ('' or a non-number leaves it). */
export function setDiffTreeWidth(client: Holder, value: string): void {
  const width = Number(value);
  if (value !== '' && Number.isFinite(width) && width > 0) (client.local as Stored).diffFileTreeWidth = Math.round(width);
}
/** load(): the saved width, if one was. */
export function adoptDiffTreeWidth(next: object, saved: Obj): void {
  const width = saved.diffFileTreeWidth;
  if (typeof width === 'number' && Number.isFinite(width) && width > 0) (next as Stored).diffFileTreeWidth = width;
}
