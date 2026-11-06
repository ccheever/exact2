// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/providers-meta.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Provider driver metadata pinned from T3's providerDriverMeta.ts and the
// contracts settings schemas (CodexSettings … OpenCodeSettings). The server
// stays the authority for values; this table only names and orders fields.
import { obj, str, type Obj } from './domain';

export type DriverField = {
  key: string; label: string; description: string; placeholder: string;
  control: 'text' | 'password' | 'select'; persist?: boolean; options?: { value: string; label: string }[];
};
export type Driver = {
  id: string; label: string; enabledByDefault: boolean; defaultSlot: boolean;
  fields: DriverField[]; env: DriverField[]; legacyDefault: Obj | null; modelPlaceholder: string; color: string;
};

const field = (key: string, label: string, description: string, placeholder = '', extra: Partial<DriverField> = {}): DriverField =>
  ({ key, label, description, placeholder, control: 'text', ...extra });
const binary = (description: string, placeholder: string) => field('binaryPath', 'Binary path', description, placeholder);

export const ANTIGRAVITY_AUTH_METHODS = [
  { value: 'oauth-personal', label: 'Google account' },
  { value: 'oauth-business', label: 'Gemini Enterprise' },
  { value: 'gemini-api-key', label: 'Gemini API key' },
  { value: 'agent-platform', label: 'Agent Platform (Vertex AI)' },
];

export const DRIVERS: Driver[] = [
  { id: 'codex', label: 'Codex', enabledByDefault: true, defaultSlot: true, color: 'light-dark(#000000, #ffffff)', modelPlaceholder: 'gpt-6.7-codex-ultra-preview',
    legacyDefault: { enabled: true, binaryPath: 'codex', homePath: '', shadowHomePath: '', launchArgs: '', customModels: [] }, env: [],
    fields: [binary('Path to the Codex binary used by this instance.', 'codex'),
      field('homePath', 'CODEX_HOME path', 'Custom Codex home and config directory.', '~/.codex'),
      field('shadowHomePath', 'Shadow home path', 'Account-specific Codex home. Keeps auth.json separate while sharing state from CODEX_HOME.', '~/.codex-t3/personal'),
      field('launchArgs', 'Launch arguments', 'Additional CLI arguments passed to codex app-server on session start.')] },
  { id: 'claudeAgent', label: 'Claude', enabledByDefault: true, defaultSlot: true, color: '#d97757', modelPlaceholder: 'claude-sonnet-5',
    legacyDefault: { enabled: true, binaryPath: 'claude', homePath: '', customModels: [], launchArgs: '', autoCompactWindow: '' }, env: [],
    fields: [binary('Path to the Claude binary used by this instance.', 'claude'),
      field('homePath', 'CLAUDE_CONFIG_DIR path', 'Custom Claude home and config directory. Keeps .claude.json and .claude separate.', '~/.claude'),
      field('autoCompactWindow', 'Auto-compact after', "Compact after 100,000 to 1,000,000 tokens. Leave empty to use Claude's default.", 'e.g. 300000'),
      field('launchArgs', 'Launch arguments', 'Additional CLI arguments passed on session start.', 'e.g. --chrome')] },
  { id: 'cursor', label: 'Cursor', enabledByDefault: false, defaultSlot: true, color: 'light-dark(#26251E, #EDECEC)', modelPlaceholder: 'claude-sonnet-4-6',
    legacyDefault: { enabled: false, customModels: [] }, fields: [],
    env: [field('CURSOR_API_KEY', 'Cursor API key', 'Optional. Overrides browser sign-in for this provider.', 'Paste API key', { control: 'password' })] },
  { id: 'grok', label: 'Grok', enabledByDefault: false, defaultSlot: true, color: 'light-dark(#0F0F0F, #F5F5F5)', modelPlaceholder: 'model-slug',
    legacyDefault: { enabled: false, binaryPath: 'grok', customModels: [] }, env: [], fields: [binary('Path to the Grok CLI binary.', 'grok')] },
  { id: 'opencode', label: 'OpenCode', enabledByDefault: false, defaultSlot: true, color: 'light-dark(#211E1E, #F1ECEC)', modelPlaceholder: 'openai/gpt-5',
    legacyDefault: { enabled: false, binaryPath: 'opencode', serverUrl: '', serverPassword: '', customModels: [] }, env: [],
    fields: [binary('Path to the OpenCode binary.', 'opencode'),
      field('serverUrl', 'Server URL', 'Leave blank to let T3 Code spawn the server when needed.', 'http://127.0.0.1:4096'),
      field('serverPassword', 'Server password', 'Stored in plain text on disk.', 'Optional', { control: 'password' })] },
  { id: 'antigravity', label: 'Antigravity', enabledByDefault: false, defaultSlot: true, color: '#5b87bf', modelPlaceholder: 'model-slug',
    legacyDefault: { enabled: false, authMethod: 'oauth-personal', apiKey: '', gcpProject: '', gcpLocation: '', binaryPath: '', customModels: [] }, env: [],
    fields: [field('authMethod', 'Sign-in method', 'Google accounts use your subscription; API keys and Agent Platform bill usage.', '', { control: 'select', options: ANTIGRAVITY_AUTH_METHODS }),
      field('apiKey', 'API key', 'Gemini or Vertex AI express key. Stored in plain text.', 'Optional', { control: 'password' }),
      field('gcpProject', 'GCP project', 'Required for Gemini Enterprise. Agent Platform uses it when no API key is set.', 'my-project-id'),
      field('gcpLocation', 'GCP location', 'Region for Gemini Enterprise or Agent Platform.', 'us-central1'),
      field('binaryPath', 'Binary path', 'Custom ACP executable. Leave empty to select automatically.', 'Automatic', { persist: true })] },
  { id: 'pi', label: 'Pi', enabledByDefault: false, defaultSlot: true, color: 'light-dark(#0F0F0F, #F5F5F5)', modelPlaceholder: 'anthropic/claude-sonnet-5',
    legacyDefault: { enabled: false, binaryPath: 'pi', launchArgs: '', customModels: [] }, env: [],
    fields: [binary('Path to the Pi coding agent binary.', 'pi'),
      field('launchArgs', 'Launch arguments', 'Additional CLI arguments passed to pi --mode rpc on session start.')] },
  { id: 'acpRegistry', label: 'ACP Registry', enabledByDefault: true, defaultSlot: false, color: 'light-dark(#000000, #ffffff)', modelPlaceholder: 'model-slug',
    legacyDefault: null, env: [],
    fields: [field('agentId', 'Registry agent ID', "Agent identifier from the official ACP Registry, for example 'devin'.", 'devin', { persist: true }),
      field('commandPath', 'Executable override', 'Optional local executable to use instead of installing the registry distribution. Registry arguments and environment are still applied.', 'Registry default'),
      field('authMethodId', 'Authentication method', 'Optional ACP authentication method ID. By default, the first agent-managed method is selected.', 'auto')] },
];

