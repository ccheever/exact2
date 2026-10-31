import type { T3Client } from '../client/client';
import { chipContexts, pullRequestRecords, selectedProvider, threadContextRecords, workspaceCwd } from './composer-editor';
import { skillChipLabels, skillDisplayName } from './composer-editor-menu';
import { workspaceValues } from './composer-workspace-snapshots';
import { workspaceFileIcon } from '../shared/file-metadata';
import type { ComposerChip } from './composer-source-chips';
import { UNREADABLE_THREAD_TIP, unreadableThreads } from './composer-thread-machines';
import { projectGlyph, type ProjectGlyph } from '../sidebar/sidebar-glyph';
import { projectIdentity } from '../client/presentation';
import { obj, str } from '../client/domain';
import { terminologyForUrl } from '../details/git-logic';
import { chipInk, contextChipInks } from '../shared/chip-accents';
import { paintOf } from '../appearance/settings-appearance-look';
import { themeRoles } from '../appearance/settings-appearance';

export type ComposerChipFace = {
  kind: string; label: string; suffix: string; icon: string; fileToken: string;
  imageId: string; available: boolean; interactive: boolean; comment: boolean; tip: string;
  /** A thread chip's project icon (ThreadContextChip, T3 Code 74541bab17); kind '' when its project is unknown. */
  glyph: ProjectGlyph;
  /** The reference chip's `aria-label`, which names it where its drawn label carries the upload state (chipAccessibleName). */
  ariaLabel: string;
  /** Its ink in both halves of the appearance (shared/chip-accents.ts): a document's accent for a PDF, Word file, sheet,
   *  slides or EPUB, the error ink when invalid; '' (the field's text) for an unresolved chip or a kind without one. */
  ink: string;
};
const NO_GLYPH: ProjectGlyph = { kind: '', d: '', emoji: '', src: '', text: '', ink: '', surface: '', gray: '', graySurface: '' };
const icons: Readonly<Record<string, string>> = {
  mention: 'at-sign', skill: 'sparkles', citation: 'quote', thread: 'messages-square',
  terminal: 'square-terminal', 'review-comment': 'message-circle', 'preview-annotation': 'mouse-pointer-2',
  image: 'image', video: 'film', file: 'file', 'pr-open': 'git-pull-request', 'pr-draft': 'git-pull-request-draft',
  'pr-merged': 'git-merge', 'pr-closed': 'git-pull-request-closed',
};

/** Product metadata describes a source reference and its Contract controls.
 *  Context resolution reuses the existing records; path spelling never guesses a folder. */
export function composerChipFaces(client: T3Client, value: string, chips: readonly ComposerChip[], preview = false): ComposerChipFace[] {
  const contexts = preview ? {} : chipContexts(client, value);
  const skills = preview ? {} : skillChipLabels(workspaceValues(selectedProvider(client), workspaceCwd(client), 'skills'));
  const threadIds = new Map(preview ? [] : threadContextRecords(client, value).map(record => [str(record.contextId), str(record.threadId)]));
  // ComposerThreadContextChip: a thread on another machine draws invalid; this machine's agent cannot read it.
  const unreadable = new Set(preview ? [] : unreadableThreads(client, value).map(thread => thread.contextId));
  const threadOf = (contextId: string) => client.shell.threads.find(entry => entry.id === threadIds.get(contextId));
  const glyphOf = (contextId: string): ProjectGlyph => {
    const thread = threadOf(contextId);
    const project = thread ? client.shell.projects.find(entry => entry.id === thread.projectId) : undefined;
    return project ? projectGlyph(client, project, projectIdentity) : NO_GLYPH;
  };
  const pullRequests = new Map(preview ? [] : pullRequestRecords(client, value).map(record => [str(record.contextId), obj(record.pullRequest)]));
  const inks = chipInks(client);
  return chips.map(chip => {
    let kind: string = chip.kind, label = chip.label, suffix = '', imageId = '', tip = '', available = true, glyph = '', expired = false, title = '';
    if (chip.kind === 'skill') label = skills[chip.name] ?? skillDisplayName({ name: chip.name });
    if (chip.kind === 'citation') tip = chip.quote;
    if (chip.kind === 'mention') tip = chip.path;
    if (chip.kind === 'context') {
      const record = contexts[`${chip.contextKind}/${chip.contextId}`];
      const fields = record?.split('\t') ?? [];
      kind = fields[0] ?? 'neutral'; suffix = fields[1] ?? '';
      available = record !== undefined;
      if (kind === 'terminal' && !fields.slice(2).join('\t')) { available = false; expired = true; }
      if (kind.startsWith('pr-')) title = fields.slice(2).join('\t');
      if (kind === 'image') imageId = fields[2] ?? '';
      if (kind === 'file' && fields[2] === 'image') glyph = 'image'; // an attached SVG (fileChipContexts)
      if (!available) tip = kind === 'terminal' ? 'This terminal excerpt has expired.' : `Unavailable context, ${label}`;
      if (!available && chip.contextKind === 'thread' && unreadable.has(chip.contextId)) { kind = 'invalid'; available = true; tip = UNREADABLE_THREAD_TIP; }
    }
    const fileToken = chip.kind === 'mention' && !preview ? workspaceFileIcon(client, chip.path) : '';
    const icon = !available ? 'circle-dashed' : fileToken === 'folder' ? 'folder' : kind === 'invalid' ? icons.thread! : icons[glyph || kind] ?? 'circle-dashed';
    const interactive = available && (chip.kind === 'skill' || !preview && icons[kind] !== undefined), comment = !preview && chip.kind === 'citation';
    const ariaLabel = chipAccessibleName({ kind, label, suffix, available, expired, title, path: chip.kind === 'mention' ? chip.path : '',
      thread: chip.kind === 'context' && chip.contextKind === 'thread' ? str(threadOf(chip.contextId)?.title).trim() : '',
      pullRequest: chip.kind === 'context' ? pullRequests.get(chip.contextId) : undefined });
    const shownKind = !available && kind === 'terminal' ? 'invalid' : kind;
    // documentIconColorForFileName: a workspace mention by its path and an attached file (not a video) by its name.
    const document = chip.kind === 'mention' ? documentKind(chip.path) : chip.kind === 'context' && kind === 'file' ? documentKind(label) : '';
    const ink = shownKind === 'invalid' ? inks.invalid : !available ? '' : inks[document || shownKind] ?? '';
    return { kind: shownKind, label, suffix, icon, fileToken: fileToken === 'folder' ? '' : fileToken,
      imageId, available, interactive, comment, tip, glyph: chip.kind === 'context' && kind === 'thread' && available ? glyphOf(chip.contextId) : NO_GLYPH, ariaLabel, ink };
  });
}

