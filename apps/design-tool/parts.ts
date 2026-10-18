// The branch editor's typed fields (LLP 1122 D6; stage 6's kinds, step 3):
// a token's value cut into the parts a person edits (a length's amount and
// unit, a shadow's lengths, inset and colour, a type's longhands), with the
// source between them kept as written. Joined again, the parts are the value
// byte for byte, so the one line `set-token` writes changes only where a
// field did. The server judges what a field holds; this only finds where.
import type { Result } from './app.contract.d.ts';

type Part = Result<'loadParts'>['parts'][number];

// A CSS number, as Tamber's parser reads one: a sign, digits, a fraction,
// an exponent. With a unit after it, a dimension.
const NUMBER = '[+-]?(?:\\d+\\.?\\d*|\\.\\d+)(?:[eE][+-]?\\d+)?';
const numeric = new RegExp(`^${NUMBER}[a-zA-Z%]*$`);
// (A name's dots join names, as a token's do: `red.0` is a name and a number.)
const ITEM = new RegExp(`^(?:${NUMBER}[a-zA-Z%]*|#[0-9a-fA-F]+|-?[a-zA-Z_][\\w-]*(?:\\.[a-zA-Z_-][\\w-]*)*(?:\\(.*\\))?)$`, 's');
/** A name as Tamber reads a type token's whole value as one (typeface.rs's
 * `is_name`). */
const isName = (t: string) => t !== '' && !/[\s/,"']/.test(t) && !/^[\d.]/.test(t);

/** The value's top-level words and the exact text around them: parentheses
 * and quotes hold a word together, and a top-level comma is a word of its
 * own. `gaps` has one more entry than `words`; null when a parenthesis or a
 * quote is left open. */
export function wordsOf(v: string): { gaps: string[]; words: string[] } | null {
  const gaps = [''], words: string[] = [];
  let depth = 0, quote = '', word = '';
  const end = () => { if (word) { words.push(word); gaps.push(''); word = ''; } };
  for (const ch of v) {
    if (quote) { word += ch; if (ch === quote) quote = ''; continue; }
    if (depth === 0 && /\s/.test(ch)) { end(); gaps[gaps.length - 1] += ch; continue; }
    if (depth === 0 && ch === ',') { end(); words.push(','); gaps.push(''); continue; }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '(') depth++;
    else if (ch === ')' && --depth < 0) return null;
    word += ch;
  }
  if (quote || depth) return null;
  end();
  return { gaps, words };
}

const part = (index: number, key: string, label: string, control: string, text: string, more: Partial<Part> = {}): Part =>
  ({ index, key, label, control, text, pre: '', post: '', optional: false, needs: '', after: -1, options: [], ...more });
const fixed = (text: string) => part(0, '', '', 'fixed', text);
/** Each part its place in the list, as the contract edits one by it, and
 * `needs` (a later part's key) as that part's place: an optional part is
 * required while that one holds something (a blur, while a spread does). */
const numbered = (parts: Part[]) => parts.map((p, i) => ({ ...p, index: i, after: p.needs === '' ? -1 : parts.findIndex(q => q.key === p.needs) }));

/** The value joined from its parts: a fixed part as written, an optional one
 * left empty dropped with what it brought, every other with its own spacing.
 * (The contract's `partEdit` joins the same way.) */
export const joined = (parts: Part[], tail: string) =>
  parts.map(p => p.control === 'fixed' ? p.text : p.text === '' && p.optional ? '' : `${p.pre}${p.text}${p.post}`).join('') + tail;
/** The parts that must hold something and are empty: required, or an
 * optional one a later part holds the place after (the contract's
 * `partMissing` reads the same). */
export const missing = (parts: Part[]) =>
  parts.filter(p => p.control !== 'fixed' && p.text === '' && (!p.optional || (p.after >= 0 && parts[p.after].text !== '')));

/** A token's value as typed parts, or null where no field fits (an
 * expression, a `calc()`, a named length, a kind with no typed editor): the
 * value's own text field edits those. `kindOf` is a name's kind in the
 * design (a token's or a dial's), undefined for none. */
