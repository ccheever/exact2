import { describe, expect, test } from 'bun:test';
import { applyConfig } from './protocol';
import { slashRows } from './composer-editor-menu';

// /usage-limits (composer-editor): the server lists the command only for a
// subscriber that answers it, and its quota arrives as a source stream.
describe('/usage-limits feed', () => {
  test('usageLimitSourcesUpdated lands on the config, a snapshot keeps the stream for later', () => {
    const snapshot = applyConfig({}, { type: 'snapshot', config: { providers: [{ instanceId: 'codex', slashCommands: [{ name: 'usage-limits' }] }], environment: { environmentId: 'env' } } });
    const sources = [{ id: 'openai', accounts: [{ driver: 'codex', label: 'OpenAI API Key', usageLimits: { windows: [], unavailable: { reason: 'unsupported' } } }] }];
    const next = applyConfig(snapshot, { type: 'usageLimitSourcesUpdated', payload: { sources } });
    expect(next.usageLimitSources).toEqual(sources);
    expect(next.providers).toEqual(snapshot.providers);
  });
  test('the server-injected command is a provider row that opens the prompt', () => {
    const rows = slashRows({ query: 'usage', atPromptStart: true, planModeUiEnabled: false, compactAvailable: false, driver: 'codex',
      slashCommands: [{ name: 'usage-limits', description: "Show this provider's usage limits" }], skills: [], showSkillsInSlashMenu: true });
    expect(rows.map(row => row.id)).toEqual(['provider-slash-command:codex:usage-limits']);
  });
});
