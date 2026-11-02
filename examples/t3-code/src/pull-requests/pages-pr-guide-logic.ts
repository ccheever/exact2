// T3 Code 9c161462c9 (MIT, LICENSE-T3): pullRequestReviewGuide.ts and guideRunStore.ts.
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError } from '../client/protocol';

export type GuideAttention = { kind: 'tradeoff' | 'contract' | 'product' | 'risk' | 'accessibility'; note: string };
export type GuideExcerpt = { path: string; side: 'new' | 'old'; start: number; end: number; caption: string };
export type GuideNote = { path: string; line: number; title: string; body: string };
export type GuideStop = { title: string; summary: string; attention: GuideAttention | null; excerpts: GuideExcerpt[]; files: { path: string; note?: string }[]; notes: GuideNote[]; questions: string[] };
export type ReviewGuide = { headSha: string; generatedAt: string; modelSelection: Obj; stops: GuideStop[] };
export type GuideRun = { _tag: 'Running'; headSha: string; startedAt: string } | { _tag: 'Failed'; headSha: string; failedAt: string; detail: string };
export type GuideResult = { guide: ReviewGuide | null; run: GuideRun | null; currentHeadSha: string | null };
export type GuideProgress = { position: number; seen: Set<number> };
export const noGuide = (): GuideResult => ({ guide: null, run: null, currentHeadSha: null });
const invalid = (): never => { throw new ClientError('The environment returned an invalid review guide.'); };
const text = (value: unknown) => typeof value === 'string' && value.trim() ? value : invalid();
const positive = (value: unknown) => typeof value === 'number' && Number.isSafeInteger(value) && value > 0 ? value : invalid();
const date = (value: unknown) => { const valueText = text(value); return Number.isFinite(Date.parse(valueText)) ? valueText : invalid(); };
function attention(value: unknown): GuideAttention | null {
  if (value === null) return null;
  const item = obj(value), kind = item.kind;
  if (kind !== 'tradeoff' && kind !== 'contract' && kind !== 'product' && kind !== 'risk' && kind !== 'accessibility') return invalid();
  return { kind, note: text(item.note) };
}
/** Decode once at the RPC boundary; malformed guide/run variants never enter reader state. */
export function decodeGuideResult(value: unknown): GuideResult {
  const result = obj(value), source = result.guide, rawRun = result.run;
  if (!('guide' in result) || !('run' in result) || !('currentHeadSha' in result)) return invalid();
  const guide: ReviewGuide | null = source === null ? null : (() => {
    const item = obj(source), model = obj(item.modelSelection);
    text(model.instanceId); text(model.model);
    if (!Array.isArray(item.stops)) return invalid();
    return { headSha: text(item.headSha), generatedAt: date(item.generatedAt), modelSelection: model, stops: arr(item.stops).map(stop => {
      if (![stop.excerpts, stop.files, stop.notes, stop.questions].every(Array.isArray)) return invalid();
      return { title: text(stop.title), summary: str(stop.summary), attention: attention(stop.attention), excerpts: arr(stop.excerpts).map(excerpt => {
        const side = excerpt.side;
        if (side !== 'new' && side !== 'old') return invalid();
        const start = positive(excerpt.start), end = positive(excerpt.end);
        if (end < start) return invalid();
        return { path: text(excerpt.path), side, start, end, caption: str(excerpt.caption) };
      }), files: arr(stop.files).map(file => ({ path: text(file.path), ...(typeof file.note === 'string' ? { note: file.note } : {}) })),
      notes: arr(stop.notes).map(note => ({ path: text(note.path), line: positive(note.line), title: text(note.title), body: str(note.body) })),
      questions: (Array.isArray(stop.questions) ? stop.questions : []).map(text) };
    }) };
  })();
  let run: GuideRun | null = null;
  if (rawRun !== null) {
    const item = obj(rawRun), headSha = text(item.headSha);
    if (item._tag === 'Running') run = { _tag: 'Running', headSha, startedAt: date(item.startedAt) };
    else if (item._tag === 'Failed') run = { _tag: 'Failed', headSha, failedAt: date(item.failedAt), detail: text(item.detail) };
    else return invalid();
  }
  return { guide, run, currentHeadSha: result.currentHeadSha === null ? null : text(result.currentHeadSha) };
}
export const guideRunKey = (environmentId: string, reference: { projectId: string; host?: string; repository: string; number: number }) =>
  JSON.stringify([environmentId, reference.projectId, reference.host ?? '', reference.repository, reference.number]);
export const guideProgressKey = (key: string, headSha: string) => `${key}:${headSha}`;
export function clampGuidePosition(position: number, count: number): number { return Math.max(0, Math.min(Math.max(0, count - 1), Math.trunc(position) || 0)); }
export function moveGuide(progress: GuideProgress, target: number, count: number, finish = false): GuideProgress {
  const position = clampGuidePosition(target, count), current = clampGuidePosition(progress.position, count), seen = new Set([...progress.seen].filter(index => index >= 0 && index < count));
  if (count > 0 && (position > current || finish)) seen.add(current);
  return { position, seen };
}
export const ATTENTION_LABELS = { tradeoff: 'A tradeoff to weigh', contract: 'A contract to check', product: 'A product call', risk: 'A risk to judge', accessibility: 'An accessibility call' };
export function guideState(result: GuideResult, starting = false) {
  const running = starting || result.run?._tag === 'Running';
  const failure = result.run?._tag === 'Failed' && result.run.headSha === result.currentHeadSha ? result.run.detail : '';
  return { running, failure, stale: !!result.guide && !!result.currentHeadSha && result.guide.headSha !== result.currentHeadSha,
    phase: result.guide ? 'ready' : running ? 'running' : failure ? 'failed' : 'empty' };
}
export type GuideWatch = { previousGuideAt: string | null; previousFailedAt: string | null; since: number; sawRunning: boolean };
export function settleGuideWatch(watch: GuideWatch, result: GuideResult, now: number): 'ready' | 'failed' | 'timeout' | null {
  if (result.run?._tag === 'Running') { watch.sawRunning = true; return null; }
  if (result.run?._tag === 'Failed' && result.run.failedAt !== watch.previousFailedAt) return 'failed';
  if (result.guide && result.guide.generatedAt !== watch.previousGuideAt) return 'ready';
  return watch.sawRunning || now - watch.since >= 60_000 ? 'timeout' : null;
}
