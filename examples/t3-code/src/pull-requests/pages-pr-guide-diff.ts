// T3 Code 9c161462c9 (MIT, LICENSE-T3): GuideExcerpt, GuideFileDiff, useGuideDiffFiles.
import type { T3Client } from '../client/client';
import { obj, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { clampGuidePosition } from './pages-pr-guide-logic';
import { guideProgress, type GuideEntry, type GuideReference } from './pages-pr-guide';
import { excerptRows, fileDiffRows, rowsToPatch, letterStopNotes, excerptExpansionOldContents } from './pages-pr-guide-excerpts';
import { filesOf, presentCode, prCodeLocal } from './pages-pr-code';
import { codeRows, type CodeItem } from './pages-pr-code-rows';
import { renderablePatch, reviewPositionAnchor, type PullRequestReviewPosition } from './pages-pr-code-logic';
import { expandRange, type Expansion } from '../diff/diff-lazy';
import { pullRequestReviewKey, pullRequestReviewStore } from './pages-pr-writes-logic';
import type { WriteReference } from './pages-pr-writes';
import { hiddenCount } from './pages-pr-code-rows';

export type GuideDiffCard = { id: string; path: string; caption: string; range: string; removed: boolean; missing: boolean; full: boolean; note: string; open: boolean; expandable: boolean; shown: number; total: number; items: CodeItem[]; rowNotes: { key: string; row: string; marker: string; title: string }[] };
const cardStates = new WeakMap<GuideEntry, { open: Set<string>; expansions: Record<string, Expansion>; activeCard: string }>();
const localState = (entry: GuideEntry) => { let state = cardStates.get(entry); if (!state) { state = { open: new Set(), expansions: {}, activeCard: '' }; cardStates.set(entry, state); } return state; };
const contentsKey = (entry: GuideEntry, path: string) => `${entry.code?.key}@all::${path}`;
export function presentGuideDiff(ctx: { client: T3Client; reference: GuideReference; detail: Obj; now: number }, entry: GuideEntry) {
  const { client, reference, detail } = ctx;
  const presented = presentCode({ ...ctx, activity: null, state: entry.code ?? undefined }), cards: GuideDiffCard[] = [];
  const guide = entry.result.guide, state = localState(entry), code = entry.code;
  if (!guide || !code) return { ...presented, cards, canComment: false };
  const stop = guide.stops[clampGuidePosition(guideProgress(client, entry).position, guide.stops.length)];
  if (!stop) return { ...presented, cards, canComment: false };
  const numberedNotes = letterStopNotes(stop);
  const files = filesOf(code), pending = pullRequestReviewStore(client).pending(pullRequestReviewKey(reference));
  const capabilities = obj(detail.capabilities), canComment = obj(capabilities.review).inlineComment === true && obj(detail.viewerPermissions).comment === true;
  const canAsk = client.ready && client.scopes.includes('orchestration:operate');
  presented.view.canComment = canComment || canAsk; presented.view.draftOpen = !!code.draft && (canComment || canAsk);
  const make = (path: string, index: number, full: boolean, caption: string, note: string) => {
    const source = files.find(file => file.path === path), excerpt = stop.excerpts[index];
    const rows = source ? fileDiffRows(source) : [], sliced = !full && excerpt ? excerptRows(rows, excerpt) : rows;
    const parsed = source && (full || sliced.length) ? full ? { kind: 'files' as const, files: [source] } : renderablePatch(rowsToPatch(path, source.previous || path, sliced), false) : null;
    const id = `${guide.headSha}:${guideProgress(client, entry).position}:${full ? 'file' : 'excerpt'}:${index}`, file = parsed?.kind === 'files' ? parsed.files[0] : undefined;
    const card: GuideDiffCard = { id, path, caption, range: !full && excerpt ? `${excerpt.start}–${excerpt.end}` : '', removed: !full && excerpt?.side === 'old', missing: !file, full, note,
      open: !full || state.open.has(path), expandable: !full && new Set(sliced.map(row => row.hunk)).size === 1, shown: sliced.filter(row => row.kind !== 'context').length, total: rows.filter(row => row.kind !== 'context').length, items: [], rowNotes: [] };
    if (file && card.open) {
      const rawContents = code.contents[contentsKey(entry, path)];
      const oldContents = !full && rawContents?.state === 'loaded' ? excerptExpansionOldContents(rawContents.newContents, sliced) : null;
      const loaded = oldContents === null ? rawContents : { ...rawContents!, oldContents };
      const expansionPrefix = `${id}::${path}`;
      const draft = code.draft?.path === path && state.activeCard === id ? code.draft : null;
      const rendered = codeRows({ files: [file], layout: 'stacked', foldOverride: 'expanded', toggled: new Set(), viewedMark: () => '', omitted: new Map(),
        annotations: () => ({ threads: [], pending: pending.filter(item => item.path === path).map(item => ({ id: item.id, body: item.body, position: item.position as PullRequestReviewPosition })), draft: draft ? { position: draft.position, label: `${path}:${reviewPositionAnchor(draft.position).line}` } : null }),
        selection: draft ? { path, range: draft.range } : code.selection, contents: loaded ? { [expansionPrefix]: loaded } : {}, contentsKey: () => expansionPrefix, expansions: state.expansions });
      card.items = rendered.items.filter(item => item.kind !== 'pr-file' && item.kind !== 'pad' && (full || card.expandable || item.kind !== 'gap')).map(item => ({ ...item, id: `${id}:${item.id}` }));
      if (!full) {
        for (const item of card.items) {
          if (item.kind !== 'line') continue;
          const line = card.removed
            ? item.tone === 'deletion' ? item.line : item.tone === 'context' ? rows.find(row => row.newLine === item.line)?.oldLine : null
            : item.tone === 'deletion' ? null : item.line;
          for (const group of numberedNotes.byExcerpt[index] ?? []) if (group.line === line) for (const marker of group.markers)
            card.rowNotes.push({ key: `${item.id}:${marker.marker}`, row: item.id, marker: marker.marker, title: marker.title });
        }
      }
    }
    cards.push(card);
  };
  stop.excerpts.forEach((excerpt, index) => make(excerpt.path, index, false, excerpt.caption, ''));
  stop.files.forEach((file, index) => make(file.path, index, true, '', file.note ?? ''));
  return { ...presented, cards, canComment };
}
export async function guideDiffLocal(ctx: { client: T3Client; native: Native; reference: WriteReference; detail: Obj }, entry: GuideEntry, op: string, value: string): Promise<string> {
  const code = entry.code;
  if (!code) return '';
  if (op === 'begin' || op === 'row' || op === 'drag') {
    const bar = value.indexOf('|');
    if (bar < 0) return '';
    localState(entry).activeCard = value.slice(0, bar); value = value.slice(bar + 1);
  }
  if (op === 'file-toggle') { const state = localState(entry); if (state.open.has(value)) state.open.delete(value); else state.open.add(value); return ''; }
  if (op === 'excerpt-expand') {
    const [cardId = '', gapText = '', ...pathParts] = value.split('|'), path = pathParts.join('|'), gap = Number(gapText);
    const view = presentGuideDiff({ ...ctx, now: 0 }, entry), card = view.cards.find(item => item.id === cardId);
    const source = filesOf(code).find(file => file.path === path), guide = entry.result.guide;
    if (!card || !source || !guide || !Number.isInteger(gap) || (!card.full && !card.expandable)) return '';
    const stop = guide.stops[clampGuidePosition(guideProgress(ctx.client, entry).position, guide.stops.length)], index = Number(cardId.slice(cardId.lastIndexOf(':') + 1)), excerpt = stop?.excerpts[index];
    const rows = card.full ? fileDiffRows(source) : excerpt ? excerptRows(fileDiffRows(source), excerpt) : [];
    const parsed = renderablePatch(rowsToPatch(path, source.previous || path, rows), false), file = parsed?.kind === 'files' ? parsed.files[0] : undefined;
    const key = contentsKey(entry, path), loaded = code.contents[key], state = localState(entry), expansion = `${cardId}::${path}::${gap}`;
    if (file && loaded?.state === 'loaded') state.expansions[expansion] = expandRange(state.expansions[expansion], hiddenCount(file, gap, loaded));
    else if (file && loaded?.state !== 'error') { code.contentsDue.set(key, { path, gap }); state.expansions[expansion] = expandRange(state.expansions[expansion], 20); }
    return '';
  }
  return prCodeLocal({ ...ctx, state: code, allowAsk: ctx.client.ready && ctx.client.scopes.includes('orchestration:operate') }, op, value);
}
