// composer-editor-mentions.ts, composer-logic.ts and citation spacing at T3 Code 9c161462c9;
// composerInlineTokens.ts (decorators), composerTrigger.ts (path words) and the provider's skill
// chips (ComposerPromptEditorTiptap.tsx composerSkillMeta) at 74541bab17.
import { codeAt, composerMarkdown, type ComposerBlock, type ComposerBlockOrigin } from './composer-markdown';
import type { ComposerRange } from './composer-markdown';
import type { ComposerEdit } from './composer-markdown-edit';
import { assistantCitationLabel, parseAssistantCitationHref } from '../timeline/diff-citations';

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
/** A citation is known by its `t3-citation:` destination, whatever its label reads. The
 *  reference reads only `[Assistant quote]` (collectAssistantCitations): its chip is an atom
 *  whose label cannot change, where here the label is source text. The label holds no `[`, so
 *  an unclosed one before a citation stays text. Group 1 is the destination. */
export const COMPOSER_CITATION = /\[[^[\]\n]{0,512}\]\((t3-citation:\/\/v1\/[^\s)]+)\)/g;
/** COMPOSER_CITATION as the whole of `link`; its destination, or null. */
export function composerCitationHref(link: string): string | null {
  const match = new RegExp(`^${COMPOSER_CITATION.source}$`).exec(link);
  return match?.[1] ?? null;
}

/** Whether `$name` is a skill the provider offers to users, so it becomes a chip; null: every name. */
export type SkillOffer = ((name: string) => boolean) | null;

