// The dials panel's controls and the token grid's cells by kind (LLP 1121
// D4, D15; LLP 1122 D7; stage 6's kinds): what the panel draws for each dial
// from its declared metadata, and what a cell says of a token of each kind.
// Pure functions of `describe` and `resolve`; app.ts's resources use them.
import type { Result } from './app.contract.d.ts';

type Design = Result<'loadDesign'>;
type Control = Design['groups'][number]['dials'][number];

/** A number as Tamber writes one: a plain number, or a length's `Npx`; null
 * for anything else. */
export const numberOf = (v: string | undefined) => {
  const t = v?.trim().replace(/px$/i, '') ?? '';
  return t !== '' && Number.isFinite(Number(t)) ? Number(t) : null;
};
/** The decimal places a number is written to, an exponent counted:
 * `5e-3` is 3, `0.25` is 2, `12px` is 0. */
const decimals = (v: string | undefined) => {
  const m = /^[+-]?\d*(?:\.(\d+))?(?:e([+-]?\d+))?$/i.exec(v?.trim().replace(/px$/i, '') ?? '');
  return m ? Math.max(0, (m[1]?.length ?? 0) - Number(m[2] ?? 0)) : 0;
};
// CSS's named colours (CSS Color 4 §6.1), as color/src/named.rs has them.
const NAMED = new Map((
  'aliceblue:f0f8ff antiquewhite:faebd7 aqua:00ffff aquamarine:7fffd4 azure:f0ffff beige:f5f5dc ' +
  'bisque:ffe4c4 black:000000 blanchedalmond:ffebcd blue:0000ff blueviolet:8a2be2 brown:a52a2a ' +
  'burlywood:deb887 cadetblue:5f9ea0 chartreuse:7fff00 chocolate:d2691e coral:ff7f50 ' +
  'cornflowerblue:6495ed cornsilk:fff8dc crimson:dc143c cyan:00ffff darkblue:00008b darkcyan:008b8b ' +
  'darkgoldenrod:b8860b darkgray:a9a9a9 darkgreen:006400 darkgrey:a9a9a9 darkkhaki:bdb76b ' +
  'darkmagenta:8b008b darkolivegreen:556b2f darkorange:ff8c00 darkorchid:9932cc darkred:8b0000 ' +
  'darksalmon:e9967a darkseagreen:8fbc8f darkslateblue:483d8b darkslategray:2f4f4f ' +
  'darkslategrey:2f4f4f darkturquoise:00ced1 darkviolet:9400d3 deeppink:ff1493 deepskyblue:00bfff ' +
  'dimgray:696969 dimgrey:696969 dodgerblue:1e90ff firebrick:b22222 floralwhite:fffaf0 ' +
  'forestgreen:228b22 fuchsia:ff00ff gainsboro:dcdcdc ghostwhite:f8f8ff gold:ffd700 ' +
  'goldenrod:daa520 gray:808080 green:008000 greenyellow:adff2f grey:808080 honeydew:f0fff0 ' +
  'hotpink:ff69b4 indianred:cd5c5c indigo:4b0082 ivory:fffff0 khaki:f0e68c lavender:e6e6fa ' +
  'lavenderblush:fff0f5 lawngreen:7cfc00 lemonchiffon:fffacd lightblue:add8e6 lightcoral:f08080 ' +
  'lightcyan:e0ffff lightgoldenrodyellow:fafad2 lightgray:d3d3d3 lightgreen:90ee90 lightgrey:d3d3d3 ' +
  'lightpink:ffb6c1 lightsalmon:ffa07a lightseagreen:20b2aa lightskyblue:87cefa ' +
  'lightslategray:778899 lightslategrey:778899 lightsteelblue:b0c4de lightyellow:ffffe0 lime:00ff00 ' +
  'limegreen:32cd32 linen:faf0e6 magenta:ff00ff maroon:800000 mediumaquamarine:66cdaa ' +
  'mediumblue:0000cd mediumorchid:ba55d3 mediumpurple:9370db mediumseagreen:3cb371 ' +
  'mediumslateblue:7b68ee mediumspringgreen:00fa9a mediumturquoise:48d1cc mediumvioletred:c71585 ' +
  'midnightblue:191970 mintcream:f5fffa mistyrose:ffe4e1 moccasin:ffe4b5 navajowhite:ffdead ' +
  'navy:000080 oldlace:fdf5e6 olive:808000 olivedrab:6b8e23 orange:ffa500 orangered:ff4500 ' +
  'orchid:da70d6 palegoldenrod:eee8aa palegreen:98fb98 paleturquoise:afeeee palevioletred:db7093 ' +
  'papayawhip:ffefd5 peachpuff:ffdab9 peru:cd853f pink:ffc0cb plum:dda0dd powderblue:b0e0e6 ' +
  'purple:800080 rebeccapurple:663399 red:ff0000 rosybrown:bc8f8f royalblue:4169e1 ' +
  'saddlebrown:8b4513 salmon:fa8072 sandybrown:f4a460 seagreen:2e8b57 seashell:fff5ee sienna:a0522d ' +
  'silver:c0c0c0 skyblue:87ceeb slateblue:6a5acd slategray:708090 slategrey:708090 snow:fffafa ' +
  'springgreen:00ff7f steelblue:4682b4 tan:d2b48c teal:008080 thistle:d8bfd8 tomato:ff6347 ' +
  'turquoise:40e0d0 violet:ee82ee wheat:f5deb3 white:ffffff whitesmoke:f5f5f5 yellow:ffff00 ' +
  'yellowgreen:9acd32 '
).trim().split(' ').map(p => p.split(':') as [string, string]));