/** The document kinds that take their own accent (documentIconColorForFileName), by a file name's extension. */
export function documentKind(name: string): string {
  const n = name.toLowerCase();
  return /\.pdf$/.test(n) ? 'doc-pdf' : /\.(docx?|odt|rtf)$/.test(n) ? 'doc-word' : /\.(xlsx?|ods|csv|tsv)$/.test(n) ? 'doc-spreadsheet'
    : /\.(pptx?|odp)$/.test(n) ? 'doc-slides' : /\.epub$/.test(n) ? 'doc-epub' : '';
}

/**
 * Every kind's ink as `light-dark()`, solved per theme half (contextChipVariables); kept while the themes stay. In a
 * sent message's bubble (`bubble`), a half whose bubble sits on the other side of the canvas's lightness draws its chips
 * in the bubble's own text colour (TimelineUserMessageRow's `--context-chip-message`).
 */
const inkCache = new WeakMap<object, { key: string; inks: Record<string, string>; bubble: Record<string, string> }>();
export function chipInks(client: T3Client, bubble = false): Record<string, string> {
  const { prefs, custom } = paintOf(client);
  // The status colours a palette leaves to the standard ones (STANDARD_STATUS_COLORS): the error foreground.
  const light = { errorForeground: '#c10007', ...themeRoles(prefs.themeLight, 'light', custom) }, dark = { errorForeground: '#ff6467', ...themeRoles(prefs.themeDark, 'dark', custom) };
  const key = JSON.stringify([light, dark]), cached = inkCache.get(client);
  if (cached?.key === key) return bubble ? cached.bubble : cached.inks;
  const a = contextChipInks(light), b = contextChipInks(dark);
  const table = (x: Record<string, string>, y: Record<string, string>) => Object.fromEntries(Object.keys(x).filter(kind => kind !== 'message').map(kind => [kind, chipInk(kind, x, y)]).filter(([, ink]) => ink));
  const inMessage = (half: Record<string, string>) => half.message ? Object.fromEntries(Object.keys(half).map(kind => [kind, half.message!])) : half;
  const entry = { key, inks: table(a, b), bubble: table(inMessage(a), inMessage(b)) };
  inkCache.set(client, entry);
  return bubble ? entry.bubble : entry.inks;
}

const capitalize = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** Each chip's `aria-label` at T3 Code 7c3b3b3605ef: ImageChipButton names an image by its name and size, never its upload
 *  state; FileChip a file or video as an attachment; ContextChipPopover's triggers (terminal, review comment, preview
 *  annotation, skill) end with ". Show details"; PullRequestChip, ThreadContextChip, ComposerMentionNodeView,
 *  AssistantCitationChip and UnresolvedChip each have their own. */
export function chipAccessibleName(chip: { kind: string; label: string; suffix: string; available: boolean; expired: boolean;
  title: string; path: string; thread: string; pullRequest: Record<string, unknown> | undefined }): string {
  const { kind, label, suffix: size } = chip;
  if (!chip.available && !chip.expired) return `Unavailable context, ${label}`;
  if (kind === 'image') return `Image attachment, ${label}, ${size}`;
  // An SVG draws as an image chip, still named as a file (SvgFileChip).
  if (kind === 'file') return `File attachment, ${label}, ${size}`;
  if (kind === 'video') return `Preview video attachment, ${label}, ${size}`;
  if (kind === 'terminal') return chip.expired ? `Terminal excerpt, ${label}, expired` : `Terminal excerpt, ${label}. Show details`;
  if (kind === 'review-comment') return `Review comment, ${label}. Show details`;
  if (kind === 'preview-annotation') return `Preview annotation, ${label}. Show details`;
  if (kind === 'skill') return `Skill ${label}. Show details`;
  if (kind === 'thread' || kind === 'invalid') return `Thread, ${chip.thread || label}`;
  if (kind === 'mention') return `Preview ${chip.path}`;
  if (kind === 'citation') return `View cited assistant text: ${label}`;
  if (kind.startsWith('pr-')) {
    // pullRequestContextKindLabel: the state, then the host's word for a change request.
    const state = kind.slice(3), term = terminologyForUrl(str(chip.pullRequest?.url)).singular;
    return `Open ${capitalize(state)} ${term} ${label}: ${chip.title}`;
  }
  return `Unavailable context, ${label}`;
}
