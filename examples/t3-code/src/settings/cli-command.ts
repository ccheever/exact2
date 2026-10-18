// CliCommandSettingsRow.tsx (9c161462c9; MIT, see LICENSE-T3).
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { coreRow, type CoreRow } from './settings-core';

export type CliCommandState = { supported: boolean; installedPath: string | null; onPath: boolean; shadowedBy?: string };

export function cliCommandRow(state: CliCommandState): CoreRow | null {
  if (!state.supported) return null;
  const installed = state.installedPath !== null;
  const description = state.shadowedBy
    ? `Another t3 at ${state.shadowedBy} runs first in a new terminal. Remove it to use T3 Code's.`
    : !installed ? "Run T3 Code's CLI as `t3` from any terminal."
    : state.onPath ? `Installed at ${state.installedPath}. Open a new terminal to use it.`
    : `Installed at ${state.installedPath}, which is not on your PATH yet. Add its folder to your PATH to run \`t3\`.`;
  return coreRow('cli-command', 't3 command', description, 'action', { label: installed ? 'Remove' : 'Install', value: installed ? 'uninstall' : 'install' });
}

export async function readCliCommand(client: T3Client, native: Native | null | undefined): Promise<CoreRow | null> {
  if (!native?.available) return null;
  try {
    const state = obj(await client.restAccess(native).call({ op: 'cliCommandState' }));
    return cliCommandRow({ supported: state.supported === true, installedPath: typeof state.installedPath === 'string' ? state.installedPath : null,
      onPath: state.onPath === true, ...(str(state.shadowedBy) ? { shadowedBy: str(state.shadowedBy) } : {}) });
  } catch (error) { if (letGo(error)) throw error; return null; }
}

export async function changeCliCommand(client: T3Client, native: Native, action: string): Promise<string> {
  if (action !== 'install' && action !== 'uninstall') return '';
  try { await client.restAccess(native).call({ op: action === 'install' ? 'cliCommandInstall' : 'cliCommandUninstall' }); }
  catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: action === 'install' ? 'Could not install t3' : 'Could not remove t3',
      description: error instanceof Error ? error.message : 'Something went wrong.', stacked: true });
  }
  return '';
}
