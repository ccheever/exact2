// T3's own MCP tools in the work log, adapted from T3 Code (MIT, see LICENSE-T3):
// packages/shared/src/t3McpToolPresentation.ts (the inventory and name
// resolution), packages/client-runtime/src/work-log/presentation.ts
// (resolveT3McpToolPresentation: tense, failure and the PR target) and
// packages/client-runtime/src/t3ToolSummary.ts (group summaries).

/** shared/changeRequestUrl parseChangeRequestUrl, reduced to the number a tool label names. */
function changeRequestNumber(target: string): number | null {
  let url: URL;
  try { url = new URL(target); } catch { return null; }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
  const host = url.hostname.toLowerCase(), path = url.pathname;
  const hostOf = (apex: string, label?: string) => host === apex || host.endsWith(`.${apex}`) || label !== undefined && host.split('.').includes(label);
  const match = hostOf('github.com', 'github') && /^\/[^/]+\/[^/]+\/pull\/(\d+)(?:\/|$)/u.exec(path)
    || /^\/[^/]+(?:\/[^/]+)+\/pulls\/(\d+)(?:\/|$)/u.exec(path) || /^\/[^/]+(?:\/[^/]+)+\/-\/merge_requests\/(\d+)(?:\/|$)/u.exec(path)
    || hostOf('bitbucket.org', 'bitbucket') && /^\/[^/]+\/[^/]+\/pull-requests\/(\d+)(?:\/|$)/u.exec(path)
    || (hostOf('dev.azure.com') || host.endsWith('.visualstudio.com')) && /^\/(?:[^/]+\/)*_git\/[^/]+\/pullrequest\/(\d+)(?:\/|$)/u.exec(path) || null;
  const number = Number(match?.[1]);
  return match && Number.isSafeInteger(number) && number > 0 ? number : null;
}

export type T3SummaryAction = 'capabilities' | 'delegate' | 'task-status' | 'task-cancel' | 'schedule-run' | 'schedule-create'
  | 'schedule-list' | 'schedule-update' | 'schedule-delete' | 'thread-create' | 'thread-list' | 'thread-read' | 'thread-send'
  | 'thread-wait' | 'thread-interrupt' | 'thread-configuration' | 'thread-configure' | 'thread-fork' | 'thread-merge'
  | 'thread-search' | 'thread-transfers' | 'thread-organize' | 'thread-update' | 'queue-list' | 'queue-read' | 'queue-edit'
  | 'queue-cancel' | 'queue-reorder' | 'queue-steer' | 'question-list' | 'question-read' | 'question-respond'
  | 'worktree-handoff' | 'worktree-list' | 'worktree-status' | 'project-list' | 'project-read' | 'project-create'
  | 'project-update' | 'project-delete' | 'project-clone' | 'environment-read' | 'environment-update'
  | 'attachment-prepare' | 'attachment-discard' | 'attachment-send' | 'link-pr' | 'unlink-pr' | 'list-prs' | 'watch-pr'
  | 'unwatch-pr' | 'browser' | 'device';
export type T3ToolIcon = 't3-code' | 'browser' | 'device' | 'pull-request';
export type PrAction = 'link-pr' | 'unlink-pr' | 'list-prs' | 'watch-pr' | 'unwatch-pr';
export interface T3ToolDefinition { displayName: string; labels: readonly [string, string, string, string]; icon: T3ToolIcon; summaryAction: T3SummaryAction }

const tool = (labels: T3ToolDefinition['labels'], summaryAction: T3SummaryAction, icon: T3ToolIcon = 't3-code', displayName = `${labels[0]} ${labels[3]}`): T3ToolDefinition =>
  ({ displayName, labels, icon, summaryAction });
const pr = (labels: T3ToolDefinition['labels'], action: PrAction) => tool(labels, action, 'pull-request');
const browser = (labels: T3ToolDefinition['labels'], displayName?: string) => tool(labels, 'browser', 'browser', displayName);
const device = (labels: T3ToolDefinition['labels']) => tool(labels, 'device', 'device');

