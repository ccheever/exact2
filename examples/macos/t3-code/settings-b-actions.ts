// Lane settings-b: Settings → Project → Actions (ProjectActionsSettings.tsx,
// ProjectActionsList.tsx, projectScriptEditor.tsx, useProjectScriptSettings.ts,
// projectScripts.ts, lib/projectScriptKeybindings.ts, shared/projectScripts.ts).
// Actions belong to a project: a project scope edits each selected checkout's
// override entry; shortcuts are environment-wide `script.<id>.run` bindings.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { shortcutInput, validShortcut } from './keybinding-settings';
import { shortcutLabel } from './keybinding-view';
import { pushToast } from './toast';
import type { T3Client } from './client';

export const SCRIPT_ICONS: [string, string][] = [['play', 'Play'], ['test', 'Test'], ['lint', 'Lint'], ['configure', 'Configure'], ['build', 'Build'], ['debug', 'Debug']];
const ICON_IDS = SCRIPT_ICONS.map(([id]) => id);
const MAX_SCRIPT_ID = 24;
const SCRIPT_ID = /^[a-z0-9][a-z0-9-]*$/;

export function normalizeScriptId(value: string): string {
  const cleaned = value.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  if (!cleaned) return 'script';
  return cleaned.length <= MAX_SCRIPT_ID ? cleaned : cleaned.slice(0, MAX_SCRIPT_ID).replace(/-+$/g, '') || 'script';
}
/** nextProjectScriptId: the name's slug, suffixed until unused. */
export function nextScriptId(name: string, existing: Iterable<string>): string {
  const taken = new Set(existing), base = normalizeScriptId(name);
  if (!taken.has(base)) return base;
  for (let suffix = 2; suffix < 10_000; suffix++) {
    const candidate = `${base}-${suffix}`;
    const safe = candidate.length <= MAX_SCRIPT_ID ? candidate : `${base.slice(0, Math.max(1, MAX_SCRIPT_ID - String(suffix).length - 1))}-${suffix}`;
    if (!taken.has(safe)) return safe;
  }
  return base;
}
/** Legacy script IDs may not take shortcuts. */
export function commandForScript(id: string): string | null {
  return id.length <= MAX_SCRIPT_ID && SCRIPT_ID.test(id) ? `script.${id}.run` : null;
}
/** keybindingValueForCommand: the last resolved binding wins. */
export function keybindingFor(keybindings: Obj[], command: string | null): string {
  if (!command) return '';
  for (let index = keybindings.length - 1; index >= 0; index--) {
    const binding = keybindings[index]!;
    if (binding.command === command) return shortcutInput(obj(binding.shortcut));
  }
  return '';
}

type Script = { id: string; name: string; command: string; icon: string; runOnWorktreeCreate: boolean; async?: boolean; previewUrl?: string; autoOpenPreview?: boolean };
export type ScriptInput = { name: string; command: string; icon: string; runOnWorktreeCreate: boolean; waitForSetup: boolean; keybinding: string; previewUrl: string; autoOpenPreview: boolean };
const asScript = (value: unknown): Script => {
  const entry = obj(value);
  return { id: str(entry.id), name: str(entry.name), command: str(entry.command), icon: ICON_IDS.includes(str(entry.icon)) ? str(entry.icon) : 'play',
    runOnWorktreeCreate: entry.runOnWorktreeCreate === true, ...(typeof entry.async === 'boolean' ? { async: entry.async } : {}),
    ...(str(entry.previewUrl) ? { previewUrl: str(entry.previewUrl), autoOpenPreview: entry.autoOpenPreview === true } : {}) };
};
/** buildProjectScript. */
export function buildScript(id: string, input: ScriptInput): Script {
  return { id, name: input.name, command: input.command, icon: input.icon, runOnWorktreeCreate: input.runOnWorktreeCreate,
    ...(input.runOnWorktreeCreate && input.waitForSetup ? { async: false } : {}),
    ...(input.previewUrl ? { previewUrl: input.previewUrl, autoOpenPreview: input.autoOpenPreview } : {}) };
}
/** resolveProjectScripts: the project's override, then (until folded) the legacy map and its own scripts, then the environment. */
export function resolveScripts(settings: Obj, project: Obj): Script[] {
  const overrides = obj(obj(settings.projectSettingsOverrides)[str(project.id)]);
  if (Array.isArray(overrides.defaultProjectScripts)) return overrides.defaultProjectScripts.map(asScript);
  const defaults = arr(settings.defaultProjectScripts).map(asScript);
  if (settings.projectSettingsFolded === true) return defaults;
  const legacy = obj(settings.projectScriptOverrides)[str(project.id)];
  if (legacy === null) return defaults;
  if (Array.isArray(legacy)) return legacy.map(asScript);
  const own = arr(project.scripts).map(asScript);
  return own.length ? own : defaults;
}
export function overridesScripts(settings: Obj, projectId: string): boolean {
  return Array.isArray(obj(obj(settings.projectSettingsOverrides)[projectId]).defaultProjectScripts);
}