/** A colour written as hex, `rgb()` (numbers or percentages, commas or
 * spaces, any alpha) or a CSS name, folded as the server folds a dial's
 * colour (`#rrggbbaa`, lower case); another colour function as written,
 * its spaces tidied. */
export function folded(v: string): string {
  const t = v.trim().toLowerCase().replace(/\s+/g, ' ');
  const hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(t)?.[1];
  if (hex) {
    const full = hex.length <= 4 ? [...hex].map(c => c + c).join('') : hex;
    return `#${full}${full.length === 6 ? 'ff' : ''}`;
  }
  if (t === 'transparent') return '#00000000';
  if (NAMED.has(t)) return `#${NAMED.get(t)}ff`;
  const num = (x: string, of: number) => (x.endsWith('%') ? (Number(x.slice(0, -1)) / 100) * of : Number(x));
  const rgb = /^rgba?\( ?([\d.]+%?)[ ,]+([\d.]+%?)[ ,]+([\d.]+%?)(?: ?[,/] ?([\d.]+%?))? ?\)$/.exec(t);
  if (rgb) {
    const byte = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, '0');
    return `#${[rgb[1], rgb[2], rgb[3]].map(x => byte(num(x, 255))).join('')}${byte(num(rgb[4] ?? '1', 1) * 255)}`;
  }
  return t;
}
/** Whether `v` is written as a colour literal: a hex, a CSS name, or a
 * colour function's call (its arguments the server's to judge); a
 * token's name is not, whatever it is spelled. */
