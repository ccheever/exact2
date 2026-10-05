// Projection and visibility rules adapted from T3 Code (MIT); see LICENSE-T3.
// Sources: packages/client-runtime/src/state/{shellReducer,orchestrationV2Projection,
// threadHistoryMerge}.ts and packages/shared/src/orchestrationV2Timeline.ts.
// Wire protocol reference: T3 4f7760e6, orchestration v2. No T3 runtime dependency.

export type Json = null | boolean | number | string | Json[] | Obj;
export interface Obj { [key: string]: Json | undefined }
export interface Shell { projects: Obj[]; threads: Obj[]; sequence: number }
export interface ThreadState {
  projection: Obj;
  sequence: number;
  historyCursor: string | null;
  hasMore: boolean;
  latestLocalTurnOrdinal: number | null;
}
export interface Activity { id: string; label: string; body: string; icon: string; output: string; result: string; failed: boolean; timestamp: string;
  tone?: string; ok?: boolean; reasoning?: boolean; expandable?: boolean; detail?: string; status?: string; targetId?: string; answer?: string; retryRunId?: string }
export interface Message { id: string; kind: string; title: string; body: string; checkpointId?: string; runId?: string; sourceThreadId?: string; completed?: boolean; createdAt?: string; activities?: Activity[]; files?: { path: string; additions: number; deletions: number }[]; folded?: Message[]; expanded?: boolean;
  icon?: string; tone?: string; failed?: boolean; live?: boolean; startedMs?: number; detail?: string; status?: string; groupId?: string; continues?: boolean;
  revert?: number; intent?: string; intentTip?: string; attribution?: string; targetId?: string; actionLabel?: string; copied?: number; copyFailed?: boolean;
  meta?: boolean; streaming?: boolean; actionsId?: string; collapsible?: boolean;
  code?: { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }[];
  diagrams?: import('./timeline-mermaid').MermaidDiagramView[];
  setup?: import('./timeline-worktree').SetupView[];
  images?: { id: string; name: string; snapshot: boolean; appName: string; appInitial: string; windowTitle: string; accessible: boolean; video: boolean }[];
  attachFiles?: { id: string; name: string; icon: string }[];
  chips?: import('./r4-timeline-chips').ChipView[] }

function isObject(value: unknown): value is Obj {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
export function obj(value: unknown): Obj { return isObject(value) ? value : {}; }
export function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}
export function num(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}
export function arr(value: unknown): Obj[] {
  return Array.isArray(value) ? value.filter(isObject) : [];
}

function invalid(path: string): never { throw new Error(`Invalid T3 data: ${path}`); }
function object(value: unknown, path: string): Obj {
  return isObject(value) ? value : invalid(`${path} must be an object`);
}
function string(value: unknown, path: string, allowEmpty = false): string {
  return typeof value === "string" && (allowEmpty || value.length > 0)
    ? value : invalid(`${path} must be a string`);
}
function integer(value: unknown, path: string): number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value : invalid(`${path} must be a nonnegative integer`);
}
function boolean(value: unknown, path: string): boolean {
  return typeof value === "boolean" ? value : invalid(`${path} must be a boolean`);
}
function nullableString(value: unknown, path: string): string | null {
  return value === null ? null : string(value, path);
}
function objects(value: unknown, path: string): Obj[] {
  if (!Array.isArray(value)) return invalid(`${path} must be an array`);
  return value.map((item, index) => object(item, `${path}[${index}]`));
}
function entity(value: unknown, path: string): Obj {
  const result = object(value, path);
  string(result.id, `${path}.id`);
  return result;
}
function oneOf(value: unknown, choices: string[], path: string): string {
  const result = string(value, path);
  return choices.includes(result) ? result : invalid(`${path} has an unknown value`);
}
function optionalFields(value: Obj, fields: string[], kind: "string" | "boolean" | "number", path: string) {
  for (const field of fields) {
    const item = value[field];
    if (item === undefined || item === null) continue;
    if (kind === "string") string(item, `${path}.${field}`, true);
    else if (kind === "boolean") boolean(item, `${path}.${field}`);
    else integer(item, `${path}.${field}`);
  }
}