export function partsOf(kind: string, value: string, kindOf: (name: string) => string | undefined = () => undefined): { parts: Part[]; tail: string } | null {
  switch (kind) {
    case 'length': {
      const m = new RegExp(`^(\\s*)(${NUMBER})(px|rem|em)?(\\s*)$`).exec(value);
      if (!m) return null;
      const unit = m[3] ?? '';
      // A bare 0's unit is none; any other amount takes one.
      const units = unit === '' ? ['', 'px', 'rem', 'em'] : ['px', 'rem', 'em'];
      return { parts: numbered([fixed(m[1]), part(0, 'amount', 'amount', 'number', m[2]), part(0, 'unit', 'unit', 'select', unit, { optional: true, options: units })]), tail: m[4] };
    }
    case 'number': {
      const m = new RegExp(`^(\\s*)(${NUMBER})(\\s*)$`).exec(value);
      return m ? { parts: numbered([fixed(m[1]), part(0, 'amount', 'amount', 'number', m[2])]), tail: m[3] } : null;
    }
    case 'box-shadow': case 'text-shadow': return shadowParts(kind === 'box-shadow', value, kindOf);
    case 'type': return typeParts(value, kindOf);
    default: return null;
  }
}

/** A shadow list: per shadow, `inset` (a box shadow's), x, y, blur, spread
 * (a box shadow's) and its colour, each in the order written. A part the
 * shadow leaves out is offered empty where adding it reads right: blur after
 * the two offsets, spread only after a blur, a text shadow's colour at its end,
 * `inset` first. */
function shadowParts(box: boolean, value: string, kindOf: (name: string) => string | undefined): { parts: Part[]; tail: string } | null {
  const w = wordsOf(value);
  // Each word one CSS item (shadow.rs reads `2px#000` as two): a number or
  // dimension, a hex colour, a name, or a function call.
  if (!w || w.words.length === 0 || !w.words.every(x => x === ',' || ITEM.test(x))) return null;
  const items: number[][] = [[]];
  w.words.forEach((x, i) => (x === ',' ? items.push([]) : items[items.length - 1].push(i)));
  // A text shadow is one shadow (shadow.rs refuses a list).
  if (items.some(it => it.length === 0) || (!box && items.length > 1)) return null;
  const parts: Part[] = [fixed(w.gaps[0])];
  const names = ['x', 'y', 'blur', 'spread'];
  for (const [k, item] of items.entries()) {
    const of = items.length > 1 ? `shadow ${k + 1} ` : '';
    // As shadow.rs reads each word: `inset`; a number or dimension, or a
    // length token's or dial's name, a length (the server judges its unit);
    // anything else the colour.
    const role = item.map(i => (/^inset$/i.test(w.words[i]) ? 'inset' : numeric.test(w.words[i]) || kindOf(w.words[i]) === 'length' ? 'length' : 'color'));
    const lengths = role.filter(r => r === 'length').length;
    const insets = role.filter(r => r === 'inset').length, colours = role.filter(r => r === 'color').length;
    if (lengths < 2 || lengths > (box ? 4 : 3) || insets > (box ? 1 : 0) || colours > 1) return null;
    // The item's own parts, in source order; each word's gap after it is
    // fixed, but for an optional part's, which goes with it.
    const own: Part[] = [];
    let nth = 0;
    item.forEach((i, j) => {
      const r = role[j], gap = w.gaps[i + 1], last = j === item.length - 1;
      if (r === 'inset') {
        own.push(part(0, `${k}.inset`, `${of}inset`, 'inset', w.words[i], { optional: true, post: last ? '' : gap }));
        if (last) own.push(fixed(gap));
        return;
      }
      if (r === 'length') {
        const n = names[nth++];
        // Blur and spread may be emptied; blur holds its place while a
        // spread is written after it.
        const more = n === 'spread' || n === 'blur' ? { optional: true, ...(n === 'blur' && box ? { needs: `${k}.spread` } : {}) } : {};
        own.push(part(0, `${k}.${n}`, `${of}${n}`, 'text', w.words[i], more));
      } else {
        own.push(part(0, `${k}.color`, `${of}colour`, 'text', w.words[i], { optional: !box }));
      }
      own.push(fixed(gap));
    });
    // An optional part takes the gap before it, so emptying it leaves none.
    for (let j = 1; j < own.length; j++) {
      const p = own[j], before = own[j - 1];
      if (p.optional && p.post === '' && before.control === 'fixed' && /^\s+$/.test(before.text)) { own[j] = { ...p, pre: before.text }; own[j - 1] = fixed(''); }
    }
    // What the shadow leaves out, offered empty.
    const lastLength = own.map(p => p.key).lastIndexOf(`${k}.${names[lengths - 1]}`);
    if (lengths === 2) own.splice(lastLength + 1, 0, part(0, `${k}.blur`, `${of}blur`, 'text', '', { optional: true, pre: ' ', ...(box ? { needs: `${k}.spread` } : {}) }));
    else if (box && lengths === 3) own.splice(lastLength + 1, 0, part(0, `${k}.spread`, `${of}spread`, 'text', '', { optional: true, pre: ' ' }));
    if (colours === 0) {
      // At the shadow's end, before the gap after its last word.
      const at = own.length - 1;
      own.splice(at, 0, part(0, `${k}.color`, `${of}colour`, 'text', '', { optional: true, pre: ' ' }));
    }
    if (box && insets === 0) own.unshift(part(0, `${k}.inset`, `${of}inset`, 'inset', '', { optional: true, post: ' ' }));
    parts.push(...own);
    // The comma after it and the gap after that, as written.
    if (k < items.length - 1) parts.push(fixed(','), fixed(w.gaps[item[item.length - 1] + 2]));
  }
  // The last gap is the value's tail.
  const tail = parts[parts.length - 1].control === 'fixed' ? parts.pop()!.text : '';
  return { parts: numbered(parts), tail };
}