const TOOLS: Readonly<Record<string, T3ToolDefinition>> = {
  link_pull_request: pr(['Link', 'Linking', 'Linked', 'a pull request'], 'link-pr'),
  unlink_pull_request: pr(['Unlink', 'Unlinking', 'Unlinked', 'a pull request'], 'unlink-pr'),
  list_thread_pull_requests: pr(['Check', 'Checking', 'Checked', 'linked pull requests'], 'list-prs'),
  watch_pull_request: pr(['Watch', 'Watching', 'Watching', 'a pull request'], 'watch-pr'),
  unwatch_pull_request: pr(['Stop watching', 'Stopping watching', 'Stopped watching', 'a pull request'], 'unwatch-pr'),
  orchestrator_capabilities: tool(['Get', 'Getting', 'Got', 'orchestration capabilities'], 'capabilities'),
  delegate_task: tool(['Delegate', 'Delegating', 'Delegated', 'a child task'], 'delegate'),
  task_status: tool(['Get', 'Getting', 'Got', 'delegated task status'], 'task-status'),
  task_cancel: tool(['Cancel', 'Canceling', 'Requested cancellation of', 'delegated task'], 'task-cancel'),
  schedule_task: tool(['Schedule', 'Scheduling', 'Scheduled', 'a recurring task'], 'schedule-create'),
  list_scheduled_tasks: tool(['List', 'Listing', 'Listed', 'scheduled tasks'], 'schedule-list'),
  update_scheduled_task: tool(['Update', 'Updating', 'Updated', 'a scheduled task'], 'schedule-update'),
  delete_scheduled_task: tool(['Delete', 'Deleting', 'Requested deletion of', 'a scheduled task'], 'schedule-delete'),
  create_threads: tool(['Create', 'Creating', 'Created', 'T3 threads'], 'thread-create'),
  t3_thread_start: tool(['Start', 'Starting', 'Started', 'a T3 thread'], 'thread-create'),
  t3_thread_list: tool(['List', 'Listing', 'Listed', 'T3 threads'], 'thread-list'),
  t3_thread_read: tool(['Read', 'Reading', 'Read', 'a T3 thread'], 'thread-read'),
  t3_thread_send: tool(['Send', 'Sending', 'Sent', 'to a T3 thread'], 'thread-send'),
  t3_thread_wait: tool(['Wait', 'Waiting', 'Waited', 'for a T3 thread'], 'thread-wait'),
  t3_thread_interrupt: tool(['Interrupt', 'Interrupting', 'Requested an interrupt of', 'a T3 thread'], 'thread-interrupt'),
  t3_worktree_handoff: tool(['Hand off', 'Handing off', 'Handed off', 'thread to a git worktree'], 'worktree-handoff'),
  t3_worktree_status: tool(['Get', 'Getting', 'Got', 'thread worktree status'], 'worktree-status'),
  preview_status: browser(['Get', 'Getting', 'Got', 'preview browser status']),
  preview_open: browser(['Open', 'Opening', 'Opened', 'a page in the preview browser']),
  preview_navigate: browser(['Navigate', 'Navigating', 'Navigated', 'the preview browser']),
  preview_snapshot: browser(['Take a snapshot of', 'Taking a snapshot of', 'Took a snapshot of', 'the preview page'], 'Snapshot the preview page'),
  preview_click: browser(['Click', 'Clicking', 'Clicked', 'in the preview browser']),
  preview_press: browser(['Press', 'Pressing', 'Pressed', 'a key in the preview browser']),
  preview_type: browser(['Type', 'Typing', 'Typed', 'in the preview browser']),
  preview_scroll: browser(['Scroll', 'Scrolling', 'Scrolled', 'the preview browser']),
  preview_resize: browser(['Resize', 'Resizing', 'Resized', 'the preview browser']),
  preview_evaluate: browser(['Evaluate', 'Evaluating', 'Evaluated', 'script in the preview browser']),
  preview_wait_for: browser(['Wait', 'Waiting', 'Waited', 'for the preview page']),
  preview_set_appearance: browser(['Set', 'Setting', 'Set', 'preview browser appearance']),
  preview_recording_start: browser(['Start', 'Starting', 'Started', 'recording the preview browser']),
  preview_recording_stop: browser(['Stop', 'Stopping', 'Stopped', 'recording the preview browser']),
  device_list: device(['List', 'Listing', 'Listed', 'simulators and emulators']),
  device_open: device(['Open', 'Opening', 'Opened', 'a device in the Device panel']),
  device_screenshot: device(['Take a screenshot of', 'Taking a screenshot of', 'Took a screenshot of', 'the device']),
  device_close: device(['Close', 'Closing', 'Closed', 'a device']),
  run_scheduled_task_now: tool(['Run', 'Running', 'Requested a run of', 'a scheduled task'], 'schedule-run'),
  t3_queue_list: tool(['List', 'Listing', 'Listed', 'queued messages'], 'queue-list'),
  t3_queue_read: tool(['Read', 'Reading', 'Read', 'a queued message'], 'queue-read'),
  t3_queue_edit: tool(['Edit', 'Editing', 'Edited', 'a queued message'], 'queue-edit'),
  t3_queue_cancel: tool(['Cancel', 'Canceling', 'Requested cancellation of', 'a queued run'], 'queue-cancel'),
  t3_queue_reorder: tool(['Reorder', 'Reordering', 'Reordered', 'a queued run'], 'queue-reorder'),
  t3_queue_promote_to_steer: tool(['Steer with', 'Steering with', 'Requested steering with', 'a queued message'], 'queue-steer'),
  t3_pending_request_list: tool(['List', 'Listing', 'Listed', 'pending questions'], 'question-list'),
  t3_pending_request_read: tool(['Read', 'Reading', 'Read', 'pending questions'], 'question-read'),
  t3_pending_request_respond: tool(['Answer', 'Answering', 'Answered', 'pending questions'], 'question-respond'),
  t3_thread_configuration: tool(['Read', 'Reading', 'Read', 'thread configuration'], 'thread-configuration'),
  t3_thread_configure: tool(['Set', 'Setting', 'Set', 'thread model'], 'thread-configure'),
  t3_thread_fork: tool(['Fork', 'Forking', 'Requested a fork of', 'this thread'], 'thread-fork'),
  t3_thread_merge_back: tool(['Merge', 'Merging', 'Requested a merge of', 'thread context'], 'thread-merge'),
  t3_thread_search: tool(['Search', 'Searching', 'Searched', 'thread content'], 'thread-search'),
  t3_thread_transfers: tool(['Read', 'Reading', 'Read', 'thread transfers'], 'thread-transfers'),
  t3_thread_organize: tool(['Organize', 'Organizing', 'Organized', 'a thread'], 'thread-organize'),
  t3_thread_update: tool(['Update', 'Updating', 'Updated', 'T3 thread metadata'], 'thread-update'),
  t3_worktree_list: tool(['List', 'Listing', 'Listed', 'workspace branches'], 'worktree-list'),
  t3_preview_list: browser(['List', 'Listing', 'Listed', 'preview tabs']),
  t3_preview_close: browser(['Close', 'Closing', 'Closed', 'a preview tab']),
  t3_environment_read: tool(['Read', 'Reading', 'Read', 'environment preferences'], 'environment-read'),
  t3_environment_preferences_update: tool(['Update', 'Updating', 'Updated', 'environment preferences'], 'environment-update'),
  t3_thread_launch: tool(['Launch', 'Launching', 'Launched', 'a project thread'], 'thread-create'),
  t3_project_list: tool(['List', 'Listing', 'Listed', 'projects'], 'project-list'),
  t3_project_read: tool(['Read', 'Reading', 'Read', 'a project'], 'project-read'),
  t3_project_create: tool(['Register', 'Registering', 'Registered', 'a project'], 'project-create'),
  t3_project_update: tool(['Update', 'Updating', 'Updated', 'a project'], 'project-update'),
  t3_project_delete: tool(['Delete', 'Deleting', 'Deleted', 'a project'], 'project-delete'),
  t3_project_clone: tool(['Clone', 'Cloning', 'Cloned', 'a repository'], 'project-clone'),
  t3_attachment_prepare_upload: tool(['Prepare', 'Preparing', 'Prepared', 'an attachment upload'], 'attachment-prepare'),
  t3_attachment_discard: tool(['Discard', 'Discarding', 'Discarded', 'a pending attachment'], 'attachment-discard'),
  t3_thread_send_attachments: tool(['Send', 'Sending', 'Sent', 'attachments'], 'attachment-send'),
};
const SERVER_ALIASES = new Set(['t3-code', 't3_code', 't3code']);
const own = (name: string) => Object.prototype.hasOwnProperty.call(TOOLS, name);

