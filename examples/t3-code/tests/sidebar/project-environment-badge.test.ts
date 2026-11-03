// ProjectEnvironmentBadge (T3 Code 5d3d78ab93 components/ProjectEnvironmentBadge.tsx, MIT; see LICENSE-T3): the machine
// a project group's checkouts off the primary environment live on, after the pin in the project pickers.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { EnvironmentFleet, environmentKey, projectEnvironmentBadge, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { LocalPrimary, type PrimaryTarget } from '../../src/connections/local-primary';
import { appText } from '../support/app-files';

const local = new LocalPrimary();
local.target = { environmentId: 'mac', httpBaseUrl: 'http://127.0.0.1:3773' } as PrimaryTarget;
const fleet = new EnvironmentFleet();
for (const [id, label, machine] of [['box', 'Build box', 'linux'], ['cloud', 'Cloud', '']] as const) {
  const key = environmentKey(`http://${id}.test`, id);
  fleet.entries.set(key, { key, origin: `http://${id}.test`, environmentId: id, phase: 'connected', generation: 1, synchronized: 1,
    config: { environment: { label, platform: { machine } } }, shell: { projects: [], threads: [], sequence: 0 } } as unknown as FleetEntry);
}
const client = { environmentId: 'mac', config: { environment: { label: 'This Mac' } } } as unknown as T3Client;
const badge = (...environments: string[]) => projectEnvironmentBadge(client, environments.map(environmentId => ({ environmentId })), local, fleet);

test('a group only on the primary has no badge', () => {
  expect(badge('mac')).toBeNull();
});

test('a group off the primary reads "On", one also on it "Also on", with the first environment\'s machine', () => {
  expect(badge('box')).toEqual({ description: 'On Build box', machine: 'linux' });
  expect(badge('mac', 'cloud', 'box')).toEqual({ description: 'Also on Build box, Cloud', machine: 'linux' });
  expect(badge('cloud', 'cloud')).toEqual({ description: 'On Cloud', machine: 'server' });
  expect(badge('gone')).toEqual({ description: 'On Remote', machine: 'server' });
});

test('the pickers draw the badge after the pin: the sidebar\'s project filter and the new thread\'s project menu', () => {
  for (const [file, pin, env] of [['sidebar-overlays.contract', 'testId=`scope-pin-', 'testId=`scope-env-'], ['src/pages/pages-hero.contract', 'testId=`hero-project-pin-', 'testId=`hero-project-env-']]) {
    const lines = appText(file).split('\n'), at = (marker: string) => lines.findIndex(line => line.includes(marker));
    expect(at(pin)).toBeGreaterThan(0);
    expect(at(env)).toBeGreaterThan(at(pin));
  }
});
