import { describe, expect, test } from 'bun:test';
import { blankHomeProvider, mobileHomeProvider } from './home-provider';
import { projectMobileHome, type HomeSource } from './home';
import { initialShell, type Obj } from './shared/domain';

const owner = (instanceId: string, driver = instanceId, rest: Obj = {}): Obj => ({ instanceId, driver, ...rest });
const thread = (instanceId = 'codex', rest: Obj = {}): Obj => ({ modelSelection: { instanceId }, ...rest });
const render = (providers: Obj[], value = thread()) => mobileHomeProvider(value, { providers });

describe('source mobile Home provider owner and badge', () => {
  test('runtime current owner overrides selection without fabricating a missing owner', () => {
    const providers = [owner('codex'), owner('work', 'claudeAgent')];
    expect(render(providers, thread('codex', { runtime: { providerInstanceId: 'work' } })).driver).toBe('claudeAgent');
    expect(render(providers, thread('codex', { runtime: { providerInstanceId: 'gone' }, providerInstanceHistory: ['codex'] }))).toEqual(blankHomeProvider());
    expect(render(providers, thread('codex', { runtime: { providerInstanceId: null } })).driver).toBe('codex');
    expect(mobileHomeProvider(thread(), {})).toEqual(blankHomeProvider());
    expect(render(providers, thread('missing'))).toEqual(blankHomeProvider());
  });
  test('newest two previous instance IDs survive, preserving same-driver handoffs', () => {
    const providers = [owner('codex'), owner('old'), owner('work', 'codex'), owner('home', 'codex')];
    const result = render(providers, thread('codex', { providerInstanceHistory: ['old', 'codex', 'work', 'home'] }));
    expect(result.previous).toEqual([{ key: '0:work', driver: 'codex' }, { key: '1:home', driver: 'codex' }]);
    expect(render(providers, thread('codex', { providerInstanceHistory: ['old', 'work', 'missing'] })).previous).toEqual([{ key: '0:work', driver: 'codex' }]);
    expect(render(providers, thread('codex', { providerInstanceHistory: ['work', 'work'] })).previous.map(row => row.key)).toEqual(['0:work', '1:work']);
  });
  test('configured disabled and unknown drivers retain their identity', () => {
    expect(render([owner('codex', 'codex', { enabled: false, status: 'disabled' })]).visible).toBe(true);
    const result = render([owner('custom', 'unknownDriver')], thread('custom'));
    expect(result.driver).toBe('unknownDriver'); expect(result.visible).toBe(true);
  });
  test('brand names, custom slugs, explicit names and Unicode produce source initials', () => {
    expect(render([owner('codex')]).badge).toBe('CO');
    expect(render([owner('codex_personal', 'codex', { displayName: 'Codex' })], thread('codex_personal')).badge).toBe('CP');
    expect(render([owner('myCustomInstance', 'codex')], thread('myCustomInstance')).badge).toBe('MC');
    expect(render([owner('codex', 'codex', { displayName: ' Team Red ' })]).badge).toBe('TR');
    expect(render([owner('codex', 'codex', { displayName: '🦊🦉' })]).badge).toBe('🦊🦉');
    expect(render([owner('codex', 'codex', { displayName: '🦊 team' })]).badge).toBe('🦊T');
  });
  test('badge uses normalized accent or all same-driver siblings, including ACP and disabled', () => {
    expect(render([owner('codex')]).showBadge).toBe(false);
    const accent = render([owner('codex', 'codex', { accentColor: ' #aAbB09 ' })]);
    expect(accent.showBadge).toBe(true); expect(accent.badgeColor).toBe('#aAbB09');
    for (const accentColor of ['red', '#abc', '#11223344', '']) {
      const result = render([owner('codex', 'codex', { accentColor })]);
      expect(result.showBadge).toBe(false); expect(result.badgeColor).toBe('');
    }
    expect(render([owner('codex'), owner('disabled', 'codex', { enabled: false })]).showBadge).toBe(true);
    expect(render([owner('devin', 'acpRegistry', { acpRegistryAgentId: 'devin' }), owner('other', 'acpRegistry', { acpRegistryAgentId: 'other' })], thread('devin')).showBadge).toBe(true);
  });
  test('only the current provider carries an official ACP icon URL', () => {
    const good = 'https://cdn.agentclientprotocol.com/icons/devin.svg';
    expect(render([owner('agent', 'acpRegistry', { iconUrl: good })], thread('agent')).iconUrl).toBe(good);
    for (const iconUrl of ['http://cdn.agentclientprotocol.com/a.svg', 'https://evil.test/a.svg', 'https://user@cdn.agentclientprotocol.com/a.svg']) {
      expect(render([owner('agent', 'acpRegistry', { iconUrl })], thread('agent')).iconUrl).toBe('');
    }
  });
  test('projection scopes colliding provider IDs and suppresses slim/draft/shelf accessories', () => {
    const now = Date.parse('2026-10-09T12:00:00Z');
    const makeSource = (id: string, driver: string): HomeSource => ({ environmentId: id, label: id, machine: 'laptop', focused: id === 'one',
      config: { providers: [owner('same', driver)], environment: { capabilities: { threadSettlement: true, threadSnooze: true } } },
      shell: { ...initialShell(), projects: [{ id: 'project', title: 'Project', workspaceRoot: '/repo' }], threads: [
        { ...thread('same'), id: 'active', projectId: 'project', title: 'Active', createdAt: new Date(now).toISOString(), archivedAt: null },
        { ...thread('same'), id: 'settled', projectId: 'project', title: 'Settled', settledOverride: 'settled', archivedAt: null },
        { ...thread('same'), id: 'snoozed', projectId: 'project', title: 'Snoozed', snoozedUntil: new Date(now + 60_000).toISOString(), archivedAt: null },
      ] } });
    const result = projectMobileHome([makeSource('one', 'codex'), makeSource('two', 'claudeAgent')], now, {
      settledExpanded: true, snoozedExpanded: true,
      drafts: [{ key: 'new-task:draft', origin: 'https://one.test', environmentId: 'one', projectId: 'project', createdAt: new Date(now).toISOString(), text: 'Draft', images: [], files: [], workspace: null }],
    });
    expect(result.items.filter(row => row.threadId === 'active').map(row => [row.environmentId, row.provider.driver])).toEqual([['one', 'codex'], ['two', 'claudeAgent']]);
    expect(result.items.some(row => row.kind === 'draft')).toBe(true);
    for (const row of result.items.filter(row => row.threadId !== 'active')) expect(row.provider).toEqual(blankHomeProvider());
    expect(result.items.find(row => row.threadId === 'active')?.faviconTarget.cwd).toBe('/repo');
  });
});