/** resolveT3McpToolName: the loose provider prefixes, gated by the inventory. */
function resolveName(value: string): string | null {
  const label = value.replace(/\s+(?:complete|completed)\s*$/i, '').trim();
  const mcp = /^mcp__(.+?)__(.+)$/i.exec(label);
  if (mcp) return SERVER_ALIASES.has(mcp[1]!.toLowerCase()) ? mcp[2]! : null;
  const namespaced = /^(?:t3-code|t3_code|t3code)(?:[.:/]|\s*·\s*)(.+)$/i.exec(label);
  if (namespaced) return namespaced[1] ?? null;
  const prefixed = /^(?:mcp[-_]{1,2})?t3[-_ ]?code(?:__|[-_.:/ ])(.+)$/i.exec(label);
  const candidate = prefixed?.[1] ?? label;
  return own(candidate) ? candidate : null;
}
export function t3ToolDefinition(toolName: string | null | undefined): T3ToolDefinition | null {
  const name = toolName ? resolveName(toolName) : null;
  return name !== null && own(name) ? TOOLS[name]! : null;
}

type Rec = Record<string, unknown>;
const record = (value: unknown): Rec | undefined => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Rec : undefined;
const id = (value: unknown): string | undefined => typeof value === 'string' && value.trim() !== '' ? value : undefined;

