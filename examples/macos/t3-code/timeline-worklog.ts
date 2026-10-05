// Work-log presentation adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/session-logic.ts (projectedWorkEntry, providerErrorPresentation),
// packages/client-runtime/src/work-log/presentation.ts (summaries, failure),
// packages/shared/src/toolActivity.ts (classification, labels) and
// apps/web/src/components/chat/MessagesTimeline.logic.ts (entry labels).
// T3's own MCP tools take their presentation from timeline-t3tools.ts;
// integration sources are not reproduced and keep the generic presentation.
import { toolCallLines } from './timeline-item-detail';
import { arr, num, obj, str, type Json, type Obj } from './domain';
import { summarizeT3ToolCalls, t3ActionPriority, t3ToolDefinition, t3ToolPresentation, t3ToolResultFailed, type T3ToolCall, type T3ToolPresentation } from './timeline-t3tools';

export const last = <T>(items: readonly T[]): T | undefined => items[items.length - 1];
export function findLast<T>(items: readonly T[], test: (item: T) => boolean): T | undefined {
  for (let index = items.length - 1; index >= 0; index--) if (test(items[index]!)) return items[index];
  return undefined;
}
export function findLastIndex<T>(items: readonly T[], test: (item: T) => boolean): number {
  for (let index = items.length - 1; index >= 0; index--) if (test(items[index]!)) return index;
  return -1;
}

export type Status = 'inProgress' | 'completed' | 'idle' | 'failed' | 'stopped' | 'declined';
export interface WorkEntry {
  id: string; createdAt: string; runId: string; label: string;
  tone: 'thinking' | 'tool' | 'info' | 'error'; itemType: string; status: Status;
  item: Obj; visibility: string; command?: string; detail?: string; changedFiles?: string[];
  toolTitle?: string; toolData?: Obj; sourceActivityKind?: string; questionAnswer?: Obj;
}
export type GroupAction = 'read' | 'edit' | 'command' | 'thread-create' | 'code-search' | 'search' | 'other' | 'update'
  | 'link-pr' | 'unlink-pr' | 'list-prs' | 'watch-pr' | 'unwatch-pr' | 'browser' | 'device';

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '0ms';
  if (ms < 1_000) return `${Math.max(1, Math.round(ms))}ms`;
  if (ms < 10_000) { const tenths = Math.round(ms / 100) / 10; return tenths >= 10 ? '10s' : `${tenths.toFixed(1)}s`; }
  if (ms < 60_000) return `${Math.round(ms / 1_000)}s`;
  const total = Math.round(ms / 1_000), parts: string[] = [];
  if (Math.floor(total / 3_600)) parts.push(`${Math.floor(total / 3_600)}h`);
  if (Math.floor(total % 3_600 / 60)) parts.push(`${Math.floor(total % 3_600 / 60)}m`);
  if (total % 60) parts.push(`${total % 60}s`);
  return parts.join(' ');
}

const trimDigits = (value: number) => value.toFixed(Math.abs(value) >= 100 ? 0 : Math.abs(value) >= 10 ? 1 : 2).replace(/\.0+$/, '');
export function formatTokens(value: number): string {
  const abs = Math.abs(value);
  if (abs >= 1e9) return `${trimDigits(value / 1e9)}B`;
  if (abs >= 1e6) return `${trimDigits(value / 1e6)}M`;
  if (abs >= 1e3) return `${trimDigits(value / 1e3)}K`;
  return new Intl.NumberFormat('en-US').format(Math.round(value));
}

export function contextCompactionLabel(item: Obj): string {
  if (item.status === 'running') return 'Compacting context';
  if (typeof item.beforeTokenCount === 'number' && typeof item.afterTokenCount === 'number') {
    return `Context compacted ${formatTokens(item.beforeTokenCount)} → ${formatTokens(item.afterTokenCount)} tokens`;
  }
  return 'Context compacted';
}