const projectionArrays = [
  "runs", "attempts", "nodes", "subagents", "providerSessions", "providerThreads",
  "providerTurns", "runtimeRequests", "messages", "plans", "turnItems",
  "checkpointScopes", "checkpoints", "contextHandoffs", "contextTransfers", "visibleTurnItems",
];
const threadEvents = new Set([
  "thread.created", "thread.archived", "thread.unarchived", "thread.deleted",
  "thread.settled", "thread.unsettled", "thread.snoozed", "thread.unsnoozed",
  "thread.auto-settle-set", "thread.pinned", "thread.unpinned", "thread.pin-reordered",
  "thread.active-reordered", "thread.metadata-updated", "thread.pull-request-synced",
  "thread.runtime-mode-updated", "thread.interaction-mode-updated",
  "thread.model-selection-updated", "thread.provider-switched", "thread.visited", "thread.marked-unread",
]);
const updateArrays: Record<string, string> = {
  "run.created": "runs", "run.updated": "runs",
  "run-attempt.created": "attempts", "run-attempt.updated": "attempts",
  "node.updated": "nodes", "subagent.updated": "subagents",
  "provider-session.attached": "providerSessions", "provider-session.updated": "providerSessions",
  "provider-thread.updated": "providerThreads", "provider-turn.updated": "providerTurns",
  "runtime-request.updated": "runtimeRequests", "message.updated": "messages",
  "plan.updated": "plans", "turn-item.updated": "turnItems",
  "checkpoint-scope.created": "checkpointScopes", "checkpoint.captured": "checkpoints",
  "context-handoff.updated": "contextHandoffs", "context-transfer.created": "contextTransfers",
  "context-transfer.updated": "contextTransfers",
};
const otherEvents = new Set([
  "provider-session.detached", "run.background-work-cancelled", "checkpoint.rollback-requested",
]);

function validateTurnItem(value: unknown, path: string): Obj {
  const item = entity(value, path);
  string(item.threadId, `${path}.threadId`);
  nullableString(item.runId, `${path}.runId`);
  nullableString(item.nodeId, `${path}.nodeId`);
  integer(item.ordinal, `${path}.ordinal`);
  oneOf(item.status, ["idle", "pending", "running", "waiting", "completed", "failed", "cancelled", "interrupted"], `${path}.status`);
  const type = oneOf(item.type, [
    "notification", "user_message", "assistant_message", "reasoning", "proposed_plan", "todo_list",
    "user_input_request", "file_change", "command_execution", "file_search", "web_search", "approval_request",
    "checkpoint", "run_interrupt_request", "run_interrupt_result", "system_notice", "error", "compaction",
    "handoff", "fork", "thread_created", "subagent", "dynamic_tool",
  ], `${path}.type`);
  if (["user_message", "assistant_message", "reasoning"].includes(type)) string(item.text, `${path}.text`, true);
  if (["assistant_message", "reasoning", "proposed_plan"].includes(type)) boolean(item.streaming, `${path}.streaming`);
  if (["user_message", "assistant_message"].includes(type)) string(item.messageId, `${path}.messageId`);
  if (type === "user_message") {
    oneOf(item.inputIntent, ["turn_start", "queued_turn", "steer", "promoted_queued_to_steer"], `${path}.inputIntent`);
    objects(item.attachments, `${path}.attachments`);
  }
  if (type === "proposed_plan") string(item.markdown, `${path}.markdown`, true);
  if (type === "todo_list") objects(item.steps, `${path}.steps`);
  if (["approval_request", "user_input_request"].includes(type)) string(item.requestId, `${path}.requestId`);
  if (type === "user_input_request") objects(item.questions, `${path}.questions`);
  if (type === "command_execution") string(item.input, `${path}.input`, true);
  if (type === "file_change") string(item.fileName, `${path}.fileName`);
  if (["run_interrupt_request", "run_interrupt_result", "system_notice"].includes(type)) string(item.message, `${path}.message`, true);
  if (type === "error") string(object(item.failure, `${path}.failure`).message, `${path}.failure.message`);
  if (type === "notification") string(item.summary, `${path}.summary`);
  optionalFields(item, ["title", "prompt", "summary", "progress", "result", "toolName", "diffStr"], "string", path);
  if (type === "command_execution") optionalFields(item, ["output"], "string", path);
  return item;
}