/** readResult: the structured or JSON MCP result envelopes the adapters keep. */
function readResult(value: unknown, depth = 0): { data?: Rec; failed: boolean } {
  if (depth > 4) return { failed: false };
  if (typeof value === 'string') { try { return readResult(JSON.parse(value), depth + 1); } catch { return { failed: false }; } }
  if (Array.isArray(value)) {
    let data: Rec | undefined, failed = false;
    for (const block of value) {
      const entry = record(block), text = entry?.text ?? record(entry?.content)?.text;
      const result = readResult(record(text)?.text ?? text, depth + 1);
      data ??= result.data; failed ||= result.failed;
    }
    return { ...data ? { data } : {}, failed };
  }
  const entry = record(value);
  if (!entry) return { failed: false };
  const failed = entry.isError === true || entry.is_error === true || typeof entry._tag === 'string' && /(?:Error|Failure)$/.test(entry._tag)
    || entry.error !== null && entry.error !== undefined;
  const content = entry.structuredContent ?? entry.content;
  if (content !== undefined) { const result = readResult(content, depth + 1); return { ...result.data ? { data: result.data } : {}, failed: failed || result.failed }; }
  return { data: entry, failed };
}
export function t3ToolResultFailed(output: unknown): boolean { return readResult(output).failed; }

export interface T3ToolPresentation { displayName: string; icon: T3ToolIcon; action?: PrAction }
/** resolveT3McpToolPresentation: the tense the status asks for, a PR's number when the input names one. */
export function t3ToolPresentation(definition: T3ToolDefinition | null, status: string | undefined, data?: unknown): T3ToolPresentation | null {
  if (!definition) return null;
  const [action, running, completed, detail] = definition.labels;
  const verb = status === 'inProgress' ? running : status === 'completed' ? completed : status === 'failed' ? `Failed to ${action.toLowerCase()}`
    : status === 'declined' ? `Declined to ${action.toLowerCase()}` : status === 'stopped' ? `Stopped ${running.toLowerCase()}` : running;
  const kind = (['link-pr', 'unlink-pr', 'list-prs', 'watch-pr', 'unwatch-pr'] as const).find(value => value === definition.summaryAction);
  const payload = record(data), input = record(payload?.arguments) ?? record(payload?.input) ?? record(payload?.rawInput);
  const number = (typeof input?.url === 'string' ? changeRequestNumber(input.url) : null) ?? input?.number;
  const target = kind !== undefined && kind !== 'list-prs' && typeof number === 'number' && Number.isSafeInteger(number) && number > 0 ? `PR #${number}` : detail;
  return { displayName: `${verb} ${target}`, icon: definition.icon, ...kind === undefined ? {} : { action: kind } };
}

export interface T3ToolCall { input: unknown; output: unknown; outcome: 'completed' | 'failed' | 'unfinished' }
const quantity = (count: number, noun: string, plural = `${noun}s`) => `${count} ${count === 1 ? noun : plural}`;
const countEntities = (ids: (string | undefined)[]) => new Set(ids.filter(value => value !== undefined)).size + ids.filter(value => value === undefined).length;