/** session-logic providerErrorPresentation. */
export function providerErrorPresentation(item: Obj): { label: string; detail: string } {
  const failure = obj(item.failure), retry = obj(item.retry);
  if (item.retry === undefined || item.retry === null) {
    return { label: failure.class === 'usage_limit' ? 'Usage limit reached' : str(item.title).trim() || 'Provider error', detail: str(failure.message) };
  }
  const progress = retry.maxAttempts === null || retry.maxAttempts === undefined ? `${num(retry.attempt)}` : `${num(retry.attempt)}/${num(retry.maxAttempts)}`;
  const label = item.status === 'running' ? `Retrying provider (${progress})`
    : item.status === 'completed' ? `Provider recovered (${progress} retries)`
      : item.status === 'failed' ? `${failure.class === 'usage_limit' ? 'Usage limit reached' : 'Provider error'} after ${progress} retries`
        : `Provider retry stopped (${progress})`;
  const delay = num(retry.retryDelayMs);
  const wait = item.status === 'running' && delay > 0
    ? delay < 1_000 ? ` Retrying in ${delay}ms.` : ` Retrying in ${(delay / 1_000).toFixed(1).replace(/\.0$/u, '')}s.` : '';
  return { label, detail: `${str(failure.message)}${wait}` };
}

function lifecycle(status: unknown): Status {
  switch (status) {
    case 'pending': case 'running': case 'waiting': return 'inProgress';
    case 'completed': return 'completed';
    case 'idle': return 'idle';
    case 'failed': return 'failed';
    default: return 'stopped';
  }
}
const toolTypes = new Set(['command_execution', 'file_change', 'file_search', 'web_search', 'dynamic_tool', 'subagent', 'thread_created', 'user_input_request', 'approval_request']);

/** session-logic projectedWorkEntry: one turn item as a work-log entry. */
export function projectedWorkEntry(row: Obj): WorkEntry {
  const item = obj(row.item), type = str(item.type), title = str(item.title).trim() || null;
  const common: WorkEntry = { id: str(item.id), createdAt: str(item.startedAt ?? item.updatedAt), runId: str(item.runId),
    label: '', tone: type === 'reasoning' ? 'thinking' : toolTypes.has(type) ? 'tool' : 'info', itemType: type,
    status: lifecycle(item.status), item, visibility: str(row.visibility, 'local') };
  switch (type) {
    case 'thread_created': return { ...common, label: 'Created thread' };
    case 'compaction': return { ...common, label: contextCompactionLabel(item), sourceActivityKind: 'context-compaction', ...(item.summary ? { detail: str(item.summary) } : {}) };
    case 'reasoning': return { ...common, label: title ?? 'Thinking', ...(item.text ? { detail: str(item.text) } : {}) };
    case 'command_execution': return { ...common, label: title ?? 'Ran command', command: str(item.input), toolTitle: title ?? 'Command', toolData: item };
    case 'file_change': {
      const changes = arr(item.changes);
      return { ...common, label: title ?? (changes.length > 1 ? `Changed ${changes.length} files` : `Changed ${str(item.fileName)}`),
        changedFiles: changes.length ? changes.map(change => str(change.path)) : [str(item.fileName)], toolTitle: title ?? 'File change', toolData: item };
    }
    case 'file_search': return { ...common, label: title ?? formatSearchToolLabel(item) ?? 'Searched files', ...(item.pattern ? { detail: str(item.pattern) } : {}), toolTitle: title ?? 'File search', toolData: item };
    case 'web_search': {
      const patterns = Array.isArray(item.patterns) ? item.patterns.filter((value): value is string => typeof value === 'string') : [];
      return { ...common, label: title ?? 'Searched the web', ...(patterns.length ? { detail: patterns.join(', ') } : {}), toolTitle: title ?? 'Web search', toolData: item };
    }
    case 'system_notice': return { ...common, label: str(item.message), sourceActivityKind: 'runtime.warning' };
    case 'error': {
      const failure = obj(item.failure);
      return { ...common, ...providerErrorPresentation(item), toolData: item,
        ...(failure.class === 'usage_limit' && item.status !== 'completed' ? { sourceActivityKind: 'runtime.warning' }
          : item.retry === undefined || item.retry === null ? { sourceActivityKind: 'runtime.error' } : {}) };
    }
    case 'dynamic_tool': {
      const input = obj(item.input), kind = classifyToolActivity({ itemType: 'dynamic_tool_call', data: { toolName: str(item.toolName), input } });
      const readPath = collectToolFilePaths({ input })[0];
      return { ...common, label: title ?? (kind === 'read' ? formatReadToolLabel(readPath ?? '') : kind === 'search'
        ? formatSearchToolLabel({ input }) ?? (str(item.toolName) || 'Tool call') : str(item.toolName) || 'Tool call'),
        toolTitle: title ?? (str(item.toolName) || 'Tool'), toolData: { input, output: item.output ?? null } };
    }
    case 'approval_request': return { ...common, label: title ?? 'Approval requested', detail: str(item.prompt, str(item.requestKind)), toolData: item };
    case 'user_input_request': return { ...common, label: title ?? (item.questionAnswer ? 'Answered questions' : 'Input requested'),
      ...(item.questionAnswer ? { questionAnswer: obj(item.questionAnswer) } : {}), toolData: item };
    case 'notification': return { ...common, label: str(item.summary), tone: 'info' };
    default: return { ...common, label: title ?? type.replace(/_/g, ' '), toolData: item };
  }
}

