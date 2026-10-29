// @exact/local-network on the web (LLP 1133.036 D18): the web has no DNS-SD,
// so `available` is false and a browse or a resolve fails
// `NotSupportedError`. The state is the browser's Local Network Access
// permission: `local-network`, else `local-network-access`, the one name
// Chrome asked before 145 split it; a browser that answers neither reads
// `unknown` (D8). A typed address is reached with `fetch` under the
// `local-network` grant source, which the page runtime admits.

const NAMES = ['local-network', 'local-network-access'];

export function implementation({ changed }) {
  let said = null;
  const query = async () => {
    for (const name of NAMES) {
      try { return await navigator.permissions.query({ name }); } catch { /* not a name this browser knows */ }
    }
    return null;
  };
  const again = () => query().then(p => { if ((p?.state ?? 'unknown') !== said) changed('local-network.localNetworkStatus'); }, () => {});
  if (typeof addEventListener === 'function') {
    addEventListener('focus', again);
    addEventListener('visibilitychange', again);
  }
  return {
    async localNetworkStatus() {
      const permission = typeof navigator === 'object' && navigator.permissions ? await query() : null;
      if (permission) permission.onchange = again;
      said = permission?.state ?? 'unknown';
      return { available: false, state: said, android: null };
    },
    async browseServices() {
      return { error: 'NotSupportedError' };
    },
    async resolveService() {
      return { error: 'NotSupportedError' };
    },
    release() {},
  };
}
