// T3 Code 9c161462c9 (MIT, LICENSE-T3): useReviewGuideWriter, GuideTour and guideRunStore.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { can, requirePermission } from '../connections/command-permissions';
import { environmentClient } from '../settings/projects-fleet';
import { pushToast } from '../shell/toast';
import { hasDesktopNotifications, hasNotificationSound, type NotifyStatus } from '../shell/shell-notify';
import { environmentRequest, pullRequestServer } from './pages-pr-environments';
import type { WriteReference } from './pages-pr-writes';
import { createCodeState, filesOf, readCode, type CodeState } from './pages-pr-code';
import { pullRequestRefreshEpoch } from './pages-pr-refresh';
import { ATTENTION_LABELS, clampGuidePosition, decodeGuideResult, guideProgressKey, guideRunKey, guideState, moveGuide, noGuide, settleGuideWatch,
  type GuideProgress, type GuideResult, type GuideWatch } from './pages-pr-guide-logic';
import { stopFilePaths } from './pages-pr-guide-excerpts';

export type GuideReference = WriteReference & { environmentId?: string };
export type GuideContext = { client: T3Client; native: Native; reference: GuideReference; detail: Obj; now: number; tab: string; refresh: number; visible?: boolean };
export type GuideEntry = { key: string; environmentId: string; reference: GuideReference; title: string; result: GuideResult; error: string; due: boolean; starting: boolean; shown: string; readAt: number; refresh: number; epoch: number; finishNonce: number; code: CodeState | null };
type Watched = GuideWatch & { entry: GuideEntry; polledAt: number };
type Reader = { entries: Map<string, GuideEntry>; progress: Map<string, GuideProgress>; watches: Map<string, Watched>; shownKey: string };
const readers = new WeakMap<object, Reader>();
export function guideReader(client: object): Reader {
  let reader = readers.get(client);
  if (!reader) { reader = { entries: new Map(), progress: new Map(), watches: new Map(), shownKey: '' }; readers.set(client, reader); }
  return reader;
}
const plainRef = (reference: GuideReference): Obj => ({ projectId: reference.projectId, ...(reference.host ? { host: reference.host } : {}), repository: reference.repository, number: reference.number });
export function guideEntry(client: T3Client, reference: GuideReference): GuideEntry {
  const environmentId = pullRequestServer(client, reference.environmentId ?? '', plainRef(reference)) || client.environmentId;
  const key = guideRunKey(environmentId, reference), entries = guideReader(client).entries;
  let entry = entries.get(key);
  if (!entry) { entry = { key, environmentId, reference, title: '', result: noGuide(), error: '', due: true, starting: false, shown: '', readAt: 0, refresh: 0, epoch: 0, finishNonce: 0, code: null }; entries.set(key, entry); }
  return entry;
}
export function guidesEnabled(client: T3Client, reference: GuideReference): boolean {
  try { return obj(environmentClient(client, guideEntry(client, reference).environmentId).config.settings).reviewGuidesEnabled === true; }
  catch { return false; }
}
export function guideProgress(client: T3Client, entry: GuideEntry): GuideProgress {
  const key = guideProgressKey(entry.key, entry.result.guide?.headSha ?? '');
  return guideReader(client).progress.get(key) ?? { position: 0, seen: new Set() };
}
export function guideNavigate(client: T3Client, reference: GuideReference, target: number, finish = false): void {
  const entry = guideEntry(client, reference), guide = entry.result.guide;
  if (!guide) return;
  guideReader(client).progress.set(guideProgressKey(entry.key, guide.headSha), moveGuide(guideProgress(client, entry), target, guide.stops.length, finish));
}
const signature = (entry: GuideEntry) => JSON.stringify([entry.due, entry.starting, entry.result, entry.error]);
function watch(client: T3Client, entry: GuideEntry, now: number): void {
  const watches = guideReader(client).watches;
  if (!watches.has(entry.key)) watches.set(entry.key, { entry, previousGuideAt: entry.result.guide?.generatedAt ?? null,
    previousFailedAt: entry.result.run?._tag === 'Failed' ? entry.result.run.failedAt : null, since: now, sawRunning: entry.result.run?._tag === 'Running', polledAt: now });
}
/** Guide RPCs deliberately bypass cross-account routing: guides are persisted on the originating environment. */
async function fetchGuide(client: T3Client, native: Native, entry: GuideEntry, now: number): Promise<void> {
  try {
    entry.result = decodeGuideResult(await environmentRequest(client, native, entry.environmentId, 'pullRequests.reviewGuide', plainRef(entry.reference)));
    entry.error = ''; entry.readAt = now;
    if (entry.result.run?._tag === 'Running') watch(client, entry, now);
  } catch (error) { if (letGo(error)) throw error; entry.error = error instanceof Error ? error.message : 'Could not load the review guide.'; }
  entry.due = false;
}
export async function readGuide(ctx: GuideContext): Promise<'wake' | 'done'> {
  const { client, native, reference, detail, now } = ctx, reader = guideReader(client), entry = guideEntry(client, reference);
  if (reader.shownKey === entry.key && (ctx.tab !== 'guide' || ctx.visible === false)) reader.shownKey = '';
  if (!guidesEnabled(client, reference)) return 'done';
  entry.title = str(detail.title);
  if (ctx.tab === 'guide' && ctx.visible !== false) reader.shownKey = entry.key;
  if (entry.refresh !== ctx.refresh || entry.epoch !== pullRequestRefreshEpoch(client)) { entry.due = true; entry.refresh = ctx.refresh; entry.epoch = pullRequestRefreshEpoch(client); }
  if (entry.readAt && now - entry.readAt >= (entry.result.run?._tag === 'Running' ? 5_000 : 60_000)) entry.due = true;
  if (entry.due && entry.shown !== signature(entry)) { entry.shown = signature(entry); return 'wake'; }
  if (entry.due) await fetchGuide(client, native, entry, now);
  entry.shown = signature(entry);
  const guide = entry.result.guide;
  if (ctx.tab === 'guide' && guide) {
    const diffKey = `${client.environmentId}|guide:${entry.key}:${guide.headSha}`;
    if (!entry.code || entry.code.key !== diffKey) { entry.code = createCodeState(client, diffKey); entry.code.ignoreWhitespace = false; entry.code.viewed.due = false; entry.code.foldOverride = 'expanded'; }
    const result = await readCode({ ...ctx, environmentId: entry.environmentId, state: entry.code, tab: 'code', activity: null });
    if (result === 'wake') return 'wake';
    const stop = guide.stops[clampGuidePosition(guideProgress(client, entry).position, guide.stops.length)], wanted = stop ? stopFilePaths(stop) : [];
    const loaded = new Set(filesOf(entry.code).map(file => file.path)), next = entry.code.slices.at(-1)?.nextCursor;
    if (wanted.some(path => !loaded.has(path)) && next && !entry.code.diffDue && !entry.code.diffError) { entry.code.cursor = next; entry.code.diffDue = true; return 'wake'; }
  }
  return 'done';
}
export async function generateGuide(ctx: GuideContext): Promise<void> {
  const { client, native, reference, now } = ctx, entry = guideEntry(client, reference);
  if (!guidesEnabled(client, reference)) throw new ClientError('Enable review guides in Settings → Source Control.');
  const owner = environmentClient(client, entry.environmentId, native);
  requirePermission(owner.scopes, 'orchestration:operate');
  if (!owner.ready) throw new ClientError('Reconnect before writing a review guide.');
  if (entry.starting || entry.result.run?._tag === 'Running') return;
  watch(client, entry, now); entry.starting = true;
  try {
    entry.result = decodeGuideResult(await environmentRequest(client, native, entry.environmentId, 'pullRequests.generateReviewGuide', plainRef(reference), { write: true }));
    entry.error = ''; entry.readAt = now;
  } catch (error) {
    if (letGo(error)) throw error;
    guideReader(client).watches.delete(entry.key);
    const message = error instanceof Error ? error.message : 'Check the review guide model in Settings → Source Control.';
    pushToast(client, { kind: 'error', title: 'Could not start a review guide', description: message });
    entry.error = message;
  } finally { entry.starting = false; }
}
export function emptyGuideView() {
  return { enabled: false, phase: 'empty', error: '', running: false, canWrite: false, stale: false, failure: '', hasGuide: false, key: '', headSha: '', generatedAt: '',
    position: 0, stopCount: 0, readCount: 0, previous: false, next: false, nextTitle: '', title: '', summaryId: '', attention: '', attentionId: '',
    stops: [] as { key: string; index: number; number: string; title: string; subline: string; seen: boolean; current: boolean; attention: boolean }[],
    questions: [] as { key: string; text: string }[], notes: [] as { key: string; marker: string; title: string; bodyId: string; path: string; line: number }[],
    canAsk: false, canFinish: false, finishNonce: 0 };
}
export function presentGuide(client: T3Client, reference: GuideReference, detail: Obj) {
  const view = emptyGuideView(), entry = guideEntry(client, reference), result = entry.result, guide = result.guide;
  view.finishNonce = entry.finishNonce; view.enabled = guidesEnabled(client, reference); view.key = entry.key; Object.assign(view, guideState(result, entry.starting)); view.error = entry.error;
  if (!guide && entry.due && !view.running && !entry.error) view.phase = 'loading';
  try { view.canWrite = can(environmentClient(client, entry.environmentId), 'orchestration:operate'); } catch { /* unavailable owner */ }
  const capabilities = obj(detail.capabilities), permissions = obj(detail.viewerPermissions), review = obj(capabilities.review);
  view.canAsk = client.ready && can(client, 'orchestration:operate');
  const verdicts = Array.isArray(review.verdicts) ? review.verdicts : [], allowed = Array.isArray(permissions.verdicts) ? permissions.verdicts : [];
  view.canFinish = verdicts.some(verdict => ['comment', 'approve', 'request-changes'].includes(verdict as string) && allowed.includes(verdict)); // unknown verdicts are dropped
  const bodies: { id: string; kind: string; title: string; body: string }[] = [];
  if (!guide) return { view, bodies };
  const progress = guideProgress(client, entry), position = clampGuidePosition(progress.position, guide.stops.length), stop = guide.stops[position];
  Object.assign(view, { hasGuide: true, headSha: guide.headSha, generatedAt: guide.generatedAt, position, stopCount: guide.stops.length, readCount: progress.seen.size, previous: position > 0, next: position + 1 < guide.stops.length, nextTitle: guide.stops[position + 1]?.title ?? '' });
  view.stops = guide.stops.map((item, index) => ({ key: String(index), index, number: String(index + 1), title: item.title, subline: `${stopFilePaths(item).length} ${stopFilePaths(item).length === 1 ? 'file' : 'files'}`, seen: progress.seen.has(index), current: position === index, attention: item.attention !== null }));
  if (stop) {
    view.title = stop.title; view.summaryId = `guide:${entry.key}:${guide.headSha}:${position}:summary`;
    view.attention = stop.attention ? ATTENTION_LABELS[stop.attention.kind] : '';
    bodies.push({ id: view.summaryId, kind: 'assistant', title: '', body: stop.summary });
    if (stop.attention) {
      view.attentionId = `${view.summaryId}:attention`;
      bodies.push({ id: view.attentionId, kind: 'assistant', title: '', body: stop.attention.note });
    }
    view.questions = stop.questions.map((text, index) => ({ key: String(index), text }));
    view.notes = stop.notes.map((note, index) => { const bodyId = `${view.summaryId}:note:${index}`; bodies.push({ id: bodyId, kind: 'assistant', title: '', body: note.body }); return { key: String(index), marker: index < 26 ? String.fromCharCode(65 + index) : String(index + 1), title: note.title, bodyId, path: note.path, line: note.line }; });
  }
  return { view, bodies };
}
export const GUIDE_TARGET_PREFIX = 'guide:';
export function guideOpenTarget(entry: Pick<GuideEntry, 'environmentId' | 'reference'>): string { return GUIDE_TARGET_PREFIX + encodeURIComponent(JSON.stringify({ ...entry.reference, environmentId: entry.environmentId })); }
export function decodeGuideOpenTarget(target: string): string | null {
  if (!target.startsWith(GUIDE_TARGET_PREFIX)) return null;
  try { const reference = obj(JSON.parse(decodeURIComponent(target.slice(GUIDE_TARGET_PREFIX.length)))); return typeof reference.projectId === 'string' && typeof reference.repository === 'string' && typeof reference.number === 'number' && reference.number > 0 && typeof reference.environmentId === 'string' ? JSON.stringify(reference) : null; } catch { return null; }
}
/** Runs from the shell even while its pull-request panel is closed. */
export async function coordinateGuides(client: T3Client, native: Native, status: NotifyStatus, now: number): Promise<void> {
  const reader = guideReader(client), prefs = obj(client.local.clientSettings), mode = str(prefs.notificationMode, 'off');
  for (const [key, watched] of [...reader.watches]) {
    if (now - watched.polledAt < 5_000) continue;
    watched.polledAt = now;
    await fetchGuide(client, native, watched.entry, now);
    const settled = settleGuideWatch(watched, watched.entry.result, now);
    if (!settled) continue;
    reader.watches.delete(key);
    if (settled === 'timeout' || (status.active && reader.shownKey === key)) continue;
    const failure = settled === 'failed', title = failure ? 'Review guide failed' : 'Review guide ready', body = watched.entry.title || `${watched.entry.reference.repository} #${watched.entry.reference.number}`;
    if (hasNotificationSound(mode)) await bridgeReply(native, { op: 'notifySound', kind: failure ? 'input' : 'completion' }).catch(() => undefined);
    const target = guideOpenTarget(watched.entry);
    if (status.active && prefs.inAppNotificationsEnabled === true) pushToast(client, { kind: failure ? 'error' : 'success', title, description: failure && watched.entry.result.run?._tag === 'Failed' ? `${body}: ${watched.entry.result.run.detail}` : body, hideCopy: true, action: { label: 'Open guide', op: 'guide-open', id: JSON.stringify({ ...watched.entry.reference, environmentId: watched.entry.environmentId }) } });
    else if (!status.active && hasDesktopNotifications(mode)) await bridgeReply(native, { op: 'notifyPost', title, body, tag: `guide:${key}`, threadId: target }).catch(() => undefined);
  }
}
