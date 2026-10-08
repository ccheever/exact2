// The ChatGPT plan surfaces, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/shared/src/usageLimits.ts:24-29 (CHATGPT_USAGE_URL, usesChatGptSharing),
// apps/web/src/components/settings/ChatGptWelcomeCoordinator.tsx (the one-time "Your ChatGPT
// plan is connected" dialog: one per environment, instance and profile, acknowledged on close),
// chat/ChatGptSharingControl.tsx (the model picker's "Using ChatGPT plan") and
// usage/UsagePage.tsx:543-548 ("ChatGPT shared usage"). Changes: the acknowledged keys live in
// the clone's versioned preference file (t3-code.json `shell.chatgptSharingWelcome`, as the
// other notice dismissals in shell-prefs.ts) instead of localStorage
// `t3:chatgpt-sharing-welcome:v1`; the command that adds one saves the file before it answers.
import { obj, str, type Obj } from './domain';
import { shellPrefs } from './shell-prefs';

export const CHATGPT_USAGE_URL = 'https://chatgpt.com/#settings/Usage';

export function usesChatGptSharing(provider: Obj | null | undefined): boolean {
  return obj(provider?.auth).status === 'authenticated' && obj(provider?.auth).subscriptionSharing === true;
}

/** A connected environment's providers, with its label (environmentPresentations' connected entries). */
export type PlanEnvironment = { environmentId: string; label: string; providers: Obj[] };

/** ChatGptWelcomeCoordinator's profiles: one key per environment, instance and profile id / email / "default". */
export function chatGptPlanProfiles(environments: PlanEnvironment[]) {
  return environments.flatMap(environment => environment.providers.filter(usesChatGptSharing).map(provider => {
    const auth = obj(provider.auth);
    return {
      key: JSON.stringify([environment.environmentId, str(provider.instanceId), str(auth.profileId) || str(auth.email) || 'default']),
      environmentLabel: environment.label, providerName: str(provider.displayName) || 'Codex',
    };
  }));
}

type Holder = { local: object };
const acknowledged = (owner: Holder): string[] => shellPrefs(owner).chatgptSharingWelcome;

/** The dialog to show now: the first connected profile not acknowledged yet, or none. */
export function chatGptPlanNotice(owner: Holder, environments: PlanEnvironment[]) {
  const done = acknowledged(owner);
  const next = chatGptPlanProfiles(environments).find(profile => !done.includes(profile.key));
  return next ? [{ ...next, detail: `${next.providerName} on ${next.environmentLabel}` }] : [];
}

/** Close or Continue: remember this profile (the caller saves the preference file). */
export function acknowledgeChatGptPlan(owner: object, key: string): void {
  if (!key || key.length > 1024 || !('local' in owner)) return;
  const holder = owner as Holder, done = acknowledged(holder);
  if (done.includes(key)) return;
  shellPrefs(holder).chatgptSharingWelcome = [...done, key].slice(-200);
}
