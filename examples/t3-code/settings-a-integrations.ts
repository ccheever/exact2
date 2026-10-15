// Integrations → Devices: the version popovers. Reference:
// components/device/DeviceToolVersions.tsx and IntegrationsSettings.tsx
// versionActions (device.list { updateTool } / { inspectOnly }).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { letGo } from './let-go';

// The popover is end-aligned by `position-area="bottom span-left"` (settings-source-control.contract DeviceVersions),
// so no estimated label width offsets it.
export type DeviceTool = { kind: string; title: string; label: string; aria: string; known: boolean; running: string; required: string; installed: string;
  update: string; inspect: boolean; error: string };

/** DeviceToolVersions for one kind ('hub' | 'agent') over the local host's tools. */
export function deviceTool(kind: 'hub' | 'agent', state: Obj | null, error = ''): DeviceTool {
  const tools = state ? obj(arr(state.hosts).find(host => host.kind === 'local')?.tools) : {};
  const selected = state && tools[kind] !== undefined && tools[kind] !== null ? obj(tools[kind]) : null;
  const installed = selected ? (Array.isArray(selected.installedVersions) ? selected.installedVersions : []).map(String) : [];
  const required = selected ? str(selected.requiredVersion) : '';
  const version = selected ? str(selected.runningVersion) || (installed.includes(required) ? required : [...installed].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).pop() || '') : '';
  const title = kind === 'hub' ? 'Device hub' : 'Agent device';
  const label = version ? `v${version}` : selected ? 'Not installed' : 'Version unknown';
  const needsUpdate = Boolean(selected) && !installed.includes(required);
  return { kind, title, label, aria: `${title}: ${version ? `version ${version}` : selected ? 'not installed' : 'version unknown'}. Show details`, known: Boolean(state),
    running: selected ? str(selected.runningVersion) || 'Not running' : '', required, installed: installed.join(', ') || 'None',
    update: state?.supportsToolUpdate === true && needsUpdate ? `Update to v${required}` : '', inspect: state?.supportsToolInspection === true,
    error };
}

/** rest:device-tools — `action=check` re-inspects; `action=update&tool=hub|agent` installs the required version. */
export async function deviceToolsCommand(client: T3Client, native: Native, input: Record<string, string>): Promise<string> {
  const access = client.restAccess(native);
  if (input.action === 'check') { await access.request('device.list', { inspectOnly: true }); return ''; }
  if (input.action === 'update' && (input.tool === 'hub' || input.tool === 'agent')) {
    try { await access.request('device.list', { updateTool: input.tool }, true); }
    catch (error) { if (letGo(error)) throw error; throw new ClientError("Update failed. Check this host's network connection and try again."); }
    return '';
  }
  throw new ClientError('Unsupported device tool action.');
}
