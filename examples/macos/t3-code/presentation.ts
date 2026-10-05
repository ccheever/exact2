import { arr, obj, str, num, messages, visibleTurnItems, type Message, type Obj } from './domain';
import { activeRun, providerAvailable } from './protocol';
import type { T3Client } from './client';

const modes: Record<string, string> = {
  'approval-required': 'Ask for approval', 'auto-accept-edits': 'Auto-accept edits',
  auto: 'Auto', 'full-access': 'Full access',
};
function section(thread: Obj): string {
  if (thread.pinnedAt) return 'pinned';
  if (thread.snoozedUntil) return 'snoozed';
  if (thread.settledOverride === 'settled' || thread.settledAt && thread.settledOverride !== 'active') return 'settled';
  if (thread.activeRunId || ['preparing', 'starting', 'running', 'waiting'].includes(str(thread.status))) return 'working';
  return 'active';
}

function projectMark(name: string): string {
  const words = name.trim().split(/[\s_-]+/);
  return ((words[0]?.[0] || '') + (name.match(/\d$/)?.[0] || words[1]?.[0] || '')).toUpperCase();
}
function age(value: unknown, now: number): string {
  const elapsed = now - Date.parse(str(value));
  if (!Number.isFinite(elapsed)) return '';
  const minutes = Math.max(1, Math.floor(elapsed / 60_000));
  return minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`;
}

/** T3's compact work disclosure: retain every detail without boxing each event. */
export function transcriptPresentation(client: T3Client): Message[] {
  const source = messages(client.thread), rows = visibleTurnItems(client.thread);
  const result: Message[] = [];
  let group: Message | undefined, groupRun = '';
  for (let index = 0; index < source.length; index++) {
    const item = source[index]!;
    if (item.kind !== 'tool' && item.kind !== 'reasoning') {
      result.push(item); group = undefined; continue;
    }
    const runId = str(obj(rows[index]?.item).runId);
    if (!group || groupRun !== runId) {
      const run = arr(client.projection.runs).find(run => run.id === runId);
      const seconds = Math.round((Date.parse(str(run?.completedAt)) - Date.parse(str(run?.startedAt))) / 1000);
      const duration = Number.isFinite(seconds) && seconds >= 0
        ? seconds >= 60 ? `${Math.floor(seconds / 60)}m ${seconds % 60}s` : `${seconds}s` : '';
      const working = run && ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status));
      group = { id: `work-${item.id}`, kind: 'work', title: working ? 'Working…' : duration ? `Worked for ${duration}` : 'Activity', body: '' };
      groupRun = runId; result.push(group);
    }
    group.body += `${group.body ? '\n\n' : ''}${item.title}${item.body ? `\n${item.body}` : ''}`;
  }
  return result;
}
function threadStatus(thread: Obj): string {
  if (thread.pendingRuntimeRequest) return 'Needs input';
  if (thread.status === 'failed') return 'Error';
  if (thread.status === 'queued') return 'Queued';
  if (['preparing', 'starting', 'running'].includes(str(thread.status))) return 'Working';
  if (thread.status === 'waiting') return 'Finishing';
  if (arr(thread.pendingBackgroundTasks).length) return 'Waiting';
  return '';
}

export function diffPresentation(diff: string) {
  const files: { id: string; name: string; additions: number; deletions: number }[] = [];
  let file: typeof files[number] | undefined;
  const lines = diff ? diff.split('\n').map((text, index) => {
    const kind = text.startsWith('diff --git ') || text.startsWith('--- ') || text.startsWith('+++ ')
      ? 'file' : text.startsWith('@@') ? 'hunk' : text.startsWith('+') ? 'addition'
        : text.startsWith('-') ? 'deletion' : 'context';
    if (text.startsWith('diff --git ')) {
      const name = text.match(/ b\/(.*)$/)?.[1] || text.slice(11);
      file = { id: String(index), name, additions: 0, deletions: 0 };
      files.push(file);
    } else if (text.startsWith('+++ b/') && file) file.name = text.slice(6);
    if (file && kind === 'addition') file.additions++;
    if (file && kind === 'deletion') file.deletions++;
    return { id: String(index), kind, text: text || ' ' };
  }) : [];
  return { diffLines: lines, diffFiles: files };
}

function responseReason(client: T3Client, responseType: string, approval: boolean): string {
  if (responseType === 'not_resumable') return 'The provider process is gone. Continue the thread in T3 Code.';
  if (approval && responseType !== 'live') return 'This approval is no longer live. Continue the thread in T3 Code.';
  if (client.connection !== 'connected') return 'Reconnect to respond to this request.';
  if (!client.ready) return 'Wait for synchronization to finish before responding.';
  if (!client.scopes.includes('orchestration:operate')) return 'This connection does not have permission to respond.';
  if (!client.writable) return 'This server does not support responses from this client.';
  if (client.pending || client.busy) return 'Wait for the current submission to finish before responding.';
  return '';
}

function requests(client: T3Client) {
  const approvals: { id: string; label: string; detail: string; canRespond: boolean; responseReason: string; options: { id: string; label: string }[] }[] = [];
  const questions: { id: string; requestId: string; header: string; question: string; multiSelect: boolean; canRespond: boolean;
    responseReason: string; answer: string; last: boolean; customAllowed: boolean; options: { id: string; label: string; description: string; selected: boolean }[] }[] = [];
  const items = arr(client.projection.turnItems);
  for (const request of arr(client.projection.runtimeRequests)) {
    if (request.status !== 'pending') continue;
    const requestId = str(request.id);
    const item = items.slice().reverse().find(item => item.requestId === request.id);
    if (!item) continue;
    const responseType = str(obj(request.responseCapability).type);
    const reason = responseReason(client, responseType, item.type === 'approval_request');
    const canRespond = !reason;
    if (item.type === 'approval_request') {
      const offered = arr(item.options);
      const options = offered.length ? offered.map(option => ({
        id: str(option.decision), label: str(option.label) + (option.warning ? ` — ${str(option.warning)}` : ''),
      })) : [
        { id: 'accept', label: 'Allow once' }, { id: 'acceptForSession', label: 'Allow for session' },
        { id: 'decline', label: 'Decline' }, { id: 'cancel', label: 'Cancel' },
      ];
      approvals.push({ id: requestId, label: str(item.appName) ? `${str(item.appName)} requests approval` : 'Approval required',
        detail: str(item.prompt, str(item.title, 'The provider is waiting for permission.')),
        canRespond, responseReason: reason, options });
    } else if (item.type === 'user_input_request') {
      const entries = arr(item.questions);
      entries.forEach((question, index) => {
        const id = `${requestId}::${str(question.id)}`, answer = client.answers[id];
        questions.push({ id, requestId, header: str(question.header), question: str(question.question),
          multiSelect: question.multiSelect === true, canRespond, responseReason: reason, last: index === entries.length - 1,
          customAllowed: question.allowCustomAnswer !== false,
          answer: typeof answer === 'string' ? answer : '',
          options: arr(question.options).map(option => {
            const value = typeof option.value === 'string' ? option.value : str(option.label);
            return { id: value, label: str(option.label), description: str(option.description),
              selected: Array.isArray(answer) ? answer.includes(value) : answer === value };
          }),
        });
      });
    }
  }
  return { approvals, questions };
}

export function snapshot(client: T3Client, now = 0) {
  const project = client.shell.projects.find(project => project.id === client.projectId);
  const providers = arr(client.config.providers);
  const provider = providers.find(provider => provider.instanceId === client.providerId);
  const currentModel = arr(provider?.models).find(model => model.slug === client.modelId);
  const option = arr(obj(currentModel?.capabilities).optionDescriptors)
    .find(option => option.type === 'select' && ['reasoningEffort', 'effort'].includes(str(option.id)));
  const selectedOption = client.modelOptions.find(selection => selection.id === option?.id)?.value
    ?? option?.currentValue ?? arr(option?.options).find(choice => choice.isDefault === true)?.id;
  const modelReady = !!provider && providerAvailable(provider) && !!currentModel;
  const run = activeRun(client.projection);
  const pending = client.pending;
  const query = client.query.toLowerCase().trim();
  const transcript = transcriptPresentation(client);
  for (const request of arr(client.projection.runtimeRequests)) {
    if (request.status === 'pending' && ['auth_refresh', 'dynamic_tool_call'].includes(str(request.kind))) {
      transcript.push({ id: `unsupported-${str(request.id)}`, kind: 'system', title: 'Continue in T3 Code',
        body: 'This provider is waiting for a request that must be handled in the T3 Code app.' });
    }
  }
  const connectionMessage = !client.available ? 'Open on macOS to connect.'
    : client.connection === 'connected' ? client.ready ? 'Connected' : 'Synchronizing…'
      : client.connection === 'reconnecting' ? 'Reconnecting…' : client.connection === 'connecting' ? 'Connecting…'
        : client.statusMessage || 'Disconnected';
  return {
    revision: client.revision, available: client.available, connected: client.connection === 'connected',
    connecting: ['connecting', 'reconnecting'].includes(client.connection), syncComplete: client.ready,
    status: connectionMessage, error: client.error, serverUrl: client.origin,
    uncertain: pending?.uncertain === true,
    uncertainMessage: pending?.uncertain ? `${pending.description} may already have reached T3. Reconnect and check the thread before retrying.` : '',
    sidebarWidth: client.local.sidebarWidth, sidebarOpen: client.local.sidebarOpen, query: client.query,
    projectId: client.projectId, projectName: str(project?.title, 'Choose a project'), threadId: client.threadId,
    threadTitle: str(obj(client.projection.thread).title, 'New thread'), draft: client.draft,
    settled: section(obj(client.projection.thread)) === 'settled', projectMark: projectMark(str(project?.title)),
    running: !!run, canSend: client.writable && !pending && !client.busy && modelReady && !!client.projectId,
    canStop: client.writable && !pending && !client.busy && !!run,
    providerId: client.providerId, modelId: client.modelId, modelLabel: str(currentModel?.name, client.modelId || 'Choose model'),
    providerDriver: str(provider?.driver), optionId: str(option?.id),
    optionLabel: str(arr(option?.options).find(choice => choice.id === selectedOption)?.label, 'Default'),
    modelOptions: arr(option?.options).map(choice => ({ id: str(choice.id), value: str(choice.id), label: str(choice.label), selected: choice.id === selectedOption })),
    runtimeMode: client.runtimeMode, modeLabel: modes[client.runtimeMode] || client.runtimeMode, interactionMode: client.interactionMode,
    hasMore: client.thread?.hasMore === true, historyLoading: client.historyLoading,
    diffOpen: client.diffOpen, diffLoading: client.diffLoading, diffError: client.diffError,
    diffText: client.diffText, diffTitle: client.diffTitle, ...diffPresentation(client.diffText),
    projects: client.shell.projects.map(project => ({ id: str(project.id), name: str(project.title), path: str(project.workspaceRoot), selected: project.id === client.projectId })),
    threads: client.shell.threads.filter(thread => (!client.projectId || thread.projectId === client.projectId)
      && (!query || `${str(thread.title)} ${str(obj(thread.latestVisibleMessage).text)}`.toLowerCase().includes(query)))
      .slice().sort((a, b) => str(b.updatedAt).localeCompare(str(a.updatedAt)))
      .map(thread => {
        const projectName = str(client.shell.projects.find(project => project.id === thread.projectId)?.title);
        const prs = arr(thread.pullRequests);
        return { id: str(thread.id), title: str(thread.title, 'Untitled thread'), projectName, projectMark: projectMark(projectName),
          age: age(thread.updatedAt, now), badge: prs.length > 1 ? String(prs.length) : prs.length === 1 ? String(num(prs[0]?.number)) : '', stacked: prs.length > 1,
          section: section(thread), status: threadStatus(thread), selected: thread.id === client.threadId };
      }),
    messages: transcript,
    providers: providers.map(provider => ({ id: str(provider.instanceId), name: str(provider.displayName, str(provider.driver)), available: providerAvailable(provider) })),
    models: providers.flatMap(provider => arr(provider.models).map(model => ({ id: str(model.slug), name: str(model.name, str(model.slug)),
      providerId: str(provider.instanceId), selected: provider.instanceId === client.providerId && model.slug === client.modelId }))),
    ...requests(client),
  };
}