export function composerChips(value: string, skills: SkillOffer = null): ComposerChip[] {
  const links: ComposerChip[] = [], tokens: ComposerChip[] = [];
  for (const match of value.matchAll(/(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/[^\s)]{1,200})\)/g)) {
    const href = match[3] ?? '', parts = href.slice('t3-context://v1/'.length).split('/'), kind = parts[0] ?? '', id = parts[1] ?? '';
    if (parts.length !== 2 || !/^[a-z][a-z0-9-]{0,39}$/.test(kind) || !/^[a-z0-9_-]{1,128}$/i.test(id)) continue;
    links.push({ kind: 'context', start: match.index, end: match.index + match[0].length, source: match[0], label: match[2] ?? '', contextKind: kind, contextId: id, image: match[1] === '!' });
  }
  for (const match of value.matchAll(COMPOSER_CITATION)) {
    const href = match[1] ?? '', citation = parseAssistantCitationHref(href);
    if (!citation) continue;
    links.push({ kind: 'citation', start: match.index, end: match.index + match[0].length, source: match[0], href,
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
    // A scoped package (`@expo/ui`) and a decorator call (`@Component(…)`) are text unless quoted.
    if (!path || quoted === undefined && (/^[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._-]*(?:\/[^\s@"]+)*$/.test(path)
      || /^[\p{ID_Start}_$][\p{ID_Continue}.$\u200c\u200d]*\(/u.test(path))) continue;
    const start = match.index + (match[1]?.length ?? 0), end = match.index + match[0].length;
    tokens.push({ kind: 'mention', start, end, source: value.slice(start, end), label: basename(path), path });
  }
  for (const match of value.matchAll(/(^|\s)\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)(?=\s)/gu)) {
    const start = match.index + (match[1]?.length ?? 0), end = match.index + match[0].length, name = match[2] ?? '';
    // `$HOME` or another provider's skill stays editable text.
    if (skills && !skills(name)) continue;
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
export function reconcileComposerChips(previous: string, value: string, current: readonly ComposerChip[], skills: SkillOffer = null): ComposerChip[] {
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
  return [...carried, ...composerChips(value, skills).filter(chip => !carried.some(old => overlaps(chip, old)))].sort((a, b) => a.start - b.start);
}


/** The selection with each end that falls inside a reference moved out of it, as the
 *  reference's atom chips never hold the caret: a caret after its reference, a range's
 *  ends outward over the references they cut. */
export function outsideComposerChips(selection: ComposerRange, chips: readonly ComposerChip[]): ComposerRange {
  const within = (at: number) => chips.find(chip => at > chip.start && at < chip.end);
  if (selection.start === selection.end) {
    const caret = within(selection.start)?.end ?? selection.start;
    return { start: caret, end: caret };
  }
  return { start: within(selection.start)?.start ?? selection.start, end: within(selection.end)?.end ?? selection.end };
}

/** Backspace or Delete at a reference removes all of its source in one edit, as at the
 *  reference's atom chip; null when the key deletes no reference. */
export function composerChipDeletion(selection: ComposerRange, chips: readonly ComposerChip[], backward: boolean): ComposerEdit | null {
  let range = outsideComposerChips(selection, chips);
  if (selection.start === selection.end) {
    const at = selection.start, chip = chips.find(chip => backward ? at > chip.start && at <= chip.end : at >= chip.start && at < chip.end);
    if (!chip) return null;
    range = chip;
  } else if (range.start === selection.start && range.end === selection.end) return null;
  return { start: range.start, end: range.end, text: '', selection: { start: range.start, end: range.start } };
}

/** Option-Backspace or Option-Delete at a reference, or with only spaces between it and the
 *  caret, removes the reference and those spaces, as word deletion takes an atom chip whole. */
export function composerChipWordDeletion(value: string, selection: ComposerRange, chips: readonly ComposerChip[], backward: boolean): ComposerEdit | null {
  if (selection.start !== selection.end) return composerChipDeletion(selection, chips, backward);
  const at = selection.start, gap = (from: number, to: number) => /^[^\S\n]*$/.test(value.slice(from, to));
  const chip = chips.find(chip => at > chip.start && at < chip.end)
    ?? (backward ? chips.filter(chip => chip.end <= at && gap(chip.end, at)).at(-1) : chips.find(chip => chip.start >= at && gap(at, chip.start)));
  if (!chip) return null;
  const start = backward ? chip.start : Math.min(at, chip.start), end = backward ? Math.max(at, chip.end) : chip.end;
  return { start, end, text: '', selection: { start, end: start } };
}

const graphemes = typeof Intl !== 'undefined' && 'Segmenter' in Intl ? new Intl.Segmenter(undefined, { granularity: 'grapheme' }) : null;
/** What Backspace or Delete removes with no reference at the caret: the selection, or the one
 *  character beside the caret (a grapheme where the runtime segments them, else a code point). */
export function composerCharacterDeletion(value: string, selection: ComposerRange, backward: boolean): ComposerEdit | null {
  let { start, end } = selection;
  if (start === end) {
    const window = backward ? value.slice(Math.max(0, start - 64), start) : value.slice(start, start + 64);
    const parts = graphemes ? [...graphemes.segment(window)].map(part => part.segment) : Array.from(window);
    const step = (backward ? parts.at(-1) : parts[0])?.length ?? 0;
    if (!step) return null;
    if (backward) start -= step; else end += step;
  }
  return { start, end, text: '', selection: { start, end: start } };
}

/** What Option-Backspace or Option-Delete removes with no reference beside the caret: the
 *  selection, or the word and the spaces beside the caret. */
export function composerWordDeletion(value: string, selection: ComposerRange, backward: boolean): ComposerEdit | null {
  let { start, end } = selection;
  if (start === end) {
    const word = backward ? /[\p{L}\p{N}_]*[^\p{L}\p{N}_\n]*$/u.exec(value.slice(0, start))?.[0] ?? '' : /^[^\p{L}\p{N}_\n]*[\p{L}\p{N}_]*/u.exec(value.slice(end))?.[0] ?? '';
    if (!word) return composerCharacterDeletion(value, selection, backward);
    if (backward) start -= word.length; else end += word.length;
  }
  return { start, end, text: '', selection: { start, end: start } };
}

export function composerTrigger(value: string, cursor: number, chips = composerChips(value), styling = true, blockOrigins?: readonly ComposerBlockOrigin[]): ComposerTrigger | null {
  if (!Number.isInteger(cursor) || cursor < 0 || cursor > value.length || chips.some(chip => cursor > chip.start && cursor < chip.end)) return null;
  if (styling && codeAt(composerMarkdown(value, { blockOrigins }).blocks, { start: cursor, end: cursor })) return null;
  const start = value.lastIndexOf('\n', cursor - 1) + 1, prefix = value.slice(start, cursor), slash = /^\/(\S*)$/.exec(prefix);
  // A slash word with a second slash, such as `/tmp/build.sh`, is a path; the whole word counts, so a caret
  // before the second slash still sees it (isPathWord).
  if (slash) return /^\/\S*\//.test(value.slice(start)) ? null : { kind: 'slash-command', query: slash[1] ?? '', start, end: cursor };
  const token = /[^ \t\r\n]*$/.exec(prefix)?.[0] ?? '', tokenStart = cursor - token.length;
  const pr = /^#([\p{L}\p{N}][\p{L}\p{N}_-]*)?$/u.exec(token);
  if (pr) return { kind: 'pull-request', query: pr[1] ?? '', start: tokenStart, end: cursor };
  const skill = /^\p{Sc}/u.exec(token);
  if (skill) return { kind: 'skill', query: token.slice(skill[0].length), start: tokenStart, end: cursor };
  return token.startsWith('@') ? { kind: 'path', query: token.slice(1), start: tokenStart, end: cursor } : null;
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
