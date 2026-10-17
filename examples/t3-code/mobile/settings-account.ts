// @ref llp/1109.009-mobile-settings.decision.md#unconfigured-account-routes
// Upstream365aa87982: cloud/publicConfig, SettingsAuthRouteScreen and ConnectOnboardingRouteScreen.

/** This build has no Clerk/managed-relay owner or public cloud configuration.
 * Environment pairing is a separate credential flow and cannot enable T3 Connect.
 * Do not turn this on from a URL, saved environment or permission response.
 */
export function mobileAccountSettings() {
  return { cloudConfigured: false as const };
}

export type MobileAccountRouteEntry =
  | { kind: 'replace'; route: 'settings' | 'home' }
  | { kind: 'back' };

/** Root applies this only to its current route, before presenting account content.
 * Auth and the source's waitlist link both replace the outer SettingsContent page.
 * Onboarding dismisses if possible, otherwise replaces with Home.
 */
export function mobileAccountRouteEntry(route: string, canGoBack: boolean): MobileAccountRouteEntry | null {
  if (route === 'settingsAuth' || route === 'settingsWaitlist') return { kind: 'replace', route: 'settings' };
  if (route === 'connectOnboarding') return canGoBack ? { kind: 'back' } : { kind: 'replace', route: 'home' };
  return null;
}
