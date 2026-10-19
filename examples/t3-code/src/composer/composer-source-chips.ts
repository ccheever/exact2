// composer-editor-mentions.ts, composer-logic.ts and citation spacing at T3 Code 9c161462c9.
import { codeAt, composerMarkdown, type ComposerAtom, type ComposerBlock, type ComposerBlockOrigin } from './composer-markdown';
import type { ComposerRange } from './composer-markdown-inline';
import type { ComposerEdit } from './composer-markdown-edit';
import { assistantCitationLabel, collectAssistantCitations } from '../timeline/diff-citations';

type ChipSource = ComposerRange & { source: string; label: string };
export type ComposerChip = ChipSource & (
  | { kind: 'mention'; path: string }
  | { kind: 'skill'; name: string }
  | { kind: 'context'; contextKind: string; contextId: string; image: boolean }
  | { kind: 'citation'; href: string; comment: string | null; quote: string }
);
export type ComposerTrigger = ComposerRange & { kind: 'slash-command' | 'pull-request' | 'skill' | 'path'; query: string };
const basename = (path: string) => path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
const unescape = (text: string) => text.replace(/\\(.)/g, '$1');
const decode = (text: string) => { try { return decodeURIComponent(text); } catch { return text; } };
const overlaps = (a: ComposerRange, b: ComposerRange) => a.start < b.end && a.end > b.start;

