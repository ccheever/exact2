import type { ProviderHost } from './providers';

/** The welcome's onAccountCreated (pages-welcome.ts registers it): the new row comes first and starts. */
let onChatGptCreated: ((host: ProviderHost, instanceId: string, displayName: string) => void) | null = null;
export function setChatGptCreatedHandler(handler: typeof onChatGptCreated): void { onChatGptCreated = handler; }

export function notifyChatGptCreated(host: ProviderHost, instanceId: string, displayName: string): void {
  onChatGptCreated?.(host, instanceId, displayName);
}
