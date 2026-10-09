// Timeline work rows and message actions adapted from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/contracts/src/orchestrationV2.ts (OrchestrationV2NotificationSource),
// packages/client-runtime/src/state/itemSupport.ts (resolveV2ItemSupport's provider session) and
// threadWorkflows.ts (canForkProjectedAssistantItem), work-log/toolPresentation.ts
// (extractToolActivityPresentation), MessagesTimeline.logic.ts (the work-toggle row's icons),
// V2ItemInspector.tsx (search results and file changes) and AgentElapsed.tsx (formatElapsedSeconds).
// Desktop audit 2026-10-09, task timeline-work-rows (TH-1, TH-2, TH-3, TH-5, TH-6).
import { arr, num, obj, str, type Obj } from './domain';

const isRecord = (value: unknown): value is Obj => value !== null && typeof value === 'object' && !Array.isArray(value);
const nonEmpty = (value: unknown): value is string => typeof value === 'string' && value.trim().length > 0;

// ---- TH-2: notification sources (OrchestrationV2NotificationSource) ----
const KNOWN_SOURCE_KINDS = new Set(['delegated_task', 'background_task', 'background_command', 'subagent', 'command', 'monitor']);

/**
 * Decodes a notification source as the reference's schema does. Servers store and send the shapes
 * older clients decode: a command is `background_command`, a subagent is `background_task` with
 * `work: "subagent"`. A kind this build does not know (or none) is generic background work. A known
 * kind whose fields do not decode is a defect, not a newer kind: `null` (the reference refuses it).
 */
export function decodeNotificationSource(value: unknown): Obj | null {
  if (!isRecord(value)) return null;
  const kind = value.kind;
  const child = (source: Obj, decoded: Obj): Obj | null => {
    if (source.childThreadId === undefined) return decoded;
    return nonEmpty(source.childThreadId) ? { ...decoded, childThreadId: source.childThreadId } : null;
  };
  switch (kind) {
    case 'delegated_task': {
      const taskIds = value.taskIds;
      if (!Array.isArray(taskIds) || !taskIds.every(nonEmpty)) return null;
      return child(value, { kind, taskIds: [...taskIds] });
    }
    case 'background_task':
      // `{ kind: background_task, work: subagent }` is a subagent; any other shape is the generic member.
      if (value.work === 'subagent') return child(value, { kind: 'subagent' }) ?? { kind: 'background_task' };
      return { kind: 'background_task' };
    case 'background_command': case 'command': return { kind: 'command' };
    case 'subagent': return child(value, { kind: 'subagent' });
    case 'monitor': return { kind: 'monitor' };
    default:
      if (kind !== undefined && typeof kind !== 'string') return null;
      return typeof kind === 'string' && KNOWN_SOURCE_KINDS.has(kind) ? null : { kind: 'background_task' };
  }
}

/** The decoded source a timeline row reads; a source the reference refuses reads as generic background work. */
export function notificationSource(value: unknown): Obj {
  return decodeNotificationSource(value) ?? { kind: 'background_task' };
}

// ---- TH-1: Fork from this response (AssistantForkButton) ----
/** resolveV2ItemSupport's provider session: the item's provider thread, then that thread's session. */
export function itemProviderSession(projection: Obj, item: Obj): Obj | null {
  const providerThreadId = item.providerThreadId;
  if (providerThreadId === null || providerThreadId === undefined) return null;
  const providerThread = arr(projection.providerThreads).find(candidate => candidate.id === providerThreadId);
  if (providerThread?.providerSessionId === null || providerThread?.providerSessionId === undefined) return null;
  return arr(projection.providerSessions).find(candidate => candidate.id === providerThread.providerSessionId) ?? null;
}

/**
 * canForkProjectedAssistantItem: a completed assistant message of a run. Without capability evidence
 * (historical and inherited rows) the server-side fallback stays available; a known incapable
 * provider is refused.
 */
export function canForkProjectedAssistantItem(item: Obj, capabilities?: Obj): boolean {
  if (item.type !== 'assistant_message' || item.runId === null || item.runId === undefined || item.status !== 'completed') return false;
  if (capabilities === undefined) return true;
  const threads = obj(capabilities.threads), identity = obj(capabilities.identity), context = obj(capabilities.context);
  const native = threads.canForkThread === true && threads.canForkFromTurn === true && identity.nativeThreadIds === 'strong';
  return native || context.supportsFullThreadHandoff === true;
}