export function composerChips(value: string): ComposerChip[] {
  const links: ComposerChip[] = [], tokens: ComposerChip[] = [];
  for (const match of value.matchAll(/(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/[^\s)]{1,200})\)/g)) {
    const href = match[3] ?? '', parts = href.slice('t3-context://v1/'.length).split('/'), kind = parts[0] ?? '', id = parts[1] ?? '';
    if (parts.length !== 2 || !/^[a-z][a-z0-9-]{0,39}$/.test(kind) || !/^[a-z0-9_-]{1,128}$/i.test(id)) continue;
    links.push({ kind: 'context', start: match.index, end: match.index + match[0].length, source: match[0], label: match[2] ?? '', contextKind: kind, contextId: id, image: match[1] === '!' });
  }
  for (const match of collectAssistantCitations(value)) {
    const { citation, source, start, end } = match;
    links.push({ kind: 'citation', start, end, source, href: source.slice('[Assistant quote]('.length, -1),
      quote: citation.text, comment: citation.comment ?? null, label: assistantCitationLabel(citation) });
  }
  for (const match of value.matchAll(/(^|\s)\[((?:\\.|[^\]\\]){0,512})\]\(([^)\s]+)\)(?=\s)/g)) {
    const label = unescape(match[2] ?? ''), path = decode(match[3] ?? '');
    if (!path || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(path) && !/^[A-Za-z]:[\\/]/.test(path) || label !== basename(path)) continue;
    const start = match.index + (match[1]?.length ?? 0), end = match.index + match[0].length;
    tokens.push({ kind: 'mention', start, end, source: value.slice(start, end), label, path });
  }
  for (const match of value.matchAll(/(^|\s)@(?:"((?:\\.|[^"\\])*)"|([^\s@"]+))(?=\s)/g)) {
    const quoted = match[2], path = quoted === undefined ? match[3] ?? '' : unescape(quoted);
    if (!path || quoted === undefined && /^[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._-]*(?:\/[^\s@"]+)*$/.test(path)) continue;
    const start = match.index + (match[1]?.length ?? 0), end = match.index + match[0].length;
    tokens.push({ kind: 'mention', start, end, source: value.slice(start, end), label: basename(path), path });
  }
  for (const match of value.matchAll(/(^|\s)\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)(?=\s)/gu)) {
    const start = match.index + (match[1]?.length ?? 0), end = match.index + match[0].length, name = match[2] ?? '';
    tokens.push({ kind: 'skill', start, end, source: value.slice(start, end), label: name, name });
  }
  const chips: ComposerChip[] = [];
  for (const chip of [...tokens.filter(token => !links.some(link => overlaps(token, link))), ...links].sort((a, b) => a.start - b.start)) {
    if ((chips.at(-1)?.end ?? 0) <= chip.start) chips.push(chip);
  }
  return chips;
}

/** Control references belong to source content, independently of field paint.
 *  Carried references need not have the delimiter that originally completed them. */
export function composerChipsInBlocks(value: string, blocks: readonly ComposerBlock[], chips: readonly ComposerChip[] = composerChips(value)): ComposerChip[] {
  return chips.filter(chip => Number.isInteger(chip.start) && Number.isInteger(chip.end)
    && chip.start >= 0 && chip.end > chip.start && chip.end <= value.length
    && value.slice(chip.start, chip.end) === chip.source
    && blocks.some(block => block.kind !== 'code' && block.kind !== 'rule'
      && chip.start >= block.contentStart && chip.end <= block.end));
}

/** Existing atoms keep their identity when an edit only changes adjacent text,
 *  including the delimiter that originally completed a mention or skill. */
export function reconcileComposerChips(previous: string, value: string, current: readonly ComposerChip[]): ComposerChip[] {
  let prefix = 0, suffix = 0;
  while (prefix < previous.length && prefix < value.length && previous[prefix] === value[prefix]) prefix++;
  while (suffix < previous.length - prefix && suffix < value.length - prefix
    && previous[previous.length - suffix - 1] === value[value.length - suffix - 1]) suffix++;
  const end = previous.length - suffix, delta = value.length - previous.length;
  const carried: ComposerChip[] = [];
  for (const chip of current) {
    if (previous.slice(chip.start, chip.end) !== chip.source
      || prefix < chip.end && end > chip.start || prefix > chip.start && prefix < chip.end) continue;
    const shift = end <= chip.start ? delta : 0, next = { ...chip, start: chip.start + shift, end: chip.end + shift };
    if (value.slice(next.start, next.end) === chip.source) carried.push(next);
  }
  return [...carried, ...composerChips(value).filter(chip => !carried.some(old => overlaps(chip, old)))].sort((a, b) => a.start - b.start);
}

const CHIP_STYLES: Readonly<Record<string, string>> = {
  mention: 'mention', skill: 'skill', citation: 'citation', thread: 'thread', 'review-comment': 'review-comment',
  terminal: 'terminal', image: 'image', file: 'file', folder: 'file', video: 'video', 'preview-annotation': 'preview-annotation',
  'pull-request': 'pull-request', 'pr-open': 'terminal', 'pr-draft': 'pr-draft', 'pr-merged': 'review-comment', 'pr-closed': 'image', invalid: 'invalid',
};
export function chipDecorations(chips: readonly ComposerChip[], labels: Readonly<Record<string, string>> = {}, faces?: readonly { kind: string; atomLabel: string }[]): ComposerAtom[] {
  return chips.map((chip, index) => {
    const face = faces?.[index], kind = face?.kind ?? (chip.kind === 'context' ? chip.contextKind : chip.kind);
    return { name: `cmp-chip-${CHIP_STYLES[kind] ?? 'neutral'}`, start: chip.start, end: chip.end,
      label: face ? face.atomLabel : chip.kind === 'skill' ? labels[chip.name] ?? chip.label : chip.label };
  });
}

export function composerTrigger(value: string, cursor: number, chips = composerChips(value), styling = true, blockOrigins?: readonly ComposerBlockOrigin[]): ComposerTrigger | null {
  if (!Number.isInteger(cursor) || cursor < 0 || cursor > value.length || chips.some(chip => cursor > chip.start && cursor < chip.end)) return null;
  if (styling && codeAt(composerMarkdown(value, { blockOrigins }).blocks, { start: cursor, end: cursor })) return null;
  const start = value.lastIndexOf('\n', cursor - 1) + 1, prefix = value.slice(start, cursor), slash = /^\/(\S*)$/.exec(prefix);
  if (slash) return { kind: 'slash-command', query: slash[1] ?? '', start, end: cursor };
  const token = /[^ \t\r\n]*$/.exec(prefix)?.[0] ?? '', tokenStart = cursor - token.length;
  const pr = /^#([\p{L}\p{N}][\p{L}\p{N}_-]*)?$/u.exec(token);
  if (pr) return { kind: 'pull-request', query: pr[1] ?? '', start: tokenStart, end: cursor };
  const skill = /^\p{Sc}/u.exec(token);
  if (skill) return { kind: 'skill', query: token.slice(skill[0].length), start: tokenStart, end: cursor };
  return token.startsWith('@') ? { kind: 'path', query: token.slice(1), start: tokenStart, end: cursor } : null;
}

export function pastedPathQueryLength(lineBefore: string, pasted: string, active: boolean, blockOrigins?: readonly ComposerBlockOrigin[]): number {
  if (!active || composerTrigger(lineBefore, lineBefore.length, undefined, true, blockOrigins)?.kind !== 'path') return 0;
  return /^\S*/.exec(pasted)?.[0].length ?? 0;
}

export type FreshCitation = ComposerRange & { value: string; source: string; spaces: { before: boolean; after: boolean } };
export function insertCitation(value: string, cursor: number, source: string): { edit: ComposerEdit; citation: FreshCitation } {
  const before = cursor > 0 && !/\s/.test(value[cursor - 1] ?? ''), end = value[cursor] === ' ' ? cursor + 1 : cursor;
  const text = `${before ? ' ' : ''}${source} `, next = value.slice(0, cursor) + text + value.slice(end), start = cursor + Number(before);
  return { edit: { start: cursor, end, text, selection: { start: cursor + text.length, end: cursor + text.length } },
    citation: { start, end: start + source.length, source, value: next, spaces: { before, after: end === cursor } } };
}

/** Surrounding edits move a fresh citation; an edit through its source ends cancellation ownership. */
export function moveFreshCitation(citation: FreshCitation, value: string): FreshCitation | null {
  if (value === citation.value) return citation;
  const old = citation.value;
  let prefix = 0, suffix = 0;
  while (prefix < old.length && prefix < value.length && old[prefix] === value[prefix]) prefix++;
  while (suffix < old.length - prefix && suffix < value.length - prefix && old[old.length - suffix - 1] === value[value.length - suffix - 1]) suffix++;
  const oldEnd = old.length - suffix;
  if (prefix < citation.end && oldEnd > citation.start || prefix > citation.start && prefix < citation.end) return null;
  const delta = oldEnd <= citation.start ? value.length - old.length : 0;
  const next = { ...citation, value, start: citation.start + delta, end: citation.end + delta };
  return value.slice(next.start, next.end) === citation.source ? next : null;
}
export function cancelFreshCitation(citation: FreshCitation, value: string): ComposerEdit | null {
  const moved = moveFreshCitation(citation, value);
  if (!moved) return null;
  const start = moved.start - Number(moved.spaces.before && value[moved.start - 1] === ' '), end = moved.end + Number(moved.spaces.after && value[moved.end] === ' ');
  return { start, end, text: '', selection: { start, end: start } };
}