function validateEntity(value: unknown, family: string, path: string): Obj {
  if (family === "turnItems") return validateTurnItem(value, path);
  const result = entity(value, path);
  optionalFields(result, ["threadId", "title", "status", "text", "role", "kind", "runId", "rootNodeId", "workspaceRoot"], "string", path);
  if (["runs", "attempts", "runtimeRequests", "checkpoints"].includes(family)) string(result.status, `${path}.status`);
  if (family === "attempts") {
    string(result.runId, `${path}.runId`);
    string(result.rootNodeId, `${path}.rootNodeId`);
  }
  if (family === "messages") {
    string(result.text, `${path}.text`, true);
    oneOf(result.role, ["user", "assistant", "system"], `${path}.role`);
  }
  if (family === "checkpoints") {
    if (result.appRunOrdinal !== null) integer(result.appRunOrdinal, `${path}.appRunOrdinal`);
    nullableString(result.runId, `${path}.runId`);
  }
  return result;
}

function validateRow(value: unknown, path: string): Obj {
  const row = object(value, path);
  integer(row.position, `${path}.position`);
  oneOf(row.visibility, ["local", "inherited", "synthetic"], `${path}.visibility`);
  string(row.sourceThreadId, `${path}.sourceThreadId`);
  string(row.sourceItemId, `${path}.sourceItemId`);
  validateTurnItem(row.item, `${path}.item`);
  return row;
}
function upsert(items: Obj[], item: Obj): Obj[] {
  const index = items.findIndex(candidate => candidate.id === item.id);
  if (index < 0) return [...items, item];
  return items.map((candidate, position) => position === index ? item : candidate);
}
function retainIdentity(previous: Obj | undefined, next: Obj): Obj {
  return next.repositoryIdentity == null && previous?.repositoryIdentity != null && previous.workspaceRoot === next.workspaceRoot
    ? { ...next, repositoryIdentity: previous.repositoryIdentity } : next;
}

export function initialShell(): Shell { return { projects: [], threads: [], sequence: 0 }; }

export function applyShell(current: Shell, value: unknown): Shell {
  const item = object(value, "shell");
  const kind = item.kind;
  if (kind === "synchronized") return current;
  if (kind === "snapshot" || kind === undefined) {
    const snapshot = kind === "snapshot" ? object(item.snapshot, "shell.snapshot") : item;
    const sequence = integer(snapshot.snapshotSequence, "shell.snapshotSequence");
    const projects = objects(snapshot.projects, "shell.projects").map(project => validateEntity(project, "projects", "project"));
    const threads = objects(snapshot.threads, "shell.threads").map(thread => validateEntity(thread, "threads", "thread"));
    const previous = new Map(current.projects.map(project => [project.id, project]));
    if (item.resolvedRepositoryIdentityRoots !== undefined) {
      if (!Array.isArray(item.resolvedRepositoryIdentityRoots)) invalid("resolvedRepositoryIdentityRoots must be an array");
      const roots = new Set(item.resolvedRepositoryIdentityRoots.map(root => string(root, "resolvedRepositoryIdentityRoots[]")));
      const candidates = new Map(projects.map(project => [project.id, project]));
      return { ...current, projects: current.projects.map(project => {
        const candidate = candidates.get(project.id);
        if (!candidate || candidate.workspaceRoot !== project.workspaceRoot) return project;
        if (roots.has(str(project.workspaceRoot)) || (project.repositoryIdentity == null && candidate.repositoryIdentity != null)) {
          return { ...project, repositoryIdentity: candidate.repositoryIdentity };
        }
        return project;
      }) };
    }
    return { projects: projects.map(project => retainIdentity(previous.get(project.id), project)), threads, sequence };
  }
  string(kind, "shell.kind");
  const sequence = integer(item.sequence, "shell.sequence");
  if (sequence <= current.sequence) return current;
  switch (kind) {
    case "project.updated": {
      const project = validateEntity(item.project, "projects", "shell.project");
      return { ...current, sequence, projects: upsert(current.projects, retainIdentity(current.projects.find(old => old.id === project.id), project)) };
    }
    case "project.removed": {
      const id = string(item.projectId, "shell.projectId");
      return { ...current, sequence, projects: current.projects.filter(project => project.id !== id) };
    }
    case "thread.updated": {
      const thread = validateEntity(item.thread, "threads", "shell.thread");
      const location = oneOf(item.location, ["active", "archive"], "shell.location");
      return { ...current, sequence, threads: location === "active" ? upsert(current.threads, thread) : current.threads.filter(old => old.id !== thread.id) };
    }
    case "thread.removed": {
      const id = string(item.threadId, "shell.threadId");
      oneOf(item.location, ["active", "archive"], "shell.location");
      return { ...current, sequence, threads: current.threads.filter(thread => thread.id !== id) };
    }
    default: return { ...current, sequence };
  }
}

