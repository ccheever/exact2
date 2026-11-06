// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-timeline-chips.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Context chips in sent messages, adapted from T3 Code (MIT, see LICENSE-T3):
// MessagesTimeline.tsx userMessageContextPresentationRegistry (each
// `[label](t3-context://v1/<kind>/<id>)` resolved against the message's
// `context.records` and attachments), contextChipParts.tsx (FileChip,
// ImageChipButton, UnresolvedChip), ThreadContextChip.tsx and ChatMarkdown's
// MarkdownFileLink tooltip (the link's full path). Contract matches each chip
// to its parsed run by `href` (markdown.contract ChipView).
import { arr, obj, str, type Obj } from './domain';
import { fileIconToken } from './timeline-files';
import type { T3Client } from './client';
import { decodeClientPrefs, type ClientPrefs } from './settings-core';
import { fontStack } from './settings-appearance';
import { collectAssistantCitations, parseAssistantCitationHref } from './diff-citations';
import { markdownMediaChips } from './media-views'; // media-actions: a message's image lines and their actions

export interface ChipView {
  id: string; href: string; kind: string; label: string; size: string; tip: string;
  detail: string; icon: string; target: string; owner: string;
}
export interface MarkdownEnv { codeFont: string; codeSize: number; wrap: boolean; chips: ChipView[]; runCommands: string[] }

const CONTEXT_LINK = /(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/([a-z][a-z0-9-]{0,39})\/([a-z0-9_-]{1,128}))\)/gi;
// MARKDOWN_LINK_HREF_PATTERN, minus web and context links: what the parser marks as a file link.
const FILE_LINK = /(!?)\[([^\]\n]*)\]\(\s*(?:<([^>\n]+)>|([^\s)]+))(?:\s+["'][^"']*["'])?\s*\)/g;

/** formatAttachmentSize (client-runtime state/attachments.ts). */
export function attachmentSize(bytes: number): string {
  return bytes >= 1024 * 1024 ? `${(bytes / (1024 * 1024)).toFixed(1)} MB` : `${Math.max(1, Math.ceil(bytes / 1024))} KB`;
}
/** middleTruncateAttachmentName (composerInlineChip.ts): the start and the extension survive. */
export function middleTruncate(name: string, max = 36): string {
  const characters = Array.from(name);
  if (characters.length <= max) return name;
  const available = max - 1;
  const suffixLength = Math.min(available - 1, available >= 18 ? 14 : Math.ceil(available / 2));
  return `${characters.slice(0, available - suffixLength).join('')}…${characters.slice(-suffixLength).join('')}`;
}
const VIDEO = /\.(mp4|m4v|mov|webm|ogv|ogg)$/i;
const isVideo = (attachment: Obj) => /^video\//.test(str(attachment.mimeType)) || VIDEO.test(str(attachment.name));

