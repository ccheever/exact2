// Settings › Connections in its two views (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: ConnectionsSettings.tsx
// `view`, PullRequestRoutingSettings.tsx): Environments holds This app, the paired environments, Load balancing and
// Pull request sharing; Sharing & access ("access") holds This machine and Authorized clients.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath, appText } from '../support/app-files';
import { contractComponent, value } from '../support/contract-action-fixture';
import { runConnectionOp } from '../../src/connections/connections';
import { environmentKey } from '../../src/connections/settings-b-fleet';
import { obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';

const source = readFileSync(appPath('src/connections/connections.contract'), 'utf8');
const component = (name: string) => { const start = source.indexOf(`\ncomponent ${name}\n`); return source.slice(start, source.indexOf('\ncomponent ', start + 1)); };

test('the real mount passes its route: Settings › Connections draws Environments, Sharing & access the rest', () => {
  // app-settings.contract mounts the panel on both routes and hands it the route as its `view`.
  const settings = appText('app-settings.contract').split('\n'), at = settings.findIndex(text => text.includes('ConnectionsPanel(view='));
  const mount = settings.slice(0, at).reverse().find(text => text.trim().startsWith('if '))!.trim().slice(3);
  for (const route of ['connections', 'access']) expect(value(mount, { settingsRoute: route })).toBe(true);
  expect(value(mount, { settingsRoute: 'providers' })).toBe(false);
  const viewArgument = /ConnectionsPanel\(view=([^,]+),/.exec(settings[at]!)![1]!;
  const view = value(viewArgument, { settingsRoute: 'connections' });
  expect(view).toBe('connections');
  expect(value(viewArgument, { settingsRoute: 'access' })).toBe('access');
  const panel = contractComponent('connections.contract', 'ConnectionsPanel'), state = panel.state();
  const drawn = (marker: string, route: unknown) => panel.drawn(marker, state, { view: route, width: 800, error: '', page: { network: { visible: true, clientsVisible: true } } });
  for (const section of ['ThisAppSection(', 'ConnectionsEnvironments(']) {
    expect(drawn(section, view)).toBe(true);
    expect(drawn(section, 'access')).toBe(false);
  }
  // The Sharing & access route draws This machine and Authorized clients, and the connections route does not.
  for (const section of ['ThisMachineSection(', 'AuthorizedClients(']) {
    expect(drawn(section, view)).toBe(false);
    expect(drawn(section, 'access')).toBe(true);
  }
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
