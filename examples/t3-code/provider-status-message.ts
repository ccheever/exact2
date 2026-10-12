// ProviderStatusBanner, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/chat/ProviderStatusBanner.tsx:9-140 (getIncompatibleVersion,
// getProviderStatusBannerKey, shouldShowProviderStatusBanner, hasProviderSetup,
// getProviderStatusMessage, the banner's title, message, severity and labels) and
// apps/web/src/providerModels.ts formatProviderDriverKindLabel. Changes: the banner's
// fields are one flat projection for the Snapshot shape (`providerBanner`).
import { obj, str, type Obj } from './domain';

/** formatProviderDriverKindLabel: "claudeAgent" → "Claude Agent". */
export function formatProviderDriverKindLabel(driver: string): string {
  return driver.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ').trim().replace(/\b\w/g, char => char.toUpperCase());
}

/** Unsupported and broken versions fail mid-turn, so they warn even when ready. */
function incompatibleVersion(status: Obj): Obj | null {
  const compatibility = status.compatibilityAdvisory && typeof status.compatibilityAdvisory === 'object' ? obj(status.compatibilityAdvisory) : null;
  if (status.status === 'error' && obj(status.auth).status === 'unauthenticated') return null;
  return compatibility && (compatibility.status === 'broken' || (status.status === 'ready' && compatibility.status === 'unsupported')) ? compatibility : null;
}

export function getProviderStatusBannerKey(status: Obj | null | undefined): string | null {
  if (!status || status.status === 'disabled') return null;
  const incompatible = incompatibleVersion(status);
  if (incompatible) return [str(status.instanceId), str(incompatible.status), str(status.version), str(incompatible.message)].join('\u0000');
  if (status.status === 'ready') return null;
  // Antigravity checks saved credentials when a session starts. Its local
  // health check leaves auth unknown after a restart, which is not a failure.
  if (status.driver === 'antigravity' && status.installed === true && status.status === 'warning' && obj(status.auth).status === 'unknown') return null;
  return [str(status.instanceId), str(status.status), str(obj(status.auth).status), str(status.message)].join('\u0000');
}

export function shouldShowProviderStatusBanner(status: Obj | null | undefined, dismissedBannerKey: string | null): boolean {
  const key = getProviderStatusBannerKey(status);
  return key !== null && key !== dismissedBannerKey;
}

export function hasProviderSetup(status: Obj): boolean {
  return status.driver === 'antigravity' || obj(status.setup).canAuthenticate === true || obj(status.setup).canInstall === true;
}

/** Broken-version guidance takes precedence over startup failures it can cause. */
export function getProviderStatusMessage(status: Obj): string {
  const auth = str(obj(status.auth).status), compatibility = obj(status.compatibilityAdvisory);
  if (auth !== 'unauthenticated' && compatibility.status === 'broken' && str(compatibility.message)) return str(compatibility.message);
  if (str(status.message)) return str(status.message);
  const providerName = str(status.displayName).trim() || formatProviderDriverKindLabel(str(status.driver));
  if (status.installed !== true && hasProviderSetup(status)) return `Open provider setup to install ${formatProviderDriverKindLabel(str(status.driver))} on this environment.`;
  if (auth === 'unauthenticated') {
    if (hasProviderSetup(status)) return status.driver === 'antigravity' ? 'Open provider setup to sign in with Google.' : 'Open provider setup to sign in.';
    return 'Sign in via the CLI to authenticate again.';
  }
  return status.status === 'ready' ? 'No models are available for this provider.'
    : status.status === 'error' ? `${providerName} provider is unavailable.` : `${providerName} provider has limited availability.`;
}

/**
 * The ProviderStatusBanner the chat overlays for the composer's instance: its key
 * (dismissal), title, message, severity, role, the dismiss label and the instance
 * "Open provider setup" opens ('' when the provider has no in-app setup).
 */
export function providerBanner(provider: Obj | undefined) {
  const empty = { providerBannerKey: '', providerBannerTitle: '', providerBannerMessage: '', providerBannerWarning: false, providerBannerAlert: false,
    providerBannerDismiss: '', providerBannerSetup: '' };
  const key = getProviderStatusBannerKey(provider);
  if (!provider || key === null) return empty;
  const providerName = str(provider.displayName).trim() || formatProviderDriverKindLabel(str(provider.driver));
  const unauthenticated = provider.status === 'error' && obj(provider.auth).status === 'unauthenticated';
  const incompatible = incompatibleVersion(provider);
  const title = unauthenticated ? `${providerName} is unauthenticated`
    : incompatible ? `${providerName} ${str(provider.version)} is ${incompatible.status === 'broken' ? 'known to be broken' : 'unsupported'}` : `${providerName} provider status`;
  return {
    providerBannerKey: key, providerBannerTitle: title,
    providerBannerMessage: (incompatible && typeof incompatible.message === 'string' ? incompatible.message : null) ?? getProviderStatusMessage(provider),
    providerBannerWarning: incompatible?.status !== 'broken' && (provider.status === 'warning' || incompatible !== null),
    providerBannerAlert: !(incompatible && incompatible.status !== 'broken'),
    providerBannerDismiss: `Dismiss ${providerName} provider ${str(provider.status)}`,
    providerBannerSetup: hasProviderSetup(provider) ? str(provider.instanceId) : '',
  };
}