export const colourLiteral = (v: string, tokens: ReadonlySet<string>) => {
  const t = v.trim();
  if (tokens.has(t)) return false;
  return /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(t) || t.toLowerCase() === 'transparent' || NAMED.has(t.toLowerCase())
    || /^(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\([^()]*\)$/i.test(t);
};
/** Whether `v` is a dial's literal of `kind`: a colour's, a plain number
 * for a number (no unit), `0` or `Npx` for a length. */
const literalOf = (kind: string, v: string, tokens: ReadonlySet<string>) =>
  kind === 'color' ? colourLiteral(v, tokens)
  : kind === 'length' ? /^(0|[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?px)$/i.test(v.trim())
  : /^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(v.trim());

/** A dial's control from its declared metadata (LLP 1121 D4, D15): its
 * `control`, else its kind's own (a colour's field, a ranged number's
 * slider, else a stepper); `none` is not drawn. Metadata is kept, not
 * checked, by Tamber, so a control that does not fit the kind (a stepper
 * for a colour, a slider with no range, a select with no options) is the
 * kind's own, said in its help; a step or priority that is no number is
 * left at its default. `token` names a picker of tokens, which a dial (a
 * literal) has none of. A select's option that is no literal of the kind
 * (a token's name: `tokens` are the design's) is left out, said so, since
 * `set-dial` takes a literal. */
export function controlOf(d: { name: string; kind: string; value: string; range: [string, string] | null; meta: Record<string, string> }, tokens: ReadonlySet<string> = new Set(), takesTokens = false): Control | null {
  const m = d.meta ?? {};
  const own = d.kind === 'color' ? 'color' : d.range ? 'slider' : 'stepper';
  const declared = m.control;
  if (declared === 'none') return null;
  const written = (m.options ?? '').split('|').map(o => o.trim()).filter(Boolean);
  // (A param takes a token as well as a literal: LLP 1121 D15.)
  const literal = (o: string) => takesTokens || literalOf(d.kind, o, tokens);
  const options = written.filter(literal);
  const dropped = written.filter(o => !literal(o));
  const fits = declared === 'color' ? d.kind === 'color'
    : declared === 'slider' ? d.kind !== 'color' && !!d.range
    : declared === 'stepper' ? d.kind !== 'color'
    : declared === 'select' ? options.length > 0
    : false;
  const control = fits ? declared : own;
  // With no step, the finest the value and the range are written in, so
  // every declared value is a step (at least 0.01).
  const finest = 10 ** -Math.max(2, decimals(d.value), decimals(d.range?.[0]), decimals(d.range?.[1]));
  // A length's step is in px (LLP 1121 D15), written either way.
  const declaredStep = m.step !== undefined && numberOf(m.step) !== null && (d.kind === 'length' || !/px$/i.test(m.step)) ? numberOf(m.step) : null;
  const step = declaredStep !== null && declaredStep > 0 ? declaredStep : d.kind === 'number' ? finest : d.kind === 'length' ? 10 ** -Math.max(0, decimals(d.value), decimals(d.range?.[0]), decimals(d.range?.[1])) : 1;
  const priority = [1, 2, 3].includes(Number(m.priority)) ? Number(m.priority) : 2;
  // What a number from the slider or stepper is written with (LLP 1121 D15: a length dial is px).
  const unit = d.kind === 'length' ? 'px' : '';
  const said = [
    declared && declared !== 'token' && !fits ? `control=${declared} does not fit this ${d.kind} dial; its own control is shown.` : '',
    dropped.length ? `${dropped.join(', ')} ${dropped.length === 1 ? 'is' : 'are'} not a value a dial can take (a dial is a literal), so ${dropped.length === 1 ? 'it is' : 'they are'} not offered.` : '',
  ].filter(Boolean).join(' ');
  // A select's options as given, the current value matched by meaning (a
  // colour folded as the server folds it), else listed first.
  const same = (o: string) => o.trim() === d.value.trim() || (d.kind === 'color' ? folded(o) === folded(d.value) : numberOf(o) !== null && numberOf(o) === numberOf(d.value));
  const matched = options.find(same);
  return {
    name: d.name, kind: d.kind, control, label: m.label ?? d.name, help: [m.help ?? '', said].filter(Boolean).join(' '),
    value: control === 'select' ? matched ?? d.value : d.value,
    // With no range a stepper is bounded by nothing a person would type.
    number: numberOf(d.value) ?? 0, min: numberOf(d.range?.[0]) ?? -1e12, max: numberOf(d.range?.[1]) ?? 1e12, step, priority,
    // (A length's in px, as its value is written.)
    bounds: control === 'slider' || control === 'stepper' ? [d.range ? `${numberOf(d.range[0])}${unit} – ${numberOf(d.range[1])}${unit}` : '', `step ${step}${unit}`].filter(Boolean).join(' · ') : '',
    options: control === 'select' && !matched ? [d.value, ...options] : options,
    unit,
    // A colour's swatch: the literal itself.
    swatch: d.kind === 'color' ? d.value : '',
  };
}

/** A token's value in a cell, by its kind, from `resolve`'s tuple
 * (docs/tamber-protocol.md, tamber/1.5-1.9): a length or a number as the
 * server spells it, a shadow's text, a type's
 * longhands (each one it leaves unsaid named so), a family, a duration in
 * ms, an easing's curve. */
export function cellText(kind: string, r: Record<string, any>): string {
  switch (kind) {
    // (The server's spelling, `value`, never JavaScript's: 0.0000004, not 4e-7.)
    case 'length': case 'number': return String(r.value ?? '');
    case 'type': return [r.size ? `${r.size}${r.leading ? `/${r.leading}` : ''}` : 'size unsaid', r.weight != null ? `weight ${r.weight}` : 'weight unsaid', r.style ?? 'style unsaid', r.family ?? 'family unsaid'].join(' · ');
    case 'family': return String(r.family ?? r.value ?? '');
    case 'duration': return String(r.value ?? '').replace(/ms$/, ' ms');
    default: return String(r.value ?? '');
  }
}

/** The dials panel: groups in the order they first appear, each by
 * priority (1 first), priority 3 behind "more". */
export function panelOf(dials: Parameters<typeof controlOf>[0][], tokens: ReadonlySet<string> = new Set()): Design['groups'] {
  const groups = new Map<string, Control[]>();
  for (const d of dials) {
    const c = controlOf(d, tokens);
    if (!c) continue;
    const g = d.meta?.group ?? '';
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g)!.push(c);
  }
  return [...groups].map(([name, all]) => {
    const sorted = all.map((c, i) => [c, i] as const).sort((a, b) => a[0].priority - b[0].priority || a[1] - b[1]).map(([c]) => c);
    return { name, dials: sorted.filter(c => c.priority < 3), more: sorted.filter(c => c.priority === 3) };
  });
}
