// The rename field's direction (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: SidebarThreadRow.tsx's `dir="auto"`):
// HTML's dir=auto takes the first strong letter's direction. Titles read in their majority direction through
// shared/text-direction.ts; this is the one rule it has no home for.
import { textDirection, type TextDirection } from '../shared/text-direction';

const LETTER = /\p{L}/u;

/** `dir="auto"`: the first letter's direction, undefined for text without letters. */
export function autoDirection(text: string): TextDirection | undefined {
  const letter = LETTER.exec(text)?.[0];
  return letter ? textDirection(letter) : undefined;
}
