// adopt-main-fixes-r7: main's LLP 1115 wave 1 (550aa871a) gives a custom button with a label, no `title` and no
// accessible text its label as AppKit's help tag (PresenterMac.propsChanged / settleTips). The reference is a web page:
// `aria-label` never shows a native tooltip there, and the clone's own tips would show beside one. So every such
// button says `title=""` (HTML's "no advisory text"). This guard applies the host's rule to the app's view tree:
//
// - a button is the `button` and `link` tags (both lower to Pressable, which the Apple host mounts as kind "button");
// - it is labelled by a non-empty `aria-label` or by `aria-labelledby` (Accessibility.swift `authoredLabel`);
// - its accessible text is NodeView.accessibleText: the text of its mounted children that are not `aria-hidden`. A
//   `when` arm is mounted only sometimes, an `each` may mount nothing, and an `aria-hidden` that is an expression may
//   hide; so a button has text for this guard only when some `text` is mounted on every arm.
//
// The walk inlines components from T3Code as view-depth.test.ts does (a use inlines its view; `children` is the use's
// body). A dynamic `text` value counts as text (an empty title would be a bug of its own).
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

type Line = { indent: number; text: string; at: string };
type Fill = { lines: Line[]; file: string; fill: Fill } | null;
/** `always`: some text is mounted whatever the state; `maybe`: some state mounts one. */
type Text = { always: boolean; maybe: boolean };
const NONE: Text = { always: false, maybe: false };
const or = (a: Text, b: Text): Text => ({ always: a.always || b.always, maybe: a.maybe || b.maybe });

/** The top-level `name=value` attributes of a view line and its positional value (after the tag), outside strings and brackets. */
function attributes(line: string): { tag: string; positional: string | null; attrs: Map<string, string> } {
  const tag = /^[A-Za-z][\w-]*/.exec(line)?.[0] ?? '';
  const attrs = new Map<string, string>();
  let positional: string | null = null;
  let p = tag.length;
  const value = () => {
    const start = p;
    let depth = 0;
    while (p < line.length) {
      const ch = line[p]!;
      if (ch === '"') { p++; while (p < line.length && line[p] !== '"') p += line[p] === '\\' ? 2 : 1; p++; continue; }
      if (ch === '`') {
        p++;
        let inner = 0;
        while (p < line.length && !(line[p] === '`' && inner === 0)) {
          if (line[p] === '\\') p++;
          else if (line.startsWith('${', p)) { inner++; p++; }
          else if (line[p] === '}' && inner > 0) inner--;
          p++;
        }
        p++;
        continue;
      }
      if ('([{'.includes(ch)) depth++;
      else if (')]}'.includes(ch)) depth--;
      else if (depth === 0 && /\s/.test(ch)) break;
      p++;
    }
    return line.slice(start, p);
  };
  for (;;) {
    while (p < line.length && /\s/.test(line[p]!)) p++;
    if (p >= line.length || line.startsWith('//', p)) break;
    const name = /^-?[A-Za-z_][\w-]*(:[\w-]+)?=(?!=)/.exec(line.slice(p));
    if (name) { p += name[0].length; attrs.set(name[0].slice(0, -1), value()); }
    else { const v = value(); if (positional === null) positional = v; }
  }
  return { tag, positional, attrs };
}

type Untitled = { at: string; label: string; text: 'never' | 'sometimes' };