export const driverMeta = (id: string): Driver | undefined => DRIVERS.find(driver => driver.id === id);

/** `resolveProviderInstanceEnabled`: explicit false wins, then envelope, config, driver default. */
export function instanceEnabled(instance: Obj): boolean {
  const config = obj(instance.config);
  if (instance.enabled === false || config.enabled === false) return false;
  if (instance.enabled === true || config.enabled === true) return true;
  return driverMeta(str(instance.driver))?.enabledByDefault ?? false;
}

const canonical = (value: unknown): unknown => Array.isArray(value) ? value.map(canonical)
  : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value as Obj).sort().map(key => [key, canonical((value as Obj)[key])])) : value;
export const sameValue = (left: unknown, right: unknown) => JSON.stringify(canonical(left)) === JSON.stringify(canonical(right));

/** Version label as T3's getProviderVersionLabel. */
export function versionLabel(version: unknown): string {
  const text = str(version);
  if (!text) return '';
  const antigravity = /^agy_acp_server_(\d{4})(\d{2})(\d{2})_\d+(?:_(\w+))?$/.exec(text);
  if (antigravity) return `${antigravity[1]}-${antigravity[2]}-${antigravity[3]}${antigravity[4] ? ` ${antigravity[4]}` : ''}`;
  return /^\d/.test(text) ? `v${text}` : text;
}

