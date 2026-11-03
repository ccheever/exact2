// SidebarUpdatePill under a managed policy (T3 Code 5d3d78ab93, MIT; LICENSE-T3): when this machine's policy turns
// updates off, the idle tooltip is "Updates unavailable" over the policy's message, and the button's label joins them.
import { afterEach, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { appText } from '../support/app-files';

afterEach(() => { primary.target = null; });
const at = '2026-11-01T00:00:00.000Z';
const identity = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };

function denial(managedPolicy?: Obj): string {
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', loaded: true, configLive: true, shellLive: true, presentation: {} });
  client.config = { environment: { label: 'This Mac', capabilities: {} }, settings: {}, providers: [], keybindings: [], ...(managedPolicy ? { managedPolicy } : {}) };
  client.shell = { projects: [], threads: [], sequence: 1 };
  primary.target = { environmentId: 'env-a' } as PrimaryTarget;
  return sidebarSnapshot(client, Date.parse(at), identity).sidebar.updatesDenial;
}

test('the denial is the primary environment\'s policy refusing updates, and nothing otherwise', () => {
  expect(denial()).toBe('');
  expect(denial({ organization: 'Example Corp', lockedSettings: [], limits: { updates: { mode: 'auto', allowNightly: false } } })).toBe('');
  expect(denial({ organization: 'Example Corp', lockedSettings: [], limits: { updates: { mode: 'off' } } })).toBe('Example Corp manages T3 Code updates.');
});

test('the pill and its tooltip read the denial', () => {
  expect(appText('src/sidebar/sidebar-icons-kit.contract')).toContain('aria-label=(updatesDenial == "" ? "Check for updates" : `Updates unavailable: ${updatesDenial}`)');
  const tips = appText('src/sidebar/sidebar-control-tips.contract');
  expect(tips).toContain('UpdateUnavailableTip(detail=updatesDenial, shown=(hoverId == "sidebar-update"))');
  expect(tips).toContain('text "Updates unavailable" font-size="0.75rem" line-height="1.03125rem" font-weight=500');
  expect(appText('src/sidebar/sidebar.contract')).toContain('updatesDenial=data.sidebar.updatesDenial)');
});
