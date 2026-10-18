/** Provider-only hosts expose writable; full session hosts name the effective permission. */
export function providerWritable(host: { writable: boolean; ready?: boolean; scopes?: readonly string[] }): boolean {
  return host.scopes ? host.ready !== false && host.scopes.includes('providers:manage') : host.writable;
}