export function threadSnapshot(value: unknown): ThreadState {
  const snapshot = object(value, "thread snapshot");
  const projection = object(snapshot.projection, "thread snapshot.projection");
  validateEntity(projection.thread, "threads", "projection.thread");
  for (const family of projectionArrays) {
    objects(projection[family], `projection.${family}`).forEach((item, index) => {
      const path = `projection.${family}[${index}]`;
      if (family === "visibleTurnItems") validateRow(item, path);
      else validateEntity(item, family, path);
    });
  }
  const historyCursor = snapshot.historyCursor === undefined ? null : nullableString(snapshot.historyCursor, "historyCursor");
  const hasMore = snapshot.hasMoreHistory === undefined ? false : boolean(snapshot.hasMoreHistory, "hasMoreHistory");
  const latestLocalTurnOrdinal = snapshot.latestLocalTurnOrdinal == null ? null : integer(snapshot.latestLocalTurnOrdinal, "latestLocalTurnOrdinal");
  return { projection, sequence: integer(snapshot.snapshotSequence, "snapshotSequence"), historyCursor, hasMore, latestLocalTurnOrdinal };
}

function visibility(projection: Obj): (item: Obj) => boolean {
  const statuses = new Map(arr(projection.runs).map(run => [run.id, run.status]));
  const superseded = new Set(arr(projection.attempts).filter(attempt => attempt.status === "superseded")
    .map(attempt => JSON.stringify([attempt.runId, attempt.rootNodeId])));
  const interruptRuns = new Set(arr(projection.turnItems).filter(item => item.type === "run_interrupt_request").map(item => item.runId));
  return item => {
    const status = item.runId == null ? undefined : statuses.get(item.runId);
    if (status === "rolled_back") return false;
    if (status === "cancelled" && item.type === "user_message" && item.inputIntent === "queued_turn") return false;
    return !(item.type === "run_interrupt_result" && item.runId != null && item.nodeId != null
      && superseded.has(JSON.stringify([item.runId, item.nodeId])) && !interruptRuns.has(item.runId));
  };
}
function renumber(rows: Obj[]): Obj[] {
  return rows.map((row, position) => row.position === position ? row : { ...row, position });
}
function activeRows(projection: Obj): Obj[] {
  const visible = visibility(projection);
  return renumber(arr(projection.visibleTurnItems).filter(row => row.visibility !== "local" || visible(obj(row.item))));
}
function missingOldItem(state: ThreadState, item: Obj): boolean {
  const rows = arr(state.projection.visibleTurnItems);
  if (rows.some(row => row.sourceItemId === item.id && row.sourceThreadId === item.threadId)) return false;
  if (state.latestLocalTurnOrdinal !== null && num(item.ordinal) <= state.latestLocalTurnOrdinal) return true;
  const ordinals = rows.filter(row => row.visibility === "local").map(row => num(obj(row.item).ordinal));
  return ordinals.length > 0 && num(item.ordinal) < ordinals.reduce((oldest, ordinal) => Math.min(oldest, ordinal));
}
function upsertRow(projection: Obj, item: Obj): Obj[] {
  const rows = arr(projection.visibleTurnItems).filter(row => !(row.sourceItemId === item.id && row.sourceThreadId === item.threadId));
  const next: Obj = { position: 0, visibility: "local", sourceThreadId: item.threadId, sourceItemId: item.id, item };
  const index = rows.findIndex(row => row.visibility === "local" && (num(obj(row.item).ordinal) > num(item.ordinal)
    || (obj(row.item).ordinal === item.ordinal && str(obj(row.item).id).localeCompare(str(item.id)) > 0)));
  rows.splice(index < 0 ? rows.length : index, 0, next);
  return renumber(rows);
}