/** resolvePathLinkTarget: a relative link joined to the workspace root, `.`/`..` folded, `#L3` as `:3`. */
export function fileLinkTarget(href: string, root: string): string {
  let target = href.trim();
  try { target = decodeURI(target); } catch { /* keep the raw spelling */ }
  target = target.replace(/^file:\/\//i, '').replace(/#L(\d+)(?:C(\d+))?$/i, (_all, line: string, column?: string) => `:${line}${column ? `:${column}` : ''}`);
  if (target.startsWith('~') || target.startsWith('/') || !root) return target;
  const parts: string[] = [];
  for (const part of `${root.replace(/\/+$/, '')}/${target}`.split('/')) {
    if (part === '.' || (part === '' && parts.length > 0)) continue;
    if (part === '..') { if (parts.length > 1) parts.pop(); continue; }
    parts.push(part);
  }
  return parts.join('/') || '/';
}
const isWebHref = (href: string) => /^(?:[a-z][a-z0-9+.-]*:|#)/i.test(href) && !/^file:/i.test(href);

/** Every chip a message's Markdown draws, by the href its run keeps (markdown.rs chat_run). */
export function messageChips(item: Obj, root: string, threads: Obj[], owner = ''): ChipView[] {
  const text = str(item.text), chips: ChipView[] = [];
  const records = new Map(arr(obj(item.context).records).map(record => [str(record.contextId), record]));
  const attachments = new Map(arr(item.attachments).map(attachment => [str(attachment.id), attachment]));
  const seen = new Set<string>();
  const add = (chip: Omit<ChipView, 'id' | 'owner'>) => { if (seen.has(chip.href)) return; seen.add(chip.href); chips.push({ id: `chip-${chips.length}`, ...chip, owner }); };
  if (text.includes('](t3-context:')) for (const match of text.matchAll(CONTEXT_LINK)) {
    const href = match[3]!, kind = match[4]!.toLowerCase(), label = match[2]!.trim() || kind;
    const record = records.get(match[5]!);
    add(contextChip(href, kind, label, record && str(record.kind) === kind ? record : undefined, attachments, threads));
  }
  if (text.includes('](t3-citation:')) for (const match of collectAssistantCitations(text)) add(citationChip(match.source.slice('[Assistant quote]('.length, -1)));
  if (text.includes('](')) for (const match of text.matchAll(FILE_LINK)) {
    const href = (match[3] ?? match[4] ?? '').trim();
    if (match[1] || !href || isWebHref(href) || /^(data|javascript|mailto|tel):/i.test(href)) continue;
    add({ href: `t3-file:${href}`, kind: 'link', label: match[2]!, size: '', tip: fileLinkTarget(href, root), detail: '', icon: fileIconToken(href), target: '' });
  }
  // media-actions: an image line's chip (kind "media") is matched by href and kind, so a link to the same file keeps its own.
  for (const media of markdownMediaChips(text, root)) chips.push({ id: `chip-${chips.length}`, ...media, owner });
  return chips;
}

/** AssistantCitationChip: the quote (or its comment) cut at 64 characters; "View source" opens the cited thread at the answer.
 * Each `[Assistant quote](t3-citation://…)` is read with the reference's parseAssistantCitationHref (diff-citations.ts). */
function citationChip(href: string): Omit<ChipView, 'id' | 'owner'> {
  const citation = parseAssistantCitationHref(href);
  const quote = (citation?.comment?.trim() || citation?.text || '').replace(/\s+/g, ' ');
  return { href, kind: 'citation', label: quote.length > 64 ? `${quote.slice(0, 64)}…` : quote, size: '', tip: 'View source', detail: citation?.messageId ?? '', icon: '', target: citation?.threadId ?? '' };
}
const UNAVAILABLE = 'This context is no longer available.';
function contextChip(href: string, kind: string, label: string, record: Obj | undefined, attachments: Map<string, Obj>, threads: Obj[]): Omit<ChipView, 'id' | 'owner'> {
  const base = { href, label, size: '', tip: '', detail: '', icon: '', target: '' };
  const unresolved = { ...base, kind: 'unresolved', tip: UNAVAILABLE };
  if (!record) return unresolved;
  switch (kind) {
    case 'mention': return { ...base, kind: 'mention', label: str(record.label, label), tip: str(record.path), icon: fileIconToken(str(record.path)) };
    case 'skill': return { ...base, kind: 'skill', label: str(record.label) || str(record.name, label), tip: `$${str(record.name)}` };
    case 'thread': {
      const threadId = str(record.threadId), shell = threads.find(thread => str(thread.id) === threadId);
      return { ...base, kind: 'thread', label: str(shell?.title).trim() || str(record.title, label), tip: shell ? 'Open thread' : 'Thread no longer available', target: shell ? threadId : '' };
    }
    case 'image': case 'file': {
      const attachment = attachments.get(str(record.attachmentId));
      if (!attachment || (kind === 'image' ? attachment.type !== 'image' : attachment.type !== 'file')) return unresolved;
      const name = str(record.name, str(attachment.name)), size = attachmentSize(Number(record.sizeBytes) || 0);
      if (kind === 'image') return { ...base, kind: 'image', label: middleTruncate(name), size, tip: `Image attachment, ${name}, ${size}`, target: str(attachment.id) };
      const video = isVideo(attachment);
      return { ...base, kind: video ? 'video' : 'file', label: middleTruncate(name), size, tip: `${name}\n${size}`, icon: video ? '' : fileIconToken(name), target: str(attachment.id) };
    }
    case 'terminal': return { ...base, kind: 'terminal', label: str(record.label, label), size: num(record.lineStart) === num(record.lineEnd) ? `Line ${num(record.lineStart)}` : `Lines ${num(record.lineStart)}–${num(record.lineEnd)}`,
      tip: str(record.terminalLabel), detail: str(record.text) };
    case 'element': return { ...base, kind: 'element', label: str(record.label, label), tip: str(record.pageTitle).trim() || str(record.pageUrl),
      detail: [str(record.selector) || `<${str(record.tagName)}>`, str(record.htmlPreview).trim()].filter(Boolean).join('\n') };
    case 'preview-annotation': return { ...base, kind: 'element', label: str(record.label, label), tip: str(record.pageTitle).trim() || str(record.pageUrl) || 'Preview annotation',
      detail: [str(record.comment), str(record.targetSummary)].filter(Boolean).join('\n') };
    case 'review-comment': return { ...base, kind: 'review', label: str(record.label, label), tip: str(record.filePath),
      detail: [`${str(record.sectionTitle)} · ${str(record.rangeLabel)}`, str(record.text).trim()].filter(Boolean).join('\n') };
    default: return unresolved;
  }
}
const num = (value: unknown) => typeof value === 'number' && Number.isFinite(value) ? value : 0;

/** Attachment ids a chip in the prose already shows: their standalone file rows are left out. */
export function chippedAttachmentIds(chips: ChipView[]): Set<string> {
  return new Set(chips.filter(chip => chip.kind === 'file' || chip.kind === 'video').map(chip => chip.target));
}

/** appearanceFonts.ts: the code family and clamped size (10–18, default 13) and the word-wrap default chat Markdown reads. */
export function markdownEnv(client: T3Client): MarkdownEnv {
  const prefs = (client.local as unknown as { clientSettings?: ClientPrefs } | undefined)?.clientSettings || decodeClientPrefs({});
  const size = Number(prefs.fontSizeCode);
  return { codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace', codeSize: Number.isFinite(size) ? Math.min(18, Math.max(10, Math.round(size))) : 13,
    wrap: prefs.wordWrap !== false, chips: [], runCommands: [] };
}
/** Settings → Appearance → Diff colors: "blue-orange" or the default "red-green". */
export function diffSchemeOf(client: { local: object }): string {
  return (client.local as unknown as { clientSettings?: ClientPrefs } | undefined)?.clientSettings?.diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green';
}

// color-mix(in oklab, …) for an image chip whose accent is its picture's average colour.
const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
function oklab(rgb: number[]): number[] {
  const [r, g, b] = rgb.map(value => toLinear(value / 255)) as [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b), m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b),
    s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function hexOf(lab: number[]): string {
  const [L, A, B] = lab as [number, number, number];
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3, m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3, s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  const rgb = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
  return `#${rgb.map(value => Math.round(Math.min(1, Math.max(0, toGamma(value))) * 255).toString(16).padStart(2, '0')).join('')}`;
}
const mix = (accent: number[], other: string, share: number) => {
  const a = oklab(accent), b = oklab([1, 3, 5].map(at => parseInt(other.slice(at, at + 2), 16)));
  return hexOf(a.map((value, index) => value * share + b[index]! * (1 - share)));
};
/** ContextChip's accent maths on an image's average colour ("r g b"): an 11% fill (17% hovered),
 * a border mixing 34% of it into the border colour, text mixing 22% into the foreground. */
export function imageChipInks(accent: string): { fill: string; hover: string; border: string; ink: string } | null {
  const rgb = accent.trim().split(/\s+/).map(Number);
  if (rgb.length !== 3 || rgb.some(value => !Number.isFinite(value) || value < 0 || value > 255)) return null;
  const hex = hexOf(oklab(rgb));
  return { fill: `${hex}1c`, hover: `${hex}2b`, border: `light-dark(${mix(rgb, '#e4e4e7', 0.34)}, ${hex}61)`, ink: `light-dark(${mix(rgb, '#27272a', 0.22)}, ${mix(rgb, '#f5f5f5', 0.22)})` };
}

// Ported from T3 Code 1e2ecbd975 SkillInlineText.tsx and providerSkills.ts (MIT).
// Exact renders parsed runs in Rust; this list supplies validated known names and
// display labels, without rewriting the message source used by Copy.
const SKILL_TOKEN_REGEX =
  /(^|\s)\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)(?=\s|$)/gu;
export interface MarkdownSkill { name: string; displayName: string }
export function formatProviderSkillDisplayName(skill: { name: string; displayName?: string }): string {
  const displayName = skill.displayName?.trim();
  if (displayName) return displayName;
  return skill.name.split(/[\s:_-]+/).filter(Boolean).map(word => word.charAt(0).toUpperCase() + word.slice(1)).join(' ');
}
export function markdownSkills(skills: readonly { name: string; displayName?: string }[]): MarkdownSkill[] {
  const seen = new Set<string>();
  return skills.flatMap(skill => {
    const matches = [...`$${skill.name}`.matchAll(SKILL_TOKEN_REGEX)];
    if (matches[0]?.[2] !== skill.name || seen.has(skill.name)) return [];
    seen.add(skill.name);
    return [{ name: skill.name, displayName: formatProviderSkillDisplayName(skill) }];
  });
}
