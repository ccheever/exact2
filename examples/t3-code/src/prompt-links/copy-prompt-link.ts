// T3 Code 9c161462c9 (MIT, LICENSE-T3): components/promptLinks/copyPromptLink.ts.
// Clipboard writes use the clone's existing native bridge; drafts and model selection live on T3Client.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { isAttachmentOnlyPrompt } from '../composer/attachment-only-prompt';
import { buildNewThreadLink, parsePromptLinkRepository, PROMPT_LINK_MAX_PROMPT_LENGTH, type PromptLinkRepository } from './prompt-links';

export function promptLinkFor(prompt = '', context?: { repository: PromptLinkRepository | null; provider: string | null; model: string | null }): string | null {
  // An attachment-only message's prompt shares as nothing (copyPromptLink at 2359643cff).
  const trimmed = isAttachmentOnlyPrompt(prompt) ? '' : prompt.trim();
  return !trimmed || trimmed.length > PROMPT_LINK_MAX_PROMPT_LENGTH ? null : buildNewThreadLink({ prompt: trimmed, ...context });
}
export function promptLinkRepositoryOf(project: Obj | undefined): PromptLinkRepository | null {
  const identity = obj(project?.repositoryIdentity);
  const key = str(obj(identity.origin).canonicalKey ?? identity.canonicalKey);
  return key.startsWith('github.com/') ? parsePromptLinkRepository(key.slice('github.com/'.length)) : null;
}
export function composerPromptLink(client: T3Client, withContext: boolean): string | null {
  if (!withContext) return promptLinkFor(client.draft);
  const project = client.shell.projects.find(candidate => candidate.id === client.projectId);
  const provider = str(arr(client.config.providers).find(candidate => candidate.instanceId === client.providerId)?.driver) || null;
  return promptLinkFor(client.draft, { repository: promptLinkRepositoryOf(project), provider, model: provider ? client.modelId || null : null });
}
export async function copyPromptLink(client: T3Client, native: Native, link: string | null): Promise<void> {
  if (!link) throw new ClientError('That prompt is empty or too long to share as a link.');
  try {
    await client.restAccess(native).call({ op: 'copyText', text: link });
    pushToast(client, { kind: 'success', title: 'Prompt link copied' });
    client.revision++;
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not copy the prompt link', description: error instanceof Error ? error.message : 'An error occurred.', stacked: true });
    client.revision++;
  }
}