/** A type token, CSS `font` as Tamber takes it: `italic`? a weight? a size,
 * `/` a line height?, a family? Each it leaves out is offered empty where it
 * goes. The family is the rest of the value, one field. */
function typeParts(value: string, kindOf: (name: string) => string | undefined): { parts: Part[]; tail: string } | null {
  // Another token's name is its value: no fields (a text style is a size).
  if (isName(value.trim()) && kindOf(value.trim()) !== undefined) return null;
  const w = wordsOf(value);
  if (!w || w.words.length === 0) return null;
  const parts: Part[] = [fixed(w.gaps[0])];
  let i = 0;
  let style: Part | null = null, weight: Part | null = null;
  // As typeface.rs reads them: `italic` before any weight, then a weight
  // (`bold` or a number: the server judges it), each with the gap after it.
  for (; i < 2 && i < w.words.length; i++) {
    const x = w.words[i];
    // `normal` and `oblique` here are refused (typeface.rs): no field says them.
    if (x === 'normal' || x === 'oblique') return null;
    if (!style && !weight && x === 'italic') parts.push(style = part(0, 'style', 'style', 'style', x, { optional: true, post: w.gaps[i + 1] }));
    else if (!weight && (x === 'bold' || new RegExp(`^${NUMBER}$`).test(x))) parts.push(weight = part(0, 'weight', 'weight', 'text', x, { optional: true, post: w.gaps[i + 1] }));
    else break;
  }
  // The size's place: `normal` or `oblique` there is refused too.
  if (i >= w.words.length || w.words[i] === 'normal' || w.words[i] === 'oblique') return null;
  // The size, its line height after a `/` touching it or apart.
  const sizeWord = w.words[i];
  const slash = sizeWord.indexOf('/');
  let size: string, leading: Part | null = null, next: number;
  if (slash > 0) {
    size = sizeWord.slice(0, slash);
    const rest = sizeWord.slice(slash + 1);
    if (rest !== '') { leading = part(0, 'leading', 'line height', 'text', rest, { optional: true, pre: '/' }); next = i + 1; }
    else if (i + 1 < w.words.length) { leading = part(0, 'leading', 'line height', 'text', w.words[i + 1], { optional: true, pre: `/${w.gaps[i + 1]}` }); next = i + 2; }
    else return null;
  } else if (w.words[i + 1]?.startsWith('/')) {
    size = sizeWord;
    const x = w.words[i + 1];
    if (x.length > 1) { leading = part(0, 'leading', 'line height', 'text', x.slice(1), { optional: true, pre: `${w.gaps[i + 1]}/` }); next = i + 2; }
    else if (i + 2 < w.words.length) { leading = part(0, 'leading', 'line height', 'text', w.words[i + 2], { optional: true, pre: `${w.gaps[i + 1]}/${w.gaps[i + 2]}` }); next = i + 3; }
    else return null;
  } else { size = sizeWord; next = i + 1; }
  if (size === '' || size === ',' || size.includes('/')) return null;
  if (!style) parts.splice(1, 0, part(0, 'style', 'style', 'style', '', { optional: true, post: ' ' }));
  if (!weight) parts.push(part(0, 'weight', 'weight', 'text', '', { optional: true, post: ' ' }));
  parts.push(part(0, 'size', 'size', 'text', size));
  parts.push(leading ?? part(0, 'leading', 'line height', 'text', '', { optional: true, pre: '/' }));
  // The family: the rest, as written, with the gap before it.
  if (next < w.words.length) {
    let text = '';
    for (let j = next; j < w.words.length; j++) text += w.words[j] + (j < w.words.length - 1 ? w.gaps[j + 1] : '');
    parts.push(part(0, 'family', 'family', 'text', text, { optional: true, pre: w.gaps[next] }));
  } else {
    parts.push(part(0, 'family', 'family', 'text', '', { optional: true, pre: ' ' }));
  }
  const tail = w.gaps[w.words.length];
  return { parts: numbered(parts), tail };
}