/** Every labelled, untitled button in the view tree from `root` whose mounted children may give it no accessible text. */
function untitledLabelledButtons(dir: string, root = 'T3Code'): { found: Untitled[]; buttons: number; errors: string[] } {
  const errors: string[] = [];
  const files = new Map<string, { components: Map<string, { file: string; view: Line[] }>; uses: Map<string, string> }>();
  for (const file of readdirSync(dir).filter(name => name.endsWith('.contract') && !name.endsWith('.test.contract'))) {
    const components = new Map<string, { file: string; view: Line[] }>();
    const uses = new Map<string, string>();
    let component: { file: string; view: Line[] } | null = null;
    let section = '';
    let sectionIndent = 0;
    readFileSync(`${dir}/${file}`, 'utf8').split('\n').forEach((raw, index) => {
      const indent = raw.length - raw.trimStart().length;
      const text = raw.trim();
      if (text === '' || text.startsWith('//')) return;
      if (indent === 0) {
        component = null;
        section = '';
        const use = /^use (.+) from "\.\/(.+?)"/.exec(text);
        if (use) for (const name of use[1]!.split(',')) uses.set(name.trim(), use[2]!);
        const declared = /^component (\w+)/.exec(text);
        if (declared) components.set(declared[1]!, component = { file, view: [] });
        return;
      }
      const into = component as { file: string; view: Line[] } | null;
      if (!into) return;
      if (section && indent <= sectionIndent) section = '';
      if (section === 'view') into.view.push({ indent, text: text.replace(/\s+\/\/ .*$/, ''), at: `${file}:${index + 1}` });
      if (section) return;
      if (/^(view|props|state|inject|slot|action|resource|mutation|derive)\b/.test(text)) { section = text === 'view' ? 'view' : 'other'; sectionIndent = indent; }
    });
    files.set(file, { components, uses });
  }
  const resolve = (file: string, name: string): { file: string; view: Line[] } | null => {
    const scope = files.get(file);
    const from = scope?.uses.get(name);
    return scope?.components.get(name) ?? (from ? resolve(from, name) : null);
  };

  const found = new Map<string, Untitled>();
  let buttons = 0;
  const hidden = (attrs: Map<string, string>) => {
    const v = attrs.get('aria-hidden');
    return v === undefined || v === 'false' || v === '"false"' ? 'no' : v === 'true' || v === '"true"' ? 'yes' : 'maybe';
  };
  // A sibling list: the text its mounted lines give. A when chain gives text always only with an else and text on every arm.
  const block = (lines: Line[], file: string, fill: Fill): Text => {
    let result = NONE;
    let chain: Text[] | null = null;
    const close = (complete: boolean) => {
      if (chain) result = or(result, { always: complete && chain.every(arm => arm.always), maybe: chain.some(arm => arm.maybe) });
      chain = null;
    };
    for (let i = 0; i < lines.length;) {
      const head = lines[i]!;
      let end = i + 1;
      while (end < lines.length && lines[end]!.indent > head.indent) end++;
      const body = lines.slice(i + 1, end);
      const text = head.text;
      const when = /^(else )?when (.*)$/.exec(text);
      if (when) {
        if (!when[1]) close(false);
        (chain ??= []).push(block(body, file, fill));
      } else if (text === 'else') {
        (chain ??= []).push(block(body, file, fill));
        close(true);
      } else {
        close(false);
        if (/^each /.test(text)) result = or(result, { always: false, maybe: block(body, file, fill).maybe });
        else if (text === 'children') { if (fill) result = or(result, block(fill.lines, fill.file, fill.fill)); }
        else if (/^[A-Z]/.test(text)) {
          const name = /^\w+/.exec(text)![0];
          const used = resolve(file, name);
          if (!used) errors.push(`${head.at}: no component ${name}`);
          else result = or(result, block(used.view, used.file, body.length ? { lines: body, file, fill } : null));
        } else {
          const { tag, positional, attrs } = attributes(text);
          const inner = block(body, file, fill);
          const own: Text = (tag === 'text' || tag === 'tspan') && positional !== null && positional !== '""' ? { always: true, maybe: true } : inner;
          const shown = hidden(attrs);
          const mounted: Text = shown === 'yes' ? NONE : shown === 'maybe' ? { always: false, maybe: own.maybe } : own;
          if (tag === 'button' || tag === 'link') {
            buttons++;
            const label = attrs.get('aria-label');
            const labelled = (label !== undefined && label !== '""') || attrs.has('aria-labelledby');
            if (labelled && !attrs.has('title') && !inner.always)
              found.set(head.at, { at: head.at, label: label ?? attrs.get('aria-labelledby')!, text: inner.maybe ? 'sometimes' : 'never' });
          }
          result = or(result, mounted);
        }
      }
      i = end;
    }
    close(false);
    return result;
  };
  const top = files.get('app.contract')?.components.get(root);
  if (!top) throw new Error(`app.contract has no component ${root}`);
  block(top.view, 'app.contract', null);
  return { found: [...found.values()].sort((a, b) => a.at.localeCompare(b.at, 'en', { numeric: true })), buttons, errors };
}

describe('no AppKit help tag from aria-label (LLP 1115 wave 1)', () => {
  test('the attribute reader keeps strings, templates and calls whole', () => {
    const { tag, positional, attrs } = attributes('button press=f("a b", `x ${y + "z w"}`) aria-label=(a ? "b c" : `d ${e}`) title="" testId=`t-${id}`');
    expect(tag).toBe('button');
    expect(positional).toBeNull();
    expect([...attrs.keys()]).toEqual(['press', 'aria-label', 'title', 'testId']);
    expect(attrs.get('aria-label')).toBe('(a ? "b c" : `d ${e}`)');
    expect(attributes('text "Hello there" font-size="0.75rem"').positional).toBe('"Hello there"');
    expect(attributes('text label aria-hidden=true').attrs.get('aria-hidden')).toBe('true');
  });

  const { found, buttons, errors } = untitledLabelledButtons(import.meta.dir);
  test('the walk resolves every component use and reaches the app\'s buttons', () => {
    expect(errors).toEqual([]);
    expect(buttons).toBeGreaterThan(1000);
  });

  test('every labelled button whose mounted children may give no text says title=""', () => {
    const report = found.map(b => `${b.at} aria-label=${b.label} (text: ${b.text})`).join('\n');
    if (found.length) throw new Error(`${found.length} labelled buttons without title="" may show their label as a help tag:\n${report}`);
    expect(found).toEqual([]);
  });
});
