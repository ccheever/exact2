// apps/library-gate's cells (LLP 1139 §5 E), read as the gate and its test
// read them: the cells a component's variants need, the placements in the
// app's one view (comments set aside), and each slot's marker.
/** The cells a component needs: its first values, then each other value of each variant alone. */
export function cellsFor(variants) {
  const first = Object.fromEntries(variants.map(v => [v.prop, v.options[0]]));
  return [first, ...variants.flatMap(v => v.options.slice(1).map(o => ({ ...first, [v.prop]: o })))];
}

/** A prop value as the app writes it, as the IR spells an option. */
export const option = text => text?.trim().replace(/^"(.*)"$/, '$1');

/** A line without its comment (a `//` outside a string). */
export function uncommented(line) {
  let quote = false;
  for (let i = 0; i < line.length; i++) {
    if (line[i] === '"' && line[i - 1] !== '\\') quote = !quote;
    else if (!quote && line[i] === '/' && line[i + 1] === '/') return line.slice(0, i).trimEnd();
  }
  return line;
}

/** Every placement of `names` in `app`'s view, comments set aside, each with
 * its args, its id, and the lines it holds. */
export function placements(app, names) {
  const lines = app.split('\n').map(uncommented);
  const out = [];
  lines.forEach((line, i) => {
    const m = new RegExp(`^(\\s+)(${names.join('|')})\\((.*)\\)\\s*$`).exec(line);
    if (!m) {
      if (new RegExp(`^\\s+(${names.join('|')})\\b`).test(line)) throw new Error(`line ${i + 1}: a placement not on one line: ${line.trim()}`);
      return;
    }
    const [, indent, name, args] = m;
    const body = [];
    for (let j = i + 1; j < lines.length && (lines[j].trim() === '' || lines[j].length - lines[j].trimStart().length > indent.length); j++) body.push(lines[j]);
    const values = Object.fromEntries([...args.matchAll(/(?:^|, )(\w+)=("[^"]*"|[^,]+)/g)].map(([, k, v]) => [k, v.trim()]));
    out.push({ name, values, id: option(values.id), body, indent: indent.length });
  });
  return out;
}

/** Whether placement `p`'s slot holds its marker: a fixed-size pressable box
 * named for it as its first child, and the glyph as that box's child. */
export function markedSlot(p) {
  const [box, glyph] = p.body.filter(l => l.trim() !== '');
  const depth = l => l.length - l.trimStart().length;
  return !!box && !!glyph && depth(box) === p.indent + 2 && depth(glyph) === p.indent + 4
    && box.trim().startsWith(`box testId="marker-${p.id}" press=toggle width=24 height=24`) && glyph.trim() === 'text glyph';
}

/** The view's control flow, which could leave a cell unrendered. */
export const conditional = app => app.split('\n').filter(l => /^\s+(if|for|match)\b/.test(l));
