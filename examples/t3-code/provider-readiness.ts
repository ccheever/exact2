// Provider readiness for first-run setup, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/onboarding/providerReadiness.logic.ts (getOnboardingProviderState) and
// components/settings/providerStatus.ts (getProviderSummary). Moved out of pages-welcome.ts so
// the managed Codex setup (codex-setup.ts) reads them without importing the welcome.
import { obj, str, type Obj } from './domain';

export function providerState(provider: Obj | undefined): string {
  if (!provider) return 'checking';
  const auth = obj(provider.auth);
  if (provider.enabled === false || provider.status === 'disabled') return 'disabled';
  if (provider.installed !== true && provider.status === 'warning' && auth.status === 'unknown') return 'checking';
  if (provider.installed !== true) return 'install';
  if (auth.status === 'unauthenticated') return 'signIn';
  if (provider.status === 'ready') return 'ready';
  return 'attention';
}
export function providerSummary(provider: Obj | undefined): { headline: string; detail: string } {
  if (!provider) return { headline: 'Checking provider status', detail: 'Waiting for the server to report installation and authentication details.' };
  const auth = obj(provider.auth), message = str(provider.message), label = str(auth.label) || str(auth.type);
  if (provider.enabled === false || provider.status === 'disabled') return { headline: 'Disabled', detail: message || 'This provider is installed but disabled for new sessions in T3 Code.' };
  if (provider.installed !== true) return { headline: 'Not found', detail: message || 'CLI not detected on PATH.' };
  if (auth.status === 'unauthenticated') return { headline: label ? `Not authenticated · ${label}` : 'Not authenticated', detail: message };
  if (provider.status === 'warning') return { headline: 'Needs attention', detail: message || 'The provider is installed, but the server could not fully verify it.' };
  if (provider.status === 'error') return { headline: 'Unavailable', detail: message || 'The provider failed its startup checks.' };
  if (auth.status === 'authenticated') return { headline: label ? `Authenticated · ${label}` : 'Authenticated', detail: message };
  return { headline: 'Available', detail: message };
}
