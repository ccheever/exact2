// Inline portions of shared/codexMarkdownDirectives.ts and codexFileCitations.ts
// at T3 Code 9c161462c9 (MIT, see LICENSE-T3). Bundled for the app by gen-codex-directives.mjs.
import { directiveFromMarkdown } from 'mdast-util-directive';
import { directive } from 'micromark-extension-directive';
import { markdownLineEnding, unicodePunctuation, unicodeWhitespace } from 'micromark-util-character';
import remarkParse from 'remark-parse';
import { unified } from 'unified';

const FILE = 'codex-file-citation', FOLLOW_UP = 'codex-followup';
function nameEnds(code) {
  return code === null || markdownLineEnding(code) || unicodeWhitespace(code)
    || (unicodePunctuation(code) && code !== 45 && code !== 95);
}
function nameGate(name) {
  return { partial: true, tokenize(effects, ok, nok) {
    let index = 0;
    return marker;
    function marker(code) {
      if (code !== 58) return nok(code);
      effects.enter('data'); effects.consume(code); return character;
    }
    function character(code) {
      if (code !== name.charCodeAt(index)) return nok(code);
      effects.consume(code); index++;
      return index === name.length ? afterName : character;
    }
    function afterName(code) { effects.exit('data'); return nameEnds(code) ? ok(code) : nok(code); }
  } };
}
function restricted(construct, name) {
  const gate = nameGate(name);
  return { ...construct, tokenize(effects, ok, nok) { return effects.check(gate, construct.tokenize.call(this, effects, ok, nok), nok); } };
}
function syntax() {
  const candidates = directive().text[58], construct = Array.isArray(candidates) ? candidates[0] : candidates;
  if (!construct) throw new Error('Missing inline directive construct');
  return { text: { 58: [restricted(construct, FILE), restricted(construct, FOLLOW_UP)] } };
}
function attach() {
  const data = this.data();
  (data.micromarkExtensions ??= []).push(syntax());
  (data.fromMarkdownExtensions ??= []).push(directiveFromMarkdown());
}
const parser = unified().use(remarkParse).use(attach).freeze();
const content = node => node.value ?? (node.children ?? []).map(content).join('');
function fileMarkdown(attributes) {
  const path = attributes?.path?.trim();
  if (!path) return null;
  const parsed = attributes.line_range_start?.trim() ? Number(attributes.line_range_start) : NaN;
  const start = Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined;
  const normalized = path.replaceAll('\\', '/').replace(/\/+$/, ''), label = normalized.slice(normalized.lastIndexOf('/') + 1) || normalized || 'File';
  const href = path.replaceAll('%', '%25').replaceAll('#', '%23').replaceAll('?', '%3F') + (start === undefined ? '' : `#L${start}`);
  const destination = href.replaceAll('\\', '%5C').replaceAll('<', '%3C').replaceAll('>', '%3E').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
  return `[${label.replace(/[\\[\]*_`<&]/g, '\\$&')}](<${destination}>)`;
}
/** Recognize only Codex's two text directives; code, malformed directives and unrelated syntax retain their source. */
export function renderCodexInlineDirectivesAsMarkdown(markdown) {
  if (!markdown.includes(`:${FILE}`) && !markdown.includes(`:${FOLLOW_UP}`)) return markdown;
  const replacements = [];
  function visit(node, insideLink = false) {
    const start = node.position?.start.offset, end = node.position?.end.offset;
    if (node.type === 'textDirective' && start !== undefined && end !== undefined) {
      if (node.name === FILE && !insideLink) {
        const replacement = fileMarkdown(node.attributes);
        if (replacement !== null) { replacements.push({ start, end, replacement }); return; }
      }
      if (node.name === FOLLOW_UP && node.attributes?.prompt?.trim() && content(node).trim()) {
        const a = node.children?.[0]?.position?.start.offset, b = node.children?.at(-1)?.position?.end.offset;
        if (a !== undefined && b !== undefined) { replacements.push({ start, end, replacement: markdown.slice(a, b) }); return; }
      }
    }
    for (const child of node.children ?? []) visit(child, insideLink || node.type === 'link' || node.type === 'linkReference');
  }
  visit(parser.parse(markdown));
  let rendered = markdown;
  for (const match of replacements.sort((a, b) => b.start - a.start)) rendered = rendered.slice(0, match.start) + match.replacement + rendered.slice(match.end);
  return rendered;
}