/** Headline/detail copy as T3's getProviderSummary. */
export function providerSummary(provider: Obj | undefined): { headline: string; detail: string } {
  if (!provider) return { headline: 'Checking provider status', detail: 'Waiting for the server to report installation and authentication details.' };
  const auth = obj(provider.auth), message = str(provider.message), label = str(auth.label) || str(auth.type);
  if (provider.enabled === false || provider.status === 'disabled') return { headline: 'Disabled', detail: message || 'This provider is installed but disabled for new sessions in T3 Code.' };
  if (provider.installed === false) return { headline: 'Not found', detail: message || 'CLI not detected on PATH.' };
  if (auth.status === 'unauthenticated') return { headline: label ? `Not authenticated · ${label}` : 'Not authenticated', detail: message };
  if (provider.status === 'warning') return { headline: 'Needs attention', detail: message || 'The provider is installed, but the server could not fully verify it.' };
  if (provider.status === 'error') return { headline: 'Unavailable', detail: message || 'The provider failed its startup checks.' };
  if (auth.status === 'authenticated') return { headline: label ? `Authenticated · ${label}` : 'Authenticated', detail: message };
  return { headline: 'Available', detail: message };
}

const COMPATIBILITY_TITLES: Record<string, string> = { graceful: 'Limited support', unsupported: 'Unsupported version', broken: 'Known broken version' };
/** getProviderVersionAdvisoryPresentation, reduced to what the row and header show. */
export function versionAdvisory(provider: Obj | undefined, enabled: boolean): { title: string; detail: string; warning: boolean; command: string } | null {
  if (!provider) return null;
  const advisory = obj(provider.versionAdvisory), compatibility = obj(provider.compatibilityAdvisory);
  const latestIncompatible = ['broken', 'unsupported'].includes(str(compatibility.latestVersionStatus));
  if (enabled && ['graceful', 'unsupported', 'broken'].includes(str(compatibility.status))) {
    const target = str(compatibility.recommendedVersion), recommendation = versionLabel(target) || str(compatibility.recommendedRange);
    return { title: COMPATIBILITY_TITLES[str(compatibility.status)], warning: true,
      detail: str(compatibility.message) || (recommendation ? `Use ${recommendation} for full support.` : 'Update for full support.'),
      command: target || latestIncompatible ? '' : str(advisory.updateCommand) };
  }
  if (!str(advisory.status) || ['current', 'unknown'].includes(str(advisory.status)) || latestIncompatible) return null;
  const latest = versionLabel(advisory.latestVersion);
  return { title: 'Update available', warning: false, command: str(advisory.updateCommand),
    detail: str(advisory.message) || (latest ? `Update available: install ${latest}.` : 'Update available: install the latest provider version.') };
}

/** Relative "Checked …" label as ProviderLastChecked. */
export function checkedLabel(iso: string, nowMs: number): string {
  const at = Date.parse(iso);
  if (!iso || !Number.isFinite(at)) return 'Not checked yet';
  const seconds = Math.max(0, Math.floor((nowMs - at) / 1000));
  if (seconds < 60) return 'Checked just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `Checked ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Checked ${hours}h ago`;
  return `Checked ${Math.floor(hours / 24)}d ago`;
}

/** Instance-id slug rules from AddProviderInstanceDialog. */
export const INSTANCE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_-]*$/;
export function slugifyLabel(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '').slice(0, 48);
}
export function validateInstanceId(id: string, existing: ReadonlySet<string>): string {
  if (id.length === 0) return 'Instance ID is required.';
  if (id.length > 64) return 'Instance ID must be 64 characters or fewer.';
  if (!INSTANCE_ID_PATTERN.test(id)) return "Instance ID must start with a letter and use only letters, digits, '-', or '_'.";
  if (existing.has(id)) return `An instance named '${id}' already exists.`;
  return '';
}
export function deriveAvailableInstanceId(derive: (label: string) => string, label: string, existing: ReadonlySet<string>): string {
  const base = derive(label);
  if (!base || !existing.has(base)) return base;
  for (let suffix = 2; ; suffix += 1) {
    const text = `_${suffix}`, candidate = `${base.slice(0, 64 - text.length)}${text}`;
    if (!existing.has(candidate)) return candidate;
  }
}
