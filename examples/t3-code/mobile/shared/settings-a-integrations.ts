// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/settings-a-integrations.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Integrations → Devices: the version popovers (lane settings-a). Reference:
// components/device/DeviceToolVersions.tsx and IntegrationsSettings.tsx
// versionActions (device.list { updateTool } / { inspectOnly }).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { letGo } from './let-go';

export type DeviceTool = { kind: string; title: string; label: string; aria: string; known: boolean; running: string; required: string; installed: string;
  update: string; inspect: boolean; offset: number; error: string };

// SF Pro 16pt medium advance widths, rounded: enough to put the popup's end at the trigger's end.
const WIDE = /[mwMW]/, NARROW = /[ilj.,:;'|! ]/, SLIM = /[tfr]/;
export function labelWidth(text: string, size = 16): number {
  let units = 0;
  for (const char of text) units += WIDE.test(char) ? 0.82 : NARROW.test(char) ? 0.25 : SLIM.test(char) ? 0.33 : /[A-Z]/.test(char) ? 0.65 : /[0-9]/.test(char) ? 0.59 : 0.55;
  return Math.round(units * size * 10) / 10;
}

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
    offset: Math.max(0, labelWidth(label)) - 320, error };
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