export function applyThread(state: ThreadState, value: unknown): ThreadState {
  const item = object(value, "thread stream");
  if (item.kind === "snapshot") return threadSnapshot(item);
  if (item.kind === "synchronized") return state;
  const sequence = integer(item.sequence, "thread stream.sequence");
  if (sequence <= state.sequence) return state;
  if (item.kind === "unknown-event") return { ...state, sequence };
  if (item.kind !== "event") return invalid("thread stream.kind");
  const event = object(item.event, "thread stream.event");
  const type = string(event.type, "event.type");
  const family = Object.prototype.hasOwnProperty.call(updateArrays, type) ? updateArrays[type] : undefined;
  if (!threadEvents.has(type) && family === undefined && !otherEvents.has(type)) return { ...state, sequence };
  string(event.threadId, "event.threadId");
  string(event.occurredAt, "event.occurredAt");
  const payload = object(event.payload, "event.payload");
  if (threadEvents.has(type)) validateEntity(payload, "threads", "event.payload");
  else if (family) validateEntity(payload, family, "event.payload");
  else if (type === "provider-session.detached") string(payload.providerSessionId, "event.payload.providerSessionId");
  else if (type === "run.background-work-cancelled") {
    string(payload.runId, "event.payload.runId");
    objects(payload.restartCancelledBackgroundWork, "event.payload.restartCancelledBackgroundWork");
  } else if (type === "checkpoint.rollback-requested") {
    string(payload.scopeId, "event.payload.scopeId");
    string(payload.checkpointId, "event.payload.checkpointId");
    string(payload.requestedAt, "event.payload.requestedAt");
  }
  // The caller guards subscription generations; also refuse to apply another thread's entity.
  if (event.threadId !== obj(state.projection.thread).id) return state;
  if (threadEvents.has(type) && payload.id !== event.threadId) invalid("event.payload.id does not match its thread");
  if (payload.threadId !== undefined && payload.threadId !== event.threadId) invalid("event.payload.threadId does not match its event");
  let projection: Obj = { ...state.projection, updatedAt: event.occurredAt };
  if (threadEvents.has(type)) {
    projection.thread = payload;
    if (type === "thread.visited" || type === "thread.marked-unread") projection.updatedAt = state.projection.updatedAt;
  } else if (type === "run.background-work-cancelled") {
    projection.runs = arr(projection.runs).map(run => run.id === payload.runId
      ? { ...run, restartCancelledBackgroundWork: payload.restartCancelledBackgroundWork } : run);
  } else if (type === "provider-session.detached") {
    projection.providerSessions = arr(projection.providerSessions).filter(session => session.id !== payload.providerSessionId);
  } else if (family === "turnItems") {
    const partial = state.hasMore || state.historyCursor !== null;
    if (partial && !arr(projection.turnItems).some(old => old.id === payload.id) && missingOldItem(state, payload)) return { ...state, sequence };
    const old = arr(projection.turnItems).find(candidate => candidate.id === payload.id);
    projection.turnItems = upsert(arr(projection.turnItems), payload);
    if (payload.type === "run_interrupt_request" || old?.type === "run_interrupt_request") projection.visibleTurnItems = activeRows(projection);
    if (visibility(projection)(payload)) {
      if (!(partial && missingOldItem(state, payload))) projection.visibleTurnItems = upsertRow(projection, payload);
    } else {
      projection.visibleTurnItems = renumber(arr(projection.visibleTurnItems).filter(row => !(row.sourceItemId === payload.id && row.sourceThreadId === payload.threadId)));
    }
    return { ...state, projection, sequence, latestLocalTurnOrdinal: partial
      ? Math.max(state.latestLocalTurnOrdinal ?? 0, num(payload.ordinal)) : state.latestLocalTurnOrdinal };
  } else if (family) {
    let next = payload;
    if (family === "providerTurns" && payload.tokenUsage == null) {
      const previous = arr(projection.providerTurns).find(turn => turn.id === payload.id);
      if (previous?.tokenUsage !== undefined) next = { ...payload, tokenUsage: previous.tokenUsage };
    }
    projection[family] = upsert(arr(projection[family]), next);
    if (family === "runs" || family === "attempts") projection.visibleTurnItems = activeRows(projection);
  }
  return { ...state, projection, sequence };
}