/** AssistantForkButton: shown only where the reference shows it (`runId` set and the item forkable). */
export function assistantCanFork(projection: Obj, item: Obj): boolean {
  const session = itemProviderSession(projection, item);
  return canForkProjectedAssistantItem(item, session && isRecord(session.capabilities) ? session.capabilities : undefined);
}

// ---- TH-3: tool presentation (work-log/toolPresentation.ts extractToolActivityPresentation) ----
function trimmedString(value: unknown, max: number): string | undefined {
  if (typeof value !== 'string') return undefined;
  const trimmed = value.trim();
  return trimmed && trimmed.length <= max ? trimmed : undefined;
}
function parsedUrl(value: unknown, protocols: string[]): string | undefined {
  const raw = trimmedString(value, 4096);
  if (!raw) return undefined;
  try {
    const url = new URL(raw);
    return protocols.includes(url.protocol) ? url.href : undefined;
  } catch { return undefined; }
}
const imageUrl = (value: unknown) => parsedUrl(value, ['http:', 'https:', 'data:']);
const pageUrl = (value: unknown) => parsedUrl(value, ['http:', 'https:']);

function activityIcon(value: unknown): Obj | undefined {
  const icon = obj(value);
  if (icon._tag === 'website') {
    const page = pageUrl(icon.pageUrl), favicon = imageUrl(icon.faviconUrl), faviconDark = imageUrl(icon.faviconUrlDark);
    if (page) return { _tag: 'website', pageUrl: page, ...(favicon ? { faviconUrl: favicon } : {}), ...(faviconDark ? { faviconUrlDark: faviconDark } : {}) };
  }
  if (icon._tag === 'native-app') {
    const app = obj(icon.app), appId = trimmedString(app.appId, 512), displayName = trimmedString(app.displayName, 160);
    if (app._tag === 'app-id' && appId && /^[A-Za-z0-9._-]+$/u.test(appId)) return { _tag: 'native-app', app: { _tag: 'app-id', appId } };
    if (app._tag === 'display-name' && displayName) return { _tag: 'native-app', app: { _tag: 'display-name', displayName } };
  }
  if (icon._tag === 'themed-logo') {
    const logo = imageUrl(icon.logoUrl), logoDark = imageUrl(icon.logoUrlDark);
    if (logo) return { _tag: 'themed-logo', logoUrl: logo, ...(logoDark ? { logoUrlDark: logoDark } : {}) };
  }
  return undefined;
}
function activitySource(value: unknown): Obj | undefined {
  const source = obj(value), key = trimmedString(source.key, 512), name = trimmedString(source.name, 160);
  if (!key || !name || !['browser', 'computer', 'integration'].includes(str(source.kind))) return undefined;
  const icon = activityIcon(source.icon);
  return { key, name, kind: source.kind, ...(icon ? { icon } : {}) };
}
export interface ToolActivityPresentation { toolSurface?: 'browser' | 'computer'; toolIcon?: Obj; toolSource?: Obj }
export function extractToolActivityPresentation(item: Obj): ToolActivityPresentation {
  const surface = item.toolSurface === 'browser' || item.toolSurface === 'computer' ? item.toolSurface : undefined;
  const icon = activityIcon(item.toolIcon), source = activitySource(item.toolSource);
  return { ...(surface ? { toolSurface: surface } : {}), ...(icon ? { toolIcon: icon } : {}), ...(source ? { toolSource: source } : {}) };
}

/**
 * The work-toggle row's image and surface (deriveMessagesTimelineRows): the primary tool source's
 * icon (an entry of that source's own icon first), else the last entry's icon; the primary
 * source's surface, else the last entry's.
 */
export function groupToolPresentation(items: Obj[]): { toolIcon?: Obj; toolSurface?: string } {
  const presented = items.map(extractToolActivityPresentation);
  const primary = presented.find(entry => entry.toolSource !== undefined);
  const key = primary?.toolSource?.key;
  const primaryIcon = key ? presented.find(entry => entry.toolSource?.key === key && entry.toolIcon !== undefined)?.toolIcon ?? obj(primary?.toolSource).icon as Obj | undefined : undefined;
  const surface = primary?.toolSurface ?? [...presented].reverse().find(entry => entry.toolSurface !== undefined)?.toolSurface;
  const icon = primaryIcon ?? [...presented].reverse().find(entry => entry.toolIcon !== undefined)?.toolIcon;
  return { ...(icon ? { toolIcon: icon } : {}), ...(surface ? { toolSurface: surface } : {}) };
}