/** useT3ProjectFileState: missing, invalid or valid with its scripts. */
export function parseProjectScripts(contents: string | null): { status: 'missing' | 'invalid' | 'valid'; scripts: Obj[] } {
  if (contents === null) return { status: 'missing', scripts: [] };
  let file: Obj;
  try { const value = JSON.parse(contents); if (!value || typeof value !== 'object' || Array.isArray(value)) return { status: 'invalid', scripts: [] }; file = value as Obj; }
  catch { return { status: 'invalid', scripts: [] }; }
  if (file.scripts === undefined) return { status: 'valid', scripts: [] };
  if (!Array.isArray(file.scripts) || file.scripts.length > 50) return { status: 'invalid', scripts: [] };
  const scripts: Obj[] = [];
  for (const raw of file.scripts) {
    const entry = obj(raw), name = str(entry.name).trim(), command = str(entry.command).trim();
    const optionalBool = (key: string) => entry[key] === undefined || typeof entry[key] === 'boolean';
    if (!name || !command || (entry.icon !== undefined && !ICON_IDS.includes(str(entry.icon))) || !optionalBool('runOnWorktreeCreate') || !optionalBool('async') || !optionalBool('autoOpenPreview')
      || (entry.previewUrl !== undefined && !str(entry.previewUrl).trim())) return { status: 'invalid', scripts: [] };
    scripts.push({ name, command, icon: str(entry.icon, 'play'), runOnWorktreeCreate: entry.runOnWorktreeCreate === true, async: entry.async,
      previewUrl: str(entry.previewUrl).trim(), autoOpenPreview: entry.autoOpenPreview === true });
  }
  return { status: 'valid', scripts };
}

// t3.json reads are cached per checkout for the page's life (useProjectFileQuery).
const fileCache = new Map<string, string | null>();
async function readProjectFile(client: T3Client, native: Native, member: Obj): Promise<string | null> {
  const key = `${client.environmentId}:${str(member.workspaceRoot)}`;
  if (fileCache.has(key)) return fileCache.get(key)!;
  let contents: string | null = null;
  try {
    const result = await client.rpc(native, 'projects.readFile', { cwd: str(member.workspaceRoot), relativePath: 't3.json' });
    contents = result.truncated === true ? null : typeof result.contents === 'string' ? result.contents : null;
  } catch { contents = null; }
  fileCache.set(key, contents);
  return contents;
}
export function forgetProjectFiles(): void { fileCache.clear(); }

/** The Actions section for the scope's checkouts (members). */
export async function projectActions(client: T3Client, native: Native | null | undefined, members: Obj[]) {
  const settings = obj(client.config.settings), keybindings = arr(client.config.keybindings);
  const representative = members[0];
  const scripts = representative ? resolveScripts(settings, representative) : [];
  const mixed = members.some(member => JSON.stringify(resolveScripts(settings, member)) !== JSON.stringify(scripts));
  const file = representative && native?.available ? parseProjectScripts(await readProjectFile(client, native, representative)) : { status: 'missing' as const, scripts: [] };
  const importable = file.scripts.filter(entry => !scripts.some(script => script.command === str(entry.command) || script.name.toLowerCase() === str(entry.name).toLowerCase()));
  const overridden = members.some(member => overridesScripts(settings, str(member.id)));
  return {
    actionsMixed: mixed, actionsOverridden: overridden, t3Invalid: file.status === 'invalid',
    actions: scripts.map((script, index) => {
      const value = keybindingFor(keybindings, commandForScript(script.id));
      return { id: script.id, first: index === 0, name: script.name, command: script.command, icon: script.icon, setup: script.runOnWorktreeCreate,
        preview: !!script.previewUrl, shortcut: value ? shortcutLabel(value) : '',
        edit: JSON.stringify({ id: script.id, name: script.name, command: script.command, icon: script.icon, runOnWorktreeCreate: script.runOnWorktreeCreate,
          waitForSetup: script.runOnWorktreeCreate && script.async === false, keybinding: value, previewUrl: script.previewUrl ?? '', autoOpenPreview: script.autoOpenPreview === true }) };
    }),
    importable: importable.map(entry => ({ key: `${str(entry.name)} ${str(entry.command)}`, name: str(entry.name), command: str(entry.command), icon: str(entry.icon, 'play'),
      payload: JSON.stringify({ name: str(entry.name), command: str(entry.command), icon: str(entry.icon, 'play'), runOnWorktreeCreate: entry.runOnWorktreeCreate === true,
        waitForSetup: entry.runOnWorktreeCreate === true && entry.async === false, keybinding: '', previewUrl: str(entry.previewUrl), autoOpenPreview: str(entry.previewUrl) ? entry.autoOpenPreview === true : false }) })),
  };
}