function rowKey(row: Obj): string { return JSON.stringify([row.sourceThreadId, row.sourceItemId]); }

export function mergeHistory(state: ThreadState, value: unknown): ThreadState {
  const page = object(value, "history page");
  integer(page.snapshotSequence, "history page.snapshotSequence");
  const older = objects(page.items, "history page.items").map((row, index) => validateRow(row, `history page.items[${index}]`));
  const historyCursor = nullableString(page.nextCursor, "history page.nextCursor");
  const hasMore = boolean(page.hasMoreHistory, "history page.hasMoreHistory");
  const current = state.projection;
  const keys = new Set(arr(current.visibleTurnItems).map(rowKey));
  const byId = new Map(arr(current.turnItems).map(item => [item.id, item]));
  const visible = visibility(current);
  const prepended: Obj[] = [];
  for (let row of older) {
    const key = rowKey(row);
    if (keys.has(key)) continue;
    if (row.visibility === "local" || row.sourceThreadId === obj(current.thread).id) {
      const live = byId.get(row.sourceItemId);
      // An in-flight page must not resurrect a row hidden by a more recent live event.
      if (live) {
        if (live.type !== "run_interrupt_request" || !visible(live)) continue;
        row = { ...row, item: live };
      }
      if (!visible(obj(row.item))) continue;
    }
    keys.add(key);
    prepended.push(row);
  }
  for (const row of prepended) {
    if ((row.visibility === "local" || row.sourceThreadId === obj(current.thread).id) && !byId.has(row.sourceItemId)) byId.set(row.sourceItemId, obj(row.item));
  }
  return {
    ...state, historyCursor, hasMore,
    projection: prepended.length === 0 ? current : { ...current, turnItems: [...byId.values()], visibleTurnItems: renumber([...prepended, ...arr(current.visibleTurnItems)]) },
  };
}

export function visibleTurnItems(state: ThreadState | null): Obj[] {
  return state ? activeRows(state.projection) : [];
}

