// ChatGptWelcomeCoordinator's input (managed-codex-chatgpt): every connected environment's
// providers, the focused connection and each background one (environmentPresentations' connected
// entries), labelled as the reference labels them (the environment's own label).
import { arr, obj, str } from './domain';
import type { T3Client } from './client';
import { fleet } from './settings-b-fleet';
import { chatGptPlanNotice, type PlanEnvironment } from './chatgpt-plan';

export function chatGptPlanSnapshot(client: T3Client) {
  const environments: PlanEnvironment[] = [];
  const label = (config: object, fallback: string) => str(obj(obj(config).environment).label) || fallback;
  if (client.connection === 'connected' && client.environmentId) environments.push({ environmentId: client.environmentId, label: label(client.config, 'this environment'), providers: arr(client.config.providers) });
  for (const entry of fleet.entries.values()) {
    if (entry.phase !== 'connected' || environments.some(environment => environment.environmentId === entry.environmentId)) continue;
    environments.push({ environmentId: entry.environmentId, label: label(entry.config, entry.origin), providers: arr(entry.config.providers) });
  }
  return chatGptPlanNotice(client, environments);
}
