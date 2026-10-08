// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) packages/shared/src/usageLimits.test.ts
// ":1193 ChatGPT sharing presentation" (original names), then ChatGptWelcomeCoordinator's
// once-per-profile rule against the clone's preference record (no reference test exists).
import { describe, expect, it } from 'bun:test';
import { acknowledgeChatGptPlan, chatGptPlanNotice, usesChatGptSharing, CHATGPT_USAGE_URL } from './chatgpt-plan';
import { adoptShellPrefs, shellPrefs } from './shell-prefs';
import type { Obj } from './domain';

const provider = (patch: Obj = {}): Obj => ({ instanceId: 'codex', driver: 'codex', displayName: 'Codex', ...patch });

describe('ChatGPT sharing presentation', () => {
  it('requires verified sharing metadata rather than the Codex driver or login type', () => {
    const codex = provider({ auth: { status: 'authenticated', type: 'chatgpt' } });
    expect(usesChatGptSharing(codex)).toBe(false);
    expect(usesChatGptSharing({ ...codex, auth: { ...(codex.auth as Obj), subscriptionSharing: true } })).toBe(true);
    expect(usesChatGptSharing({ ...codex, auth: { status: 'unauthenticated', subscriptionSharing: true } })).toBe(false);
  });
});

describe('"Your ChatGPT plan is connected" (ChatGptWelcomeCoordinator)', () => {
  it('shows once per environment, instance and profile, and the acknowledgment survives a reload', () => {
    const owner = { local: {} as Obj };
    const shared = (instanceId: string, profileId: string) => provider({ instanceId, displayName: `ChatGPT - ${instanceId}`, auth: { status: 'authenticated', subscriptionSharing: true, profileId } });
    const environments = [{ environmentId: 'env-a', label: 'This Mac', providers: [shared('codex_one', 'oaiapp_one'), provider({ auth: { status: 'authenticated' } })] },
      { environmentId: 'env-b', label: 'Devbox', providers: [shared('codex_two', 'oaiapp_two')] }];
    const first = chatGptPlanNotice(owner, environments);
    expect(first).toEqual([{ key: JSON.stringify(['env-a', 'codex_one', 'oaiapp_one']), environmentLabel: 'This Mac', providerName: 'ChatGPT - codex_one', detail: 'ChatGPT - codex_one on This Mac' }]);
    acknowledgeChatGptPlan(owner, first[0]!.key);
    const second = chatGptPlanNotice(owner, environments);
    expect(second.map(notice => notice.detail)).toEqual(['ChatGPT - codex_two on Devbox']);
    acknowledgeChatGptPlan(owner, second[0]!.key);
    expect(chatGptPlanNotice(owner, environments)).toEqual([]);
    // A relaunch reads the saved record: no dialog for those profiles; a new profile asks again.
    const relaunched = { local: {} as Obj };
    adoptShellPrefs(relaunched.local, JSON.parse(JSON.stringify({ shell: shellPrefs(owner) })));
    expect(chatGptPlanNotice(relaunched, environments)).toEqual([]);
    expect(chatGptPlanNotice(relaunched, [{ environmentId: 'env-a', label: 'This Mac', providers: [shared('codex_one', 'oaiapp_other')] }])).toHaveLength(1);
    expect(CHATGPT_USAGE_URL).toBe('https://chatgpt.com/#settings/Usage');
  });
});