// ---- shared/toolActivity ----
const trimmed = (value: unknown): string | undefined => typeof value === 'string' && value.trim() ? value.trim() : undefined;
/** claudeSkillInvocation: the skill a Claude `Skill` call loads and the arguments it passes. */
export function claudeSkillInvocation(toolName: unknown, input: unknown): { name: string; args: string | undefined } | undefined {
  if (toolName !== 'Skill') return undefined;
  const name = trimmed(obj(input).skill);
  return name === undefined ? undefined : { name, args: trimmed(obj(input).args) };
}
/** dynamicToolTitle: CUA's `title`, or the skill a Claude `Skill` call loads. */
export function dynamicToolTitle(toolName: unknown, input: unknown): string | undefined {
  if (toolName === 'cua_repl.js') return trimmed(obj(input).title);
  const skill = claudeSkillInvocation(toolName, input);
  return skill === undefined ? undefined : `Skill: ${skill.name}`;
}
const dynamicTitle = (entry: WorkEntry) => entry.itemType === 'dynamic_tool' ? dynamicToolTitle(entry.item.toolName, entry.item.input) : undefined;
const PATH_KEYS = ['path', 'filePath', 'file_path', 'relativePath', 'filename', 'fileName', 'newPath', 'oldPath'];
function collectPaths(value: unknown, paths: string[], seen: Set<string>, depth: number): void {
  if (depth > 4 || paths.length >= 8) return;
  if (Array.isArray(value)) { for (const entry of value) { collectPaths(entry, paths, seen, depth + 1); if (paths.length >= 8) return; } return; }
  if (!value || typeof value !== 'object') return;
  const record = value as Obj;
  for (const key of PATH_KEYS) {
    const candidate = str(record[key]).trim();
    if (!candidate || seen.has(candidate)) continue;
    seen.add(candidate); paths.push(candidate);
    if (paths.length >= 8) return;
  }
  for (const nested of ['locations', 'item', 'input', 'result', 'rawInput', 'data', 'changes']) {
    if (nested in record) collectPaths(record[nested], paths, seen, depth + 1);
    if (paths.length >= 8) return;
  }
}
export function collectToolFilePaths(data: unknown): string[] { const paths: string[] = []; collectPaths(data, paths, new Set(), 0); return paths; }
function toolNameToken(value: string): string | undefined {
  const trimmed = value.trim();
  return !trimmed || /__|[./]/u.test(trimmed) ? undefined : trimmed.replace(/[_\s-]/gu, '').toLowerCase();
}
export function classifyToolActivity(input: { itemType?: string; title?: string; data?: Obj }): 'command' | 'read' | 'file_change' | 'search' | 'other' {
  const kind = str(input.data?.kind).trim().toLowerCase();
  const toolName = toolNameToken(str(input.data?.toolName) || str(obj(input.data?.item).tool));
  if (input.itemType === 'command_execution') return 'command';
  if (input.itemType === 'image_view') return 'read';
  if (input.itemType === 'file_change') return 'file_change';
  if (input.itemType === 'web_search') return 'search';
  if (kind === 'execute') return 'command';
  if (['edit', 'move', 'delete', 'write'].includes(kind)) return 'file_change';
  if (kind === 'search' || ['find', 'grep', 'glob', 'rg', 'ls'].includes(toolName ?? '')) return 'search';
  if (kind === 'read') return 'read';
  if (['terminal', 'bash', 'shell'].includes(toolName ?? '')) return 'command';
  if (toolName === 'read' || toolName === 'readfile') return 'read';
  return 'other';
}
function firstString(record: Obj | undefined, keys: string[]): string | undefined {
  for (const key of keys) { const value = str(record?.[key]).trim(); if (value) return value; }
  return undefined;
}
export function formatSearchToolLabel(data: Obj | undefined): string | undefined {
  const input = [data?.rawInput, data?.input, obj(data?.item).input].map(value => obj(value)).find(record => Object.keys(record).length > 0) ?? data;
  const query = firstString(input, ['pattern', 'query', 'searchTerm', 'regex', 'grep', 'needle']);
  const glob = firstString(input, ['glob', 'globPattern', 'glob_pattern', 'include', 'filePattern', 'file_pattern']);
  const target = firstString(input, ['path', 'target_directory', 'targetDirectory', 'directory', 'cwd', 'root'])?.split(/[\\/]/u).filter(part => part.length > 0 && part !== '.').pop();
  if (query && target) return `Searched ${query} in ${target}`;
  if (glob && target) return `Searched files ${glob} in ${target}`;
  if (glob) return `Searched files ${glob}`;
  if (query) return `Searched ${query}`;
  if (target) return `Searched in ${target}`;
  return undefined;
}
export function formatReadToolLabel(path: string, extra = 0): string {
  const suffix = extra > 0 ? ` +${extra} more` : '';
  return path.trim() ? `Read ${path.trim()}${suffix}` : `Read file${suffix}`;
}