/** summarizeT3ToolCalls: successful effects, counted apart from failed or unfinished calls. */
export function summarizeT3ToolCalls(action: T3SummaryAction, calls: readonly T3ToolCall[]): { label: string; failedCount: number } {
  const results = calls.map(call => {
    const result = readResult(call.output), inputData = readResult(call.input).data;
    const input = inputData && typeof inputData.toolName === 'string' ? record(inputData.args) : inputData;
    return { input, output: result.data, outcome: result.failed ? 'failed' as const : call.outcome };
  });
  const completed = results.filter(call => call.outcome === 'completed');
  const failedCount = results.filter(call => call.outcome === 'failed').length;
  const selected = completed.length > 0 ? completed : results;
  const times = quantity(selected.length, 'time');
  const phrase = (past: string, infinitive: string, object: string) => `${completed.length > 0 ? past : `Tried to ${infinitive}`} ${object}`;
  const entities = (key: string) => selected.map(call => id(call.output?.[key]) ?? id(call.input?.[key]));
  const count = (key: string, noun: string, plural?: string) => quantity(countEntities(entities(key)), noun, plural);
  const projectIds = selected.map(call => id(call.output?.id) ?? id(call.output?.projectId) ?? id(call.input?.projectId));
  const threadIds = selected.map(call => id(call.output?.threadId) ?? id(record(call.output?.thread)?.threadId) ?? id(call.input?.threadId));
  const threadsKnown = threadIds.every(value => value !== undefined);
  const label = ((): string => {
    switch (action) {
      case 'thread-send': {
        const messages = countEntities(selected.map(call => id(call.output?.messageId))), threads = new Set(threadIds).size;
        return phrase('Sent', 'send', threadsKnown ? messages === threads && messages > 1 ? `messages to ${quantity(threads, 'thread')}`
          : `${quantity(messages, 'message')} to ${quantity(threads, 'thread')}` : quantity(messages, 'message'));
      }
      case 'thread-create': {
        const created: string[] = [];
        const known = completed.length > 0 && completed.every(call => (Array.isArray(call.output?.threads) ? call.output.threads : [call.output]).every(thread => {
          const entry = record(thread);
          if (entry?.status === 'rolled_back') return true;
          const threadId = id(entry?.threadId);
          if (!threadId) return false;
          created.push(threadId); return true;
        }));
        return known ? `Created ${quantity(new Set(created).size, 'thread')}` : `Requested thread creation ${times}`;
      }
      case 'delegate': return phrase('Delegated', 'delegate', count('taskId', 'task'));
      case 'thread-read': return phrase('Read', 'read', threadsKnown ? quantity(new Set(threadIds).size, 'thread') : `threads ${times}`);
      case 'thread-wait': return phrase('Waited on', 'wait on', threadsKnown ? quantity(new Set(threadIds).size, 'thread') : `threads ${times}`);
      case 'thread-list': return phrase('Listed', 'list', `threads ${times}`);
      case 'thread-interrupt': return phrase('Requested interrupts for', 'interrupt', quantity(countEntities(threadIds), 'thread'));
      case 'task-status': return phrase('Checked', 'check', `task status ${times}`);
      case 'task-cancel': return phrase('Requested cancellation of', 'cancel', count('taskId', 'task'));
      case 'schedule-create': return phrase('Scheduled', 'schedule', count('scheduledTaskId', 'task'));
      case 'schedule-list': return phrase('Listed', 'list', `scheduled tasks ${times}`);
      case 'schedule-update': return phrase('Updated', 'update', count('scheduledTaskId', 'scheduled task'));
      case 'schedule-delete': return phrase('Requested deletion of', 'delete', count('scheduledTaskId', 'scheduled task'));
      case 'schedule-run': return phrase('Requested', 'request', quantity(selected.length, 'scheduled task run'));
      case 'thread-configuration': return phrase('Checked', 'check', `thread configuration ${times}`);
      case 'thread-configure': return phrase('Set', 'set', `thread model ${times}`);
      case 'thread-fork': return phrase('Requested', 'request', quantity(selected.length, 'thread fork'));
      case 'thread-merge': return phrase('Requested', 'request', quantity(selected.length, 'context merge'));
      case 'thread-search': return phrase('Searched', 'search', `threads ${times}`);
      case 'thread-transfers': return phrase('Checked', 'check', `thread transfers ${times}`);
      case 'thread-organize': return phrase('Organized', 'organize', `threads ${times}`);
      case 'thread-update': return phrase('Updated', 'update', quantity(countEntities(threadIds), 'thread'));
      case 'queue-list': return phrase('Listed', 'list', `queued messages ${times}`);
      case 'queue-read': return phrase('Read', 'read', count('queuedRunId', 'queued message'));
      case 'queue-edit': return phrase('Edited', 'edit', count('queuedRunId', 'queued message'));
      case 'queue-cancel': return phrase('Requested cancellation of', 'cancel', count('queuedRunId', 'queued run'));
      case 'queue-reorder': return phrase('Reordered', 'reorder', count('queuedRunId', 'queued run'));
      case 'queue-steer': return phrase('Requested steering with', 'steer with', count('queuedRunId', 'queued message'));
      case 'question-list': return phrase('Listed', 'list', `pending questions ${times}`);
      case 'question-read': return phrase('Read', 'read', count('requestId', 'pending question request'));
      case 'question-respond': return phrase('Answered', 'answer', count('requestId', 'pending question request'));
      case 'worktree-handoff': return phrase('Handed off to', 'hand off to', count('worktreePath', 'worktree'));
      case 'worktree-list': return phrase('Listed', 'list', `workspace branches ${times}`);
      case 'worktree-status': return phrase('Checked', 'check', `worktree status ${times}`);
      case 'project-list': return phrase('Listed', 'list', `projects ${times}`);
      case 'project-read': return phrase('Read', 'read', quantity(countEntities(projectIds), 'project'));
      case 'project-create': return phrase('Registered', 'register', quantity(countEntities(projectIds), 'project'));
      case 'project-update': return phrase('Updated', 'update', quantity(countEntities(projectIds), 'project'));
      case 'project-delete': return phrase('Deleted', 'delete', quantity(countEntities(projectIds), 'project'));
      case 'project-clone': return phrase('Cloned', 'clone', count('cwd', 'repository', 'repositories'));
      case 'environment-read': return phrase('Checked', 'check', `environment preferences ${times}`);
      case 'environment-update': return phrase('Updated', 'update', `environment preferences ${times}`);
      case 'attachment-prepare': return phrase('Prepared', 'prepare', count('attachmentId', 'attachment upload'));
      case 'attachment-discard': return phrase('Discarded', 'discard', count('attachmentId', 'pending attachment'));
      case 'attachment-send': {
        const messages = new Map<string, (typeof selected)[number]>();
        selected.forEach((call, index) => messages.set(id(call.output?.messageId) ?? `call-${index}`, call));
        const sent = [...messages.values()];
        const attachments = sent.reduce((total, call) => total + (Array.isArray(call.input?.attachments) ? call.input.attachments.length : 0), 0);
        const countsKnown = sent.every(call => Array.isArray(call.input?.attachments) && call.input.attachments.length > 0);
        const targets = threadsKnown ? ` to ${quantity(new Set(threadIds).size, 'thread')}` : '';
        return phrase('Sent', 'send', countsKnown ? `${quantity(attachments, 'attachment')}${targets}` : `attachments${targets} ${times}`);
      }
      case 'link-pr': return phrase('Linked', 'link', quantity(selected.length, 'pull request'));
      case 'unlink-pr': return phrase('Unlinked', 'unlink', quantity(selected.length, 'pull request'));
      case 'watch-pr': return phrase('Watching', 'watch', quantity(selected.length, 'pull request'));
      case 'unwatch-pr': return phrase('Stopped watching', 'stop watching', quantity(selected.length, 'pull request'));
      case 'list-prs': return phrase('Checked', 'check', `linked pull requests${selected.length === 1 ? '' : ` ${times}`}`);
      case 'browser': return phrase('Used', 'use', `browser ${times}`);
      case 'device': return phrase('Used', 'use', `device controls ${times}`);
      case 'capabilities': return phrase('Checked', 'check', `orchestration capabilities ${times}`);
    }
  })();
  return { label, failedCount };
}

/** summaryActionPriority: actions with effects lead the group sentence. */
export function t3ActionPriority(action: string): number {
  return ['command', 'edit', 'delegate', 'task-cancel', 'thread-create', 'thread-send', 'thread-interrupt', 'schedule-create', 'schedule-update',
    'schedule-delete', 'schedule-run', 'thread-configure', 'thread-fork', 'thread-merge', 'thread-organize', 'thread-update', 'queue-edit',
    'queue-cancel', 'queue-reorder', 'queue-steer', 'question-respond', 'worktree-handoff', 'project-create', 'project-update', 'project-delete',
    'project-clone', 'environment-update', 'attachment-prepare', 'attachment-discard', 'attachment-send'].includes(action) ? 0
    : action === 'other' || action === 'update' ? 2 : 1;
}
