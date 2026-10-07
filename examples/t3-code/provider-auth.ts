// ProviderAuthenticationSection and ProviderWizardAuthenticationStep, T3 Code 1e2ecbd975
// (MIT, see LICENSE-T3): apps/web/src/components/settings/ProviderAuthenticationSection.tsx:59-101,
// 162-472 (active, signedIn, isDiscovering, needsExternalSetup, the description and status
// text, which controls show and when they are disabled, the forms under the row) and
// ProviderWizardAuthenticationStep.tsx:24-98. Changes: one view model for Contract
// (`ProviderAccount`, providers-setup.contract); `pending` is the row's command in flight
// (provider-setup.ts) or the setup mutation's (the view's `busy`); the Sign out confirmation's
// copy is split as ConfirmDialogHost splits it (server-update.ts confirmDialogCopy).
import { arr, num, obj, str, type Obj } from './domain';
import { authDraftId, authUrl, type ProviderSetupEntry } from './provider-setup';
import { redactedValue } from './redacted-text';
import { confirmDialogCopy } from './server-update';

export type ProviderAccount = ReturnType<typeof providerAccount>;

/** One Account row (ProviderAuthenticationSection) for `provider` on `environmentLabel`. */
export function providerAccount(instanceId: string, provider: Obj, entry: ProviderSetupEntry, environmentLabel: string, readOnly = false) {
  const auth = entry.auth.state, interaction = obj(auth?.interaction), phase = str(auth?.phase), queryError = entry.auth.error;
  const methods = auth && Array.isArray(auth.methods) ? arr(auth.methods) : undefined;
  const providerAuth = obj(provider.auth), setup = provider.setup && typeof provider.setup === 'object' ? obj(provider.setup) : undefined;
  const active = phase === 'starting' || phase === 'waiting' || phase === 'verifying';
  const signedIn = providerAuth.status === 'authenticated' || (providerAuth.status === 'unknown' && phase === 'succeeded');
  const isDiscovering = provider.driver === 'acpRegistry' && !active && !signedIn && !queryError && methods === undefined;
  const needsExternalSetup = !active && !signedIn && (setup?.canAuthenticate === false || (provider.driver === 'acpRegistry' && methods?.length === 0));
  const description = active
    ? phase === 'starting' ? 'Starting sign-in…' : phase === 'verifying' ? 'Checking your account…'
      : interaction.type === 'terminal' ? 'Complete sign-in in the terminal below.'
        : interaction.type === 'credentials' ? 'Enter your credentials below.' : 'Finish signing in in your browser.'
    : signedIn ? 'Signed in.' : isDiscovering ? 'Discovering sign-in methods…'
      : needsExternalSetup ? "No in-app sign-in advertised. Follow the provider's docs to finish setup." : `Sign in on ${environmentLabel}.`;
  const email = signedIn && !active ? redactedValue(providerAuth.email) : redactedValue('');
  const disabled = readOnly || entry.authPending || queryError !== '' || isDiscovering;
  const draftId = authDraftId(auth), url = authUrl(auth), flowId = str(auth?.flowId);
  const docsUrl = needsExternalSetup ? str(setup?.documentationUrl) : '';
  const showCancel = !docsUrl && active && flowId !== '';
  const showStart = !docsUrl && !showCancel && !active && !needsExternalSetup && setup?.canAuthenticate !== false;
  const canLogout = typeof providerAuth.canLogout === 'boolean' ? providerAuth.canLogout : setup?.canAuthenticate === true;
  const signOut = confirmDialogCopy(`Sign out of ${typeof provider.displayName === 'string' ? provider.displayName : str(provider.driver)} on ${environmentLabel}? This stops running threads that share this sign-in. Thread history is kept.`);
  const callback = url !== '' && (interaction.type === 'browser' ? interaction.acceptsCallback === true : !auth?.interaction);
  const error = entry.authError || queryError;
  return {
    instanceId, key: instanceId, description, email: email.value, emailPlaceholder: email.placeholder,
    status: phase === 'failed' ? str(auth?.message) : '', disabled, token: `${phase}:${flowId}`, draftId,
    methods: (methods ?? []).map(method => ({ id: str(method.id), name: str(method.name) })), pickMethod: !active && (methods?.length ?? 0) > 1,
    url, docsUrl, showCancel, showStart,
    startLabel: signedIn ? 'Change account' : phase === 'failed' || phase === 'cancelled' ? 'Retry sign-in' : 'Sign in',
    startDisabled: disabled || provider.enabled !== true || provider.installed !== true || !auth,
    showSignOut: !active && signedIn && canLogout, signOutDisabled: disabled || !auth, signOutTitle: signOut.title, signOutBody: signOut.description,
    details: interaction.type === 'terminal' || interaction.type === 'credentials' || interaction.type === 'deviceCode' || callback || error !== '',
    deviceCode: interaction.type === 'deviceCode' ? str(interaction.userCode) : '',
    terminal: interaction.type === 'terminal', flow: draftId, output: str(interaction.output), offset: num(interaction.outputOffset, str(interaction.output).length),
    credentials: interaction.type === 'credentials' ? arr(interaction.fields).slice(0, 16).map((field, index) => ({ index, name: str(field.name), label: str(field.label), secret: field.secret === true })) : [],
    callback, formKey: `${draftId}:${entry.cleared}`, error,
  };
}

/**
 * ProviderWizardAuthenticationStep: the Account row once the created instance can sign in
 * (or is signed in), else a plain row that is discovering, explains, or offers its docs.
 */
export function providerWizardAuth(instanceId: string, provider: Obj | undefined, entry: ProviderSetupEntry, environmentLabel: string) {
  const auth = entry.auth.state, phase = str(auth?.phase), queryError = entry.auth.error;
  const methods = auth && Array.isArray(auth.methods) ? arr(auth.methods) : undefined;
  const active = phase === 'starting' || phase === 'waiting' || phase === 'verifying';
  const signedIn = obj(provider?.auth).status === 'authenticated' || (obj(provider?.auth).status === 'unknown' && phase === 'succeeded');
  const isDiscovering = !signedIn && !queryError && methods === undefined;
  const canAuthenticate = (methods?.length ?? 0) > 0;
  const account = provider && (canAuthenticate || signedIn)
    ? [providerAccount(instanceId, { ...provider, setup: { ...obj(provider.setup), canInstall: obj(provider.setup).canInstall === true, canAuthenticate } }, entry, environmentLabel)] : [];
  return {
    instanceId, account, discovering: isDiscovering, signedIn, active,
    description: isDiscovering ? 'Discovering sign-in methods…'
      : queryError || (typeof auth?.message === 'string' ? auth.message : "No in-app sign-in advertised. Follow the provider's docs to finish setup."),
    docsUrl: isDiscovering ? '' : str(obj(provider?.setup).documentationUrl),
  };
}