/** filePathDisplay.formatWorkspaceRelativePath: paths under the root keep the root's name. */
export function workspaceRelativePath(path: string, root: string): string {
  const normalized = path.replace(/\\/g, '/');
  const base = root.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!base) return normalized;
  const label = base.split('/').filter(Boolean).pop() ?? base;
  if (normalized === base) return label;
  if (normalized.startsWith(`${base}/`)) return `${label}/${normalized.slice(base.length + 1)}`;
  if (!normalized.startsWith('/')) {
    const relative = normalized.replace(/^\.\/+/, '').replace(/^\/+/, '');
    return normalized.startsWith(`${label}/`) ? normalized : `${label}/${relative}`;
  }
  return normalized;
}

// ---- command labels (client-runtime/work-log/commandLabel.ts, reduced) ----
const SHELLS = new Set(['sh', 'bash', 'zsh', 'dash', 'ash', 'ksh', 'fish']);
function tokenize(command: string): string[] {
  const tokens: string[] = []; let current = '', quote = '', active = false;
  for (const character of command) {
    if (quote) { if (character === quote) quote = ''; else current += character; continue; }
    if (character === '"' || character === "'") { quote = character; active = true; continue; }
    if (/\s/.test(character)) { if (active || current) tokens.push(current); current = ''; active = false; continue; }
    current += character;
  }
  if (active || current) tokens.push(current);
  return tokens;
}
/** Removes a plain `sh -c` wrapper for display. */
export function commandDisplayText(command: string): string {
  const trimmed = command.trim();
  if (/&&|\|\||;|\|/.test(trimmed.replace(/(["'])(?:(?!\1).)*\1/g, ''))) return trimmed;
  const tokens = tokenize(trimmed), program = tokens[0]?.split(/[\\/]/u).pop()?.replace(/\.exe$/iu, '');
  if (!program || !SHELLS.has(program)) return trimmed;
  const flag = tokens.findIndex((token, index) => index > 0 && /^-[a-z]*c[a-z]*$/i.test(token));
  return flag >= 0 && flag === tokens.length - 2 ? tokens[flag + 1]!.trim() || trimmed : trimmed;
}
/** The first real program of a command line: wrappers, assignments and `cd` steps skipped. */
export function commandProgramName(command: string, depth = 0): string | null {
  const display = commandDisplayText(command);
  if (display !== command.trim() && depth < 3) return commandProgramName(display, depth + 1);
  for (const segment of command.split(/&&|\|\||;|\|/)) {
    const tokens = tokenize(segment.trim());
    let index = 0;
    while (index < tokens.length && (/^[A-Za-z_][A-Za-z0-9_]*=/.test(tokens[index]!) || ['env', 'sudo', 'command', 'builtin', 'exec', 'time', 'noglob', 'nocorrect'].includes(tokens[index]!))) index++;
    const token = tokens[index];
    if (!token || ['cd', 'pushd', 'popd', 'export', 'set', 'source', '.', 'true', ':'].includes(token)) continue;
    const name = token.split(/[\\/]/u).pop()?.replace(/\.exe$/iu, '');
    if (!name || /^[<>(){}[\];|&$`#!%@:]/.test(name)) continue;
    if (SHELLS.has(name) && depth < 3) {
      const script = tokens.slice(index + 1).find((value, position, all) => position > 0 && /^-[a-z]*c/i.test(all[position - 1]!));
      if (script) return commandProgramName(script, depth + 1) ?? name;
    }
    return name;
  }
  return null;
}

// ---- T3 tool identity (work-log/presentation.ts workEntryToolName, resolveWorkEntryToolPresentation) ----
function entryToolName(entry: WorkEntry): string | undefined {
  if (entry.itemType === 'dynamic_tool' && str(entry.item.toolName)) return str(entry.item.toolName);
  const data = entry.toolData ?? {};
  if (typeof data.server === 'string' && typeof data.tool === 'string') return `${data.server}.${data.tool}`;
  if (typeof data.toolName === 'string') return data.toolName;
  return t3ToolDefinition(entry.toolTitle) ? entry.toolTitle : entry.label;
}
function entryToolOutput(entry: WorkEntry): unknown {
  const data = entry.toolData ?? {};
  return entry.itemType === 'dynamic_tool' ? entry.item.output : data.output ?? data.result ?? data.rawOutput ?? data.content;
}
export function toolPresentation(entry: WorkEntry, status: string = entry.status): T3ToolPresentation | null {
  const definition = t3ToolDefinition(entryToolName(entry));
  return t3ToolPresentation(definition, definition && t3ToolResultFailed(entryToolOutput(entry)) ? 'failed' : status, entry.toolData);
}

// ---- failure, visibility and grouping (work-log/presentation.ts) ----
export function toolOutputIndicatesFailure(text: string): boolean {
  return /file not found|no files found|enoent|no such file|commandnotfoundexception|command not found|is not recognized as the name of a cmdlet|a parameter cannot be found that matches parameter name/i.test(text)
    || /cannot find path/i.test(text) && /because it does not exist/i.test(text)
    || /is not recognized/i.test(text) && /the term '/i.test(text)
    || /<exited with exit code\s+[1-9]\d*\s*>/i.test(text) || /exit(?:ed)? with exit code\s+[1-9]\d*/i.test(text)
    || /exit code\s*[:\s]\s*[1-9]\d*\b/i.test(text);
}
export function isToolLike(entry: WorkEntry): boolean {
  return entry.tone === 'tool' || entry.tone === 'thinking' || entry.tone === 'error' || entry.command !== undefined;
}
export function displayFailed(entry: WorkEntry): boolean {
  if (entry.tone === 'error' || entry.status === 'failed' || entry.status === 'declined') return true;
  if (!isToolLike(entry)) return false;
  if (t3ToolDefinition(entryToolName(entry)) && t3ToolResultFailed(entryToolOutput(entry))) return true;
  if (entry.itemType === 'command_execution') {
    if (entry.item.outputIndicatesFailure === true || typeof entry.item.exitCode === 'number' && entry.item.exitCode !== 0) return true;
    if (typeof entry.item.output === 'string' && toolOutputIndicatesFailure(entry.item.output.slice(0, 32_768))) return true;
  }
  return !!entry.detail && toolOutputIndicatesFailure(entry.detail);
}
function succeeded(entry: WorkEntry): boolean {
  return isToolLike(entry) && !displayFailed(entry) && !(entry.command && toolOutputIndicatesFailure(entry.command))
    && !(entry.tone === 'thinking' && entry.itemType !== 'reasoning') && !['inProgress', 'stopped', 'idle', 'failed', 'declined'].includes(entry.status);
}
export function visibleInGroup(entry: WorkEntry, expanded = false): boolean {
  if (entry.itemType === 'reasoning') return !!entry.detail?.trim();
  const neutral = isToolLike(entry) && !displayFailed(entry) && !succeeded(entry);
  return expanded && entry.status === 'inProgress' || entry.status === 'stopped' || !neutral;
}
export function groupAction(entry: WorkEntry): GroupAction {
  if (entry.itemType === 'thread_created') return 'thread-create';
  const presentation = toolPresentation(entry);
  if (presentation?.action !== undefined) return presentation.action;
  if (presentation?.icon === 'browser') return 'browser';
  if (presentation?.icon === 'device') return 'device';
  if (entry.itemType === 'approval_request' || entry.itemType === 'user_input_request') return isToolLike(entry) ? 'other' : 'update';
  const data = entry.toolData ?? {};
  const toolName = entry.itemType === 'dynamic_tool' ? str(entry.item.toolName) : str(data.toolName) || entry.toolTitle;
  const itemType = ['command_execution', 'file_change', 'web_search'].includes(entry.itemType) ? entry.itemType : entry.itemType === 'dynamic_tool' ? 'dynamic_tool_call' : undefined;
  const kind = classifyToolActivity({ itemType, data: { ...data, ...(toolName ? { toolName } : {}) } });
  const localSearch = entry.itemType === 'file_search' || entry.itemType === 'web_search' && /\bgrep\b/i.test(entry.toolTitle ?? entry.label);
  if (kind === 'read') return 'read';
  if (kind === 'file_change' || entry.itemType === 'file_change') return 'edit';
  if (kind === 'command' || entry.itemType === 'command_execution' || entry.command) return 'command';
  if (kind === 'search') return entry.itemType === 'web_search' && !localSearch ? 'search' : 'code-search';
  if (localSearch) return 'code-search';
  if (entry.itemType === 'web_search') return 'search';
  if ((entry.changedFiles?.length ?? 0) > 0) return 'edit';
  return isToolLike(entry) ? 'other' : 'update';
}
function actionLabel(action: GroupAction, count: number): string {
  const plural = (one: string, many: string) => count === 1 ? one : many;
  switch (action) {
    case 'read': return `Read ${count} ${plural('file', 'files')}`;
    case 'edit': return `Changed ${count} ${plural('file', 'files')}`;
    case 'command': return `Ran ${count} ${plural('command', 'commands')}`;
    case 'thread-create': return `Created ${count} ${plural('thread', 'threads')}`;
    case 'search': return `Searched the web ${count} ${plural('time', 'times')}`;
    case 'code-search': return `Searched code ${count} ${plural('time', 'times')}`;
    case 'other': return `Used ${count} ${plural('tool', 'tools')}`;
    case 'update': return `Received ${count} ${plural('update', 'updates')}`;
    case 'link-pr': return `Linked ${count} ${plural('pull request', 'pull requests')}`;
    case 'unlink-pr': return `Unlinked ${count} ${plural('pull request', 'pull requests')}`;
    case 'watch-pr': return `Watching ${count} ${plural('pull request', 'pull requests')}`;
    case 'unwatch-pr': return `Stopped watching ${count} ${plural('pull request', 'pull requests')}`;
    case 'list-prs': return count === 1 ? 'Checked linked pull requests' : `Checked linked pull requests ${count} times`;
    case 'device': return `Used device controls ${count} ${plural('time', 'times')}`;
    case 'browser': return `Used browser ${count} ${plural('time', 'times')}`;
  }
}
function t3Call(entry: WorkEntry): T3ToolCall {
  const data = entry.toolData ?? {};
  return { input: entry.itemType === 'dynamic_tool' ? entry.item.input : data.arguments ?? data.input ?? data.rawInput, output: entryToolOutput(entry),
    outcome: entry.status === 'failed' || entry.status === 'declined' || entry.tone === 'error' ? 'failed' : entry.status === 'completed' ? 'completed' : 'unfinished' };
}
/** summarizeToolGroup: at most two action categories, the rest counted, joined as a sentence. */
export function summarizeToolGroup(all: WorkEntry[]): { summary: string; hasFailure: boolean } {
  const entries = all.filter(entry => entry.itemType !== 'reasoning');
  if (all.length > 0 && entries.length === 0) return { summary: all.length === 1 ? 'Thought' : `Thought (×${all.length})`, hasFailure: false };
  const groups = new Map<string, { action: GroupAction; t3: ReturnType<typeof t3Action>; members: WorkEntry[] }>();
  for (const entry of entries) {
    const t3 = t3Action(entry), action = groupAction(entry), key = t3 ?? action;
    const group = groups.get(key);
    if (group) group.members.push(entry); else groups.set(key, { action, t3, members: [entry] });
  }
  const summaries = [...groups].map(([key, { action, t3, members }], index) => {
    if (t3) { const { label, failedCount } = summarizeT3ToolCalls(t3, members.map(t3Call)); return { index, count: members.length, priority: t3ActionPriority(key), label, failed: failedCount }; }
    let count = members.length;
    if (action === 'edit') {
      const files = new Set<string>(); let bare = 0;
      for (const member of members) { if (member.changedFiles?.length) member.changedFiles.forEach(file => files.add(file)); else bare++; }
      count = files.size + bare;
    }
    return { index, count: members.length, priority: t3ActionPriority(action), label: actionLabel(action, count), failed: members.filter(displayFailed).length };
  });
  const selected = [...summaries].sort((a, b) => a.priority - b.priority || a.index - b.index).slice(0, 2).sort((a, b) => a.index - b.index);
  const labels = selected.map(({ label }) => label);
  const remaining = entries.length - selected.reduce((total, group) => total + group.count, 0);
  if (remaining > 0) labels.push(`Performed ${remaining} other ${remaining === 1 ? 'action' : 'actions'}`);
  const sentence = labels.map((label, index) => index === 0 ? label : label.charAt(0).toLowerCase() + label.slice(1));
  return { summary: sentence.length < 3 ? sentence.join(' and ') : `${sentence.slice(0, -1).join(', ')}, and ${last(sentence)}`,
    hasFailure: summaries.some(group => group.failed > 0) };
}
const t3Action = (entry: WorkEntry) => t3ToolDefinition(entryToolName(entry))?.summaryAction ?? null;
export type SummaryKind = GroupAction | 'reasoning' | 'agent-tool' | 'tone-tool' | 'dynamic-tool' | 'mixed' | 'pull-request';
export function summaryKind(all: WorkEntry[]): SummaryKind {
  const entries = all.filter(entry => entry.itemType !== 'reasoning');
  if (all.length > 0 && entries.length === 0) return 'reasoning';
  if (entries.length > 0 && entries.every(entry => toolPresentation(entry)?.icon === 'pull-request')) return 'pull-request';
  const actions = new Set(entries.map(groupAction));
  if (actions.size !== 1) return 'mixed';
  const action = [...actions][0]!;
  if (action !== 'other') return action;
  const kinds = new Set(entries.map((entry): SummaryKind => entry.itemType === 'dynamic_tool' ? 'dynamic-tool'
    : entry.itemType === 'subagent' || entry.tone === 'thinking' ? 'agent-tool' : entry.tone === 'tool' ? 'tone-tool' : 'other'));
  return kinds.size === 1 ? [...kinds][0]! : 'mixed';
}
/** MessagesTimeline toolGroupSummaryIconName, as icons.contract names. */
export function summaryIcon(kind: SummaryKind): string {
  switch (kind) {
    case 'pull-request': case 'link-pr': case 'unlink-pr': case 'list-prs': case 'watch-pr': case 'unwatch-pr': return 'pull-request';
    case 'browser': return 'browser';
    case 'device': return 'device';
    case 'read': return 'eye';
    case 'edit': return 'square-pen';
    case 'command': return 'terminal';
    case 'thread-create': return 't3-code';
    case 'search': return 'globe';
    case 'code-search': return 'search';
    case 'other': return 'wrench';
    case 'reasoning': return 'brain';
    case 'agent-tool': return 'bot';
    case 'tone-tool': return 'zap';
    default: return 'hammer';
  }
}
export function entryIcon(entry: WorkEntry): string {
  if (entry.itemType === 'notification') {
    if (entry.item.outcome === 'failed') return 'circle-alert';
    const kind = str(obj(entry.item.source).kind);
    return kind === 'subagent' || kind === 'delegated_task' ? 'bot' : kind === 'command' ? 'terminal' : kind === 'monitor' ? 'eye' : 'zap';
  }
  if (entry.itemType === 'user_input_request' || entry.itemType === 'approval_request') return 'message-circle';
  const presentation = toolPresentation(entry);
  if (presentation) return presentation.icon;
  const action = groupAction(entry);
  if (action !== 'other') return summaryIcon(action);
  if (entry.itemType === 'dynamic_tool') return 'wrench';
  if (entry.itemType === 'subagent') return 'bot';
  return entry.tone === 'error' ? 'circle-alert' : entry.tone === 'thinking' ? 'brain' : entry.tone === 'info' ? 'check-mark' : 'zap';
}

/** Reasoning headings read as plain text (MessagesTimeline remarkThoughtPreview). */
export function thoughtText(markdown: string): string {
  return markdown.replace(/```[\s\S]*?```/g, ' ').replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^#{1,6}\s+/gm, '').replace(/^\s*(?:[-*+]|\d+[.)])\s+/gm, '').replace(/^>\s?/gm, '')
    .replace(/(\*\*|__)(.+?)\1/g, '$2').replace(/(\*|_)(.+?)\1/g, '$2').replace(/`([^`]*)`/g, '$1').replace(/~~(.+?)~~/g, '$1')
    .replace(/\s+/g, ' ').trim();
}

export function entryDisplayLabel(entry: WorkEntry, root: string): string {
  if (entry.itemType === 'system_notice') return entry.label;
  if (entry.itemType === 'reasoning' || entry.tone === 'thinking') return thoughtText(entry.detail ?? '') || entry.label;
  const presentation = toolPresentation(entry);
  if (presentation) return presentation.displayName;
  if (entry.command) return commandDisplayText(entry.command);
  const action = groupAction(entry);
  if (action === 'code-search' || action === 'search') {
    const search = entry.itemType === 'file_search' ? entry.label : formatSearchToolLabel(entry.toolData);
    if (search) return search;
  }
  const paths = action === 'read' ? collectToolFilePaths(entry.toolData).map(path => workspaceRelativePath(path, root)) : [];
  if (action === 'read' && paths[0]) return formatReadToolLabel(paths[0], paths.length - 1);
  const title = dynamicTitle(entry);
  if (title) return title;
  const retry = entry.itemType === 'error' && entry.item.retry !== undefined && entry.item.retry !== null;
  const detail = entry.detail?.trim();
  if (detail && !retry && action !== 'read' && !((action === 'code-search' || action === 'search') && /[\r\n]/.test(detail))) return detail;
  const first = entry.changedFiles?.[0];
  if (first) { const path = workspaceRelativePath(first, root); return entry.changedFiles!.length === 1 ? path : `${path} +${entry.changedFiles!.length - 1} more`; }
  if (action === 'read') return 'Read file';
  const heading = (entry.toolTitle || entry.label).replace(/\s+(?:complete|completed)\s*$/i, '').trim();
  return `${heading.charAt(0).toUpperCase()}${heading.slice(1)}`;
}
export function singleToolCallLabel(entry: WorkEntry, root: string): string {
  if (entry.itemType === 'reasoning') return entry.detail?.trim().replace(/\s+/g, ' ') || 'Thought';
  const presentation = toolPresentation(entry);
  if (presentation) return presentation.displayName;
  const title = dynamicTitle(entry);
  if (title) return title;
  if (entry.itemType === 'web_search') return entry.toolTitle ?? 'Web search';
  return entryDisplayLabel(entry, root);
}
export function liveEntryLabel(entry: WorkEntry, root: string, active: boolean): string {
  const status = ['failed', 'declined', 'stopped', 'idle'].includes(entry.status) ? entry.status : active || entry.status === 'inProgress' ? 'inProgress' : 'completed';
  if (entry.itemType === 'reasoning') return thoughtText(entry.detail ?? '') || (status === 'inProgress' ? 'Thinking' : 'Thought');
  const presentation = toolPresentation(entry, status);
  if (presentation) return presentation.displayName;
  const command = entry.command?.trim();
  if (command) {
    const verb = status === 'inProgress' ? 'Running' : status === 'failed' ? 'Failed' : status === 'declined' ? 'Declined' : status === 'stopped' ? 'Stopped' : 'Ran';
    return `${verb} ${commandProgramName(command) ?? 'command'}`;
  }
  return entryDisplayLabel(entry, root);
}
export function activeTurnActivity(entry: WorkEntry): boolean { return entry.status === 'inProgress'; }
export function successKeepsLive(entry: WorkEntry): boolean { return succeeded(entry); }

/** The V2ItemInspector text for an expanded entry: input, exit status, paths or results. */
export function inspectorDetail(entry: WorkEntry, root: string): { input: string; result: string; ok: boolean } {
  const item = entry.item;
  switch (entry.itemType) {
    case 'command_execution': return { input: str(item.input), result: typeof item.exitCode === 'number' && item.exitCode !== 0 ? `exit ${item.exitCode}` : '', ok: item.exitCode === 0 };
    case 'file_change': return { input: item.status === 'failed' && str(item.diffStr) ? str(item.diffStr) : [workspaceRelativePath(str(item.fileName), root), ...arr(item.changes).map(change => `${str(change.operation, str(obj(change.kind).type))} ${workspaceRelativePath(str(change.path), root)}`)].filter(Boolean).join('\n'), result: '', ok: true };
    case 'web_search': return { input: Array.isArray(item.patterns) ? item.patterns.map(pattern => str(pattern)).filter(Boolean).join('\n') : '', result: '', ok: true };
    case 'file_search': return { input: str(item.pattern), result: '', ok: true };
    case 'dynamic_tool': {
      const lines = toolCallLines({ args: item.input });
      return { input: lines.command ?? lines.args?.map(([key, value]) => `${key} ${value}`).join('\n') ?? lines.argsText ?? '', result: '', ok: true };
    }
    case 'approval_request': return { input: str(item.prompt), result: '', ok: true };
    case 'user_input_request': return { input: arr(item.questions).map(question => str(question.question)).join('\n\n'), result: '', ok: true };
    case 'notification': return { input: display(item.detail), result: '', ok: true };
    case 'system_notice': return { input: str(item.message), result: '', ok: true };
    case 'reasoning': return { input: str(item.text), result: '', ok: true };
    default: return { input: '', result: '', ok: true };
  }
}
function display(value: Json | undefined): string { return typeof value === 'string' ? value : value == null ? '' : JSON.stringify(value, null, 2); }