/** The editor's fields as the Contract encodes them (URL query form). */
export function decodeInput(text: string): ScriptInput & { id: string } {
  const raw = new URLSearchParams(text || '');
  const icon = raw.get('icon') ?? 'play';
  return { id: (raw.get('id') ?? '').trim(), name: (raw.get('name') ?? '').trim(), command: (raw.get('command') ?? '').trim(), icon: ICON_IDS.includes(icon) ? icon : 'play',
    runOnWorktreeCreate: raw.get('setup') === 'true', waitForSetup: raw.get('wait') === 'true', keybinding: (raw.get('keybinding') ?? '').trim(),
    previewUrl: (raw.get('preview') ?? '').trim(), autoOpenPreview: raw.get('auto') === 'true' };
}
/** The editor's submit validation (projectScriptEditor submit). */
export function validateInput(input: ScriptInput, scriptId: string | null, existing: string[]): ScriptInput {
  if (!input.name) throw new ClientError('Name is required.');
  if (!input.command) throw new ClientError('Command is required.');
  const id = scriptId ?? nextScriptId(input.name, existing);
  if (input.keybinding && (!commandForScript(id) || !validShortcut(input.keybinding))) throw new ClientError('Invalid keybinding.');
  return { ...input, waitForSetup: input.runOnWorktreeCreate && input.waitForSetup, autoOpenPreview: input.previewUrl ? input.autoOpenPreview : false };
}

/**
 * Project action writes. `scope` is the projects page's member ids joined by
 * ","; `op` is action-save (id: existing script id or ""), action-delete,
 * action-import or action-reset.
 */
export async function runActionOp(client: T3Client, native: Native, op: string, scope: string, value: string): Promise<string> {
  if (!client.writable) throw new ClientError('Reconnect before changing project actions.');
  const ids = scope.split(',').filter(Boolean);
  const projects = client.shell.projects.filter(project => ids.includes(str(project.id)));
  if (!ids.length || projects.length !== ids.length) throw new ClientError('That project is no longer available.');
  const capabilities = obj(obj(client.config.environment).capabilities);
  if (capabilities.projectSettingsOverrides !== true) throw new ClientError('Update the selected environment to edit project actions.');
  const input = decodeInput(value);
  const scriptId = op === 'action-save' || op === 'action-delete' ? input.id || null : null;
  const config = await client.rpc(native, 'server.getConfig', {});
  const settings = obj(config.settings), keybindings = arr(config.keybindings);
  const existingIds = [...client.shell.projects.flatMap(project => arr(project.scripts).map(script => str(script.id))),
    ...arr(settings.defaultProjectScripts).map(script => str(script.id)),
    ...Object.values(obj(settings.projectSettingsOverrides)).flatMap(entry => arr(obj(entry).defaultProjectScripts).map(script => str(script.id)))];
  let transform: (current: Script[]) => Script[] | null;
  let changedId: string | null = null, keybinding: string | null | undefined;
  if (op === 'action-reset') transform = () => null;
  else if (op === 'action-delete') {
    if (!scriptId) throw new ClientError('That action is no longer available.');
    changedId = scriptId; keybinding = null;
    transform = current => current.filter(script => script.id !== scriptId);
  } else {
    const valid = validateInput(input, scriptId, existingIds);
    const id = scriptId ?? nextScriptId(valid.name, existingIds);
    const next = buildScript(id, valid);
    changedId = id; keybinding = valid.keybinding || null;
    transform = current => {
      const updated = current.map(script => script.id === id ? next : valid.runOnWorktreeCreate ? { ...script, runOnWorktreeCreate: false } : script);
      return current.some(script => script.id === id) ? updated : [...updated, next];
    };
  }
  const overrides: Obj = {};
  for (const project of projects) {
    const id = str(project.id), current = resolveScripts(settings, project), next = transform(current);
    const entry = { ...obj(obj(settings.projectSettingsOverrides)[id]) };
    if (next === null) delete entry.defaultProjectScripts; else entry.defaultProjectScripts = next as unknown as Obj[];
    overrides[id] = Object.keys(entry).length ? entry : null;
  }
  try {
    await client.rpc(native, 'server.updateSettings', { patch: { projectSettingsOverrides: overrides } }, true);
    if (changedId !== null) {
      const command = commandForScript(changedId);
      const previous = keybindingFor(keybindings, command);
      if (command && keybinding) {
        if (previous !== keybinding) await client.rpc(native, 'server.upsertKeybinding', { key: keybinding, command, ...(previous ? { replace: { key: previous, command } } : {}) }, true);
      } else if (command && previous && keybinding === null) {
        const retained = client.shell.projects.some(project => !ids.includes(str(project.id)) && resolveScripts(settings, project).some(script => script.id === changedId))
          || arr(settings.defaultProjectScripts).some(script => str(script.id) === changedId);
        const stillHere = op !== 'action-delete';
        if (!retained || stillHere) await client.rpc(native, 'server.removeKeybinding', { key: previous, command }, true);
      }
    }
  } catch (error) {
    pushToast(client, { kind: 'error', title: 'Failed to save project actions', description: error instanceof Error ? error.message : 'An error occurred.' });
    throw error;
  }
  client.config = await client.rpc(native, 'server.getConfig', {});
  return '';
}
export const ACTION_OPS = ['action-save', 'action-add', 'action-import', 'action-delete', 'action-reset'];
