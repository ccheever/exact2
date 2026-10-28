// Settings › Connections in its two views (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: ConnectionsSettings.tsx
// `view`, PullRequestRoutingSettings.tsx): Environments holds This app, the paired environments, Load balancing and
// Pull request sharing; Sharing & access ("access") holds This machine and Authorized clients.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
import { runConnectionOp } from '../../src/connections/connections';
import { environmentKey } from '../../src/connections/settings-b-fleet';
import { obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';

const source = readFileSync(appPath('src/connections/connections.contract'), 'utf8');
const component = (name: string) => { const start = source.indexOf(`\ncomponent ${name}\n`); return source.slice(start, source.indexOf('\ncomponent ', start + 1)); };

test('Sharing & access shows This machine and Authorized clients; Environments the rest', () => {
  const panel = component('ConnectionsPanel'), split = panel.indexOf('if view == "access"'), rest = panel.indexOf('      else\n', split);
  expect(split).toBeGreaterThan(0);
  const access = panel.slice(split, rest), environments = panel.slice(rest);
  expect(access).toContain('ThisMachineSection(');
  expect(access).toContain('AuthorizedClients(');
  expect(access).not.toContain('ThisAppSection(');
  expect(environments).toContain('ThisAppSection(');
  expect(environments).toContain('ConnectionsEnvironments(');
  // This app holds the Local environment switch; This machine no longer does.
  expect(component('ThisAppSection')).toContain('PvSectionTitle(title="This app")');
  expect(component('ThisAppSection')).toContain('id="local-environment-switch"');
  expect(component('ThisMachineSection')).not.toContain('local-environment-switch');
});

test('GitHub sharing is Pull request sharing, in the reference\'s words', async () => {
  const environments = component('ConnectionsEnvironments');
  expect(environments).toContain('CnFolded(title="Pull request sharing"');
  expect(environments).toContain("through each other's source control access");
  expect(environments).toContain('label=`${machine.label} pull request routing`');
  expect(source).not.toContain('GitHub');
  const native: Native = { available: true, watch() {}, later: async input => {
    const request = obj(input);
    if (request.op === 'environments') return { ok: true, generation: 1, value: { saved: [] } };
    if (request.op === 'connectionPreferences') return { ok: true, generation: 1, value: { text: '{}' } };
    return { ok: false, generation: 1, error: { kind: 'Storage', message: 'disk full', uncertain: false } };
  } };
  await expect(runConnectionOp(native, 'pull-request-routing', environmentKey('https://remote.example.com', 'env-b'), 'admin', true))
    .rejects.toThrow('Choose a pull request routing permission.');
});
