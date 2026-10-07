// ProviderSetupSection, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/ProviderSetupSection.tsx:37-93 (readAntigravityAuthMethod,
// the Environment row, "Setup unavailable", "Update required") and :95-327
// (ProviderSetupActions: the Runtime row's status text, byte progress, notes, the
// Install / Update / Reinstall / Retry / Cancel installation / Remove controls, the
// sr-only pending label and "Retry setup status"). Changes: one view model for Contract
// (`ProviderRuntime`, providers-setup.contract); the Remove confirmation's copy is split as
// ConfirmDialogHost splits it (server-update.ts confirmDialogCopy).
import { num, obj, str, type Obj } from './domain';
import type { ProviderSetupEntry } from './provider-setup';
import { confirmDialogCopy } from './server-update';

const megabytes = (bytes: number) => (bytes / 1_000_000).toFixed(1);

/** The Runtime row of an Antigravity instance whose environment advertises setup. */
export function providerRuntime(provider: Obj, entry: ProviderSetupEntry, environmentLabel: string, binaryPath: string, enabled: boolean) {
  const auth = entry.auth.state, installation = entry.install.state, phase = str(installation?.phase);
  const authActive = ['starting', 'waiting', 'verifying'].includes(str(auth?.phase));
  const installActive = phase === 'downloading' || phase === 'extracting' || phase === 'verifying';
  const usesCustomBinary = binaryPath.trim() !== '';
  const installedVersion = installation && installation.installedVersion != null ? str(installation.installedVersion) : '';
  const installed = provider.installed === true || (!usesCustomBinary && installation !== null && installation.installedVersion != null);
  const queryError = entry.auth.error || entry.install.error;
  const actionsDisabled = entry.installPending !== '' || queryError !== '';
  const total = installation && typeof installation.totalBytes === 'number' ? installation.totalBytes : null;
  const downloaded = num(installation?.downloadedBytes);
  const statusMessage = phase === 'downloading' ? `Downloading ${megabytes(downloaded)} MB${total === null ? '' : ` of ${megabytes(total)} MB`}.`
    : phase === 'extracting' ? 'Extracting Antigravity.' : phase === 'verifying' ? 'Checking the downloaded runtime.'
      : installed ? 'Installed.'
        : usesCustomBinary ? (enabled ? 'The configured Antigravity runtime is unavailable.' : 'The configured Antigravity runtime has not been checked.')
          : total ? `${Math.ceil(total / 1_000_000)} MB download.` : 'Not installed.';
  const message = str(installation?.message);
  const canInstall = obj(provider.setup).canInstall === true;
  const version = installation && installation.version != null ? str(installation.version) : '';
  const remove = confirmDialogCopy(`Remove the downloaded Antigravity runtime from ${environmentLabel}? Google sign-in and thread history are kept.`);
  return {
    key: str(provider.instanceId), statusMessage,
    progressValue: phase === 'downloading' && total !== null && total > 0 ? downloaded : 0, progressMax: phase === 'downloading' && total !== null && total > 0 ? total : 0,
    progressPercent: phase === 'downloading' && total !== null && total > 0 ? Math.min(100, Math.max(0, Math.round((downloaded / total) * 1000) / 10)) : 0,
    message: !installActive && message && message !== statusMessage ? message : '',
    customNote: usesCustomBinary, unavailableNote: !installed && !canInstall,
    token: `${phase}:${str(installation?.operationId)}`,
    showCancel: installActive && str(installation?.operationId) !== '', cancelDisabled: actionsDisabled,
    showInstall: !installActive && canInstall, installDisabled: actionsDisabled || installation === null || authActive,
    installLabel: installedVersion ? (version && version !== installedVersion ? 'Update Antigravity' : 'Reinstall Antigravity')
      : phase === 'failed' || phase === 'cancelled' ? 'Retry installation' : installed ? 'Install managed runtime' : 'Install Antigravity',
    showRemove: installation?.canRemove === true && !installActive, removeDisabled: actionsDisabled || authActive,
    removeTitle: remove.title, removeBody: remove.description,
    pendingLabel: entry.installPending ? `${entry.installPending}.` : '',
    error: entry.installError || queryError, showRetry: queryError !== '',
  };
}

/** ProviderSettingsPanel configuredBinaryPath: the instance's trimmed binary path ('' when none). */
export const configuredBinaryPath = (config: unknown) => str(obj(config).binaryPath).trim();