// ---- TH-5: V2ItemInspector's search results and file changes ----
export interface InspectorResult { id: string; kind: 'file' | 'web' | 'change'; label: string; href: string; preview: string }
export interface InspectorExtras { results: InspectorResult[]; changePath: string; stats: boolean; additions: number; deletions: number; diffRunId: string; diffPath: string }
export const NO_INSPECTOR_EXTRAS: InspectorExtras = { results: [], changePath: '', stats: false, additions: 0, deletions: 0, diffRunId: '', diffPath: '' };

/** resolveExternalWebLinkHref: an http(s) URL (a scheme-relative one as https), else ''. */
export function externalWebLinkHref(href: unknown): string {
  if (typeof href !== 'string' || !href) return '';
  try {
    const url = new URL(href.startsWith('//') ? `https:${href}` : href);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : '';
  } catch { return ''; }
}

/**
 * The inspector's structured parts. `relative` formats a path as filePathDisplay does. file_search:
 * each result's path, line and column over its preview. web_search: each result's title as a link
 * (with the external-link mark) over its snippet. file_change: the path, its +/- counts, Open diff
 * when the change belongs to a run, and each change of a multi-file edit.
 */
export function inspectorExtras(item: Obj, relative: (path: string) => string): InspectorExtras {
  switch (item.type) {
    case 'file_search': return { ...NO_INSPECTOR_EXTRAS, results: arr(item.results).map((result, index) => ({ id: `${index}`, kind: 'file',
      label: `${relative(str(result.fileName))}${result.line === undefined ? '' : `:${num(result.line)}`}${result.column === undefined ? '' : `:${num(result.column)}`}`,
      href: '', preview: str(result.preview) })) };
    case 'web_search': return { ...NO_INSPECTOR_EXTRAS, results: arr(item.results).map((result, index) => {
      const href = externalWebLinkHref(result.url);
      return { id: `${index}`, kind: 'web', href, label: str(result.title, str(result.url, href ? '' : 'Search result')), preview: str(result.snippet) };
    }) };
    case 'file_change': {
      const fileName = str(item.fileName);
      return { ...NO_INSPECTOR_EXTRAS, changePath: relative(fileName),
        stats: item.additions !== undefined || item.deletions !== undefined, additions: num(item.additions), deletions: num(item.deletions),
        diffRunId: str(item.runId), diffPath: fileName,
        results: arr(item.changes).map((change, index) => {
          const types = [str(change.fileType), str(change.mimeType)].filter(Boolean);
          return { id: `${index}`, kind: 'change', href: '', preview: '',
            label: `${str(change.operation)} ${str(change.oldPath) ? `${str(change.oldPath)} → ` : ''}${relative(str(change.path))}${types.length ? ` (${types.join(', ')})` : ''}` };
        }) };
    }
    default: return NO_INSPECTOR_EXTRAS;
  }
}

// ---- TH-6: the subagent card's elapsed time (AgentElapsed) ----
/** formatElapsedSeconds: "0s", "12s", "1m 05s", "1h 02m". */
export function formatElapsedSeconds(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(Number.isFinite(totalSeconds) ? totalSeconds : 0));
  const minutes = Math.floor(seconds / 60);
  if (minutes === 0) return `${seconds}s`;
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return `${minutes}m ${String(seconds % 60).padStart(2, '0')}s`;
  return `${hours}h ${String(minutes % 60).padStart(2, '0')}m`;
}
const ACTIVE = new Set(['pending', 'running', 'waiting']);
/** isOrchestrationV2WorkActive. */
export const workActive = (status: unknown) => ACTIVE.has(str(status));
/** deriveSubagentElapsedMs: live agents run to now, settled ones to completedAt; null without a start. */
export function subagentElapsedMs(agent: { status: unknown; startedAt: unknown; completedAt: unknown }, nowMs: number): number | null {
  if (typeof agent.startedAt !== 'string') return null;
  const end = workActive(agent.status) ? nowMs : typeof agent.completedAt === 'string' ? Date.parse(agent.completedAt) : null;
  if (end === null) return null;
  const start = Date.parse(agent.startedAt);
  return Number.isFinite(start) && Number.isFinite(end) ? Math.max(0, end - start) : null;
}
