// The rename field's direction (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: SidebarThreadRow.tsx's `dir="auto"`):
// HTML's dir=auto takes the direction of the first character whose bidi class is strong (UAX #9 P2: L, R or AL).
// Titles read in their majority direction through shared/text-direction.ts; this is the one rule it has no home for.
import { textDirection, type TextDirection } from '../shared/text-direction';

// The characters that can be strong: letters, letter numbers (Roman numerals are L), and the directional marks (LRM
// is L, RLM is R, ALM is AL). These modifier and tone letters are neutral (ON) despite their category.
const CANDIDATE = /[\p{L}\p{Nl}‎‏؜]/gu;
const NEUTRAL_LETTER = /[ʹʺ˂-ˏ˒-˟˥-˭˯-˿꜀-꜡ꞈ]/u;

/** `dir="auto"`: the first strong character's direction, undefined for text with none. */
export function autoDirection(text: string): TextDirection | undefined {
  for (const [char] of text.matchAll(CANDIDATE)) {
    if (NEUTRAL_LETTER.test(char)) continue;
    if (char === '‏' || char === '؜') return 'rtl';
    if (char === '‎') return 'ltr';
    // A letter reads right to left in the right-to-left blocks, as the shared rule reads it; a letter number is L.
    return textDirection(char) ?? 'ltr';
  }
  return undefined;
}