function display(value: Json | undefined): string {
  return typeof value === "string" ? value : value == null ? "" : JSON.stringify(value, null, 2);
}
function present(row: Obj): Message {
  const item = obj(row.item);
  const type = str(item.type);
  const id = rowKey(row);
  let kind = "tool";
  let title = str(item.title) || type.replace(/_/g, " ");
  let body = "";
  switch (type) {
    case "user_message": kind = "user"; title = "You"; body = str(item.text); break;
    case "assistant_message": kind = "assistant"; title = "Assistant"; body = str(item.text); break;
    case "compaction": kind = "system"; title = "Context compacted"; body = str(item.summary); break;
    case "reasoning": kind = "reasoning"; title = "Thinking"; body = str(item.text); break;
    case "proposed_plan": kind = "plan"; title = "Plan"; body = str(item.markdown); break;
    case "todo_list": kind = "plan"; title = "Tasks"; body = [str(item.explanation), ...arr(item.steps).map(step => `- [${step.status === "completed" ? "x" : " "}] ${str(step.text)}`)].filter(Boolean).join("\n"); break;
    case "command_execution": title = str(item.title) || "Command"; body = [str(item.input), str(item.output)].filter(Boolean).join("\n\n"); break;
    case "file_change": title = str(item.fileName) || title; body = str(item.diffStr) || `${num(item.additions)} additions, ${num(item.deletions)} deletions`; break;
    case "file_search": title = "File search"; body = [str(item.pattern), ...arr(item.results).map(result => [str(result.fileName), str(result.preview)].filter(Boolean).join(": "))].filter(Boolean).join("\n"); break;
    case "web_search": title = "Web search"; body = arr(item.results).map(result => [str(result.title), str(result.url), str(result.snippet)].filter(Boolean).join("\n")).join("\n\n"); break;
    case "dynamic_tool": title = str(item.toolName) || title; body = [display(item.input), display(item.output)].filter(Boolean).join("\n\n"); break;
    case "subagent": title = "Agent"; body = [str(item.prompt), str(item.progress), str(item.result)].filter(Boolean).join("\n\n"); break;
    case "error": kind = "error"; title = "Error"; body = str(obj(item.failure).message); break;
    case "approval_request": kind = "request"; title = "Approval requested"; body = str(item.prompt); break;
    case "user_input_request": kind = "request"; title = "Input requested"; body = arr(item.questions).map(question => str(question.question)).join("\n\n"); break;
    case "fork": kind = "fork"; title = "Conversation fork"; body = "Forked from conversation"; break;
    case "notification": title = "Notification"; body = [str(item.summary), str(item.detail)].filter(Boolean).join("\n\n"); break;
    case "checkpoint": return { id, kind: "checkpoint", title: "Changes", body: "", checkpointId: str(item.checkpointId),
      files: arr(item.files).map(file => ({ path: str(file.path), additions: num(file.additions), deletions: num(file.deletions) })) };
    default: body = str(item.message) || str(item.summary);
  }
  return { id, kind, title, body, runId: str(item.runId), sourceThreadId: str(row.sourceThreadId) };
}

export function messages(state: ThreadState | null): Message[] {
  return visibleTurnItems(state).map(row => {
    const item = obj(row.item);
    const stored = arr(state?.projection.messages).find(message => message.id === item.messageId);
    return { ...present(row), runId: str(item.runId), sourceThreadId: str(row.sourceThreadId),
      completed: item.status === 'completed', createdAt: str(stored?.createdAt ?? item.startedAt ?? item.updatedAt) };
  });
}

export function readyCheckpoint(state: ThreadState | null): number | null {
  if (!state) return null;
  let latest: number | null = null;
  for (const checkpoint of arr(state.projection.checkpoints)) {
    if (checkpoint.status !== "ready" || checkpoint.runId == null || checkpoint.appRunOrdinal == null) continue;
    const ordinal = num(checkpoint.appRunOrdinal, -1);
    if (ordinal >= 0 && (latest === null || ordinal > latest)) latest = ordinal;
  }
  return latest;
}

// Reference projectSettings.resolveWorktreeCleanup: off clears every rule,
// custom owns a complete rule object, otherwise inherit environment retention.
export function effectiveWorktreeRules(settings: Obj, override: Obj): Obj {
  const policy = obj(override.worktreeCleanup ?? settings.worktreeCleanup);
  const rules = policy.mode === 'off' ? {} : policy.mode === 'custom' ? obj(policy.rules) : obj(settings.storageCleanup);
  return { worktreeAfterDays: typeof rules.worktreeAfterDays === 'number' ? rules.worktreeAfterDays : null,
    worktreeOnMerge: rules.worktreeOnMerge === true, worktreeOnDelete: rules.worktreeOnDelete === true, worktreeUnchanged: rules.worktreeUnchanged === true };
}
