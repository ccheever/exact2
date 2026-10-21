// timelineMinimapItems.ts's markdown preview text at T3 Code 74541bab17 (MIT, see LICENSE-T3).
// Bundled for the app by gen-minimap-text.mjs.
import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import { unified } from 'unified';

const markdownPreviewParser = unified().use(remarkParse).use(remarkGfm).freeze();

const INLINE_PARENT_TYPES = new Set(['paragraph', 'heading', 'tableCell', 'emphasis', 'strong', 'delete', 'link', 'linkReference']);

function markdownNodeText(node) {
  if (node.type === 'html') {
    return node.value
      .replace(/<!--[\s\S]*?(?:-->|$)/g, '')
      .replace(/<\/?([a-z][\w-]*)(?:[^"'<>]|"[^"]*"|'[^']*')*>/gi, (_, tag) =>
        /^(?:address|article|aside|blockquote|br|dd|details|div|dl|dt|figcaption|figure|footer|h[1-6]|header|hr|li|main|nav|ol|p|pre|section|summary|table|tbody|td|th|thead|tr|ul)$/i.test(tag) ? ' ' : '');
  }
  if (node.type === 'break') return ' ';
  if ('value' in node) return node.value;
  if (node.type === 'image' || node.type === 'imageReference') return node.alt ?? '';
  if (!('children' in node)) return '';
  const separator = INLINE_PARENT_TYPES.has(node.type) ? '' : ' ';
  return node.children.map(markdownNodeText).join(separator);
}

/** resolveTimelineMinimapPreview's assistant text before compaction: the first 1,500 UTF-16 units as plain text. */
export function minimapPreviewText(markdown) {
  return markdownNodeText(markdownPreviewParser.parse(markdown.slice(0, 1_500)));
}
